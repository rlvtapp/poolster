#[path = "grpc_conformance/harness.rs"]
mod harness;
use harness::generate;
use poolster_core::native::rpc::{RpcClientStyle, RpcGenerationOptions};
use std::process::Command;

#[test]
fn owned_rendering_needs_no_generator_and_splits_models() {
    let tree = generate(Default::default());
    assert!(tree.get("sdk/wire/portable_ScalarValues.pb.go").is_some());
    assert!(tree.get("sdk/legacy/legacy_RequiredValues.pb.go").is_some());
    assert!(
        tree.get("sdk/wire/portable_grpc.pb.go")
            .unwrap()
            .contains("ConformanceClient")
    );
}
#[test]
fn message_only_options_omit_service_stubs() {
    let tree = generate(RpcGenerationOptions {
        emit_clients: false,
        emit_servers: false,
        ..Default::default()
    });
    assert!(tree.get("sdk/wire/portable_grpc.pb.go").is_none());
}
#[test]
#[ignore = "requires Go toolchain and pinned runtime cache"]
fn native_corpus_preserves_wire_json_extensions_groups_and_wkt() {
    let tree = generate(Default::default());
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    std::fs::write(dir.path().join("sdk/conformance_test.go"), RUNTIME).unwrap();
    let result = Command::new("go")
        .args(["test", "-mod=readonly", "-race", "./..."])
        .env("GOCACHE", "/private/tmp/kaji-go-cache")
        .current_dir(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
const RUNTIME: &str = r#"package conformance_test
import("testing";"math";"bytes";"time";pb "example.com/poolster/conformance/wire";legacy "example.com/poolster/conformance/legacy";"google.golang.org/protobuf/proto";"google.golang.org/protobuf/encoding/protojson";"google.golang.org/protobuf/encoding/protowire";"google.golang.org/protobuf/types/known/timestamppb";"google.golang.org/protobuf/types/known/anypb";"google.golang.org/protobuf/types/known/durationpb")
func TestConformance(t *testing.T){
 optional:="";message:=&pb.ScalarValues{Int64Value:math.MinInt64,Uint64Value:math.MaxUint64,Sint64Value:math.MinInt64,Fixed64Value:math.MaxUint64,Int32Value:math.MinInt32,Sfixed32Value:math.MinInt32,FloatValue:float32(math.Inf(1)),DoubleValue:math.Inf(-1),BytesValue:[]byte{0,255},OptionalValue:&optional,PackedValues:[]int32{1,-1},UnpackedValues:[]int32{2,-2},Counters:map[string]uint64{"max":math.MaxUint64},Children:map[int32]*pb.ScalarValues{1:{StringValue:"nested"}},Choice:&pb.ScalarValues_Number{Number:math.MaxInt64},State:pb.ScalarValues_State(99),Renamed:"custom"}
 data,err:=message.MarshalBinary();if err!=nil{t.Fatal(err)};var decoded pb.ScalarValues;if err=decoded.UnmarshalBinary(data);err!=nil || !proto.Equal(message,&decoded){t.Fatalf("binary %v",err)}
 unknown:=protowire.AppendTag(nil,200,protowire.VarintType);unknown=protowire.AppendVarint(unknown,42);data=append(data,unknown...);if err=decoded.UnmarshalBinary(data);err!=nil{t.Fatal(err)};roundtrip,_:=decoded.MarshalBinary();if !bytes.Contains(roundtrip,unknown){t.Fatal("unknown lost")}
 jsonData,err:=protojson.Marshal(message);if err!=nil{t.Fatal(err)};if !bytes.Contains(jsonData,[]byte(`"displayName"`)) || !bytes.Contains(jsonData,[]byte(`"-9223372036854775808"`)){t.Fatalf("JSON %s",jsonData)};var fromJSON pb.ScalarValues;if err=protojson.Unmarshal(jsonData,&fromJSON);err!=nil || !proto.Equal(message,&fromJSON){t.Fatalf("json %v",err)}
 if err=protojson.Unmarshal([]byte(`{"int64Value":"1","int64_value":"2"}`),&fromJSON);err==nil{t.Fatal("duplicate JSON")};if err=decoded.UnmarshalBinary([]byte{0x80});err==nil{t.Fatal("truncated accepted")}
 required:=&legacy.RequiredValues{};if _,err=required.MarshalBinary();err==nil{t.Fatal("missing required")};required.Id=proto.Int32(7);if required.GetNote()!="fallback" || required.GetLarge()!=9007199254740993{t.Fatal("defaults")};required.Legacygroup=&legacy.RequiredValues_LegacyGroup{Name:proto.String("group")};proto.SetExtension(required,legacy.E_Audit,"audit");proto.SetExtension(required,legacy.E_Samples,[]int32{1,2});data,err=required.MarshalBinary();if err!=nil{t.Fatal(err)};var restored legacy.RequiredValues;if err=restored.UnmarshalBinary(data);err!=nil{t.Fatal(err)};if proto.GetExtension(&restored,legacy.E_Audit).(string)!="audit" || restored.Legacygroup.GetName()!="group"{t.Fatal("extension/group")}
 payload,err:=anypb.New(message);if err!=nil{t.Fatal(err)};known:=&pb.WellKnownValues{Timestamp:timestamppb.New(time.Unix(1,123)),Duration:durationpb.New(-time.Second-123*time.Nanosecond),Payload:payload};jsonData,err=known.MarshalJSON();if err!=nil{t.Fatal(err)};var knownAgain pb.WellKnownValues;if err=knownAgain.UnmarshalJSON(jsonData);err!=nil || !proto.Equal(known,&knownAgain){t.Fatalf("WKT %s %v",jsonData,err)}
 }
func TestBoundedNativeJSONOptions(t *testing.T){value:=&pb.ScalarValues{Renamed:"name",State:pb.ScalarValues_State(99)};data,err:=value.MarshalJSONWithOptions(protojson.MarshalOptions{UseProtoNames:true,UseEnumNumbers:true,EmitDefaultValues:true});compact:=bytes.ReplaceAll(data,[]byte(" "),nil);if err!=nil||!bytes.Contains(compact,[]byte(`"renamed"`))||!bytes.Contains(compact,[]byte(`"state":99`))||!bytes.Contains(compact,[]byte(`"string_value":""`)){t.Fatalf("JSON options %s %v",data,err)};var decoded pb.ScalarValues;if err=decoded.UnmarshalJSON([]byte(`{"future":true}`));err==nil{t.Fatal("strict JSON weakened")};if err=decoded.UnmarshalJSONWithOptions([]byte(`{"future":true}`),protojson.UnmarshalOptions{DiscardUnknown:true,RecursionLimit:1000000});err!=nil{t.Fatal("explicit JSON leniency",err)}}
"#;

#[test]
fn style_aliases_and_internal_codec_types_are_explicit() {
    for style in [
        RpcClientStyle::Raw,
        RpcClientStyle::Flat,
        RpcClientStyle::Idiomatic,
    ] {
        let tree = generate(RpcGenerationOptions {
            client_style: style,
            ..Default::default()
        });
        let facade = tree.get("sdk/wire/poolster_client.pb.go").unwrap();
        let expected = match style {
            RpcClientStyle::Raw => "type Client = RawClient",
            RpcClientStyle::Flat => "type Client = FlatClient",
            RpcClientStyle::Idiomatic => "type Client = GroupedClient",
        };
        assert!(facade.contains(expected));
    }
    let tree = generate(RpcGenerationOptions {
        emit_types: false,
        emit_clients: false,
        emit_servers: false,
        emit_reflection: false,
        ..Default::default()
    });
    assert!(
        tree.get("sdk/wire/portable_poolsterScalarValues.pb.go")
            .unwrap()
            .contains("func NewScalarValues")
    );
    assert!(
        !tree
            .get("sdk/wire/portable.pb.go")
            .unwrap()
            .contains("var File_")
    );
}

#[test]
#[ignore = "requires Go toolchain and pinned runtime cache"]
fn independent_emission_profiles_compile_and_bounded_codecs_execute() {
    let profiles = [
        RpcGenerationOptions {
            emit_binary: false,
            emit_json: false,
            emit_clients: false,
            emit_servers: false,
            ..Default::default()
        },
        RpcGenerationOptions {
            emit_types: false,
            emit_clients: false,
            emit_servers: false,
            emit_reflection: false,
            max_message_bytes: 32,
            max_depth: 2,
            ..Default::default()
        },
        RpcGenerationOptions {
            emit_binary: false,
            emit_json: false,
            emit_servers: false,
            ..Default::default()
        },
        RpcGenerationOptions {
            emit_binary: false,
            emit_json: false,
            emit_clients: false,
            ..Default::default()
        },
    ];
    for (index, options) in profiles.into_iter().enumerate() {
        let tree = generate(options);
        let dir = tempfile::tempdir().unwrap();
        tree.write_to(dir.path()).unwrap();
        if index == 1 {
            std::fs::write(dir.path().join("sdk/wire/bounds_test.go"),r#"package wire_test
import("testing";"strings";pb "example.com/poolster/conformance/wire")
func TestBounds(t *testing.T){value:=pb.NewScalarValues();value.StringValue=strings.Repeat("x",33);if _,err:=value.MarshalBinary();err==nil{t.Fatal("encode bytes limit")};if err:=value.UnmarshalBinary(make([]byte,33));err==nil{t.Fatal("decode bytes limit")};value.StringValue="";value.Recursive=pb.NewScalarValues();value.Recursive.Recursive=pb.NewScalarValues();if _,err:=value.MarshalBinary();err==nil{t.Fatal("depth limit")};value.Recursive=value;if _,err:=value.MarshalJSON();err==nil{t.Fatal("cycle limit")}}
"#).unwrap();
        }
        let output = Command::new("go")
            .args(["test", "-timeout=30s", "-mod=readonly", "./..."])
            .env("GOCACHE", "/private/tmp/kaji-go-cache")
            .current_dir(dir.path().join("sdk"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "profile {index}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
#[ignore = "requires Go toolchain and pinned Connect runtime cache"]
fn protocol_adapters_compile_and_reject_unsupported_before_io() {
    use poolster_core::native::rpc::RpcTransport;
    let tree = generate(RpcGenerationOptions {
        transports: vec![
            RpcTransport::Grpc,
            RpcTransport::GrpcWeb,
            RpcTransport::Twirp,
        ],
        ..Default::default()
    });
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    std::fs::write(
        dir.path().join("sdk/adapters_test.go"),
        include_str!("fixtures/grpc/adapters_test.go"),
    )
    .unwrap();
    let output = Command::new("go")
        .args(["test", "-timeout=30s", "-mod=readonly", "./..."])
        .env("GOCACHE", "/private/tmp/kaji-go-cache")
        .current_dir(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        tree.get("sdk/.poolster/rpc-capabilities.json")
            .unwrap()
            .contains("typed RpcUnsupportedError before network I/O")
    );
}

#[test]
#[ignore = "requires Go toolchain and pinned runtime cache"]
fn explicit_request_batch_scope_preserves_keys_cancellation_and_isolation() {
    use poolster_core::native::rpc::RpcBatchMapping;
    let tree = generate(RpcGenerationOptions {
        batching: vec![RpcBatchMapping {
            single_method: "poolster.conformance.batch.Reader.Read".into(),
            batch_method: "poolster.conformance.batch.Reader.ReadMany".into(),
            input_field: "id".into(),
            batch_input_field: "ids".into(),
            batch_output_field: "entities".into(),
            output_key: Some("id".into()),
            cache: true,
            max_batch_size: 2,
        }],
        ..Default::default()
    });
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    std::fs::write(dir.path().join("sdk/batch_test.go"),r#"package conformance_test
import("context";"testing";"errors";pb "example.com/poolster/conformance/batch";"google.golang.org/grpc")
func TestRequestBatchScope(t *testing.T){calls:=0;call:=func(_ context.Context,input *pb.ReadManyRequest,_ ...grpc.CallOption)(*pb.ReadManyResponse,error){calls++;output:=&pb.ReadManyResponse{};for index:=len(input.Ids)-1;index>=0;index--{output.Entities=append(output.Entities,&pb.Entity{Id:input.Ids[index],Value:"found"})};return output,nil};scope:=pb.NewReaderReadBatchScope(context.Background(),call);one:=scope.Queue(context.Background(),&pb.ReadRequest{Id:"one"});two:=scope.Queue(context.Background(),&pb.ReadRequest{Id:"two"});if err:=scope.Flush();err!=nil{t.Fatal(err)};first,err:=one.Await(context.Background());if err!=nil||first.Id!="one"{t.Fatal("association",err)};if second,err:=two.Await(context.Background());err!=nil||second.Id!="two"{t.Fatal(err)};first.Value="mutated";cached:=scope.Queue(context.Background(),&pb.ReadRequest{Id:"one"});if value,err:=cached.Await(context.Background());err!=nil||value.Value!="found"||calls!=1{t.Fatal("positive cache mutation",err)};other:=pb.NewReaderReadBatchScope(context.Background(),call);future:=other.Queue(context.Background(),&pb.ReadRequest{Id:"one"});other.Flush();future.Await(context.Background());if calls!=2{t.Fatal("scope cache leaked")};ctx,cancel:=context.WithCancel(context.Background());canceled:=scope.Queue(ctx,&pb.ReadRequest{Id:"cancel"});cancel();scope.Flush();if _,err:=canceled.Await(context.Background());!errors.Is(err,context.Canceled){t.Fatal("cancel lost",err)};failed:=pb.NewReaderReadBatchScope(context.Background(),func(context.Context,*pb.ReadManyRequest,...grpc.CallOption)(*pb.ReadManyResponse,error){return nil,errors.New("failed")});pending:=failed.Queue(context.Background(),&pb.ReadRequest{Id:"one"});if err:=failed.Flush();err==nil{t.Fatal("error swallowed")};if _,err:=pending.Await(context.Background());err==nil{t.Fatal("future error swallowed")}}
"#).unwrap();
    let output = Command::new("go")
        .args(["test", "-timeout=30s", "-mod=readonly", "-race", "./..."])
        .env("GOCACHE", "/private/tmp/kaji-go-cache")
        .current_dir(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires Go toolchain and pinned runtime cache"]
fn immutable_reactive_and_checked_value_profiles_execute() {
    let tree = generate(RpcGenerationOptions {
        immutable_views: true,
        reactive_streams: true,
        value_adapters: true,
        ..Default::default()
    });
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    std::fs::write(
        dir.path().join("sdk/conveniences_test.go"),
        include_str!("fixtures/grpc/conveniences_test.go"),
    )
    .unwrap();
    let output = Command::new("go")
        .args(["test", "-timeout=30s", "-mod=readonly", "-race", "./..."])
        .env("GOCACHE", "/private/tmp/kaji-go-cache")
        .current_dir(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires Go toolchain and pinned runtime cache"]
fn portable_naming_file_and_codec_filters_compile() {
    let tree = generate(RpcGenerationOptions {
        type_prefix: "Wire".into(),
        type_suffix: "DTO".into(),
        snake_type_names: true,
        file_suffix: "_gen".into(),
        comments: false,
        annotate_files_with_version: false,
        encode_include_types: vec!["poolster.conformance.ScalarValues".into()],
        decode_include_types: vec!["poolster.conformance.legacy.RequiredValues".into()],
        ..Default::default()
    });
    let source = tree
        .get("sdk/wire/portable_WireScalarValuesDTO_codecs.pb_gen.go")
        .unwrap();
    assert!(source.contains("MarshalBinary"));
    assert!(!source.contains("UnmarshalBinary"));
    assert!(!source.contains("// Poolster 0."));
    let legacy = tree
        .get("sdk/legacy/legacy_WireRequiredValuesDTO_codecs.pb_gen.go")
        .unwrap();
    assert!(legacy.contains("UnmarshalBinary"));
    assert!(!legacy.contains("MarshalBinary"));
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    std::fs::write(dir.path().join("sdk/naming_test.go"),r#"package conformance_test
import("testing";pb "example.com/poolster/conformance/wire";"google.golang.org/protobuf/proto")
func TestRenamedTypesKeepWireIdentity(t *testing.T){value:=&pb.WireScalarValuesDTO{StringValue:"named",Recursive:&pb.WireScalarValuesDTO{StringValue:"child"}};if string(value.ProtoReflect().Descriptor().FullName())!="poolster.conformance.ScalarValues"{t.Fatal("wire identity changed")};data,err:=value.MarshalBinary();if err!=nil{t.Fatal(err)};var decoded pb.WireScalarValuesDTO;if err=proto.Unmarshal(data,&decoded);err!=nil||!proto.Equal(value,&decoded){t.Fatal("renamed native codec",err)}}
"#).unwrap();
    let output = Command::new("go")
        .args(["test", "-timeout=30s", "-mod=readonly", "./..."])
        .env("GOCACHE", "/private/tmp/kaji-go-cache")
        .current_dir(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires Go toolchain and pinned runtime cache"]
fn typed_representation_values_preserve_native_wire_semantics() {
    use poolster_core::native::rpc::*;
    let tree = generate(RpcGenerationOptions {
        representations: RpcRepresentations {
            int64: RpcInt64Representation::String,
            bytes: RpcBytesRepresentation::Base64,
            maps: RpcMapsRepresentation::Entries,
            enums: RpcEnumsRepresentation::Name,
            optional: RpcOptionalRepresentation::Nullable,
            timestamp: RpcTimestampRepresentation::Rfc3339,
            duration: RpcDurationRepresentation::TimeDelta,
        },
        ..Default::default()
    });
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    std::fs::write(
        dir.path().join("sdk/representations_test.go"),
        include_str!("fixtures/grpc/representations_test.go"),
    )
    .unwrap();
    let output = Command::new("go")
        .args(["test", "-timeout=30s", "-mod=readonly", "-race", "./..."])
        .env("GOCACHE", "/private/tmp/kaji-go-cache")
        .current_dir(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires Go toolchain and pinned runtime cache"]
fn exact_number_datetime_seconds_and_native_map_values_execute() {
    use poolster_core::native::rpc::*;
    let tree = generate(RpcGenerationOptions {
        representations: RpcRepresentations {
            int64: RpcInt64Representation::NumberExact,
            maps: RpcMapsRepresentation::Map,
            timestamp: RpcTimestampRepresentation::DateTime,
            duration: RpcDurationRepresentation::SecondsExact,
            ..Default::default()
        },
        ..Default::default()
    });
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    std::fs::write(dir.path().join("sdk/representation_alternates_test.go"),r#"package conformance_test
import("testing";"time";pb "example.com/poolster/conformance/wire";"google.golang.org/protobuf/proto";"google.golang.org/protobuf/types/known/timestamppb";"google.golang.org/protobuf/types/known/durationpb")
func TestRepresentationAlternates(t *testing.T){if _,err:=pb.ScalarValuesValuesFromNative(&pb.ScalarValues{Int64Value:9007199254740993});err==nil{t.Fatal("unsafe exact number accepted")};value:=&pb.ScalarValues{Int64Value:7,Counters:map[string]uint64{"x":9},Children:map[int32]*pb.ScalarValues{1:{Int64Value:8}}};represented,err:=pb.ScalarValuesValuesFromNative(value);if err!=nil||represented.Int64Value!=7||represented.Counters["x"]!=9{t.Fatal("number/map",err)};native,err:=represented.ToNative();if err!=nil||!proto.Equal(value,native){t.Fatal("number/map roundtrip",err)};represented.Int64Value=1.5;if _,err:=represented.ToNative();err==nil{t.Fatal("fractional integer accepted")};known:=&pb.WellKnownValues{Timestamp:timestamppb.New(time.Unix(1,123)),Duration:&durationpb.Duration{Seconds:-1,Nanos:-123}};converted,err:=pb.WellKnownValuesValuesFromNative(known);if err!=nil||converted.Timestamp.Nanosecond()!=123{t.Fatal("datetime seconds",err)};restored,err:=converted.ToNative();if err!=nil||!proto.Equal(known,restored){t.Fatal("datetime/seconds roundtrip",err)};if _,err=pb.WellKnownValuesValuesFromNative(&pb.WellKnownValues{Duration:&durationpb.Duration{Seconds:315576000000,Nanos:1}});err==nil{t.Fatal("duration precision loss accepted")}}
"#).unwrap();
    let output = Command::new("go")
        .args(["test", "-timeout=30s", "-mod=readonly", "-race", "./..."])
        .env("GOCACHE", "/private/tmp/kaji-go-cache")
        .current_dir(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[path = "grpc_conformance/profiles.rs"]
mod profiles;
