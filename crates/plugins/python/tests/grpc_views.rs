use poolster_core::{
    engine::{Package, Packages},
    input::{InputProvider, InputRegistry},
    native::rpc::{RpcContract, RpcGenerationOptions},
};
use poolster_plugin_python::{Python, grpc};
use std::{path::Path, process::Command, sync::Arc};
#[test]
#[ignore = "requires pinned Python runtime"]
fn checked_views_value_adapters_and_async_stream_cancellation() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grpc/service.proto");
    let mut registry = InputRegistry::new();
    registry
        .register(poolster_input_protobuf::ProtobufInput)
        .unwrap();
    let input = InputProvider::<RpcContract>::new(Arc::new(registry), "protobuf", source);
    let plugin = grpc("poolster-rpc-test")
        .input(input.handle())
        .options(RpcGenerationOptions {
            immutable_views: true,
            reactive_streams: true,
            value_adapters: true,
            ..Default::default()
        });
    let tree = Packages::new()
        .package(Package::<Python>::new("sdk").with(plugin).with(input))
        .generate_native()
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let python = std::env::var("POOLSTER_PYTHON_GRPC").unwrap();
    let output = Command::new(&python)
        .arg("-c")
        .arg(include_str!("fixtures/grpc/views.py"))
        .env("PYTHONPATH", root.path().join("sdk/src"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let consumer = root.path().join("consumer.py");
    std::fs::write(&consumer,"from common_pb2 import Item\nfrom views_poolster_rpc_test import freeze\nitem = freeze(Item(name='typed'))\nname: str = item.name\nlabels: str = item.labels['key']\nchild: str = item.children[0].value\n").unwrap();
    let output = Command::new(&python)
        .args(["-m", "mypy", "--strict", "--no-incremental"])
        .arg(&consumer)
        .env("MYPYPATH", root.path().join("sdk/src"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::write(&consumer,"from common_pb2 import Item\nfrom views_poolster_rpc_test import freeze\nitem = freeze(Item())\nitem.name = 'mutable'\nitem.labels['key'] = 'mutable'\nitem.children[0].value = 'mutable'\n").unwrap();
    let output = Command::new(&python)
        .args(["-m", "mypy", "--strict", "--no-incremental"])
        .arg(&consumer)
        .env("MYPYPATH", root.path().join("sdk/src"))
        .output()
        .unwrap();
    let errors = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success()
            && errors.contains("read-only")
            && errors.contains("Unsupported target"),
        "{errors}"
    );
}
