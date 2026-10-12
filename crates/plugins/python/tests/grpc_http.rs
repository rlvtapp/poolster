use poolster_core::{
    engine::{Package, Packages},
    input::{InputProvider, InputRegistry},
    native::rpc::RpcContract,
};
use poolster_plugin_python::{Python, grpc};
use std::{path::Path, process::Command, sync::Arc};
#[test]
#[ignore = "requires pinned Python runtime in POOLSTER_PYTHON_GRPC"]
fn additive_twirp_and_grpc_web_clients_execute_native_http_protocols() {
    use poolster_core::native::rpc::{RpcGenerationOptions, RpcTransport};
    let python = std::env::var("POOLSTER_PYTHON_GRPC").unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grpc/service.proto");
    let mut registry = InputRegistry::new();
    registry
        .register(poolster_input_protobuf::ProtobufInput)
        .unwrap();
    let input = InputProvider::<RpcContract>::new(Arc::new(registry), "protobuf", source);
    let plugin = grpc("poolster-rpc-test")
        .input(input.handle())
        .options(RpcGenerationOptions {
            transports: vec![
                RpcTransport::Grpc,
                RpcTransport::Twirp,
                RpcTransport::GrpcWeb,
            ],
            ..Default::default()
        });
    let tree = Packages::new()
        .package(Package::<Python>::new("sdk").with(plugin).with(input))
        .generate_native()
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let result = Command::new(python)
        .arg("-c")
        .arg(include_str!("fixtures/grpc/http.py"))
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
