// Local conformance peer; this file is copied into the generated test module.
package main

import (
    "context"
    "fmt"
    "io"
    "net"
    "os"
    "time"

    wire "example.com/poolster/conformance/wire"
    "google.golang.org/grpc"
    "google.golang.org/grpc/credentials/insecure"
    "google.golang.org/protobuf/proto"
)

type peer struct { wire.UnimplementedConformanceServer }
func (peer) Echo(_ context.Context, value *wire.ScalarValues) (*wire.ScalarValues,error) {return value,nil}
func (peer) WellKnown(_ context.Context, value *wire.WellKnownValues) (*wire.WellKnownValues,error) {return value,nil}
func (peer) Watch(value *wire.ScalarValues, stream grpc.ServerStreamingServer[wire.ScalarValues]) error {
    if err:=stream.Send(value);err!=nil{return err};return stream.Send(value)
}
func (peer) Upload(stream grpc.ClientStreamingServer[wire.ScalarValues,wire.ScalarValues]) error {
    result:=&wire.ScalarValues{}
    for {value,err:=stream.Recv();if err==io.EOF{return stream.SendAndClose(result)};if err!=nil{return err};result=value}
}
func (peer) Chat(stream grpc.BidiStreamingServer[wire.ScalarValues,wire.ScalarValues]) error {
    for {value,err:=stream.Recv();if err==io.EOF{return nil};if err!=nil{return err};if err=stream.Send(value);err!=nil{return err}}
}
func value() *wire.ScalarValues {
    optional:=""
    return &wire.ScalarValues{
        Int64Value:-9223372036854775808,Uint64Value:18446744073709551615,
        Sint64Value:-9007199254740993,Fixed64Value:18446744073709551615,
        Sfixed64Value:-9223372036854775808,Int32Value:-2147483648,Uint32Value:4294967295,
        Sint32Value:-2147483648,Fixed32Value:4294967295,Sfixed32Value:-2147483648,
        FloatValue:1.5,DoubleValue:-2.25,BoolValue:true,StringValue:"wire 😀",
        BytesValue:[]byte{0,255,128},OptionalValue:&optional,
        PackedValues:[]int32{0,1,-1,2147483647},UnpackedValues:[]int32{-2147483648,2},
        Counters:map[string]uint64{"max":18446744073709551615},
        Recursive:&wire.ScalarValues{StringValue:"child"},
        Choice:&wire.ScalarValues_Text{Text:"chosen"},State:12345,Renamed:"alias",
    }
}
func same(actual,expected *wire.ScalarValues) {if !proto.Equal(actual,expected){panic("independent peer changed message")}}
func client(address string) {
    conn,err:=grpc.NewClient(address,grpc.WithTransportCredentials(insecure.NewCredentials()));if err!=nil{panic(err)};defer conn.Close()
    ctx,cancel:=context.WithTimeout(context.Background(),10*time.Second);defer cancel()
    peer:=wire.NewConformanceClient(conn);request:=value()
    response,err:=peer.Echo(ctx,request);if err!=nil{panic(err)};same(response,request)
    watch,err:=peer.Watch(ctx,request);if err!=nil{panic(err)}
    for range 2 {response,err=watch.Recv();if err!=nil{panic(err)};same(response,request)}
    if _,err=watch.Recv();err!=io.EOF{panic(fmt.Sprintf("expected EOF: %v",err))}
    upload,err:=peer.Upload(ctx);if err!=nil{panic(err)}
    for range 2 {if err=upload.Send(request);err!=nil{panic(err)}}
    response,err=upload.CloseAndRecv();if err!=nil{panic(err)};same(response,request)
    chat,err:=peer.Chat(ctx);if err!=nil{panic(err)}
    for range 2 {if err=chat.Send(request);err!=nil{panic(err)}}
    if err=chat.CloseSend();err!=nil{panic(err)}
    for range 2 {response,err=chat.Recv();if err!=nil{panic(err)};same(response,request)}
    if _,err=chat.Recv();err!=io.EOF{panic(fmt.Sprintf("expected EOF: %v",err))}
}
func main() {
    if os.Args[1]=="client" {client(os.Args[2]);return}
    listener,err:=net.Listen("tcp","127.0.0.1:0");if err!=nil{panic(err)}
    server:=grpc.NewServer();wire.RegisterConformanceServer(server,peer{})
    if err=os.WriteFile(os.Args[2],[]byte(listener.Addr().String()),0600);err!=nil{panic(err)}
    if err=server.Serve(listener);err!=nil{panic(err)}
}
