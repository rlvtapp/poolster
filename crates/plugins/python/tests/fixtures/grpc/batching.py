import asyncio
import concurrent.futures
import grpc
import batching_pb2 as models
import batching_pb2_grpc as service
from batching_poolster_rpc_test import BatchScope, AsyncBatchScope
METHOD='poolster.conformance.batch.Reader.Read'
class Reader(service.ReaderServicer):
    def ReadMany(self,request,context):
        calls.append(list(request.ids))
        if 'error' in request.ids: context.abort(grpc.StatusCode.INVALID_ARGUMENT,'invalid key')
        return models.ReadManyResponse(entities=[models.Entity(id=key,value='v:'+key) for key in reversed(request.ids)])
calls=[]
server=grpc.server(concurrent.futures.ThreadPoolExecutor(max_workers=4))
service.add_ReaderServicer_to_server(Reader(),server)
port=server.add_insecure_port('127.0.0.1:0');server.start()
try:
    with grpc.insecure_channel(f'127.0.0.1:{port}') as channel:
        with BatchScope(channel,timeout=5) as scope:
            a=scope.queue(METHOD,models.ReadRequest(id='a'));b=scope.queue(METHOD,models.ReadRequest(id='b'))
            duplicate=scope.queue(METHOD,models.ReadRequest(id='a'))
            cancelled=scope.queue(METHOD,models.ReadRequest(id='cancel'));cancelled.cancel()
            scope.flush()
            assert a.result().id=='a' and b.result().id=='b' and duplicate.result().id=='a'
            assert calls==[['a','b']]
            a.result().value='mutated'
            assert scope.queue(METHOD,models.ReadRequest(id='a')).result().value=='v:a'
            error=scope.queue(METHOD,models.ReadRequest(id='error'));scope.flush()
            try:error.result()
            except grpc.RpcError as failure:assert failure.code()==grpc.StatusCode.INVALID_ARGUMENT
            else:raise AssertionError('batch error hidden')
            error2=scope.queue(METHOD,models.ReadRequest(id='error'));scope.flush()
            assert len(calls)==3 and error2.done()  # failed response is not cached
            pending=scope.queue(METHOD,models.ReadRequest(id='closed'))
        assert pending.cancelled()
        with BatchScope(channel) as other:
            future=other.queue(METHOD,models.ReadRequest(id='a'));other.flush();assert future.result().id=='a'
            assert len(calls)==4  # independent request scopes never share caches
    async def native_async():
        async with grpc.aio.insecure_channel(f'127.0.0.1:{port}') as channel:
            async with AsyncBatchScope(channel,timeout=5) as scope:
                a=scope.queue(METHOD,models.ReadRequest(id='async'))
                b=scope.queue(METHOD,models.ReadRequest(id='async2'))
                await scope.flush();assert (await a).id=='async' and (await b).id=='async2'
    asyncio.run(native_async())
finally:server.stop(0).wait()
