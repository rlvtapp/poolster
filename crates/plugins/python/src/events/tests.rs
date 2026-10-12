use super::*;
use crate::PackageExt;
use poolster_core::{
    engine::Packages,
    native::events::{EventAction, EventMessage, EventOperation, KafkaBroker},
};
use std::process::Command;
struct Provider {
    meta: Meta,
    value: EventOperations,
    revision: String,
}
impl Plugin<Python> for Provider {
    fn kind(&self) -> &'static str {
        "replacement-events"
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
    fn generate(&self, cx: &mut PluginContext<Python>) -> Result<()> {
        cx.publish_with_reference(
            self.value.clone(),
            ContractReference {
                contract: EventOperations::NAME.into(),
                instance: "custom".into(),
                revision: self.revision.clone(),
            },
        )
    }
}
fn object(fields: &[(&str, bool, bool)]) -> poolster_core::SchemaValue {
    poolster_core::SchemaValue::new(poolster_core::SchemaKind::Object {
        fields: fields
            .iter()
            .map(|(name, required, nullable)| {
                let mut value = poolster_core::SchemaValue::new(poolster_core::SchemaKind::String);
                value.nullable = *nullable;
                poolster_core::Field {
                    name: (*name).into(),
                    value,
                    required: *required,
                    annotations: Default::default(),
                }
            })
            .collect(),
        additional_properties: poolster_core::AdditionalProperties::Forbidden,
    })
}
fn contract(topic: &str) -> EventOperations {
    let payload_schema = serde_json::json!({"type":"object","properties":{"id":{"type":"string"},"nickname":{"type":["string","null"]}},"required":["id"],"additionalProperties":false});
    let headers_schema = serde_json::json!({"type":"object","properties":{"trace":{"type":"string"}},"required":["trace"],"additionalProperties":false});
    let msg = EventMessage {
        name: "User".into(),
        location: "#/components/messages/User".into(),
        payload: object(&[("id", true, false), ("nickname", false, true)]),
        payload_schema,
        headers: Some(object(&[("trace", true, false)])),
        headers_schema: Some(headers_schema),
        key: Some(poolster_core::SchemaValue::new(
            poolster_core::SchemaKind::String,
        )),
        key_schema: Some(serde_json::json!({"type":"string"})),
        bindings: serde_json::Value::Null,
    };
    EventOperations {
        source: serde_json::json!({"asyncapi":"3.0.0"}),
        broker: KafkaBroker {
            brokers: vec!["127.0.0.1:29092".into()],
            client_id: Some("default".into()),
            bindings: serde_json::Value::Null,
        },
        operations: [
            ("sendUser", EventAction::Send),
            ("receiveUser", EventAction::Receive),
        ]
        .into_iter()
        .map(|(name, action)| EventOperation {
            name: name.into(),
            action,
            channel: "users".into(),
            topic: topic.into(),
            message: msg.clone(),
            group_id: Some(format!("group-{topic}")),
            client_id: Some(format!("op-{name}")),
            bindings: serde_json::Value::Null,
            channel_bindings: serde_json::Value::Null,
        })
        .collect(),
    }
}
fn generate(value: EventOperations) -> poolster_core::GeneratedTree {
    let p = Provider {
        meta: Meta::new(),
        value,
        revision: "opaque-provider-revision".into(),
    };
    let shapes = poolster_core::native::events::model_blocks(Some(p.meta.handle()), "custom");
    let models = crate::shared_models(None).input_shapes(shapes.handle());
    let events = asyncapi(Some(p.meta.handle())).using_models(models.handle());
    Packages::new()
        .package(
            crate::package("sdk")
                .name("kafka-test")
                .with(p)
                .with(shapes)
                .with(models)
                .with(events),
        )
        .generate_native()
        .unwrap()
}
#[test]
fn package_structure_and_provider_substitution() {
    let a = generate(contract("first"));
    let b = generate(contract("second"));
    assert!(a.iter().any(|(_, v)| v.contains("first")));
    assert!(!b.iter().any(|(_, v)| v.contains("first")));
    assert_eq!(
        a.iter().collect::<Vec<_>>(),
        generate(contract("first")).iter().collect::<Vec<_>>()
    );
    assert_eq!(
        a.iter()
            .filter(|(p, _)| p.ends_with("pyproject.toml"))
            .count(),
        1
    );
    for (p, s) in a.iter() {
        if p.extension().is_some_and(|e| e == "py") {
            assert!(s.lines().count() < 400, "{}", p.display());
        }
    }
}
#[test]
#[ignore = "requires pinned Python Kafka dependencies; set POOLSTER_KAFKA_PYTHON"]
fn generated_python_codec_and_local_broker() {
    let topic = format!(
        "poolster-python-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let tree = generate(contract(&topic));
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    let script = include_str!("probe.py");
    std::fs::write(dir.path().join("probe.py"), script).unwrap();
    let python = std::env::var("POOLSTER_KAFKA_PYTHON").expect("Python dependency environment");
    let output = Command::new(&python)
        .arg("-m")
        .arg("compileall")
        .arg("-q")
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let installed = dir.path().join("installed");
    let installation = Command::new(&python)
        .args([
            "-m",
            "pip",
            "install",
            "--no-deps",
            "--no-build-isolation",
            "--target",
        ])
        .arg(&installed)
        .arg(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        installation.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&installation.stdout),
        String::from_utf8_lossy(&installation.stderr)
    );
    let output = Command::new(python)
        .arg(dir.path().join("probe.py"))
        .env("PYTHONPATH", installed)
        .env("POOLSTER_KAFKA_TOPIC", topic)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn opaque_revision_and_stale_shared_models() {
    let first = Provider {
        meta: Meta::new(),
        value: contract("users"),
        revision: "revision1".into(),
    };
    let second = Provider {
        meta: Meta::new(),
        value: contract("users"),
        revision: "revision2".into(),
    };
    let shapes = poolster_core::native::events::model_blocks(Some(first.meta.handle()), "custom");
    let models = crate::shared_models(None).input_shapes(shapes.handle());
    let events = asyncapi(Some(second.meta.handle())).using_models(models.handle());
    let error = Packages::new()
        .package(
            crate::package("sdk")
                .name("kafka-test")
                .with(first)
                .with(second)
                .with(shapes)
                .with(models)
                .with(events),
        )
        .generate_native()
        .unwrap_err();
    assert!(format!("{error:#}").contains("revision"));
}
#[test]
fn standalone_and_large_packages_reject_unsupported_features() {
    let mut value = contract("users");
    let template = value.operations[0].clone();
    value.operations = (0..250)
        .map(|i| {
            let mut op = template.clone();
            op.name = format!("sendUser{i}");
            op
        })
        .collect();
    let tree = generate(value);
    for (p, s) in tree.iter() {
        if p.extension().is_some_and(|e| e == "py") {
            assert!(s.lines().count() < 400, "{}", p.display());
        }
    }
    let input = Provider {
        meta: Meta::new(),
        value: contract("users"),
        revision: "v1".into(),
    };
    let events = asyncapi(Some(input.meta.handle()));
    let tree = Packages::new()
        .package(
            crate::package("sdk")
                .name("kafka-test")
                .with(input)
                .with(events),
        )
        .generate_native()
        .unwrap();
    assert!(tree.iter().any(|(p, _)| p.ends_with("events/client.py")));
    let mut value = contract("users");
    value.operations[0].message.payload_schema["unevaluatedProperties"] = false.into();
    assert!(format!("{:#}", validation::check(&value).unwrap_err()).contains("unsupported"));
    value = contract("users");
    value.operations[0].message.key = Some(poolster_core::SchemaValue::new(
        poolster_core::SchemaKind::Integer,
    ));
    assert!(validation::check(&value).is_err());
    value = contract("{user}");
    assert!(validation::check(&value).is_err());
}
