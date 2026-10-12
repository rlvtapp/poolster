import XCTest
import Foundation
import PoolsterKafkaShim
@testable import PoolsterModels

private enum ProbeError: Error { case handlerFailed; case timeout }
private actor Seen { var offset: Int64 = -1; func set(_ value: Int64) { offset = value } }
private typealias Record = KafkaRecord<OrderPayload, OrderHeaders, OrderKey>
private typealias Subscription = KafkaSubscription<OrderPayload, OrderHeaders, OrderKey>
private func next(_ subscription: Subscription) async throws -> Record {
    try await withThrowingTaskGroup(of: Record.self) { group in
        group.addTask { try await subscription.receive() }
        group.addTask { try await Task.sleep(for: .seconds(15)); throw ProbeError.timeout }
        defer { group.cancelAll() }
        return try await group.next()!
    }
}
private func rejected(_ body: () throws -> Void) {
    do { try body(); XCTFail("Invalid value accepted") } catch { }
}
@MainActor final class KafkaTests: XCTestCase {
    func testCodec() throws {
        guard case .object(let schema) = try KafkaJSON.parse(Data(SendOrderEvent.definition.payloadSchema.utf8)) else { return XCTFail("Missing schema object") }
        XCTAssertEqual(schema["description"], .string(#"literal \(fatalError("metadata executed"))"# + "\n\0"))
        let payload = OrderPayload(id: "order-1", quantity: 1)
        let headers = OrderHeaders(trace: "trace-1")
        let key = OrderKey("account-1")
        let wire = try SendOrderEvent.definition.encode(payload, headers, key)
        let (p, h, k) = try ReceiveOrderEvent.definition.decode(wire)
        XCTAssertEqual(p.id, "order-1"); XCTAssertEqual(h.trace, "trace-1"); XCTAssertEqual(k, "account-1")
        rejected { _ = try SendOrderEvent.definition.encode(OrderPayload(id: "bad", quantity: 0), headers, key) }
        rejected { _ = try SendOrderEvent.definition.encode(payload, nil, key) }
        rejected { _ = try SendOrderEvent.definition.encode(payload, headers, OrderKey("")) }
        for bytes in [Data("{\"id\":\"a\",\"id\":\"b\",\"quantity\":1}".utf8), Data("{\"id\":\"a\",\"\\u0069d\":\"b\",\"quantity\":1}".utf8), Data("{\"id\":\"\\ud800\",\"quantity\":1}".utf8), Data("{\"id\":\"a\",\"quantity\":NaN}".utf8), Data([0xff])] {
            rejected { _ = try KafkaJSON.parse(bytes) }
        }
    }
    func testBrokerReplayCancellationAndPoison() async throws {
        let runtime = KafkaDefaults.connect()
        let payload = OrderPayload(id: "order-1", quantity: 1)
        let headers = OrderHeaders(trace: "trace-1")
        let key = OrderKey("account-1")
        do { _ = try await ReceiveOrderEvent.subscribe(runtime, group: "wrong"); XCTFail("Conflicting group accepted") } catch KafkaError.invalid { }
        let delivery = try await SendOrderEvent.send(runtime, payload: payload, headers: headers, key: key)
        XCTAssertGreaterThanOrEqual(delivery.offset, 0)
        let names = await runtime.producerClientNames
        XCTAssertTrue(names.contains { $0.hasPrefix("swift-bound-producer#") })
        let first = try await ReceiveOrderEvent.subscribe(runtime)
        XCTAssertTrue(first.clientName.hasPrefix("swift-bound-consumer#"))
        let seen = Seen()
        do {
            try await first.run { record in
                XCTAssertEqual(record.payload.id, "order-1")
                XCTAssertEqual(record.headers.trace, "trace-1")
                XCTAssertEqual(record.key, "account-1")
                await seen.set(record.offset)
                throw ProbeError.handlerFailed
            }
            XCTFail("Failed handler returned normally")
        } catch ProbeError.handlerFailed { }
        do { _ = try await first.receive(); XCTFail("Pending record allowed next receive") } catch KafkaError.invalid { }
        await first.close()
        let replay = try await ReceiveOrderEvent.subscribe(runtime)
        let record = try await next(replay)
        let offset = await seen.offset
        XCTAssertEqual(record.offset, offset)
        try await replay.ack(record)
        do { try await replay.ack(record); XCTFail("Double ack accepted") } catch KafkaError.invalid { }
        let waiting = Task { try await replay.receive() }
        try await Task.sleep(for: .milliseconds(100)); waiting.cancel()
        do { _ = try await waiting.value; XCTFail("Receive ignored cancellation") } catch is CancellationError { }
        _ = try await SendOrderEvent.send(runtime, payload: payload, headers: headers, key: key)
        var iterator = replay.messages.makeAsyncIterator()
        let afterCancellation = try await iterator.next()!
        try await replay.ack(afterCancellation)
        await replay.close()
        // Nonnull empty key/header bytes must remain empty, never become null.
        let emptyDefinition = KafkaDefinition<OrderPayload, OrderHeaders, OrderKey>(
            topic: ReceiveOrderEvent.definition.topic, clientID: "swift-empty-wire", group: ReceiveOrderEvent.definition.group,
            payloadSchema: ReceiveOrderEvent.definition.payloadSchema,
            headersSchema: "{\"type\":\"object\",\"properties\":{\"trace\":{\"type\":\"string\"}},\"required\":[\"trace\"],\"additionalProperties\":false}", keySchema: "{\"type\":\"string\"}")
        _ = try await runtime.send(emptyDefinition, payload: payload, headers: OrderHeaders(trace: ""), key: OrderKey(""))
        let emptySubscription = try await runtime.subscribe(emptyDefinition)
        let emptyRecord = try await next(emptySubscription)
        XCTAssertEqual(emptyRecord.headers.trace, ""); XCTAssertEqual(emptyRecord.key, "")
        try await emptySubscription.ack(emptyRecord)
        await emptySubscription.close()
        await runtime.close()

        let native = try kafkaOpen("127.0.0.1:29092", "swift-invalid-producer", nil, nil)
        defer { pk_close(native) }
        let valid = Data("{\"id\":\"valid\",\"quantity\":1}".utf8)
        let base = ReceiveOrderEvent.definition
        // Unique topics/groups ensure an earlier poison record cannot mask a case.
        for n in 0..<9 {
            let topic = "\(base.topic)-invalid-\(n)"
            let headerBytes: [UInt8] = n == 2 ? [0xff] : Array("trace-1".utf8)
            let payload: Data = n == 0 ? Data([0xff]) : n == 3 ? Data("{\"id\":\"a\",\"id\":\"b\",\"quantity\":1}".utf8) : n == 7 ? Data("{\"id\":\"a\",\"quantity\":0}".utf8) : n == 8 ? Data("{\"id\":\"a\",\"quantity\":NaN}".utf8) : valid
            let keyBytes: [UInt8] = n == 1 ? [0xff] : Array("account-1".utf8)
            let error = "trace".withCString { name in headerBytes.withUnsafeBufferPointer { bytes in
                let header = pk_header(name: name, value: n == 6 ? nil : bytes.baseAddress, length: bytes.count, is_null: n == 6 ? 1 : 0)
                let headers = n == 4 ? [header, header] : [header]
                return headers.withUnsafeBufferPointer { headers in topic.withCString { topic in payload.withUnsafeBytes { payload in keyBytes.withUnsafeBufferPointer { key in
                    pk_send(native, topic, n == 5 ? nil : payload.bindMemory(to: UInt8.self).baseAddress, n == 5 ? 0 : payload.count, key.baseAddress, key.count, headers.baseAddress, headers.count, UInt64(n + 1))
                } } } }
            } }
            XCTAssertEqual(error, 0)
            var delivered = false
            for _ in 0..<1500 {
                let report = pk_delivery_poll(native)
                if report.available != 0 { XCTAssertEqual(report.error, 0); delivered = true; break }
                try await Task.sleep(for: .milliseconds(10))
            }
            XCTAssertTrue(delivered)
            let definition = KafkaDefinition<OrderPayload, OrderHeaders, OrderKey>(topic: topic, clientID: "swift-invalid-consumer", group: "\(topic)-group", payloadSchema: base.payloadSchema, headersSchema: base.headersSchema, keySchema: base.keySchema)
            let runtime = KafkaDefaults.connect()
            let subscription = try await runtime.subscribe(definition)
            do { _ = try await next(subscription); XCTFail("Malformed case \(n) accepted") } catch ProbeError.timeout { XCTFail("Malformed case \(n) timed out") } catch { }
            do { _ = try await subscription.receive(); XCTFail("Poisoned subscription resumed") } catch KafkaError.invalid { }
            await subscription.close()
            let reopened = try await runtime.subscribe(definition)
            do { _ = try await next(reopened); XCTFail("Poison case \(n) committed") } catch ProbeError.timeout { XCTFail("Poison case \(n) replay timed out") } catch { }
            await reopened.close(); await runtime.close()
        }
    }
}
