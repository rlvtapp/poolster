from collections.abc import AsyncIterator, Iterable
import grpc
import common_pb2 as models
import service_pb2_grpc as service

item = models.Item(name="typed", children=[models.Item.Detail(value="child")], details={"key": {"value": "value"}})
name: str = item.name
child: models.Item.Detail = item.children.add(value="child")
mapped: models.Item.Detail = item.details["key"]

class SyncServer(service.StoreServicer):
    def Get(self, request: models.Item, context: grpc.ServicerContext) -> models.Item:
        return request
    def Watch(self, request: models.Item, context: grpc.ServicerContext) -> Iterable[models.Item]:
        yield request
    def Upload(self, request: Iterable[models.Item], context: grpc.ServicerContext) -> models.Item:
        return next(iter(request))
    def Chat(self, request: Iterable[models.Item], context: grpc.ServicerContext) -> Iterable[models.Item]:
        yield from request

def sync_consumer(channel: grpc.Channel) -> models.Item:
    client = service.StoreStub(channel)
    result: models.Item = client.Get(item, metadata=(("typed", "yes"),), timeout=1)
    for value in client.Watch(item):
        value.name = name
    return result

class AsyncServer(service.AsyncStoreServicer):
    async def Get(self, request: models.Item, context: grpc.aio.ServicerContext[models.Item, models.Item]) -> models.Item:
        return request
    async def Watch(self, request: models.Item, context: grpc.aio.ServicerContext[models.Item, models.Item]) -> AsyncIterator[models.Item]:
        yield request
    async def Upload(self, request: AsyncIterator[models.Item], context: grpc.aio.ServicerContext[models.Item, models.Item]) -> models.Item:
        async for item in request:
            return item
        return models.Item()
    async def Chat(self, request: AsyncIterator[models.Item], context: grpc.aio.ServicerContext[models.Item, models.Item]) -> AsyncIterator[models.Item]:
        async for item in request:
            yield item

async def async_consumer(channel: grpc.aio.Channel) -> models.Item:
    client = service.AsyncStoreStub(channel)
    result: models.Item = await client.Get(item, timeout=1)
    async for value in client.Watch(item):
        value.name = name
    return result

import client_poolster_rpc_test as facade

def styled_consumer(channel: grpc.Channel) -> models.Item:
    grouped = facade.Client(channel)
    result: models.Item = grouped.store.get(item, timeout=1)
    flat = facade.FlatClient(channel)
    result = flat.store_get(item)
    result = facade.store_get(channel, item)
    return result

async def async_styled_consumer(channel: grpc.aio.Channel) -> models.Item:
    grouped = facade.AsyncClient(channel)
    result: models.Item = await grouped.store.get(item)
    return await facade.async_store_get(channel, result)
