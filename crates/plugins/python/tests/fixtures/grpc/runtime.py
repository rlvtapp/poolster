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
import client_poolster_rpc_test as facade
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
        assert facade.Client(channel).store.get(messages.Item(name="grouped")).name == "grouped"
        assert facade.FlatClient(channel).store_get(messages.Item(name="flat")).name == "flat"
        assert facade.store_get(channel, messages.Item(name="raw")).name == "raw"
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
        old.legacygroup.value = 7
        assert old.HasField("legacygroup")
        assert legacy.OptionsMessage.DESCRIPTOR.fields_by_name["value"].GetOptions().Extensions[legacy.field_note] == "custom-value"
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

# Async API runs on a separate native server and channel for all four shapes.
import asyncio

class AsyncStore(service.AsyncStoreServicer):
    async def Get(self, request, context):
        return request
    async def Watch(self, request, context):
        for index in range(3):
            yield messages.Item(name=f"{request.name}{index}")
    async def Upload(self, request_iterator, context):
        names = [item.name async for item in request_iterator]
        return messages.Item(name=",".join(names))
    async def Chat(self, request_iterator, context):
        async for item in request_iterator:
            yield item

async def async_runtime():
    server = grpc.aio.server()
    service.add_AsyncStoreServicer_to_server(AsyncStore(), server)
    port = server.add_insecure_port("127.0.0.1:0")
    await server.start()
    try:
        async with grpc.aio.insecure_channel(f"127.0.0.1:{port}") as channel:
            await asyncio.wait_for(channel.channel_ready(), 5)
            client = service.AsyncStoreStub(channel)
            item = messages.Item(name="async")
            assert (await client.Get(item, timeout=5)).name == "async"
            assert [x.name async for x in client.Watch(item, timeout=5)] == ["async0", "async1", "async2"]
            async def requests():
                for name in ["a", "b"]:
                    yield messages.Item(name=name)
            assert (await client.Upload(requests(), timeout=5)).name == "a,b"
            assert [x.name async for x in client.Chat(requests(), timeout=5)] == ["a", "b"]
    finally:
        await server.stop(0)

asyncio.run(async_runtime())
