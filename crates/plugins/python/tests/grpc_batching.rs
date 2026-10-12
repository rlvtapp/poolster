use poolster_core::{
    engine::{Package, Packages},
    input::{InputProvider, InputRegistry},
    native::rpc::{RpcBatchMapping, RpcContract, RpcGenerationOptions},
};
use poolster_plugin_python::{Python, grpc};
use std::{path::Path, process::Command, sync::Arc};
#[test]
#[ignore = "requires pinned Python runtime and local RPC server"]
fn explicit_request_owned_batching_sync_async_association_cache_and_errors() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../inputs/protobuf/tests/fixtures/conformance/batching.proto");
    let mut registry = InputRegistry::new();
    registry
        .register(poolster_input_protobuf::ProtobufInput)
        .unwrap();
    let input = InputProvider::<RpcContract>::new(Arc::new(registry), "protobuf", source);
    let plugin = grpc("poolster-rpc-test")
        .input(input.handle())
        .options(RpcGenerationOptions {
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
    let tree = Packages::new()
        .package(Package::<Python>::new("sdk").with(plugin).with(input))
        .generate_native()
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let output = Command::new(std::env::var("POOLSTER_PYTHON_GRPC").unwrap())
        .arg("-c")
        .arg(include_str!("fixtures/grpc/batching.py"))
        .env("PYTHONPATH", root.path().join("sdk/src"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
