use poolster_core::{
    engine::{Package, Packages},
    input::{InputOptions, InputProvider, InputRegistry},
    native::rpc::{RpcContract, RpcGenerationOptions},
};
use poolster_plugin_go::{Go, GrpcToolchain, grpc};
use std::{path::PathBuf, sync::Arc};
pub(super) fn generate(options: RpcGenerationOptions) -> poolster_core::GeneratedTree {
    generate_result(options).unwrap()
}
pub(super) fn generate_result(
    options: RpcGenerationOptions,
) -> anyhow::Result<poolster_core::GeneratedTree> {
    generate_with_source(options, "")
}
pub(super) fn generate_with_source(
    options: RpcGenerationOptions,
    definitions: &str,
) -> anyhow::Result<poolster_core::GeneratedTree> {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../inputs/protobuf/tests/fixtures/conformance");
    let source = tempfile::tempdir().unwrap();
    let schema = source.path().join("bundle.proto");
    std::fs::write(&schema,format!("syntax=\"proto3\";package bundle;option go_package=\"example.com/poolster/conformance/bundle;bundle\";import \"portable.proto\";import \"legacy.proto\";import \"batching.proto\";{definitions}")).unwrap();
    let mut registry = InputRegistry::new();
    registry
        .register(poolster_input_protobuf::ProtobufInput)
        .unwrap();
    let input = InputProvider::<RpcContract>::new(Arc::new(registry), "protobuf", schema)
        .with_options(InputOptions {
            import_roots: vec![corpus],
            ..Default::default()
        });
    let output = grpc("example.com/poolster/conformance")
        .input(input.handle())
        .options(options)
        .toolchain(GrpcToolchain {
            protoc: "/missing/protoc".into(),
            protoc_gen_go: "/missing/plugin".into(),
            protoc_gen_go_grpc: "/missing/plugin".into(),
        });
    Packages::new()
        .package(Package::<Go>::new("sdk").with(output).with(input))
        .generate_native()
}
