"""Test peer: used with independently generated or Poolster-owned Python modules."""
import asyncio
import pathlib
import sys

import grpc
import portable_pb2 as messages
import portable_pb2_grpc as services


class Peer(services.ConformanceServicer):
    def Echo(self, request, context):
        return request

    def Watch(self, request, context):
        yield request
        yield request

    def Upload(self, requests, context):
        result = messages.ScalarValues()
        for request in requests:
            result.CopyFrom(request)
        return result

    def Chat(self, requests, context):
        yield from requests

    def WellKnown(self, request, context):
        return request


def value():
    return messages.ScalarValues(
        int64_value=-9223372036854775808,
        uint64_value=18446744073709551615,
        sint64_value=-9007199254740993,
        fixed64_value=18446744073709551615,
        sfixed64_value=-9223372036854775808,
        int32_value=-2147483648,
        uint32_value=4294967295,
        sint32_value=-2147483648,
        fixed32_value=4294967295,
        sfixed32_value=-2147483648,
        float_value=1.5,
        double_value=-2.25,
        bool_value=True,
        string_value="wire 😀",
        bytes_value=b"\x00\xff\x80",
        optional_value="",
        packed_values=[0, 1, -1, 2147483647],
        unpacked_values=[-2147483648, 2],
        counters={"max": 18446744073709551615},
        recursive=messages.ScalarValues(string_value="child"),
        text="chosen",
        state=12345,
        renamed="alias",
    )


def client(address):
    with grpc.insecure_channel(address) as channel:
        grpc.channel_ready_future(channel).result(timeout=10)
        peer = services.ConformanceStub(channel)
        request = value()
        assert peer.Echo(request, timeout=10) == request
        assert list(peer.Watch(request, timeout=10)) == [request, request]
        assert peer.Upload(iter([request, request]), timeout=10) == request
        assert list(peer.Chat(iter([request, request]), timeout=10)) == [request, request]
        known = messages.WellKnownValues()
        known.timestamp.seconds = -62135596800
        known.timestamp.nanos = 123456789
        known.duration.seconds = -1
        known.duration.nanos = -123456789
        known.signed.value = -9223372036854775808
        known.unsigned.value = 18446744073709551615
        known.payload.Pack(request)
        known.object.update({"nested": [None, True, 1.5, "text"]})
        known.mask.paths.extend(["string_value", "recursive.string_value"])
        known.empty.SetInParent()
        assert peer.WellKnown(known, timeout=10) == known


def server(ready):
    import concurrent.futures
    import threading

    peer = grpc.server(concurrent.futures.ThreadPoolExecutor(max_workers=4))
    services.add_ConformanceServicer_to_server(Peer(), peer)
    port = peer.add_insecure_port("127.0.0.1:0")
    peer.start()
    pathlib.Path(ready).write_text(f"127.0.0.1:{port}")
    try:
        threading.Event().wait()
    finally:
        peer.stop(0)


if __name__ == "__main__":
    if sys.argv[1] == "server":
        server(sys.argv[2])
    else:
        client(sys.argv[2])
