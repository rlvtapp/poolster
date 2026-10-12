use anyhow::Result;
use poolster_core::{
    blocks::ContractReference,
    engine::{Contract, Handle, Meta, Packages, Plugin, PluginContext, Provision},
    native::events::{EventOperations, model_blocks},
};
use poolster_plugin_java as output;
struct Source {
    meta: Meta,
    input: EventOperations,
    revision: Option<&'static str>,
}
impl Source {
    fn new() -> Self {
        let document = poolster_input_asyncapi::parse(include_str!(
            "../../../inputs/asyncapi/tests/fixtures/kafka-json.yaml"
        ))
        .unwrap();
        Self {
            meta: Meta::new(),
            input: poolster_input_asyncapi::lower_operations(&document, &Default::default())
                .unwrap(),
            revision: None,
        }
    }
    fn handle(&self) -> Handle<EventOperations> {
        self.meta.handle()
    }
}
impl Plugin<output::Java> for Source {
    fn kind(&self) -> &'static str {
        "fixture-events"
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
    fn generate(&self, cx: &mut PluginContext<'_, output::Java>) -> Result<()> {
        if let Some(revision) = self.revision {
            let mut reference = ContractReference::from_bytes(
                EventOperations::NAME,
                "events",
                &serde_json::to_vec(&self.input)?,
            );
            reference.revision = revision.into();
            cx.publish_with_reference(self.input.clone(), reference)
        } else {
            cx.publish(self.input.clone())
        }
    }
}
fn generate(mut source: Source, shared: bool, broker: bool) -> poolster_core::GeneratedTree {
    if broker {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        for op in &mut source.input.operations {
            op.topic = format!("poolster-java-{nonce}");
            op.group_id = if op.action == poolster_core::native::events::EventAction::Receive {
                Some(format!("poolster-java-group-{nonce}"))
            } else {
                None
            };
            op.client_id = Some(
                if op.action == poolster_core::native::events::EventAction::Send {
                    "java-bound-producer"
                } else {
                    "java-bound-consumer"
                }
                .into(),
            );
        }
    }
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
    Packages::new().package(package).generate_native().unwrap()
}
#[test]
fn shared_and_standalone_packages_regenerate_and_split() {
    for shared in [false, true] {
        let a = generate(Source::new(), shared, false);
        let b = generate(Source::new(), shared, false);
        assert_eq!(a.iter().collect::<Vec<_>>(), b.iter().collect::<Vec<_>>());
        let text = a.iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n");
        assert!(text.contains("kafka-clients</artifactId><version>3.9.1"));
        assert!(text.contains("json-schema-validator</artifactId><version>1.5.6"));
        assert!(text.contains("SendOrderEvent"));
        assert!(text.contains("poolster-alpha2-consumer"));
        for (path, source) in a.iter() {
            if path.extension().is_some_and(|v| v == "java") {
                assert!(source.len() < 128 * 1024);
            }
        }
    }
}
#[test]
fn opaque_revision_and_selected_provider_are_supported() {
    let mut selected = Source::new();
    selected.revision = Some("opaque-v1");
    let blocks = model_blocks(Some(selected.handle()), "events");
    let models = output::shared_models(None).input_shapes(blocks.handle());
    let events = output::asyncapi(Some(selected.handle())).using_models(models.handle());
    let mut ignored = Source::new();
    ignored.input.operations[0].topic = "ignored-topic".into();
    let tree = Packages::new()
        .package(
            output::package("sdk")
                .with(events)
                .with(models)
                .with(blocks)
                .with(ignored)
                .with(selected),
        )
        .generate_native()
        .unwrap();
    let source = tree.iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n");
    assert!(!source.contains("ignored-topic"));
    assert!(source.contains("opaque-v1"));
}
#[test]
fn stale_revision_is_rejected() {
    let mut original = Source::new();
    original.revision = Some("old");
    let mut changed = Source::new();
    changed.revision = Some("new");
    let blocks = model_blocks(Some(original.handle()), "events");
    let models = output::shared_models(None).input_shapes(blocks.handle());
    let events = output::asyncapi(Some(changed.handle())).using_models(models.handle());
    let error = Packages::new()
        .package(
            output::package("sdk")
                .with(events)
                .with(models)
                .with(blocks)
                .with(original)
                .with(changed),
        )
        .generate_native()
        .expect_err("stale models");
    assert!(format!("{error:#}").contains("revision"));
}
#[test]
fn unsupported_shapes_and_bindings_fail_clearly() {
    for case in 0..5 {
        let mut source = Source::new();
        match case {
            0 => source.input.operations[0].topic = "{dynamic}".into(),
            1 => {
                source.input.operations[0]
                    .message
                    .key
                    .as_mut()
                    .unwrap()
                    .nullable = true
            }
            2 => {
                source.input.operations[0].message.payload_schema["format"] =
                    serde_json::json!("custom")
            }
            3 => {
                source.input.operations[0].bindings =
                    serde_json::json!({"kafka":{"transactionalId":"bad"}})
            }
            _ => {
                source.input.source["servers"]["local"]["security"] =
                    serde_json::json!([{"scheme":"auth"}])
            }
        };
        let events = output::asyncapi(Some(source.handle()));
        let error = Packages::new()
            .package(output::package("sdk").with(events).with(source))
            .generate_native()
            .expect_err("unsupported feature");
        assert!(format!("{error:#}").contains("Java Kafka"));
    }
}
#[test]
fn oversized_schema_literal_is_rejected_before_java_emission() {
    let mut source = Source::new();
    source.input.operations[0].message.payload_schema["description"] =
        serde_json::json!("x".repeat(61_000));
    let events = output::asyncapi(Some(source.handle()));
    let error = Packages::new()
        .package(output::package("sdk").with(events).with(source))
        .generate_native()
        .expect_err("oversized JVM literal");
    assert!(format!("{error:#}").contains("JVM string-constant"));
}
#[test]
#[ignore = "requires Java17, Maven and pinned Kafka/JSON Schema dependencies"]
fn generated_package_compiles_and_validates_wire() {
    native(false);
}
#[test]
#[ignore = "requires Java17, Maven and local Kafka broker"]
fn generated_package_roundtrips_local_kafka() {
    native(true);
}
fn native(broker: bool) {
    let dir = tempfile::tempdir().unwrap();
    generate(Source::new(), true, broker)
        .write_to(dir.path())
        .unwrap();
    let sdk = dir.path().join("sdk");
    let tests = sdk.join("src/test/java");
    std::fs::create_dir_all(&tests).unwrap();
    std::fs::write(
        tests.join("Wire.java"),
        include_str!("fixtures/events-wire.java"),
    )
    .unwrap();
    let maven = std::env::var("POOLSTER_MAVEN").unwrap_or("mvn".into());
    let result = std::process::Command::new(maven)
        .args([
            "-B",
            "test-compile",
            "dependency:build-classpath",
            "-Dmdep.outputFile=classpath.txt",
        ])
        .current_dir(&sdk)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let java = std::env::var("JAVA_HOME")
        .map(|p| format!("{p}/bin/java"))
        .unwrap_or("java".into());
    let cp = format!(
        "target/classes:target/test-classes:{}",
        std::fs::read_to_string(sdk.join("classpath.txt"))
            .unwrap()
            .trim()
    );
    let result = std::process::Command::new(java)
        .args(["-cp", &cp, "Wire", if broker { "broker" } else { "wire" }])
        .current_dir(&sdk)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
