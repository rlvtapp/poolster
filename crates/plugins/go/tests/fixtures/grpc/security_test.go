package storev1_test

import (
	"context"
	"crypto/rand"
	"crypto/rsa"
	"crypto/tls"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/pem"
	pb "example.com/poolster/grpc/storev1"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/credentials"
	"google.golang.org/grpc/metadata"
	"google.golang.org/grpc/status"
	"math/big"
	"net"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

func certificate(t *testing.T) (tls.Certificate, *x509.CertPool) {
	t.Helper()
	key, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		t.Fatal(err)
	}
	template := &x509.Certificate{SerialNumber: big.NewInt(1), Subject: pkix.Name{CommonName: "localhost"}, NotBefore: time.Now().Add(-time.Minute), NotAfter: time.Now().Add(time.Hour), DNSNames: []string{"localhost"}, KeyUsage: x509.KeyUsageDigitalSignature | x509.KeyUsageKeyEncipherment, ExtKeyUsage: []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth}}
	der, err := x509.CreateCertificate(rand.Reader, template, template, &key.PublicKey, key)
	if err != nil {
		t.Fatal(err)
	}
	certPEM := pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: der})
	keyPEM := pem.EncodeToMemory(&pem.Block{Type: "RSA PRIVATE KEY", Bytes: x509.MarshalPKCS1PrivateKey(key)})
	cert, err := tls.X509KeyPair(certPEM, keyPEM)
	if err != nil {
		t.Fatal(err)
	}
	roots := x509.NewCertPool()
	roots.AppendCertsFromPEM(certPEM)
	return cert, roots
}

type slowStore struct {
	store
	sent atomic.Int64
	done chan struct{}
}

func (s *slowStore) Watch(_ *pb.Item, stream pb.Store_WatchServer) error {
	defer close(s.done)
	for index := 0; index < 1000; index++ {
		if err := stream.Send(&pb.Item{Name: strings.Repeat("x", 128*1024)}); err != nil {
			return err
		}
		s.sent.Add(1)
	}
	return nil
}

func TestTLSInterceptorsBackpressureAndCancellation(t *testing.T) {
	certificate, roots := certificate(t)
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	var mu sync.Mutex
	var order []string
	record := func(value string) { mu.Lock(); order = append(order, value); mu.Unlock() }
	implementation := &slowStore{done: make(chan struct{})}
	server := grpc.NewServer(grpc.Creds(credentials.NewTLS(&tls.Config{Certificates: []tls.Certificate{certificate}, MinVersion: tls.VersionTLS12})), grpc.ChainUnaryInterceptor(func(ctx context.Context, request any, info *grpc.UnaryServerInfo, handler grpc.UnaryHandler) (any, error) {
		record("server-before")
		response, err := handler(ctx, request)
		record("server-after")
		return response, err
	}), grpc.StreamInterceptor(func(srv any, stream grpc.ServerStream, info *grpc.StreamServerInfo, handler grpc.StreamHandler) error {
		record("server-stream")
		return handler(srv, stream)
	}))
	pb.RegisterStoreServer(server, implementation)
	go func() { _ = server.Serve(listener) }()
	t.Cleanup(server.Stop)
	conn, err := grpc.NewClient(listener.Addr().String(), grpc.WithTransportCredentials(credentials.NewTLS(&tls.Config{RootCAs: roots, ServerName: "localhost", MinVersion: tls.VersionTLS12})), grpc.WithChainUnaryInterceptor(func(ctx context.Context, method string, request, response any, cc *grpc.ClientConn, invoke grpc.UnaryInvoker, options ...grpc.CallOption) error {
		record("client-before")
		err := invoke(ctx, method, request, response, cc, options...)
		record("client-after")
		return err
	}), grpc.WithStreamInterceptor(func(ctx context.Context, descriptor *grpc.StreamDesc, cc *grpc.ClientConn, method string, streamer grpc.Streamer, options ...grpc.CallOption) (grpc.ClientStream, error) {
		record("client-stream")
		return streamer(ctx, descriptor, cc, method, options...)
	}))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = conn.Close() })
	client := pb.NewFlatClient(conn)
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	ctx = metadata.NewOutgoingContext(ctx, metadata.Pairs("caller", "poolster"))
	if _, err = client.Get(ctx, &pb.Item{Name: "secure"}); err != nil {
		t.Fatal(err)
	}
	mu.Lock()
	got := strings.Join(order, ",")
	mu.Unlock()
	if got != "client-before,server-before,server-after,client-after" {
		t.Fatalf("interceptor ordering %s", got)
	}
	if _, err = client.Get(ctx, &pb.Item{Name: "error"}); status.Code(err) != codes.InvalidArgument {
		t.Fatal("interceptor changed status", err)
	}
	streamCtx, stop := context.WithCancel(ctx)
	stream, err := client.Watch(streamCtx, &pb.Item{})
	if err != nil {
		t.Fatal(err)
	}
	time.Sleep(100 * time.Millisecond)
	if implementation.sent.Load() >= 1000 {
		t.Fatal("server buffered entire unread stream instead of applying flow control")
	}
	stop()
	if _, err = stream.Recv(); status.Code(err) != codes.Canceled {
		t.Fatal("stream cancellation", err)
	}
	select {
	case <-implementation.done:
	case <-time.After(2 * time.Second):
		t.Fatal("backpressured server did not stop on cancellation")
	}
	mu.Lock()
	got = strings.Join(order, ",")
	mu.Unlock()
	if !strings.Contains(got, "client-stream") || !strings.Contains(got, "server-stream") {
		t.Fatal("stream interceptors not used", got)
	}
	// The same endpoint rejects a client that does not trust the server certificate.
	wrong, err := grpc.NewClient(listener.Addr().String(), grpc.WithTransportCredentials(credentials.NewTLS(&tls.Config{RootCAs: x509.NewCertPool(), ServerName: "localhost", MinVersion: tls.VersionTLS12})))
	if err != nil {
		t.Fatal(err)
	}
	defer wrong.Close()
	short, end := context.WithTimeout(context.Background(), time.Second)
	defer end()
	if _, err = pb.NewStoreClient(wrong).Get(short, &pb.Item{}); err == nil {
		t.Fatal("untrusted TLS certificate accepted")
	}
}
