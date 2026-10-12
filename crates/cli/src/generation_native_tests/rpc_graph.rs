//! Real output plugins must bind to the selected contract revision.
use poolster::{go, python, ts};
use poolster_core::{
    GeneratedFile,
    blocks::ContractReference,
    engine::{Contract, Package, Packages, hooks},
    input::InputProvider,
    native::rpc::RpcContract,
};
use std::sync::Arc;

#[test]
fn every_owned_output_uses_transformed_wire_contract_and_symbol_provenance() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("original.proto");
    let changed = root.path().join("changed.proto");
    for (path, message) in [(&original, "Original"), (&changed, "Transformed")] {
        std::fs::write(path, format!(
            "syntax=\"proto3\"; package revision; option go_package=\"example.com/revision/wire\"; message {message} {{string value=1;}}"
        )).unwrap();
    }
    let replacement = poolster_inputs::protobuf::load(&changed)
        .unwrap()
        .rpc_contract();
    let expected_revision =
        ContractReference::from_bytes(RpcContract::NAME, "unused", &replacement.descriptor_set)
            .revision;
    macro_rules! check {
        ($language:ty, $generator:expr, $symbols:ty, $provenance:ident) => {{
            let input = InputProvider::<RpcContract>::new(
                Arc::new(poolster_inputs::default_registry().unwrap()),
                "protobuf",
                &original,
            );
            let value = replacement.clone();
            let transform = hooks::<$language>().provides::<RpcContract>().on_contract(
                Some(input.handle()),
                move |_, cx| {
                    let reference = ContractReference::from_bytes(
                        RpcContract::NAME,
                        cx.input_reference.unwrap().instance.clone(),
                        &value.descriptor_set,
                    );
                    assert_ne!(reference.revision, cx.input_reference.unwrap().revision);
                    cx.publish_with_reference(value.clone(), reference)
                },
            );
            let output = ($generator).input(transform.handle::<RpcContract>());
            let revision = expected_revision.clone();
            let companion = hooks::<$language>().on_contract(
                Some(output.handle()),
                move |symbols: &$symbols, cx| {
                    assert!(symbols.messages.contains_key("revision.Transformed"));
                    assert!(!symbols.messages.contains_key("revision.Original"));
                    assert_eq!(symbols.$provenance.as_ref().unwrap().revision, revision);
                    cx.files
                        .emit(GeneratedFile::new("selected-revision.txt", &revision)?)
                },
            );
            let tree = Packages::new()
                .package(
                    Package::<$language>::new("sdk")
                        .with(companion)
                        .with(output)
                        .with(transform)
                        .with(input),
                )
                .generate_native()
                .unwrap();
            assert_eq!(
                tree.get("sdk/selected-revision.txt"),
                Some(expected_revision.as_str())
            );
        }};
    }
    check!(ts::TypeScript, ts::grpc(), ts::ProtobufSymbols, source);
    check!(
        python::Python,
        python::grpc("revision"),
        python::ProtobufSymbols,
        source
    );
    check!(
        go::Go,
        go::grpc("example.com/revision"),
        go::contracts::rpc::GrpcApi,
        provenance
    );
}
