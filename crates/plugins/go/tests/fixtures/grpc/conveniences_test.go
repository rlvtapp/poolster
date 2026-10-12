package conformance_test

import (
	"context"
	"errors"
	pb "example.com/poolster/conformance/wire"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/metadata"
	"google.golang.org/grpc/status"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/durationpb"
	"google.golang.org/protobuf/types/known/timestamppb"
	"io"
	"math"
	"sync/atomic"
	"testing"
	"time"
)

type fakeReactive struct {
	pb.ConformanceClient
	count atomic.Int32
}

func (client *fakeReactive) Echo(ctx context.Context, input *pb.ScalarValues, _ ...grpc.CallOption) (*pb.ScalarValues, error) {
	if input.StringValue == "error" {
		return nil, status.Error(codes.InvalidArgument, "invalid")
	}
	return input, nil
}
func (client *fakeReactive) Watch(ctx context.Context, input *pb.ScalarValues, _ ...grpc.CallOption) (grpc.ServerStreamingClient[pb.ScalarValues], error) {
	return &fakeStream{ctx: ctx, client: client}, nil
}

type fakeStream struct {
	grpc.ClientStream
	ctx    context.Context
	client *fakeReactive
}

func (stream *fakeStream) Header() (metadata.MD, error) {
	return metadata.Pairs("probe", "header"), nil
}
func (stream *fakeStream) Trailer() metadata.MD { return metadata.Pairs("probe", "trailer") }
func (stream *fakeStream) Recv() (*pb.ScalarValues, error) {
	if err := stream.ctx.Err(); err != nil {
		return nil, err
	}
	if stream.client.count.Add(1) > 1000 {
		return nil, io.EOF
	}
	return &pb.ScalarValues{StringValue: "event"}, nil
}
func TestOptInConveniences(t *testing.T) {
	value := &pb.ScalarValues{BytesValue: []byte{1}, Children: map[int32]*pb.ScalarValues{1: {StringValue: "child"}}, OptionalValue: proto.String("")}
	view := pb.NewScalarValuesView(value)
	value.BytesValue[0] = 9
	value.Children[1].StringValue = "mutated"
	copy := view.GetBytesValue()
	copy[0] = 7
	if view.GetBytesValue()[0] != 1 || view.GetChildren()[1].StringValue != "child" || view.GetOptionalValue() == nil {
		t.Fatal("view leaked storage or presence")
	}
	children := view.GetChildren()
	children[1].StringValue = "changed"
	if view.GetChildren()[1].StringValue != "child" {
		t.Fatal("nested view exposed mutable snapshot")
	}
	if text := pb.Int64String(math.MinInt64); text != "-9223372036854775808" {
		t.Fatal(text)
	}
	if _, err := pb.ParseInt64String("9223372036854775808"); err == nil {
		t.Fatal("overflow accepted")
	}
	if _, err := pb.Int64Float64(9007199254740993); err == nil {
		t.Fatal("rounded integer")
	}
	if _, err := pb.Float64Int64(math.NaN()); err == nil {
		t.Fatal("NaN integer")
	}
	if encoded := pb.BytesBase64([]byte{0, 255}); encoded != "AP8=" {
		t.Fatal(encoded)
	}
	if _, err := pb.ParseBytesBase64("%%%"); err == nil {
		t.Fatal("invalid base64")
	}
	timestamp := &timestamppb.Timestamp{Seconds: -1, Nanos: 123}
	converted, err := pb.TimestampTime(timestamp)
	if err != nil || converted.Nanosecond() != 123 {
		t.Fatal("timestamp precision", err)
	}
	restored, err := pb.TimeTimestamp(converted)
	if err != nil || !proto.Equal(timestamp, restored) {
		t.Fatal("timestamp roundtrip", err)
	}
	if _, err = pb.TimestampTime(&timestamppb.Timestamp{Nanos: 1000000000}); err == nil {
		t.Fatal("timestamp range")
	}
	if _, err = pb.DurationTime(&durationpb.Duration{Seconds: 315576000000}); err == nil {
		t.Fatal("Go duration overflow")
	}
	duration, err := pb.DurationTime(&durationpb.Duration{Seconds: -1, Nanos: -123})
	if err != nil || duration != -time.Second-123*time.Nanosecond {
		t.Fatal("duration precision", err)
	}
	adapter := pb.ValueAdapter[*pb.ScalarValues, string]{NewNative: func() *pb.ScalarValues { return &pb.ScalarValues{} }, ToNative: func(value string) (*pb.ScalarValues, error) { return &pb.ScalarValues{StringValue: value}, nil }, FromNative: func(value *pb.ScalarValues) (string, error) { return value.StringValue, nil }}
	data, err := adapter.Marshal("adapted")
	if err != nil {
		t.Fatal(err)
	}
	if value, err := adapter.Unmarshal(data); err != nil || value != "adapted" {
		t.Fatal("adapter", err)
	}
	if _, err := adapter.Unmarshal([]byte{0x80}); err == nil {
		t.Fatal("adapter accepted malformed wire")
	}
	client := &fakeReactive{}
	subscription := pb.SubscribeConformanceEcho(context.Background(), client, &pb.ScalarValues{StringValue: "echo"})
	if value := <-subscription.Values; value.StringValue != "echo" {
		t.Fatal("reactive unary")
	}
	if err := <-subscription.Errors; err != nil {
		t.Fatal(err)
	}
	failed := pb.SubscribeConformanceEcho(context.Background(), client, &pb.ScalarValues{StringValue: "error"})
	if err := <-failed.Errors; status.Code(err) != codes.InvalidArgument {
		t.Fatal("reactive status", err)
	}
	watch := pb.SubscribeConformanceWatch(context.Background(), client, &pb.ScalarValues{})
	time.Sleep(20 * time.Millisecond)
	if count := client.count.Load(); count > 17 {
		t.Fatalf("unbounded reactive buffering %d", count)
	}
	watch.Cancel()
	select {
	case err := <-watch.Errors:
		if !errors.Is(err, context.Canceled) {
			t.Fatal("reactive cancel", err)
		}
	case <-time.After(time.Second):
		t.Fatal("reactive worker leaked")
	}
	if watch.Header().Get("probe")[0] != "header" || watch.Trailer().Get("probe")[0] != "trailer" {
		t.Fatal("reactive metadata")
	}
	headers := watch.Header()
	headers.Set("probe", "mutated")
	if watch.Header().Get("probe")[0] != "header" {
		t.Fatal("metadata storage leaked")
	}
}
