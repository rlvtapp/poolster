use anyhow::Result;
use poolster_core::{
    AdditionalProperties, Field, SchemaKind, SchemaValue,
    blocks::ContractReference,
    engine::{Contract, Handle, Meta, Packages, Plugin, PluginContext, Provision},
    native::events::{
        EventAction, EventMessage, EventOperation, EventOperations, KafkaBroker, model_blocks,
    },
};
use poolster_plugin_swift as output;
use std::collections::BTreeMap;
struct Source {
    meta: Meta,
    input: EventOperations,
    revision: Option<String>,
}
impl Source {
    fn new() -> Self {
        let value = |fields: Vec<Field>| {
            SchemaValue::new(SchemaKind::Object {
                fields,
                additional_properties: AdditionalProperties::Forbidden,
            })
        };
        let field = |name: &str, kind| Field {
            name: name.into(),
            value: SchemaValue::new(kind),
            required: true,
            annotations: BTreeMap::new(),
        };
        let message = EventMessage {
            name: "Order".into(),
            location: "/messages/order".into(),
            payload: value(vec![
                field("id", SchemaKind::String),
                field("quantity", SchemaKind::Integer),
            ]),
            payload_schema: serde_json::json!({"type":"object","properties":{"id":{"type":"string","minLength":1},"quantity":{"type":"integer","minimum":1}},"required":["id","quantity"],"additionalProperties":false}),
            headers: Some(value(vec![field("trace", SchemaKind::String)])),
            headers_schema: Some(
                serde_json::json!({"type":"object","properties":{"trace":{"type":"string","minLength":1}},"required":["trace"],"additionalProperties":false}),
            ),
            key: Some(SchemaValue::new(SchemaKind::String)),
            key_schema: Some(serde_json::json!({"type":"string","minLength":1})),
            bindings: serde_json::Value::Null,
        };
        Self {
            meta: Meta::new(),
            revision: Some("opaque-v1".into()),
            input: EventOperations {
                source: serde_json::json!({"asyncapi":"3.0.0"}),
                broker: KafkaBroker {
                    brokers: vec!["127.0.0.1:29092".into()],
                    client_id: Some("swift-default".into()),
                    bindings: serde_json::Value::Null,
                },
                operations: vec![
                    EventOperation {
                        name: "sendOrder".into(),
                        action: EventAction::Send,
                        channel: "orders".into(),
                        topic: "poolster-swift-orders".into(),
                        message: message.clone(),
                        group_id: None,
                        client_id: Some("swift-bound-producer".into()),
                        bindings: serde_json::Value::Null,
                        channel_bindings: serde_json::Value::Null,
                    },
                    EventOperation {
                        name: "receiveOrder".into(),
                        action: EventAction::Receive,
                        channel: "orders".into(),
                        topic: "poolster-swift-orders".into(),
                        message,
                        group_id: Some("swift-bound-group".into()),
                        client_id: Some("swift-bound-consumer".into()),
                        bindings: serde_json::Value::Null,
                        channel_bindings: serde_json::Value::Null,
                    },
                ],
            },
        }
    }
    fn handle(&self) -> Handle<EventOperations> {
        self.meta.handle()
    }
}
impl Plugin<output::Swift> for Source {
    fn kind(&self) -> &'static str {
        "swift-event-fixture"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<EventOperations>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, output::Swift>) -> Result<()> {
        if let Some(revision) = &self.revision {
            cx.publish_with_reference(
                self.input.clone(),
                ContractReference {
                    contract: EventOperations::NAME.into(),
                    instance: "events".into(),
                    revision: revision.clone(),
                },
            )
        } else {
            cx.publish(self.input.clone())
        }
    }
}
fn generate(source: Source, shared: bool) -> Result<poolster_core::GeneratedTree> {
    let events = output::asyncapi(Some(source.handle()));
    let package = if shared {
        let blocks = model_blocks(Some(source.handle()), "events");
        let models = output::shared_models(None).input_shapes(blocks.handle());
        output::package("sdk")
            .with(events.using_models(models.handle()))
            .with(models)
            .with(blocks)
            .with(source)
    } else {
        output::package("sdk").with(events).with(source)
    };
    Packages::new().package(package).generate_native()
}
#[test]
fn shared_standalone_opaque_provenance_and_regeneration() {
    for shared in [false, true] {
        let tree = generate(Source::new(), shared).unwrap();
        assert_eq!(tree, generate(Source::new(), shared).unwrap());
        assert!(
            tree.get("sdk/Package.swift")
                .unwrap()
                .contains("CPoolsterKafka")
        );
        assert!(
            tree.get("sdk/Sources/PoolsterModels/SendOrderEvent.swift")
                .unwrap()
                .contains("OrderPayload")
        );
    }
    let source = Source::new();
    let original = Source::new();
    let blocks = model_blocks(Some(original.handle()), "events");
    let models = output::shared_models(None).input_shapes(blocks.handle());
    let mut changed = source;
    changed.revision = Some("opaque-v2".into());
    let result = Packages::new()
        .package(
            output::package("sdk")
                .with(output::asyncapi(Some(changed.handle())).using_models(models.handle()))
                .with(models)
                .with(blocks)
                .with(original)
                .with(changed),
        )
        .generate_native();
    assert!(result.is_err());
}
#[test]
fn unsupported_keywords_bindings_and_large_layout() {
    let mut invalid = Source::new();
    invalid.input.operations[0].message.payload_schema["pattern"] = "unsupported".into();
    assert!(generate(invalid, true).is_err());
    let mut invalid = Source::new();
    invalid.input.operations[0]
        .message
        .key
        .as_mut()
        .unwrap()
        .nullable = true;
    assert!(generate(invalid, true).is_err());
    for topic in ["", ".", "..", "not a topic", "bad\0topic"] {
        let mut invalid = Source::new();
        invalid.input.operations[0].topic = topic.into();
        assert!(generate(invalid, true).is_err(), "accepted topic {topic:?}");
    }
    for schema in [
        serde_json::json!({"type":"object","required":42}),
        serde_json::json!({"type":"object","properties":[]}),
        serde_json::json!({"type":"made-up"}),
        serde_json::json!({"type":"number","minimum":1e100}),
    ] {
        let mut invalid = Source::new();
        invalid.input.operations[0].message.payload_schema = schema;
        assert!(generate(invalid, true).is_err());
    }
    let mut source = Source::new();
    let template = source.input.operations[0].clone();
    source.input.operations = (0..250)
        .map(|n| {
            let mut op = template.clone();
            op.name = format!("sendOrder{n}");
            op
        })
        .collect();
    let mut reversed = Source::new();
    reversed.input.operations = source.input.operations.clone();
    reversed.input.operations.reverse();
    let tree = generate(source, true).unwrap();
    assert_eq!(tree, generate(reversed, true).unwrap());
    assert_eq!(
        tree.iter()
            .filter(|(p, _)| p.to_string_lossy().ends_with("Event.swift"))
            .count(),
        250
    );
    for (path, contents) in tree.iter() {
        if path.extension().is_some_and(|s| s == "swift") {
            assert!(contents.len() <= 128 * 1024);
        }
    }
}
#[test]
#[ignore = "requires Swift6, pinned librdkafka2.12.0, Docker pinned local Kafka broker"]
fn generated_swift_package_compiles_and_roundtrips_broker() {
    let mut source = Source::new();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    for op in &mut source.input.operations {
        op.topic = format!("poolster-swift-{nonce}");
        if op.group_id.is_some() {
            op.group_id = Some(format!("poolster-swift-group-{nonce}"));
        }
    }
    for op in &mut source.input.operations {
        op.message.payload_schema["description"] =
            serde_json::json!("literal \\(fatalError(\"metadata executed\"))\n\0");
    }
    let topic = source.input.operations[0].topic.clone();
    let topics: Vec<_> = std::iter::once(topic.clone())
        .chain((0..9).map(|n| format!("{topic}-invalid-{n}")))
        .collect();
    let provision = std::process::Command::new("docker")
        .args(["exec", &broker_container(), "rpk", "topic", "create"])
        .args(&topics)
        .output()
        .unwrap();
    assert!(
        provision.status.success(),
        "test topic provisioning: {}",
        String::from_utf8_lossy(&provision.stderr)
    );
    run_native(
        generate(source, true).unwrap(),
        include_str!("fixtures/events.swift"),
    );
}

fn run_native(tree: poolster_core::GeneratedTree, probe: &str) {
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    let sdk = dir.path().join("sdk");
    // SwiftPM discovers nested model sources in the same module. Keep this probe
    // independent from canonical shared-model file placement.
    let model_root = sdk.join("Sources/PoolsterModels");
    let nested = model_root.join("models/provider");
    std::fs::create_dir_all(&nested).unwrap();
    for name in ["ProviderPayload", "ProviderHeaders", "ProviderKey"] {
        let path = model_root.join(format!("{name}.swift"));
        if path.exists() {
            std::fs::rename(&path, nested.join(format!("{name}.swift"))).unwrap();
        }
    }

    let tests = sdk.join("Tests/PoolsterModelsTests");
    std::fs::create_dir_all(&tests).unwrap();
    std::fs::write(tests.join("KafkaTests.swift"), probe).unwrap();
    let manifest=std::fs::read_to_string(sdk.join("Package.swift")).unwrap().replace(".target(name: \"PoolsterModels\", dependencies: [\"PoolsterKafkaShim\"])",".target(name: \"PoolsterModels\", dependencies: [\"PoolsterKafkaShim\"]),\n        .testTarget(name: \"PoolsterModelsTests\", dependencies: [\"PoolsterModels\", \"PoolsterKafkaShim\"])");
    std::fs::write(sdk.join("Package.swift"), manifest).unwrap();
    let prefix = std::env::var("POOLSTER_SWIFT_KAFKA_PREFIX")
        .expect("POOLSTER_SWIFT_KAFKA_PREFIX points to pinned native install");
    let result = std::process::Command::new("swift")
        .args([
            "test",
            "--jobs",
            "4",
            "-Xcc",
            &format!("-I{prefix}/include"),
            "-Xlinker",
            &format!("-L{prefix}/lib"),
        ])
        .env("DYLD_LIBRARY_PATH", format!("{prefix}/lib"))
        .env("LD_LIBRARY_PATH", format!("{prefix}/lib"))
        .current_dir(sdk)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

struct RenamedShapes {
    meta: Meta,
    provider: Handle<poolster_core::blocks::Blocks<poolster_core::models::SchemaModelBlock>>,
}
impl RenamedShapes {
    fn handle(
        &self,
    ) -> Handle<poolster_core::blocks::Blocks<poolster_core::models::SchemaModelBlock>> {
        self.meta.handle()
    }
}
impl Plugin<output::Swift> for RenamedShapes {
    fn kind(&self) -> &'static str {
        "substituted-model-shapes"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<poolster_core::engine::Requirement> {
        vec![poolster_core::engine::Requirement::on(Some(self.provider))]
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<
            poolster_core::blocks::Blocks<poolster_core::models::SchemaModelBlock>,
        >()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, output::Swift>) -> Result<()> {
        let mut blocks = cx
            .inputs
            .get::<poolster_core::blocks::Blocks<poolster_core::models::SchemaModelBlock>>()?
            .clone();
        for block in &mut blocks.items {
            block.value.name = if block.metadata.id.local.ends_with("/payload") {
                "ProviderPayload"
            } else if block.metadata.id.local.ends_with("/headers") {
                "ProviderHeaders"
            } else {
                "ProviderKey"
            }
            .into();
        }
        cx.publish(blocks)
    }
}
#[test]
fn selected_provider_symbols_and_standalone_manifest_are_preserved() {
    let tree = renamed_tree();
    let operation = tree
        .get("sdk/Sources/PoolsterModels/SendOrderEvent.swift")
        .unwrap();
    assert!(operation.contains("ProviderPayload"));
    assert!(operation.contains("ProviderHeaders"));
    assert!(operation.contains("ProviderKey"));
    assert!(!operation.contains("OrderPayload"));
    let source = Source::new();
    let shapes = model_blocks(Some(source.handle()), "events");
    let models = output::shared_models(None).input_shapes(shapes.handle());
    let tree = Packages::new()
        .package(
            output::package("sdk")
                .with(models)
                .with(shapes)
                .with(source),
        )
        .generate_native()
        .unwrap();
    assert_eq!(
        tree.get("sdk/Package.swift").unwrap(),
        "// swift-tools-version: 5.9\nimport PackageDescription\n\nlet package = Package(\n    name: \"PoolsterModels\",\n    platforms: [.macOS(.v13), .iOS(.v16)],\n    products: [.library(name: \"PoolsterModels\", targets: [\"PoolsterModels\"])],\n    targets: [.target(name: \"PoolsterModels\")]\n)\n"
    );
}

fn renamed_tree() -> poolster_core::GeneratedTree {
    let source = Source::new();
    let shapes = model_blocks(Some(source.handle()), "events");
    let renamed = RenamedShapes {
        meta: Meta::new(),
        provider: shapes.handle(),
    };
    let models = output::shared_models(None).input_shapes(renamed.handle());
    let events = output::asyncapi(Some(source.handle())).using_models(models.handle());
    Packages::new()
        .package(
            output::package("sdk")
                .with(events)
                .with(models)
                .with(renamed)
                .with(shapes)
                .with(source),
        )
        .generate_native()
        .unwrap()
}

#[test]
#[ignore = "requires Swift6 and pinned librdkafka2.12.0"]
fn generated_selected_provider_and_standalone_compile() {
    run_native(
        renamed_tree(),
        r#"import XCTest
@testable import PoolsterModels
final class SelectedProviderTests: XCTestCase {
    func testActualSymbols() throws {
        let payload = ProviderPayload(id: "selected", quantity: 1)
        let headers = ProviderHeaders(trace: "selected")
        let key: ProviderKey = "selected"
        let encoded = try SendOrderEvent.definition.encode(payload, headers, key)
        let decoded = try SendOrderEvent.definition.decode(encoded)
        XCTAssertEqual(decoded.0.id, "selected")
    }
}
"#,
    );
    run_native(
        generate(Source::new(), false).unwrap(),
        r#"import XCTest
@testable import PoolsterModels
final class StandaloneTests: XCTestCase {
    func testStandaloneCodec() throws {
        let encoded = try SendOrderEvent.definition.encode(OrderPayload(id: "standalone", quantity: 1), OrderHeaders(trace: "standalone"), "standalone")
        let decoded = try SendOrderEvent.definition.decode(encoded)
        XCTAssertEqual(decoded.0.id, "standalone")
    }
}
"#,
    );
}

fn broker_container() -> String {
    std::env::var("POOLSTER_KAFKA_BROKER_CONTAINER")
        .unwrap_or_else(|_| "poolster-alpha2-kafka".into())
}
