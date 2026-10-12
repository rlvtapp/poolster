package conformance_test

import (
	"connectrpc.com/connect"
	"context"
	"encoding/json"
	"errors"
	pb "example.com/poolster/conformance/wire"
	"google.golang.org/protobuf/proto"
	"io"
	"net/http"
	"net/http/httptest"
	"sync/atomic"
	"testing"
)

type adapterService struct{}

func (adapterService) Echo(_ context.Context, request *connect.Request[pb.ScalarValues]) (*connect.Response[pb.ScalarValues], error) {
	return connect.NewResponse(request.Msg), nil
}
func (adapterService) Watch(_ context.Context, request *connect.Request[pb.ScalarValues], stream *connect.ServerStream[pb.ScalarValues]) error {
	return stream.Send(request.Msg)
}
func (adapterService) WellKnown(_ context.Context, request *connect.Request[pb.WellKnownValues]) (*connect.Response[pb.WellKnownValues], error) {
	return connect.NewResponse(request.Msg), nil
}

type twirpService struct{}

func (twirpService) Echo(_ context.Context, request *pb.ScalarValues) (*pb.ScalarValues, error) {
	return request, nil
}
func (twirpService) WellKnown(_ context.Context, request *pb.WellKnownValues) (*pb.WellKnownValues, error) {
	return request, nil
}
func TestAdapterIndependentPeers(t *testing.T) {
	mux := http.NewServeMux()
	mux.Handle(pb.Conformance_Echo_FullMethodName, connect.NewUnaryHandler(pb.Conformance_Echo_FullMethodName, func(_ context.Context, r *connect.Request[pb.ScalarValues]) (*connect.Response[pb.ScalarValues], error) {
		reply := connect.NewResponse(r.Msg)
		reply.Header().Set("X-Probe", "independent")
		return reply, nil
	}))
	mux.Handle(pb.Conformance_Watch_FullMethodName, connect.NewServerStreamHandler(pb.Conformance_Watch_FullMethodName, func(_ context.Context, r *connect.Request[pb.ScalarValues], s *connect.ServerStream[pb.ScalarValues]) error {
		return s.Send(r.Msg)
	}))
	server := httptest.NewServer(mux)
	defer server.Close()
	client := pb.NewConformanceGRPCWebClient(server.Client(), server.URL)
	response, err := client.Echo(context.Background(), connect.NewRequest(&pb.ScalarValues{Int64Value: 9007199254740993}))
	if err != nil || response.Msg.Int64Value != 9007199254740993 || response.Header().Get("X-Probe") != "independent" {
		t.Fatalf("web unary %v", err)
	}
	stream, err := client.Watch(context.Background(), connect.NewRequest(&pb.ScalarValues{StringValue: "stream"}))
	if err != nil {
		t.Fatal(err)
	}
	if !stream.Receive() || stream.Msg().StringValue != "stream" {
		t.Fatal("web server stream", stream.Err())
	}
	if stream.Receive() || stream.Err() != nil {
		t.Fatal("web completion", stream.Err())
	}
	stream.Close()
	if _, err = client.Upload(context.Background()); err == nil {
		t.Fatal("web unsupported")
	} else {
		var unsupported *pb.RpcUnsupportedError
		if !errors.As(err, &unsupported) {
			t.Fatal(err)
		}
	}
	_, handler := pb.NewConformanceGRPCWebHandler(adapterService{})
	generated := httptest.NewServer(handler)
	defer generated.Close()
	peer := connect.NewClient[pb.ScalarValues, pb.ScalarValues](generated.Client(), generated.URL+pb.Conformance_Echo_FullMethodName, connect.WithGRPCWeb())
	if reply, err := peer.CallUnary(context.Background(), connect.NewRequest(&pb.ScalarValues{StringValue: "owned"})); err != nil || reply.Msg.StringValue != "owned" {
		t.Fatal("web generated server", err)
	}
	var calls atomic.Int32
	httpPeer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		calls.Add(1)
		data, _ := io.ReadAll(r.Body)
		if r.Header.Get("Content-Type") == "application/json" {
			w.Header().Set("Content-Type", "application/json")
		} else {
			var message pb.ScalarValues
			if err := proto.Unmarshal(data, &message); err != nil {
				t.Error(err)
			}
			w.Header().Set("Content-Type", "application/protobuf")
		}
		w.Header().Set("X-Probe", "twirp")
		_, _ = w.Write(data)
	}))
	defer httpPeer.Close()
	twirp := pb.NewConformanceTwirpClient(httpPeer.Client(), httpPeer.URL)
	for _, isJSON := range []bool{false, true} {
		twirp.JSON = isJSON
		reply, headers, err := twirp.Echo(context.Background(), &pb.ScalarValues{Int64Value: 9007199254740993}, nil)
		if err != nil || reply.Int64Value != 9007199254740993 || headers.Get("X-Probe") != "twirp" {
			t.Fatal("twirp independent peer", err)
		}
	}
	before := calls.Load()
	if _, _, err := twirp.Upload(context.Background(), nil, nil); err == nil || calls.Load() != before {
		t.Fatal("twirp unsupported performed I/O")
	}
	_, twirpHandler := pb.NewConformanceTwirpHandler(twirpService{})
	owned := httptest.NewServer(twirpHandler)
	defer owned.Close()
	request, _ := http.NewRequest("POST", owned.URL+"/twirp/poolster.conformance.Conformance/Echo", nil)
	request.Header.Set("Content-Type", "application/json")
	reply, err := owned.Client().Do(request)
	if err != nil {
		t.Fatal(err)
	}
	defer reply.Body.Close()
	if reply.StatusCode != 400 {
		t.Fatal("malformed JSON accepted")
	}
	var problem map[string]any
	if err = json.NewDecoder(reply.Body).Decode(&problem); err != nil || problem["code"] != "invalid_argument" {
		t.Fatal("twirp error envelope", err)
	}
}
