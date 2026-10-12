import com.fasterxml.jackson.databind.*;
import io.poolster.models.events.*;
import io.poolster.models.model.*;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.*;
import org.apache.kafka.clients.admin.*;
import org.apache.kafka.clients.consumer.*;
import org.apache.kafka.clients.producer.*;
import org.apache.kafka.common.*;
import org.apache.kafka.common.header.internals.RecordHeaders;
import org.apache.kafka.common.serialization.ByteArraySerializer;

public class Wire {
  static final ObjectMapper JSON = new ObjectMapper();
  static final byte[] PAYLOAD =
      "{\"id\":\"broker\",\"quantity\":1,\"note\":null}".getBytes(StandardCharsets.UTF_8);

  static void expectBad(Runnable call) {
    try {
      call.run();
      throw new AssertionError("malformed event accepted");
    } catch (IllegalArgumentException expected) {
    }
  }

  static ConsumerRecord<byte[], byte[]> record(byte[] payload, byte[] key, RecordHeaders headers) {
    return new ConsumerRecord<>(
        SendOrderEvent.DEFINITION.topic,
        0,
        0,
        0L,
        org.apache.kafka.common.record.TimestampType.CREATE_TIME,
        -1,
        -1,
        key,
        payload,
        headers,
        Optional.empty());
  }

  static RecordHeaders headers(byte[] value) {
    var h = new RecordHeaders();
    h.add("trace", value);
    return h;
  }

  static RecordHeaders duplicateHeaders() {
    var h = headers("a".getBytes());
    h.add("trace", "b".getBytes());
    return h;
  }

  static KafkaSubscription<OrderCreatedPayload, OrderCreatedHeaders, OrderCreatedKey>.Received poll(
      KafkaSubscription<OrderCreatedPayload, OrderCreatedHeaders, OrderCreatedKey> subscription) {
    for (int i = 0; i < 30; i++) {
      var result = subscription.poll(Duration.ofSeconds(1));
      if (!result.isEmpty()) return result.get(0);
    }
    throw new AssertionError("no Kafka delivery");
  }

  public static void main(String[] args) throws Exception {
    var payload = JSON.readValue(PAYLOAD, OrderCreatedPayload.class);
    var headers = JSON.readValue("{\"trace\":\"local\"}", OrderCreatedHeaders.class);
    var key = JSON.readValue("\"account\"", OrderCreatedKey.class);
    var wire = KafkaCodec.encode(SendOrderEvent.DEFINITION, payload, headers, key);
    var decoded =
        KafkaCodec.decode(
            SendOrderEvent.DEFINITION, record(wire.payload(), wire.key(), wire.headers()));
    if (!decoded.payload().id().equals("broker") || !decoded.key().value().equals("account"))
      throw new AssertionError("wire mismatch");
    if (!JSON.readTree(JSON.writeValueAsString(decoded.payload())).has("note"))
      throw new AssertionError("presence lost");
    for (byte[] bad :
        new byte[][] {
          null,
          new byte[] {(byte) 0xff},
          "{}".getBytes(),
          "{\"id\":\"a\",\"quantity\":0}".getBytes(),
          "{\"id\":\"a\",\"quantity\":1,\"quantity\":2}".getBytes(),
          "{\"id\":\"a\",\"quantity\":1} trailing".getBytes()
        })
      expectBad(
          () ->
              KafkaCodec.decode(
                  SendOrderEvent.DEFINITION, record(bad, wire.key(), wire.headers())));
    expectBad(
        () ->
            KafkaCodec.decode(
                SendOrderEvent.DEFINITION,
                record(PAYLOAD, new byte[] {(byte) 0xff}, wire.headers())));
    expectBad(
        () ->
            KafkaCodec.decode(
                SendOrderEvent.DEFINITION, record(PAYLOAD, new byte[0], wire.headers())));
    expectBad(
        () ->
            KafkaCodec.decode(
                SendOrderEvent.DEFINITION,
                record(PAYLOAD, wire.key(), headers(new byte[] {(byte) 0xff}))));
    expectBad(
        () ->
            KafkaCodec.decode(
                SendOrderEvent.DEFINITION, record(PAYLOAD, wire.key(), headers(null))));
    expectBad(
        () ->
            KafkaCodec.decode(
                SendOrderEvent.DEFINITION, record(PAYLOAD, wire.key(), duplicateHeaders())));
    expectBad(
        () ->
            KafkaCodec.decode(
                SendOrderEvent.DEFINITION, record(PAYLOAD, wire.key(), new RecordHeaders())));
    expectBad(
        () ->
            KafkaCodec.encode(
                SendOrderEvent.DEFINITION, payload, headers, new OrderCreatedKey("")));
    expectBad(
        () ->
            KafkaCodec.encode(
                SendOrderEvent.DEFINITION, payload, headers, new OrderCreatedKey("\ud800")));
    expectBad(() -> KafkaCodec.encode(SendOrderEvent.DEFINITION, payload, null, key));
    var number =
        new KafkaCodec.Definition<Double, Void, Void>(
            "numbers",
            null,
            null,
            Double.class,
            Void.class,
            Void.class,
            "{\"type\":\"number\"}",
            null,
            null);
    for (var bad : new Double[] {Double.NaN, Double.POSITIVE_INFINITY, Double.NEGATIVE_INFINITY}) {
      expectBad(() -> KafkaCodec.encode(number, bad, null, null));
    }
    var nested =
        new KafkaCodec.Definition<Map, Void, Void>(
            "nested",
            null,
            null,
            Map.class,
            Void.class,
            Void.class,
            "{\"type\":\"object\"}",
            null,
            null);
    expectBad(() -> KafkaCodec.encode(nested, Map.of("nested", List.of(Double.NaN)), null, null));
    expectBad(() -> KafkaCodec.encode(nested, Map.of("unicode", "\ud800"), null, null));
    try (var runtime = KafkaDefaults.connect()) {
      expectBad(() -> ReceiveOrderEvent.subscribe(runtime, "conflicting-group"));
      if (!args[0].equals("broker")) return;
      SendOrderEvent.send(runtime, payload, headers, key)
          .toCompletableFuture()
          .get(40, java.util.concurrent.TimeUnit.SECONDS);
      var field = KafkaRuntime.class.getDeclaredField("producers");
      field.setAccessible(true);
      if (!((Map<?, ?>) field.get(runtime)).containsKey("java-bound-producer"))
        throw new AssertionError("send clientId binding lost");
      try (var sub = ReceiveOrderEvent.subscribe(runtime)) {
        var consumerField = KafkaSubscription.class.getDeclaredField("consumer");
        consumerField.setAccessible(true);
        var consumer = (KafkaConsumer<?, ?>) consumerField.get(sub);
        if (!consumer.groupMetadata().groupId().equals(ReceiveOrderEvent.DEFINITION.groupId))
          throw new AssertionError("group binding lost");
        if (!consumer.metrics().keySet().stream()
            .anyMatch(metric -> "java-bound-consumer".equals(metric.tags().get("client-id"))))
          throw new AssertionError("receive clientId binding lost");
        boolean handlerFailed = false;
        for (int i = 0; i < 30 && !handlerFailed; i++) {
          try {
            sub.consume(
                Duration.ofSeconds(1),
                received -> {
                  if (!received.payload().id().equals("broker")
                      || !received.key().value().equals("account"))
                    throw new AssertionError("broker mismatch");
                  throw new IllegalStateException("application handler failed");
                });
          } catch (IllegalStateException expected) {
            if (!expected.getMessage().equals("application handler failed")) throw expected;
            handlerFailed = true;
          }
        }
        if (!handlerFailed) throw new AssertionError("handler did not run");
      }
      try (var sub = ReceiveOrderEvent.subscribe(runtime)) {
        var received = poll(sub);
        if (received.offset != 0) throw new AssertionError("failed handler committed offset");
        received.ack();
        try {
          received.ack();
          throw new AssertionError("double ack accepted");
        } catch (IllegalStateException expected) {
        }
        sub.cancel();
        try {
          sub.poll(Duration.ZERO);
          throw new AssertionError("cancel did not wake poll");
        } catch (org.apache.kafka.common.errors.WakeupException expected) {
        }
      }
      var props = new Properties();
      props.setProperty("bootstrap.servers", "127.0.0.1:29092");
      try (var producer =
          new KafkaProducer<byte[], byte[]>(
              props, new ByteArraySerializer(), new ByteArraySerializer())) {
        producer
            .send(
                new ProducerRecord<>(
                    SendOrderEvent.DEFINITION.topic,
                    null,
                    wire.key(),
                    new byte[] {(byte) 0xff},
                    wire.headers()))
            .get();
      }
      try (var sub = ReceiveOrderEvent.subscribe(runtime)) {
        boolean rejected = false;
        for (int i = 0; i < 30; i++) {
          try {
            sub.poll(Duration.ofSeconds(1));
          } catch (IllegalArgumentException expected) {
            rejected = true;
            break;
          }
        }
        if (!rejected) throw new AssertionError("broker invalid UTF8 accepted");
      }
      try (var admin = Admin.create(props)) {
        var offsets =
            admin
                .listConsumerGroupOffsets(ReceiveOrderEvent.DEFINITION.groupId)
                .partitionsToOffsetAndMetadata()
                .get();
        if (offsets.get(new TopicPartition(SendOrderEvent.DEFINITION.topic, 0)).offset() != 1)
          throw new AssertionError("malformed event committed");
      }
    }
    System.out.println(
        "Kafka roundtrip, binding, ack, cancellation and malformed-byte checks passed");
  }
}
