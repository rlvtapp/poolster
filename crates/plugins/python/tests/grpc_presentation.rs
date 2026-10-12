use poolster_core::{
    engine::{Package, Packages},
    input::{InputProvider, InputRegistry},
    native::rpc::{RpcContract, RpcGenerationOptions},
};
use poolster_plugin_python::{Python, grpc};
use std::{path::Path, process::Command, sync::Arc};
fn generate(source: &str, options: RpcGenerationOptions) -> poolster_core::GeneratedTree {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/grpc")
        .join(source);
    let mut registry = InputRegistry::new();
    registry
        .register(poolster_input_protobuf::ProtobufInput)
        .unwrap();
    let input = InputProvider::<RpcContract>::new(Arc::new(registry), "protobuf", source);
    let plugin = grpc("presentation-test")
        .input(input.handle())
        .options(options);
    Packages::new()
        .package(Package::<Python>::new("sdk").with(plugin).with(input))
        .generate_native()
        .unwrap()
}
#[test]
#[ignore = "requires pinned Python/mypy runtime"]
fn configured_aliases_suffixes_filters_index_and_external_imports_are_usable() {
    let python = std::env::var("POOLSTER_PYTHON_GRPC").unwrap();
    let options = RpcGenerationOptions {
        type_prefix: "Pre".into(),
        type_suffix: "Model".into(),
        file_suffix: "_custom".into(),
        encode_include_types: vec!["store.v1.Item".into()],
        decode_include_types: vec!["store.v1.Item".into()],
        ..Default::default()
    };
    let tree = generate("service.proto", options.clone());
    assert_eq!(tree, generate("service.proto", options));
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let script = r#"import common_custom_pb2 as m
import _poolster_codecs_presentation_test as codec
import poolster_rpc_presentation_test as index
assert m.PreItemModel is m.Item and m.PreItemModel.PreDetailModel is m.Item.Detail
item=m.PreItemModel(name='configured',detail=m.Item.PreDetailModel(value='nested'))
assert codec.decode(m.Item,codec.encode(item))==item and index.common.Item is m.Item
try:codec.encode(m.Item.Detail(value='excluded'))
except ValueError:pass
else:raise AssertionError('encoder selection ignored')
try:codec.decode(m.Item.Detail,b'')
except ValueError:pass
else:raise AssertionError('decoder selection ignored')
"#;
    let result = Command::new(&python)
        .arg("-c")
        .arg(script)
        .env("PYTHONPATH", root.path().join("sdk/src"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let consumer = root.path().join("consumer.py");
    std::fs::write(&consumer,"from common_custom_pb2 import PreItemModel\nitem: PreItemModel = PreItemModel(name='typed')\nchild: PreItemModel.PreDetailModel = PreItemModel.PreDetailModel(value='child')\n").unwrap();
    let result = Command::new(&python)
        .args(["-m", "mypy", "--strict", "--no-incremental"])
        .arg(consumer)
        .env("MYPYPATH", root.path().join("sdk/src"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let external = generate(
        "common.proto",
        RpcGenerationOptions {
            file_suffix: "_external".into(),
            ..Default::default()
        },
    );
    let imported = generate(
        "service.proto",
        RpcGenerationOptions {
            file_suffix: "_client".into(),
            emit_imported_files: false,
            import_mappings: [
                ("common.proto".into(), "common_external_pb2".into()),
                ("legacy.proto".into(), "legacy_external_pb2".into()),
            ]
            .into(),
            ..Default::default()
        },
    );
    assert!(imported.get("sdk/src/common_client_pb2.py").is_none());
    let external_root = tempfile::tempdir().unwrap();
    external.write_to(external_root.path()).unwrap();
    let imported_root = tempfile::tempdir().unwrap();
    imported.write_to(imported_root.path()).unwrap();
    let paths = std::env::join_paths([
        imported_root.path().join("sdk/src"),
        external_root.path().join("sdk/src"),
    ])
    .unwrap();
    let result=Command::new(python).arg("-c").arg("import common_external_pb2 as m\nimport service_client_pb2_grpc as s\nclass Channel:\n def unary_unary(self,*args,**kw): return lambda request,**options:kw['response_deserializer'](kw['request_serializer'](request))\n unary_stream=stream_unary=stream_stream=unary_unary\nassert s.StoreStub(Channel()).Get(m.Item(name='external')).name=='external'\n").env("PYTHONPATH",paths).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[ignore = "requires pinned Python/mypy/build runtime"]
fn declaration_only_packages_and_enum_aliases_are_usable() {
    let python = std::env::var("POOLSTER_PYTHON_GRPC").unwrap();
    let options = RpcGenerationOptions {
        remove_enum_prefix: true,
        ..Default::default()
    };
    let tree = generate("aliases.proto", options.clone());
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let result=Command::new(&python).args(["-c", "import aliases_pb2 as m\nassert m.RED==m.COLOR_RED==1 and m.Item.READY==m.Item.STATE_READY==1\nassert m.Color.Name(m.RED)=='COLOR_RED'\nassert m.Item.State.Name(m.Item.READY)=='STATE_READY'\nassert m.Item(color=m.RED,state=m.Item.READY).SerializeToString()==b'\\x08\\x01\\x10\\x01'\n"])
        .env("PYTHONPATH", root.path().join("sdk/src")).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let types = generate(
        "aliases.proto",
        RpcGenerationOptions {
            emit_binary: false,
            emit_json: false,
            emit_clients: false,
            emit_servers: false,
            emit_reflection: false,
            ..options
        },
    );
    let types_root = tempfile::tempdir().unwrap();
    types.write_to(types_root.path()).unwrap();
    assert!(types.get("sdk/src/aliases_pb2.py").is_none());
    assert!(
        types
            .get("sdk/src/_poolster_rpc_presentation_test.py")
            .is_none()
    );
    assert!(types.get("sdk/src/aliases_pb2.pyi").is_some());
    let consumer = types_root.path().join("consumer.py");
    std::fs::write(
        &consumer,
        "from aliases_pb2 import Item, RED\nitem: Item = Item(color=RED,state=Item.READY)\n",
    )
    .unwrap();
    let result = Command::new(&python)
        .args(["-m", "mypy", "--strict", "--no-incremental"])
        .arg(&consumer)
        .env("MYPYPATH", types_root.path().join("sdk/src"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let result = Command::new(&python)
        .args(["setup.py", "bdist_wheel"])
        .current_dir(types_root.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let result=Command::new(&python).args(["-c","from pathlib import Path\nimport zipfile\nwheel=next(Path('dist').glob('*.whl'))\nwith zipfile.ZipFile(wheel) as z:\n assert 'aliases_pb2.pyi' in z.namelist()\n assert not any(n.endswith('.py') for n in z.namelist())\n"])
        .current_dir(types_root.path().join("sdk")).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
