import grpc
import common_pb2 as models
import service_pb2_grpc as service

item = models.Item(name=42)
item.note = 23
item.children.append("bad")
item.details["wrong"] = "bad"
def wrong_channel(channel: grpc.Channel) -> None:
    service.AsyncStoreStub(channel)
def wrong_request(channel: grpc.Channel) -> None:
    service.StoreStub(channel).Get("bad")
