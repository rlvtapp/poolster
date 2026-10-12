use poolster_input_protobuf as protobuf;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/rpc")
        .join(name)
}

#[test]
fn protobuf_preserves_imports_and_all_streaming_modes() {
    let document = protobuf::load(&fixture("service.proto")).unwrap();
    assert!(
        document
            .descriptors
            .get_message_by_name("demo.Item")
            .is_some()
    );
    let summary = document.summary();
    assert_eq!(summary.title, "demo");
    assert!(summary.types.contains(&"demo.Item.State".to_owned()));
    assert!(summary.types.contains(&"demo.Store".to_owned()));
    assert!(
        !summary
            .types
            .iter()
            .any(|name| name.ends_with("AttributesEntry"))
    );
    let item = document
        .descriptors
        .get_message_by_name("demo.Item")
        .unwrap();
    assert!(item.get_field_by_name("attributes").unwrap().is_map());
    let operations: Vec<_> = summary
        .operations
        .iter()
        .map(|operation| (operation.name.as_str(), operation.kind.as_str()))
        .collect();
    assert_eq!(
        operations,
        vec![
            ("demo.Store.Chat", "bidirectional_streaming"),
            ("demo.Store.Get", "unary"),
            ("demo.Store.Upload", "client_streaming"),
            ("demo.Store.Watch", "server_streaming")
        ]
    );
}

#[test]
fn protobuf_rejects_unknown_types() {
    assert!(protobuf::load(&fixture("invalid.proto")).is_err());
}

#[test]
fn protobuf_reports_missing_import() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("missing.proto");
    std::fs::write(&file, r#"syntax = "proto3"; import "absent.proto";"#).unwrap();
    assert!(protobuf::load(&file).is_err());
}

fn compile_source(source: &str) -> anyhow::Result<protobuf::ProtobufDocument> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("schema.proto");
    std::fs::write(&file, source)?;
    protobuf::load(&file)
}

#[test]
fn package_less_models_have_filename_title_and_no_operations() {
    let document =
        compile_source(r#"syntax = "proto3"; message Item { string name = 1; }"#).unwrap();
    assert_eq!(document.summary().title, "schema.proto");
    assert_eq!(document.summary().types, ["Item"]);
    assert!(document.summary().operations.is_empty());
}

#[test]
fn proto2_preserves_required_defaults_and_oneofs() {
    let document = compile_source(r#"syntax = "proto2"; message Item { required string name = 1 [default = "hello"]; oneof choice { int32 count = 2; string label = 3; } }"#).unwrap();
    let item = document.descriptors.get_message_by_name("Item").unwrap();
    let name = item.get_field_by_name("name").unwrap();
    assert_eq!(
        name.cardinality(),
        protox::prost_reflect::Cardinality::Required
    );
    assert_eq!(
        name.field_descriptor_proto().default_value.as_deref(),
        Some("hello")
    );
    assert_eq!(item.oneofs().next().unwrap().fields().count(), 2);
}

#[test]
fn standard_well_known_imports_resolve_without_vendoring() {
    let document = compile_source(r#"syntax = "proto3"; import "google/protobuf/timestamp.proto"; message Event { google.protobuf.Timestamp time = 1; }"#).unwrap();
    assert!(
        document
            .descriptors
            .get_message_by_name("google.protobuf.Timestamp")
            .is_some()
    );
    let event = document.descriptors.get_message_by_name("Event").unwrap();
    assert_eq!(
        event
            .get_field_by_name("time")
            .unwrap()
            .field_descriptor_proto()
            .type_name
            .as_deref(),
        Some(".google.protobuf.Timestamp")
    );
}

#[test]
fn nested_directory_and_transitive_imports_resolve() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("models")).unwrap();
    std::fs::write(
        dir.path().join("models/base.proto"),
        r#"syntax = "proto3"; package base; message Item { string name = 1; }"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("bridge.proto"),
        r#"syntax = "proto3"; import "models/base.proto"; message Wrapper { base.Item item = 1; }"#,
    )
    .unwrap();
    let root = dir.path().join("root.proto");
    std::fs::write(&root, r#"syntax = "proto3"; import "bridge.proto"; service Store { rpc Get(Wrapper) returns (Wrapper); }"#).unwrap();
    let document = protobuf::load(&root).unwrap();
    assert!(
        document
            .descriptors
            .get_message_by_name("base.Item")
            .is_some()
    );
    assert_eq!(document.descriptors.files().count(), 3);
    assert_eq!(document.summary().operations[0].name, "Store.Get");
}

#[test]
fn rejects_duplicate_field_tags() {
    assert!(
        compile_source(r#"syntax = "proto3"; message Item { string a = 1; string b = 1; }"#)
            .is_err()
    );
}

#[test]
fn rejects_rpc_with_non_message_types() {
    assert!(
        compile_source(r#"syntax = "proto3"; service Store { rpc Get(string) returns (string); }"#)
            .is_err()
    );
}

#[test]
fn missing_source_reports_its_path() {
    let error = protobuf::load(&fixture("absent.proto")).unwrap_err();
    assert!(error.to_string().contains("absent.proto"));
}

#[test]
fn provider_publishes_native_descriptor_contract() {
    use poolster_core::input::InputPlugin;
    let input = protobuf::ProtobufInput
        .load(&fixture("service.proto"))
        .unwrap();
    let document = input.get::<protobuf::ProtobufDocument>().unwrap();
    assert_eq!(input.summary, document.summary());
    assert!(
        document
            .descriptors
            .get_service_by_name("demo.Store")
            .is_some()
    );
}

#[test]
fn provider_propagates_invalid_schema() {
    use poolster_core::input::InputPlugin;
    assert!(
        protobuf::ProtobufInput
            .load(&fixture("invalid.proto"))
            .is_err()
    );
}

#[test]
fn official_protobuf_conformance_corpus_preserves_native_descriptors() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/upstream");
    let document = protobuf::load_with_includes(
        &root.join("google/protobuf/unittest.proto"),
        std::slice::from_ref(&root),
    )
    .unwrap();
    let summary = document.summary();
    eprintln!(
        "Official protobuf corpus: {} files, {} types, {} operations",
        document.descriptors.files().count(),
        summary.types.len(),
        summary.operations.len()
    );
    assert_eq!(document.descriptors.files().count(), 3);
    assert!(summary.types.len() > 100);
    assert!(
        document
            .descriptors
            .get_message_by_name("protobuf_unittest.TestAllTypes")
            .unwrap()
            .fields()
            .count()
            > 70
    );
    assert!(
        document
            .descriptors
            .get_message_by_name("protobuf_unittest_import.ImportMessage")
            .is_some()
    );
    assert!(
        document
            .descriptors
            .get_service_by_name("protobuf_unittest.TestService")
            .is_some()
    );
}

#[test]
fn official_editions_corpus_reports_unsupported_capability() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/upstream-editions");
    let error = protobuf::load_with_includes(
        &root.join("google/protobuf/unittest.proto"),
        std::slice::from_ref(&root),
    )
    .unwrap_err();
    assert!(format!("{error:#}").contains("Protobuf editions are not supported"));
}

#[test]
fn owned_wire_metadata_preserves_comments_and_deprecation() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("comments.proto");
    std::fs::write(&path, "syntax=\"proto3\"; package docs;\n// Documented payload.\nmessage Payload {\n// Old field.\nstring value=1 [deprecated=true];\n}\n// Documented service.\nservice Store {\n// Read payload.\nrpc Read(Payload) returns(Payload) {option deprecated=true;}\n}\n").unwrap();
    let wire = protobuf::load(&path)
        .unwrap()
        .rpc_contract()
        .wire_schema()
        .unwrap();
    let metadata = wire.metadata().unwrap();
    use poolster_core::native::rpc::RpcDefinitionKind;
    let file = metadata
        .iter()
        .find(|f| f.file == "comments.proto")
        .unwrap();
    let message = file
        .definition(RpcDefinitionKind::Message, "docs.Payload")
        .unwrap();
    assert!(message.leading_comments.contains("Documented payload."));
    assert!(
        file.definition(RpcDefinitionKind::Field, "docs.Payload.value")
            .unwrap()
            .deprecated
    );
    let method = file
        .definition(RpcDefinitionKind::Method, "docs.Store.Read")
        .unwrap();
    assert!(method.leading_comments.contains("Read payload.") && method.deprecated);
}

#[test]
fn explicit_batch_mappings_are_checked_against_owned_wire_types() {
    use poolster_core::native::rpc::{RpcBatchMapping, RpcGenerationOptions};
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/conformance/batching.proto");
    let wire = protobuf::load(&path)
        .unwrap()
        .rpc_contract()
        .wire_schema()
        .unwrap();
    let mapping = RpcBatchMapping {
        single_method: "poolster.conformance.batch.Reader.Read".into(),
        batch_method: "poolster.conformance.batch.Reader.ReadMany".into(),
        input_field: "id".into(),
        batch_input_field: "ids".into(),
        batch_output_field: "entities".into(),
        output_key: Some("id".into()),
        cache: true,
        max_batch_size: 100,
    };
    let mut options = RpcGenerationOptions {
        batching: vec![mapping.clone()],
        ..Default::default()
    };
    options.validate_for(&wire).unwrap();
    options.batching.push(mapping.clone());
    assert!(options.validate_for(&wire).is_err());
    for invalid in [
        RpcBatchMapping {
            input_field: "missing".into(),
            ..mapping.clone()
        },
        RpcBatchMapping {
            output_key: Some("value".into()),
            max_batch_size: 0,
            ..mapping.clone()
        },
        RpcBatchMapping {
            batch_method: mapping.single_method.clone(),
            ..mapping.clone()
        },
        RpcBatchMapping {
            batch_output_field: "missing".into(),
            ..mapping.clone()
        },
    ] {
        assert!(invalid.validate(&wire).is_err());
    }
    let positional = RpcBatchMapping {
        output_key: None,
        ..mapping
    };
    positional.validate(&wire).unwrap();
}

#[test]
fn generation_selections_resolve_owned_message_identities() {
    use poolster_core::native::rpc::RpcGenerationOptions;
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/conformance/batching.proto");
    let wire = protobuf::load(&path)
        .unwrap()
        .rpc_contract()
        .wire_schema()
        .unwrap();
    let mut options = RpcGenerationOptions {
        encode_include_types: vec!["poolster.conformance.batch.Entity".into()],
        ..Default::default()
    };
    options.validate_for(&wire).unwrap();
    options
        .decode_include_types
        .push("poolster.conformance.batch.Missing".into());
    assert!(
        options
            .validate_for(&wire)
            .unwrap_err()
            .to_string()
            .contains("unknown message")
    );
    options.decode_include_types.clear();
    options
        .import_mappings
        .insert("missing.proto".into(), "external.models".into());
    assert!(
        options
            .validate_for(&wire)
            .unwrap_err()
            .to_string()
            .contains("unknown descriptor file")
    );
}

#[test]
fn owned_wire_metadata_retains_javascript_integer_overrides() {
    use poolster_core::native::rpc::{RpcDefinitionKind, RpcJavaScriptType};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("jstype.proto");
    std::fs::write(
        &path,
        r#"syntax="proto3"; package overrides;
        message Value {int64 text=1 [jstype=JS_STRING]; uint64 number=2 [jstype=JS_NUMBER];}
    "#,
    )
    .unwrap();
    let wire = protobuf::load(&path)
        .unwrap()
        .rpc_contract()
        .wire_schema()
        .unwrap();
    let metadata = wire.metadata().unwrap();
    let file = metadata.iter().find(|f| f.file == "jstype.proto").unwrap();
    assert_eq!(
        file.definition(RpcDefinitionKind::Field, "overrides.Value.text")
            .unwrap()
            .javascript_type,
        Some(RpcJavaScriptType::String)
    );
    assert_eq!(
        file.definition(RpcDefinitionKind::Field, "overrides.Value.number")
            .unwrap()
            .javascript_type,
        Some(RpcJavaScriptType::Number)
    );
}
