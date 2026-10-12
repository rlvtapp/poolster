use super::*;
use std::path::Path;
fn contract() -> RpcContract {
    poolster_input_protobuf::load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grpc/service.proto"),
    )
    .unwrap()
    .rpc_contract()
}
#[test]
fn malformed_contracts_and_unsupported_names_are_rejected() {
    let contract = contract();
    let schema = contract.wire_schema().unwrap();
    assert_eq!(
        validation::validate(&contract, &schema, "demo-sdk").unwrap(),
        vec!["common.proto", "legacy.proto", "service.proto"]
    );
    for name in ["", "../escape", "pkg\"\ninjected", "-bad", "bad-"] {
        assert!(validation::validate(&contract, &schema, name).is_err());
    }
    for file in [
        "../service.proto",
        "bad-name.proto",
        "class/service.proto",
        "/service.proto",
        "service.PROTO",
    ] {
        let mut bad = schema.clone();
        bad.files.last_mut().unwrap().name = file.into();
        assert!(validation::validate(&contract, &bad, "demo").is_err());
    }
    let mut bad = contract.clone();
    bad.descriptor_set = vec![255];
    assert!(bad.wire_schema().is_err());
    let mut bad = schema.clone();
    bad.files.last_mut().unwrap().syntax = "editions".into();
    assert!(validation::validate(&contract, &bad, "demo").is_err());
    let mut bad = schema.clone();
    bad.files.push(schema.files[0].clone());
    assert!(validation::validate(&contract, &bad, "demo").is_err());
}
#[test]
fn generated_sources_preserve_wire_descriptors_and_do_not_need_tools() {
    let c = contract();
    let schema = c.wire_schema().unwrap();
    let files = validation::validate(&c, &schema, "demo-sdk").unwrap();
    let output = render::package(&c, &schema, &files, "demo-sdk", "1.2.3").unwrap();
    let manifest = &output
        .iter()
        .find(|(p, _)| p == "pyproject.toml")
        .unwrap()
        .1;
    assert!(manifest.contains("grpcio==1.78.0"));
    assert!(manifest.contains("protobuf==6.33.5"));
    assert!(manifest.contains("version = \"1.2.3\""));
    assert!(!manifest.contains("grpcio-tools"));
    let src = &output
        .iter()
        .find(|(p, _)| p.ends_with("service_pb2_grpc.py"))
        .unwrap()
        .1;
    for method in [
        "unary_unary",
        "unary_stream",
        "stream_unary",
        "stream_stream",
    ] {
        assert!(src.contains(method));
    }
    assert_eq!(
        output
            .iter()
            .find(|(p, _)| p == "proto/descriptor-set.json")
            .unwrap()
            .1,
        serde_json::to_string(&c.descriptor_set).unwrap()
    );
    assert!(render::package(&c, &schema, &files, "demo-sdk", "1.0\"\ncode").is_err());
}
