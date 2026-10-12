use anyhow::Result;
use poolster_core::{
    blocks::ContractReference,
    engine::{Contract, Handle, Meta, Packages, Plugin, PluginContext, Provision},
    native::events::{EventOperations, model_blocks},
};
use poolster_plugin_ruby as output;
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
        let mut input =
            poolster_input_asyncapi::lower_operations(&document, &Default::default()).unwrap();
        for op in &mut input.operations {
            op.message.payload_schema["description"] =
                serde_json::json!("#{raise 'metadata was executed'}");
            let malicious = "#{raise 'metadata was executed'}";
            op.message.payload_schema["properties"][malicious] =
                serde_json::json!({"type":"string"});
            if let poolster_core::SchemaKind::Object { fields, .. } = &mut op.message.payload.kind {
                fields.push(poolster_core::Field {
                    name: malicious.into(),
                    value: poolster_core::SchemaValue::new(poolster_core::SchemaKind::String),
                    required: false,
                    annotations: Default::default(),
                });
            }
        }
        Self {
            meta: Meta::new(),
            input,
            revision: None,
        }
    }
    fn handle(&self) -> Handle<EventOperations> {
        self.meta.handle()
    }
}
impl Plugin<output::Ruby> for Source {
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
    fn generate(&self, cx: &mut PluginContext<'_, output::Ruby>) -> Result<()> {
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
            op.topic = format!("poolster-ruby-{nonce}");
            op.group_id = if op.action == poolster_core::native::events::EventAction::Receive {
                Some(format!("poolster-ruby-group-{nonce}"))
            } else {
                None
            };
            op.client_id = Some(
                if op.action == poolster_core::native::events::EventAction::Send {
                    "ruby-bound-producer"
                } else {
                    "ruby-bound-consumer"
                }
                .into(),
            );
        }
    }
    if broker {
        let result = std::process::Command::new("docker")
            .args([
                "exec",
                "poolster-alpha2-kafka",
                "rpk",
                "topic",
                "create",
                &source.input.operations[0].topic,
                "-p",
                "1",
                "-r",
                "1",
            ])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "topic provisioning: {}",
            String::from_utf8_lossy(&result.stderr)
        );
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
        assert!(text.contains("rdkafka"));
        assert!(text.contains("SendOrder"));
        assert!(text.contains("poolster-alpha2-consumer"));
        for (path, source) in a.iter() {
            if path.extension().is_some_and(|v| v == "rb") {
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
        assert!(format!("{error:#}").contains("Ruby Kafka"));
    }
}
#[test]
#[ignore = "requires pinned native Kafka libraries"]
fn generated_package_compiles_and_validates_wire() {
    native(false);
}
#[test]
#[ignore = "requires pinned native Kafka libraries and local broker"]
fn generated_package_roundtrips_local_kafka() {
    native(true);
}
fn native(broker: bool) {
    for shared in if broker {
        vec![true]
    } else {
        vec![true, false]
    } {
        let dir = tempfile::tempdir().unwrap();
        let tree = generate(Source::new(), shared, broker);
        if broker {
            let source = tree
                .iter()
                .find(|(p, _)| p.to_string_lossy().ends_with("/events/send_order.rb"))
                .unwrap()
                .1;
            let suffix = source.split("poolster-ruby-").nth(1).unwrap();
            let nonce = suffix
                .chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>();
            let topic = format!("poolster-ruby-{nonce}");
            for index in 0..8 {
                let invalid = format!("{topic}-bad-{index}");
                let result = std::process::Command::new("docker")
                    .args([
                        "exec",
                        "poolster-alpha2-kafka",
                        "rpk",
                        "topic",
                        "create",
                        &invalid,
                        "-p",
                        "1",
                        "-r",
                        "1",
                    ])
                    .output()
                    .unwrap();
                assert!(result.status.success(), "invalid topic provisioning");
            }
            let script = dir.path().join("WireInjector.java");
            std::fs::write(&script, include_str!("fixtures/events-broker-wire.java")).unwrap();
            let java = std::env::var("POOLSTER_JAVA").unwrap_or_else(|_| "java".into());
            let classpath = std::env::var("POOLSTER_KAFKA_JAVA_CLASSPATH").expect(
                "native Kafka 3.9.1 Java classpath is required for malformed wire injection",
            );
            let result = std::process::Command::new(java)
                .args(["--class-path", &classpath])
                .arg(script)
                .arg(&topic)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "inject wire: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        tree.write_to(dir.path()).unwrap();
        let sdk = dir.path().join("sdk");
        std::fs::write(
            sdk.join("probe.rb"),
            include_str!("fixtures/events-wire.rb"),
        )
        .unwrap();
        let runner =
            std::env::var("POOLSTER_RUBY_EVENTS_RUNNER").expect("native runner must be configured");
        let result = std::process::Command::new(runner)
            .arg(&sdk)
            .arg(if broker { "broker" } else { "wire" })
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
#[test]
fn large_operation_layout_stays_split_and_bounded() {
    let mut source = Source::new();
    let base = source.input.operations[0].clone();
    source.input.operations = (0..250)
        .map(|i| {
            let mut op = base.clone();
            op.name = format!("event_{i}");
            op
        })
        .collect();
    let tree = generate(source, true, false);
    assert_eq!(
        tree.iter()
            .filter(|(p, _)| p.to_string_lossy().contains("/events/event_"))
            .count(),
        250
    );
    for (_, source) in tree.iter() {
        assert!(source.len() < 128 * 1024);
    }
}

#[test]
fn shared_model_manifest_preserves_existing_standalone_bytes() {
    let source = Source::new();
    let blocks = model_blocks(Some(source.handle()), "events");
    let models = output::shared_models(None).input_shapes(blocks.handle());
    let tree = Packages::new()
        .package(
            output::package("sdk")
                .with(models)
                .with(blocks)
                .with(source),
        )
        .generate_native()
        .unwrap();
    let manifest = tree
        .iter()
        .find(|(p, _)| p.to_string_lossy() == "sdk/poolster_models.gemspec")
        .unwrap()
        .1;
    assert_eq!(
        manifest,
        "Gem::Specification.new do |spec|\n  spec.name = \"poolster_models\"\n  spec.version = \"0.1.0\"\n  spec.summary = 'Generated Poolster models'\n  spec.authors = ['Poolster']\n  spec.files = Dir['lib/**/*.rb']\n  spec.require_paths = ['lib']\n  spec.required_ruby_version = '>= 3.1'\nend\n"
    );
}

#[test]
fn reserved_runtime_operation_names_are_rejected() {
    let mut source = Source::new();
    source.input.operations[0].name = "kafkaClient".into();
    let events = output::asyncapi(Some(source.handle()));
    let error = Packages::new()
        .package(output::package("sdk").with(events).with(source))
        .generate_native()
        .unwrap_err();
    assert!(format!("{error:#}").contains("runtime symbol"));
}
