import os
import threading
from kafka import KafkaProducer, TopicPartition
from kafka_test.events import KafkaClient
from kafka_test.models import UserPayload, UserHeaders, _to_wire
from kafka_test.events._codec import decode, encode
from kafka_test.events.send_user import _DEFINITION

p = UserPayload.from_dict({"id": "u1", "nickname": None})
h = UserHeaders.from_dict({"trace": "你好"})
wire = encode(p, h, "clé", _DEFINITION)
p2, h2, key = decode(*wire, _DEFINITION, UserPayload, UserHeaders)
assert _to_wire(p2) == {"id": "u1", "nickname": None}
assert h2.trace == "你好" and key == "clé"
for payload, headers, key in [
    (b"\xff", [], None), (b'{}', [], None),
    (b'{"id":"a"}', [("trace", b"\xff")], None),
    (b'{"id":"a"}', [("trace", b"x")], b"\xff"),
    (None, [], None),
    (b'{"id":"a","id":"b"}', [("trace", b"x")], None),
    (b'{"id":"a"}', [("trace", b"x"), ("trace", b"x")], None),
]:
    try:
        decode(payload, headers, key, _DEFINITION, UserPayload, UserHeaders)
    except (ValueError, UnicodeError, Exception) as error:
        assert error is not None
    else:
        raise AssertionError("invalid bytes/schema accepted")
try:
    encode(UserPayload.from_dict({"id": "ok"}), {}, None, _DEFINITION)
except Exception:
    pass
else:
    raise AssertionError("missing required header accepted")
with KafkaClient() as client:
    try:
        client.receive_user(group_id="conflicting")
    except ValueError:
        pass
    else:
        raise AssertionError("conflicting group binding accepted")
    with client.receive_user(group_id="group-" + os.environ["POOLSTER_KAFKA_TOPIC"]):
        pass
    cancel = threading.Event()
    cancel.set()
    with client.receive_user(cancel=cancel) as cancelled:
        assert list(cancelled) == []
    metadata = client.send_user(p, headers=h, key="clé")
    assert metadata.topic == os.environ["POOLSTER_KAFKA_TOPIC"]
    assert client._producers["op-sendUser"].config["client_id"] == "op-sendUser"
    with client.receive_user() as subscription:
        assert subscription._consumer.config["client_id"] == "op-receiveUser"
        assert subscription._consumer.config["group_id"] == "group-" + metadata.topic
        timer = threading.Timer(30, subscription._cancel.set)
        timer.start()
        try:
            record = next(iter(subscription))
            assert _to_wire(record.payload) == _to_wire(p)
            assert record.headers.trace == "你好" and record.key == "clé"
            record.ack()
            try:
                record.ack()
            except ValueError:
                pass
            else:
                raise AssertionError("duplicate ack accepted")
        finally:
            timer.cancel()
    client.send_user(p, headers=h)
    with client.receive_user() as subscription:
        def fail(record):
            raise RuntimeError("handler failed")
        try:
            subscription.run(fail)
        except RuntimeError as error:
            assert str(error) == "handler failed"
            assert subscription._pending is not None
        else:
            raise AssertionError("handler failure did not propagate")
    with client.receive_user() as subscription:
        record = next(iter(subscription))
        assert record.payload.id == "u1"
        record.ack()
    raw = KafkaProducer(bootstrap_servers=["127.0.0.1:29092"])
    try:
        for value, headers, key in [
            (b"\xff", [("trace", b"ok")], None),
            (b'{"id":"a"}', [("trace", b"\xff")], None),
            (b'{"id":"a"}', [("trace", b"ok")], b"\xff"),
        ]:
            bad = raw.send(metadata.topic, value=value, headers=headers, key=key).get(timeout=30)
            with client.receive_user() as subscription:
                subscription._consumer.unsubscribe()
                partition = TopicPartition(bad.topic, bad.partition)
                subscription._consumer.assign([partition])
                subscription._consumer.seek(partition, bad.offset)
                try:
                    next(iter(subscription))
                except UnicodeError:
                    assert subscription._pending is None and subscription._failed
                    try:
                        next(iter(subscription))
                    except RuntimeError:
                        pass
                    else:
                        raise AssertionError("failed subscription resumed")
                else:
                    raise AssertionError("malformed broker bytes accepted")
    finally:
        raw.close()
print("Python generated compile/codec/broker/ack tests passed")
