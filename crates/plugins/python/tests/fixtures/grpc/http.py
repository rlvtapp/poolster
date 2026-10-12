import threading
import struct
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import common_pb2 as models
from http_poolster_rpc_test import TwirpClient, GrpcWebClient, HttpRpcError
class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args): pass
    def do_POST(self):
        data = self.rfile.read(int(self.headers['Content-Length']))
        if self.path.startswith('/twirp/'):
            self.send_response(200); self.end_headers(); self.wfile.write(data)
            return
        request = models.Item.FromString(data[5:])
        self.send_response(200); self.end_headers()
        def frame(flag, payload): self.wfile.write(bytes([flag]) + struct.pack('>I', len(payload)) + payload)
        for name in ([request.name + '0', request.name + '1'] if self.path.endswith('/Watch') else [request.name]):
            frame(0, models.Item(name=name).SerializeToString())
        frame(128, b'grpc-status: 0\r\n')
server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
try:
    endpoint = 'http://127.0.0.1:' + str(server.server_port)
    item = models.Item(name='http')
    assert TwirpClient(endpoint).call('store.v1.Store/Get', item, timeout=5) == item
    assert GrpcWebClient(endpoint).call('store.v1.Store/Get', item, timeout=5) == item
    assert [x.name for x in GrpcWebClient(endpoint).call('store.v1.Store/Watch', item, timeout=5)] == ['http0', 'http1']
    for client in [TwirpClient(endpoint), GrpcWebClient(endpoint)]:
        try: client.call('store.v1.Store/Upload', iter([item]))
        except NotImplementedError: pass
        else: raise AssertionError('unsupported streaming accepted')
finally: server.shutdown(); server.server_close(); thread.join()

# Owned server adapter exercised independently through standard-library HTTP.
import urllib.request
from http_server_poolster_rpc_test import handler
server = ThreadingHTTPServer(('127.0.0.1', 0), handler({
    'store.v1.Store/Get': lambda request: request,
    'store.v1.Store/Watch': lambda request: iter([request, request]),
}))
thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
try:
    endpoint = 'http://127.0.0.1:' + str(server.server_port)
    payload = models.Item(name='independent').SerializeToString()
    request = urllib.request.Request(endpoint + '/twirp/store.v1.Store/Get', data=payload, headers={'Content-Type':'application/protobuf'})
    with urllib.request.urlopen(request, timeout=5) as response:
        assert models.Item.FromString(response.read()).name == 'independent'
    framed = b'\0' + struct.pack('>I',len(payload)) + payload
    request = urllib.request.Request(endpoint + '/store.v1.Store/Watch', data=framed,headers={'Content-Type':'application/grpc-web+proto'})
    with urllib.request.urlopen(request, timeout=5) as response:
        result = response.read()
        assert result.count(payload) == 2 and b'grpc-status: 0' in result
finally: server.shutdown(); server.server_close(); thread.join()
