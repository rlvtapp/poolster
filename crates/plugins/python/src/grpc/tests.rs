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
    let output = render::package(
        &c,
        &schema,
        &files,
        "demo-sdk",
        "1.2.3",
        &Default::default(),
        poolster_core::native::rpc::RpcClientStyle::Idiomatic,
    )
    .unwrap();
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
    assert!(
        render::package(
            &c,
            &schema,
            &files,
            "demo-sdk",
            "1.0\"\ncode",
            &Default::default(),
            poolster_core::native::rpc::RpcClientStyle::Idiomatic,
        )
        .is_err()
    );
}

#[test]
fn emission_controls_are_independent_and_natively_validated() {
    use poolster_core::{
        engine::{Package, Packages},
        input::{InputProvider, InputRegistry},
    };
    use std::sync::Arc;
    let c = contract();
    let schema = c.wire_schema().unwrap();
    let files = validation::validate(&c, &schema, "controls").unwrap();
    for (clients, servers) in [(false, false), (true, false), (false, true)] {
        let options = poolster_core::native::rpc::RpcGenerationOptions {
            emit_clients: clients,
            emit_servers: servers,
            emit_types: !clients && !servers,
            emit_binary: false,
            emit_json: false,
            emit_reflection: false,
            ..Default::default()
        };
        let output = render::package(
            &c,
            &schema,
            &files,
            "controls",
            "1.0.0",
            &options,
            poolster_core::native::rpc::RpcClientStyle::Idiomatic,
        )
        .unwrap();
        assert_eq!(
            output.iter().any(|(p, _)| p.ends_with(".pyi")),
            !clients && !servers
        );
        assert!(!output.iter().any(|(p, _)| p.contains("codecs")));
        let service = output
            .iter()
            .find(|(p, _)| p.ends_with("service_pb2_grpc.py"));
        if clients || servers {
            let src = &service.unwrap().1;
            assert_eq!(src.contains("class StoreStub:"), clients);
            assert_eq!(src.contains("class StoreServicer:"), servers);
        } else {
            assert!(service.is_none());
            assert!(
                !output
                    .iter()
                    .find(|(p, _)| p == "pyproject.toml")
                    .unwrap()
                    .1
                    .contains("\"grpcio==")
            );
        }
    }
    let mut registry = InputRegistry::new();
    registry
        .register(poolster_input_protobuf::ProtobufInput)
        .unwrap();
    let input = InputProvider::<RpcContract>::new(
        Arc::new(registry),
        "protobuf",
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grpc/service.proto"),
    );
    let plugin = grpc("invalid").input(input.handle()).options(
        poolster_core::native::rpc::RpcGenerationOptions {
            max_depth: 101,
            ..Default::default()
        },
    );
    let error = Packages::new()
        .package(
            Package::<crate::Python>::new("sdk")
                .with(plugin)
                .with(input),
        )
        .generate_native()
        .unwrap_err();
    assert!(format!("{error:#}").contains("at most 100"));
}

#[test]
fn native_facade_styles_and_bounded_descriptor_modules_are_deterministic() {
    use poolster_core::native::rpc::RpcClientStyle;
    let contract = contract();
    let schema = contract.wire_schema().unwrap();
    for style in [
        RpcClientStyle::Raw,
        RpcClientStyle::Flat,
        RpcClientStyle::Idiomatic,
    ] {
        let output = styles::render(&schema, &Default::default(), style)
            .unwrap()
            .unwrap();
        assert!(output.0.contains("def store_get(channel, request"));
        assert!(output.0.contains("self.store = Group"));
        match style {
            RpcClientStyle::Raw => assert!(!output.0.contains("\nClient =")),
            RpcClientStyle::Flat => assert!(output.0.contains("\nClient = FlatClient")),
            RpcClientStyle::Idiomatic => assert!(output.0.contains("\nClient = GroupedClient")),
        }
        assert_eq!(
            output,
            styles::render(&schema, &Default::default(), style)
                .unwrap()
                .unwrap()
        );
    }
    let files = validation::validate(&contract, &schema, "demo").unwrap();
    let output = render::package(
        &contract,
        &schema,
        &files,
        "demo",
        "0.1.0",
        &Default::default(),
        RpcClientStyle::Idiomatic,
    )
    .unwrap();
    assert!(output.iter().any(|(path, _)| path.contains("_part_0.py")));
    for (_, content) in output.iter().filter(|(path, _)| path.contains("_part_")) {
        assert!(content.lines().count() <= 346);
    }
}
