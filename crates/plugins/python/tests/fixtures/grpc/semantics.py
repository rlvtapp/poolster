import json
import math
from google.protobuf import descriptor_pb2, descriptor_pool, message_factory
from google.protobuf.message import DecodeError, EncodeError
from google.protobuf.timestamp_pb2 import Timestamp
from google.protobuf.duration_pb2 import Duration
from google.protobuf.any_pb2 import Any
from google.protobuf.wrappers_pb2 import Int64Value
from google.protobuf.struct_pb2 import Struct, Value, ListValue
from google.protobuf.field_mask_pb2 import FieldMask
import portable_pb2 as messages
import _poolster_codecs_poolster_rpc_test as codec

value = messages.ScalarValues(
    double_value=float("inf"), float_value=float("nan"),
    int64_value=-(1 << 63), uint64_value=(1 << 64)-1, int32_value=-(1 << 31),
    fixed64_value=(1 << 64)-1, fixed32_value=(1 << 32)-1,
    bool_value=True, string_value="Unicode λ", bytes_value=b"\x00\xff",
    uint32_value=(1 << 32)-1, sfixed32_value=-(1 << 31), sfixed64_value=-(1 << 63),
    sint32_value=-(1 << 31), sint64_value=-(1 << 63),
    packed_values=[1, -1, 2147483647], unpacked_values=[-2147483648, 0, 1],
    counters={"large":(1 << 64)-1}, state=2345, renamed="custom",
)
value.children[3].string_value="mapped"
value.recursive.string_value="recursive"
assert not value.HasField("optional_value")
value.optional_value=""
assert value.HasField("optional_value")
value.number=0
assert value.WhichOneof("choice")=="number"
value.text=""
assert value.WhichOneof("choice")=="text"
encoded=codec.encode(value)
decoded=codec.decode(messages.ScalarValues, encoded)
assert decoded.int64_value==-(1 << 63) and decoded.uint64_value==(1 << 64)-1
assert decoded.state==2345 and decoded.HasField("optional_value")
assert math.isnan(decoded.float_value) and math.isinf(decoded.double_value)
assert list(decoded.packed_values)==[1,-1,2147483647] and decoded.children[3].string_value=="mapped"
for field,bad in [("uint64_value",1<<64),("int64_value",1<<63),("int32_value",1<<31),("uint32_value",-1)]:
    try: messages.ScalarValues(**{field:bad})
    except (ValueError, TypeError): pass
    else: raise AssertionError("out-of-range scalar accepted")
first=messages.ScalarValues(); second=messages.ScalarValues();first.packed_values.append(1);assert not second.packed_values
merge=messages.ScalarValues.FromString(messages.ScalarValues(text="first",packed_values=[1]).SerializeToString()+messages.ScalarValues(number=2,packed_values=[2]).SerializeToString())
assert merge.WhichOneof("choice")=="number" and merge.number==2 and list(merge.packed_values)==[1,2]
unknown=b"\xa2\x06\x06future"
old=codec.decode(messages.ScalarValues, encoded+unknown)
assert unknown in codec.encode(old)
old.DiscardUnknownFields();assert unknown not in codec.encode(old)
for malformed in [b"\x00",b"\x08",b"\x0f",b"\x4a\x01\xff",b"\x4a\x02a"]:
    try: codec.decode(messages.ScalarValues, malformed)
    except DecodeError: pass
    else: raise AssertionError("malformed Protobuf accepted")

json_value=json.loads(codec.to_json(value))
assert json_value["int64Value"]==str(-(1<<63)) and json_value["uint64Value"]==str((1<<64)-1)
assert json_value["bytesValue"]=="AP8=" and json_value["doubleValue"]=="Infinity" and json_value["floatValue"]=="NaN"
assert json_value["displayName"]=="custom" and json_value["state"]==2345
parsed=codec.from_json(messages.ScalarValues, codec.to_json(value));assert parsed.int64_value==value.int64_value and parsed.HasField("optional_value")
assert codec.from_json(messages.ScalarValues,'{"renamed":"original"}').renamed=="original"
assert codec.from_json(messages.ScalarValues,'{"optionalValue":null}').HasField("optional_value")==False
for invalid in ['{"renamed":"a","renamed":"b"}','{"renamed":"a","displayName":"b"}','{"unknown":true}','{"int64Value":"9223372036854775808"}']:
    try: codec.from_json(messages.ScalarValues,invalid)
    except Exception: pass
    else: raise AssertionError("ambiguous or invalid JSON accepted")
assert codec.from_json(messages.ScalarValues,'{"unknown":true}',ignore_unknown_fields=True)==messages.ScalarValues()
assert "stringValue" in json.loads(codec.to_json(messages.ScalarValues(),always_print_fields_with_no_presence=True))
assert json.loads(codec.to_json(messages.ScalarValues(state=1),use_integers_for_enums=True))["state"]==1
assert messages.ScalarValues.State.Value("ACTIVE")==1

known=messages.WellKnownValues()
known.timestamp.CopyFrom(Timestamp(seconds=1,nanos=123456789))
known.duration.CopyFrom(Duration(seconds=-1,nanos=-123456789))
known.signed.CopyFrom(Int64Value(value=0))
known.unsigned.value=(1<<64)-1
known.empty.SetInParent()
known.mask.CopyFrom(FieldMask(paths=["string_value","optional_value"]))
known.object.update({"number":1.25,"null":None,"nested":{"bool":True}})
known.value.string_value="value"
known.list.values.add().bool_value=True
known.payload.Pack(messages.ScalarValues(string_value="packed"))
known_json=codec.to_json(known); data=json.loads(known_json)
assert data["timestamp"]=="1970-01-01T00:00:01.123456789Z" and data["duration"]=="-1.123456789s"
assert data["signed"]=="0" and data["unsigned"]==str((1<<64)-1) and data["empty"]=={}
assert data["mask"]=="stringValue,optionalValue" and data["object"]["null"] is None
roundtrip=codec.from_json(messages.WellKnownValues,known_json)
assert roundtrip.timestamp.nanos==123456789 and roundtrip.duration.nanos==-123456789
assert roundtrip.HasField("signed") and not codec.from_json(messages.WellKnownValues,'{"signed":null}').HasField("signed")
expanded=messages.ScalarValues();assert roundtrip.payload.Unpack(expanded) and expanded.string_value=="packed"
assert codec.message_type("poolster.conformance.ScalarValues") is messages.ScalarValues
assert codec.descriptors().file
print("Python native scalar/presence/merge/unknown/malformed/ProtoJSON/WKT conformance passed")
