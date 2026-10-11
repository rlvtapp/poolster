use super::*;
use poolster_core::{
    AdditionalProperties, Field, SchemaKind, SchemaValue,
    engine::Packages,
    native::events::{EventMessage, EventOperation, KafkaBroker},
};
struct Source {
    meta: Meta,
    events: EventOperations,
}
impl Plugin<Rust> for Source {
    fn kind(&self) -> &'static str {
        "event-fixture"
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
    fn generate(&self, cx: &mut PluginContext<'_, Rust>) -> Result<()> {
        cx.publish(self.events.clone())
    }
}
fn fixture() -> EventOperations {
    let message = EventMessage {
        name: "Notice".into(),
        location: "/messages/notice".into(),
        payload: SchemaValue::new(SchemaKind::String),
        payload_schema: serde_json::json!({"type":"string","minLength":2}),
        headers: Some(SchemaValue::new(SchemaKind::Object {
            fields: vec![Field {
                name: "trace".into(),
                value: SchemaValue::new(SchemaKind::String),
                required: true,
                annotations: BTreeMap::new(),
            }],
            additional_properties: AdditionalProperties::Forbidden,
        })),
        headers_schema: Some(
            serde_json::json!({"type":"object","properties":{"trace":{"type":"string","minLength":2}},"required":["trace"],"additionalProperties":false}),
        ),
        key: Some(SchemaValue::new(SchemaKind::String)),
        key_schema: Some(serde_json::json!({"type":"string","minLength":2})),
        bindings: serde_json::Value::Null,
    };
    EventOperations {
        source: serde_json::json!({"asyncapi":"3.0.0"}),
        broker: KafkaBroker {
            brokers: vec!["127.0.0.1:29092".into()],
            client_id: None,
            bindings: serde_json::Value::Null,
        },
        operations: vec![
            EventOperation {
                name: "sendNotice".into(),
                action: EventAction::Send,
                channel: "notices".into(),
                topic: "poolster-rust-probe".into(),
                message: message.clone(),
                group_id: None,
                client_id: Some("poolster-rust-operation".into()),
                bindings: serde_json::Value::Null,
                channel_bindings: serde_json::Value::Null,
            },
            EventOperation {
                name: "receiveNotice".into(),
                action: EventAction::Receive,
                channel: "notices".into(),
                topic: "poolster-rust-probe".into(),
                message,
                group_id: Some("poolster-rust-probe".into()),
                client_id: None,
                bindings: serde_json::Value::Null,
                channel_bindings: serde_json::Value::Null,
            },
        ],
    }
}
fn generate(events: EventOperations) -> Result<poolster_core::GeneratedTree> {
    let source = Source {
        meta: Meta::new(),
        events,
    };
    let plugin = asyncapi(Some(source.meta.handle()));
    Packages::new()
        .package(crate::package("sdk").with(plugin).with(source))
        .generate_native()
}
#[test]
fn package_structure_and_subset_diagnostics() {
    let tree = generate(fixture()).unwrap();
    assert!(
        tree.iter()
            .any(|(path, _)| path.ends_with("events/operations/send_notice.rs"))
    );
    let mut bad = fixture();
    bad.operations[0].message.payload_schema["default"] = serde_json::json!("x");
    assert!(format!("{:#}", generate(bad).unwrap_err()).contains("keyword"));
}
#[test]
#[ignore = "requires a local Kafka broker, network access, C/C++ toolchain and CMake"]
fn native_generated_kafka_package() {
    let mut events = fixture();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    for op in &mut events.operations {
        op.topic = format!("poolster-rust-{unique}");
        op.group_id = Some(format!("poolster-rust-{unique}"));
    }
    let tree = generate(events).unwrap();
    let destination = std::path::Path::new("/private/tmp/poolster-rust-kafka-generated");
    for (path, contents) in tree.iter() {
        let path = path.strip_prefix("sdk").unwrap_or(path);
        let target = destination.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, contents).unwrap();
    }
    let kafka = destination.join("src/events/runtime/kafka.rs");
    let mut runtime = std::fs::read_to_string(&kafka).unwrap();
    runtime.push_str(include_str!("runtime/bindings_probe.rs.tmpl"));
    std::fs::write(kafka, runtime).unwrap();
    std::fs::create_dir_all(destination.join("tests")).unwrap();
    std::fs::write(
        destination.join("tests/probe.rs"),
        include_str!("runtime/probe.rs.tmpl"),
    )
    .unwrap();
    let mut command = std::process::Command::new("cargo");
    if std::path::Path::new("/private/tmp/poolster-rust-kafka-tools/bin/cmake").exists() {
        command.env("CMAKE", "/private/tmp/poolster-rust-kafka-tools/bin/cmake");
    }
    let result = command
        .args(["test", "--manifest-path"])
        .arg(destination.join("Cargo.toml"))
        .env(
            "CARGO_TARGET_DIR",
            "/private/tmp/poolster-rust-kafka-target",
        )
        .status()
        .unwrap();
    assert!(result.success());
}
#[test]
fn explicit_event_model_provider_and_regeneration() {
    let source = Source {
        meta: Meta::new(),
        events: fixture(),
    };
    let blocks = poolster_core::native::events::model_blocks(Some(source.meta.handle()), "fixture");
    let models = crate::shared_models(None).input_shapes(blocks.handle());
    let events = asyncapi(Some(source.meta.handle())).using_models(models.handle());
    let tree = Packages::new()
        .package(
            crate::package("sdk")
                .with(events)
                .with(models)
                .with(blocks)
                .with(source),
        )
        .generate_native()
        .unwrap();
    assert!(tree.iter().any(|(_, source)| source.contains("rdkafka")));
    let first = generate(fixture()).unwrap();
    let second = generate(fixture()).unwrap();
    assert_eq!(
        first.iter().collect::<Vec<_>>(),
        second.iter().collect::<Vec<_>>()
    );
}
#[test]
fn many_operations_are_split_into_bounded_group_modules() {
    let mut events = fixture();
    let template = events.operations[0].clone();
    events.operations = (0..250)
        .map(|i| {
            let mut op = template.clone();
            op.name = format!("sendNotice{i}");
            op
        })
        .collect();
    let tree = generate(events).unwrap();
    assert_eq!(
        tree.iter()
            .filter(|(path, _)| path.to_string_lossy().contains("events/groups/"))
            .count(),
        3
    );
    for (_, source) in tree.iter() {
        assert!(source.len() < 128 * 1024);
    }
}
#[test]
fn transformed_event_revision_cannot_reuse_stale_models() {
    let original = Source {
        meta: Meta::new(),
        events: fixture(),
    };
    let mut transformed = fixture();
    transformed.operations[0].topic = "another-topic".into();
    let selected = Source {
        meta: Meta::new(),
        events: transformed,
    };
    let blocks =
        poolster_core::native::events::model_blocks(Some(original.meta.handle()), "fixture");
    let models = crate::shared_models(None).input_shapes(blocks.handle());
    let events = asyncapi(Some(selected.meta.handle())).using_models(models.handle());
    let error = Packages::new()
        .package(
            crate::package("sdk")
                .with(events)
                .with(models)
                .with(blocks)
                .with(original)
                .with(selected),
        )
        .generate_native()
        .unwrap_err();
    assert!(format!("{error:#}").contains("revision"));
}
