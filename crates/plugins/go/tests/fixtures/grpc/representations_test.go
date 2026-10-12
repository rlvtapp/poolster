package conformance_test

import (
	legacy "example.com/poolster/conformance/legacy"
	pb "example.com/poolster/conformance/wire"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/durationpb"
	"google.golang.org/protobuf/types/known/timestamppb"
	"math"
	"testing"
	"time"
)

func TestRepresentations(t *testing.T) {
	input := &pb.ScalarValues{Int64Value: math.MinInt64, Uint64Value: math.MaxUint64, BytesValue: []byte{0, 255}, OptionalValue: proto.String(""), State: pb.ScalarValues_State(99), Children: map[int32]*pb.ScalarValues{1: {Int64Value: 7}}, Counters: map[string]uint64{"x": math.MaxUint64}, Choice: &pb.ScalarValues_Number{Number: math.MinInt64}}
	input.ProtoReflect().SetUnknown([]byte{0xc0, 0x0c, 0x2a})
	values, err := pb.ScalarValuesValuesFromNative(input)
	if err != nil {
		t.Fatal(err)
	}
	if values.Int64Value != "-9223372036854775808" || values.Uint64Value != "18446744073709551615" || values.BytesValue != "AP8=" || values.OptionalValue == nil || *values.OptionalValue != "" || values.State.Number != 99 || values.State.Name != "" || values.State.DisplayName != "UNRECOGNIZED" || values.Choice.(*pb.ScalarValuesValues_Number).Number != "-9223372036854775808" {
		t.Fatal("represented scalar/presence/enum/oneof mismatch")
	}
	restored, err := values.ToNative()
	if err != nil || !proto.Equal(input, restored) {
		t.Fatal("representation roundtrip", err)
	}
	values.StringValue = "edited"
	values.OptionalValue = nil
	values.Choice = &pb.ScalarValuesValues_Text{Text: "replacement"}
	edited, editErr := values.ToNative()
	if editErr != nil || edited.OptionalValue != nil || edited.GetText() != "replacement" || edited.GetNumber() != 0 || len(edited.ProtoReflect().GetUnknown()) == 0 {
		t.Fatal("edited DTO lost presence/unknowns", editErr)
	}
	values.Choice = nil
	cleared, clearErr := values.ToNative()
	if clearErr != nil || cleared.Choice != nil {
		t.Fatal("cleared DTO restored a stale oneof", clearErr)
	}
	values.Int64Value = "9223372036854775808"
	if _, err = values.ToNative(); err == nil {
		t.Fatal("range loss accepted")
	}
	values.Int64Value = "0"
	values.Counters = append(values.Counters, values.Counters[0])
	if _, err = values.ToNative(); err == nil {
		t.Fatal("duplicate map key")
	}
	values.Counters = values.Counters[:1]
	values.Recursive = values
	if _, err = values.ToNative(); err == nil {
		t.Fatal("cyclic value accepted")
	}
	original := &legacy.RequiredValues{Id: proto.Int32(7)}
	proto.SetExtension(original, legacy.E_Audit, "audit")
	represented, err := legacy.RequiredValuesValuesFromNative(original)
	if err != nil {
		t.Fatal(err)
	}
	represented.Id = proto.Int32(8)
	legacyAgain, err := represented.ToNative()
	if err != nil || legacyAgain.GetId() != 8 || proto.GetExtension(legacyAgain, legacy.E_Audit).(string) != "audit" {
		t.Fatal("extension lost", err)
	}
	known := &pb.WellKnownValues{Timestamp: timestamppb.New(time.Unix(-1, 123)), Duration: &durationpb.Duration{Seconds: -1, Nanos: -123}}
	knownValues, err := pb.WellKnownValuesValuesFromNative(known)
	if err != nil {
		t.Fatal(err)
	}
	if *knownValues.Timestamp != "1969-12-31T23:59:59.000000123Z" {
		t.Fatal("RFC3339 nanos")
	}
	knownAgain, err := knownValues.ToNative()
	if err != nil || !proto.Equal(known, knownAgain) {
		t.Fatal("WKT conversion", err)
	}
	knownValues.Timestamp = proto.String("invalid")
	if _, err = knownValues.ToNative(); err == nil {
		t.Fatal("invalid timestamp accepted")
	}
}
