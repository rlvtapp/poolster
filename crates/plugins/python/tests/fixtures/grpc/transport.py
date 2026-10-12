"""Native channel injection preserves interceptors, TLS and stream lifecycle."""
import concurrent.futures
import pathlib
import subprocess
import tempfile
import threading
import time
import grpc
import common_pb2 as messages
import service_pb2_grpc as service
import client_poolster_rpc_test as facade

class Store(service.StoreServicer):
    def Get(self, request, context):
        if dict(context.invocation_metadata()).get("authorization") != "Bearer test":
            context.abort(grpc.StatusCode.UNAUTHENTICATED, "token required")
        if request.name == "error":
            context.abort(grpc.StatusCode.INVALID_ARGUMENT, "test error")
        return request
    def Watch(self, request, context):
        try:
            for index in range(50000):
                if not context.is_active():
                    break
                produced.append(index)
                yield messages.Item(name="x" * 1048576)
        finally:
            cancelled.set()
    def Upload(self, requests, context):
        return messages.Item(name=",".join(request.name for request in requests))
    def Chat(self, requests, context):
        yield from requests

class ObservedStream:
    def __init__(self, call, name): self.call, self.name = call, name
    def __getattr__(self, name): return getattr(self.call, name)
    def __iter__(self): return self
    def __next__(self):
        value = next(self.call)
        response_messages.append((self.name, value.name))
        return value

class Hooks(grpc.UnaryUnaryClientInterceptor, grpc.UnaryStreamClientInterceptor,
            grpc.StreamUnaryClientInterceptor, grpc.StreamStreamClientInterceptor):
    def __init__(self, name): self.name = name
    def invoke(self, continuation, details, request):
        events.append((self.name, details.method))
        if details.method.endswith(('/Upload', '/Chat')):
            original = request
            def observed():
                for value in original:
                    request_messages.append((self.name, value.name))
                    yield value
            request = observed()
        else: request_messages.append((self.name, request.name))
        result = continuation(details, request)
        def completed(call):
            completions.append((self.name, call.code()))
            if not details.method.endswith(('/Watch', '/Chat')) and call.code() == grpc.StatusCode.OK:
                response_messages.append((self.name, call.result().name))
        result.add_done_callback(completed)
        return ObservedStream(result, self.name) if details.method.endswith(('/Watch', '/Chat')) else result
    intercept_unary_unary = invoke
    intercept_unary_stream = invoke
    intercept_stream_unary = invoke
    intercept_stream_stream = invoke

events, completions, produced = [], [], []
request_messages, response_messages = [], []
cancelled = threading.Event()
with tempfile.TemporaryDirectory() as directory:
    root = pathlib.Path(directory)
    key, certificate = root / "key.pem", root / "cert.pem"
    subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
                    "-subj", "/CN=localhost", "-addext", "subjectAltName=DNS:localhost",
                    "-keyout", str(key), "-out", str(certificate)], check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    server = grpc.server(concurrent.futures.ThreadPoolExecutor(max_workers=4))
    service.add_StoreServicer_to_server(Store(), server)
    credentials = grpc.ssl_server_credentials(((key.read_bytes(), certificate.read_bytes()),))
    port = server.add_secure_port("127.0.0.1:0", credentials)
    server.start()
    try:
        tls = grpc.ssl_channel_credentials(root_certificates=certificate.read_bytes())
        auth = grpc.access_token_call_credentials("test")
        with grpc.secure_channel(f"localhost:{port}", grpc.composite_channel_credentials(tls, auth)) as native:
            grpc.channel_ready_future(native).result(timeout=5)
            channel = grpc.intercept_channel(native, Hooks("first"), Hooks("second"))
            client = facade.FlatClient(channel)
            assert client.store_get(messages.Item(name="secure"), timeout=5).name == "secure"
            assert events[:2] == [("first", "/store.v1.Store/Get"), ("second", "/store.v1.Store/Get")]
            assert client.store_upload(iter([messages.Item(name="one")]), timeout=5).name == "one"
            assert next(client.store_chat(iter([messages.Item(name="chat")]), timeout=5)).name == "chat"
            stream = client.store_watch(messages.Item(), timeout=10)
            assert len(next(stream).name) == 1048576
            time.sleep(.1)  # Intentionally slow reader: HTTP/2 flow control must bound production.
            assert len(produced) < 50000
            assert stream.cancel()
            assert cancelled.wait(5)
            assert stream.code() == grpc.StatusCode.CANCELLED
            assert ("first", "one") in request_messages and ("second", "chat") in response_messages
            try: client.store_get(messages.Item(name="error"), timeout=5)
            except grpc.RpcError as error: assert error.code() == grpc.StatusCode.INVALID_ARGUMENT
            else: raise AssertionError("error hooks must retain status")
            deadline = time.monotonic() + 5
            while not any(code == grpc.StatusCode.INVALID_ARGUMENT for _, code in completions) and time.monotonic() < deadline:
                time.sleep(.01)
            assert any(code == grpc.StatusCode.INVALID_ARGUMENT for _, code in completions)
        with grpc.secure_channel(f"localhost:{port}", tls) as channel:
            try: service.StoreStub(channel).Get(messages.Item(), timeout=5)
            except grpc.RpcError as error: assert error.code() == grpc.StatusCode.UNAUTHENTICATED
            else: raise AssertionError("authentication omitted")
    finally:
        server.stop(0).wait()
print("TLS, auth, all-shape interceptor order/status, slow-reader flow control and stream cancellation passed")
