use poolster_core::{
    engine::{Package, Packages},
    input::{InputProvider, InputRegistry},
    native::rpc::*,
};
use poolster_plugin_python::{Python, grpc};
use std::{path::Path, process::Command, sync::Arc};
#[test]
#[ignore = "requires pinned Python/mypy runtime"]
fn policy_models_are_typed_and_preserve_native_presence_wire_and_unknowns() {
    let python = std::env::var("POOLSTER_PYTHON_GRPC").unwrap();
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grpc/representation.proto");
    let mut registry = InputRegistry::new();
    registry
        .register(poolster_input_protobuf::ProtobufInput)
        .unwrap();
    let input = InputProvider::<RpcContract>::new(Arc::new(registry), "protobuf", source);
    let options = RpcGenerationOptions {
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
    };
    let plugin = grpc("values-test").input(input.handle()).options(options);
    let tree = Packages::new()
        .package(Package::<Python>::new("sdk").with(plugin).with(input))
        .generate_native()
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let script = r#"import common_values as v
import common_pb2 as n
import representation_values as rv
import representation_pb2 as rn
from datetime import timedelta
import _poolster_codecs_values_test as codec
native=n.Item(name='name', note='',state=234,labels={'a':'b'},detail=n.Item.Detail(value='child'))
native.created_at.FromJsonString('2020-01-01T00:00:00.123456789Z')
wire=native.SerializeToString()+b'\xa0\x06\x07'
native.ParseFromString(wire)
value=rn.Value(signed=-(2**63),unsigned=2**64-1,content=b'\x00\xff',present=0,item=native)
value.elapsed.FromTimedelta(timedelta(seconds=2,microseconds=5))
public=rv.Value.from_native(value)
assert public.signed==str(-(2**63)) and public.unsigned==str(2**64-1)
assert public.content=='AP8=' and public.elapsed==timedelta(seconds=2,microseconds=5)
assert public.present==0 and rv.Value().present is None
assert public.to_native()==value
public.unsigned=str(2**64)
try:public.to_native()
except ValueError:pass
else:raise AssertionError('uint64 overflow accepted')
public.unsigned='1';public.content='invalid!'
try:public.to_native()
except ValueError:pass
else:raise AssertionError('invalid base64 accepted')
precise=rn.Value();precise.elapsed.nanos=1
try:rv.Value.from_native(precise)
except ValueError:pass
else:raise AssertionError('lossy duration accepted')
model=v.Item.from_native(native)
assert model.note=='' and model.count is None and model.text is None
assert model.labels==[('a','b')] and model.state.number==234
assert model.created_at=='2020-01-01T00:00:00.123456789Z'
assert model.detail.value=='child'
model.name='edited'
converted=model.to_native()
assert converted.name=='edited' and converted.HasField('note') and b'\xa0\x06\x07' in converted.SerializeToString()
model.count=0
assert model.to_native().HasField('count')
model.count=None;model.text='changed alternative'
assert model.to_native().WhichOneof('value')=='text'
model.text=None
assert codec.decode(v.Item,codec.encode(model)).name=='edited'
assert codec.from_json(v.Item,codec.to_json(model)).name=='edited'
a=v.Item();b=v.Item();a.labels.append(('x','y'));assert b.labels==[]
a.count=1;a.text='conflict'
try:a.to_native()
except ValueError:pass
else:raise AssertionError('oneof conflict accepted')
model.labels=[('a','b'),('a','duplicate')]
try:model.to_native()
except ValueError:pass
else:raise AssertionError('duplicate map key accepted')
"#;
    let result = Command::new(&python)
        .args(["-c", script])
        .env("PYTHONPATH", root.path().join("sdk/src"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let consumer = root.path().join("consumer.py");
    std::fs::write(&consumer,"from common_values import Item, Item_Detail\nitem = Item(name='typed', labels=[('key','value')], detail=Item_Detail(value='nested'))\nname: str = item.name\nitem.note = None\nitem.state = 'READY'\n").unwrap();
    let result = Command::new(&python)
        .args(["-m", "mypy", "--strict", "--no-incremental"])
        .arg(&consumer)
        .env("MYPYPATH", root.path().join("sdk/src"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[ignore = "requires pinned Python runtime"]
fn exact_number_datetime_and_seconds_models_reject_precision_loss() {
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grpc/representation.proto");
    let mut registry = InputRegistry::new();
    registry
        .register(poolster_input_protobuf::ProtobufInput)
        .unwrap();
    let input = InputProvider::<RpcContract>::new(Arc::new(registry), "protobuf", source);
    let plugin = grpc("exact-values-test")
        .input(input.handle())
        .options(RpcGenerationOptions {
            representations: RpcRepresentations {
                int64: RpcInt64Representation::NumberExact,
                timestamp: RpcTimestampRepresentation::DateTime,
                duration: RpcDurationRepresentation::SecondsExact,
                ..Default::default()
            },
            ..Default::default()
        });
    let tree = Packages::new()
        .package(Package::<Python>::new("sdk").with(plugin).with(input))
        .generate_native()
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let result = Command::new(std::env::var("POOLSTER_PYTHON_GRPC").unwrap())
        .args(["-c", include_str!("fixtures/grpc/value_variants.py")])
        .env("PYTHONPATH", root.path().join("sdk/src"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
