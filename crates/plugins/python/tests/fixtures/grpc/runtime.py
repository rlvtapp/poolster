import ast
import pathlib
import pickle
import concurrent.futures
import threading
import time
import grpc
import common_pb2 as messages
import service_pb2_grpc as service
import legacy_pb2 as legacy
from google.protobuf.timestamp_pb2 import Timestamp

class Store(service.StoreServicer):
    def Get(self, request, context):
        metadata = dict(context.invocation_metadata())
        if request.name == "error":
            context.abort(grpc.StatusCode.INVALID_ARGUMENT, "invalid item")
        if request.name == "slow":
            time.sleep(0.2)
        context.set_trailing_metadata((("received", metadata.get("client", "none")),))
        return request
    def Watch(self, request, context):
        for i in range(3):
            yield messages.Item(name=f"{request.name}{i}")
    def Upload(self, request_iterator, context):
        return messages.Item(name=",".join(x.name for x in request_iterator))
    def Chat(self, request_iterator, context):
        for item in request_iterator:
            yield item

server = grpc.server(concurrent.futures.ThreadPoolExecutor(max_workers=4))
service.add_StoreServicer_to_server(Store(), server)
port = server.add_insecure_port("127.0.0.1:0")
server.start()
try:
    with grpc.insecure_channel(f"127.0.0.1:{port}") as channel:
        grpc.channel_ready_future(channel).result(timeout=5)
        client = service.StoreStub(channel)
        item = messages.Item(name="sample", count=2, labels={"tag":"value"}, state=messages.Item.READY)
        assert not item.HasField("note")
        item.note = ""
        assert item.HasField("note") and item.WhichOneof("value") == "count"
        item.text = "oneof"
        assert item.WhichOneof("value") == "text"
        item.created_at.CopyFrom(Timestamp(seconds=123))
        old = legacy.Legacy()
        assert old.note == "default-note" and not old.HasField("note") and not old.IsInitialized()
        old.id = 1
        old.label = "choice"
        old.Extensions[legacy.audit] = "audit-value"
        assert old.IsInitialized() and old.WhichOneof("choice") == "label"
        item.legacy.CopyFrom(old)
        assert legacy.Legacy.FromString(old.SerializeToString()).Extensions[legacy.audit] == "audit-value"
        item.detail.value = "detail"
        item.children.add(value="child")
        item.details["map"].value = "mapped"
        assert pickle.loads(pickle.dumps(item.detail)) == item.detail
        assert messages.Item(details={"map": {"value": "mapped"}}).details["map"].value == "mapped"
        assert pickle.loads(pickle.dumps(item)) == item
        [ast.parse(p.read_text()) for p in pathlib.Path(messages.__file__).parent.rglob("*.pyi")]
        result, call = client.Get.with_call(item, metadata=(("client","test"),), timeout=5)
        assert result == item and dict(call.trailing_metadata())["received"] == "test"
        assert [i.name for i in client.Watch(messages.Item(name="watch"), timeout=5)] == ["watch0","watch1","watch2"]
        assert client.Upload(iter([messages.Item(name="a"),messages.Item(name="b")]), timeout=5).name == "a,b"
        assert [i.name for i in client.Chat(iter([item,item]), timeout=5)] == ["sample","sample"]
        try: client.Get(messages.Item(name="error"), timeout=5)
        except grpc.RpcError as e: assert e.code() == grpc.StatusCode.INVALID_ARGUMENT
        else: raise AssertionError("status was hidden")
        try: client.Get(messages.Item(name="slow"), timeout=.01)
        except grpc.RpcError as e: assert e.code() == grpc.StatusCode.DEADLINE_EXCEEDED
        else: raise AssertionError("deadline ignored")
        pending = client.Get.future(messages.Item(name="slow"), timeout=5)
        assert pending.cancel()
        try: pending.result()
        except grpc.FutureCancelledError: pass
        else: raise AssertionError("cancellation ignored")
finally:
    server.stop(0).wait()
print("Python gRPC unary, all streaming modes, presence, metadata, status, deadline and cancellation passed")
