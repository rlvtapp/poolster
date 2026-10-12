use anyhow::Result;
use poolster_core::{
    blocks::ContractReference,
    engine::{Contract, Handle, Meta, Packages, Plugin, PluginContext, Provision},
    native::events::{EventOperations, model_blocks},
};
use poolster_plugin_csharp as output;
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
impl Plugin<output::CSharp> for Source {
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
    fn generate(&self, cx: &mut PluginContext<'_, output::CSharp>) -> Result<()> {
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
            op.topic = format!("poolster-csharp-{nonce}");
            op.group_id = if op.action == poolster_core::native::events::EventAction::Receive {
                Some(format!("poolster-csharp-group-{nonce}"))
            } else {
                None
            };
            op.client_id = Some(
                if op.action == poolster_core::native::events::EventAction::Send {
                    "csharp-bound-producer"
                } else {
                    "csharp-bound-consumer"
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
        assert!(text.contains("Confluent.Kafka\" Version=\"2.12.0"));
        assert!(text.contains("JsonSchema.Net\" Version=\"7.3.4"));
        assert!(text.contains("SendOrderEvent"));
        assert!(text.contains("poolster-alpha2-consumer"));
        for (path, source) in a.iter() {
            if path.extension().is_some_and(|v| v == "cs") {
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
        assert!(format!("{error:#}").contains("C# Kafka"));
    }
}
#[test]
#[ignore = "requires .NET8 and pinned Kafka/JSON Schema dependencies"]
fn generated_package_compiles_and_validates_wire() {
    native(false);
}
#[test]
#[ignore = "requires .NET8 and local Kafka broker"]
fn generated_package_roundtrips_local_kafka() {
    native(true);
}
fn native(broker: bool) {
    native_variant(broker, true);
    if !broker {
        native_variant(false, false);
    }
}
fn native_variant(broker: bool, shared: bool) {
    let dir = tempfile::tempdir().unwrap();
    generate(Source::new(), shared, broker)
        .write_to(dir.path())
        .unwrap();
    let sdk = dir.path().join("sdk");
    let probe = dir.path().join("probe");
    std::fs::create_dir_all(&probe).unwrap();
    std::fs::write(probe.join("Probe.csproj"), "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><TargetFramework>net8.0</TargetFramework><OutputType>Exe</OutputType><Nullable>enable</Nullable><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><ProjectReference Include=\"../sdk/Models.csproj\" /></ItemGroup></Project>").unwrap();
    std::fs::write(
        probe.join("Program.cs"),
        include_str!("fixtures/events-wire.cs"),
    )
    .unwrap();
    let dotnet = std::env::var("POOLSTER_DOTNET").unwrap_or("dotnet".into());
    let result = std::process::Command::new(dotnet)
        .args([
            "run",
            "--project",
            "Probe.csproj",
            "--",
            if broker { "broker" } else { "wire" },
        ])
        .current_dir(&probe)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr),
        sdk.display()
    );
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
        .find(|(p, _)| p.to_string_lossy() == "sdk/Models.csproj")
        .unwrap()
        .1;
    assert_eq!(
        manifest,
        "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><TargetFramework>net8.0</TargetFramework><Nullable>enable</Nullable><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>\n"
    );
}

#[test]
fn large_operation_layout_stays_split_and_bounded() {
    let mut source = Source::new();
    let base = source.input.operations[0].clone();
    source.input.operations = (0..250)
        .map(|i| {
            let mut op = base.clone();
            op.name = format!("sendOrder{i}");
            op
        })
        .collect();
    let tree = generate(source, true, false);
    assert_eq!(
        tree.iter()
            .filter(|(p, _)| p.to_string_lossy().contains("/Events/SendOrder"))
            .count(),
        250
    );
    for (path, text) in tree.iter() {
        if path.extension().is_some_and(|v| v == "cs") {
            assert!(text.len() <= 128 * 1024);
        }
    }
    assert_eq!(
        tree.iter()
            .filter(|(p, _)| p.to_string_lossy().contains("/Models/OrderCreatedPayload"))
            .count(),
        1
    );
}

#[test]
fn unsafe_names_and_external_references_fail_before_emission() {
    for case in 0..3 {
        let mut source = Source::new();
        match case {
            0 => source.input.operations[1].name = source.input.operations[0].name.clone(),
            1 => {
                source.input.operations[0].message.payload_schema["$ref"] =
                    serde_json::json!("https://example.test/model.json")
            }
            _ => source.input.operations[0].name = "x".repeat(200),
        }
        let events = output::asyncapi(Some(source.handle()));
        assert!(
            Packages::new()
                .package(output::package("sdk").with(events).with(source))
                .generate_native()
                .is_err()
        );
    }
}

#[test]
fn kafka_and_http_require_separate_packages_in_either_registration_order() {
    for events_first in [true, false] {
        let source = Source::new();
        let package = if events_first {
            let events = output::asyncapi(Some(source.handle()));
            let sdk = output::sdk();
            output::package("sdk").with(events).with(sdk).with(source)
        } else {
            let sdk = output::sdk();
            let events = output::asyncapi(Some(source.handle()));
            output::package("sdk").with(sdk).with(events).with(source)
        };
        let error = Packages::new()
            .package(package)
            .generate(&poolster_core::Api::default(), None)
            .expect_err("mixed project ownership");
        assert!(
            format!("{error:#}").contains("separate output packages"),
            "{error:#}"
        );
    }
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
impl Plugin<output::CSharp> for RenamedShapes {
    fn kind(&self) -> &'static str {
        "renamed-model-provider"
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
    fn generate(&self, cx: &mut PluginContext<'_, output::CSharp>) -> Result<()> {
        let mut blocks = cx
            .inputs
            .get::<poolster_core::blocks::Blocks<poolster_core::models::SchemaModelBlock>>()?
            .clone();
        for block in &mut blocks.items {
            block.value.name = if block.metadata.id.local.ends_with("/payload") {
                "KafkaRuntime"
            } else if block.metadata.id.local.ends_with("/headers") {
                "SendOrderEvent"
            } else {
                "KafkaDefinition"
            }
            .into();
        }
        cx.publish(blocks)
    }
}
fn generate_renamed_models() -> poolster_core::GeneratedTree {
    let source = Source::new();
    let blocks = model_blocks(Some(source.handle()), "events");
    let renamed = RenamedShapes {
        meta: Meta::new(),
        provider: blocks.handle(),
    };
    let models = output::shared_models(None).input_shapes(renamed.handle());
    let events = output::asyncapi(Some(source.handle())).using_models(models.handle());
    Packages::new()
        .package(
            output::package("sdk")
                .with(events)
                .with(models)
                .with(renamed)
                .with(blocks)
                .with(source),
        )
        .generate_native()
        .unwrap()
}
#[test]
fn selected_model_symbols_are_qualified_when_names_collide() {
    let tree = generate_renamed_models();
    let operation = tree
        .iter()
        .find(|(path, _)| path.to_string_lossy() == "sdk/Events/SendOrderEvent.cs")
        .unwrap()
        .1;
    for name in ["KafkaRuntime", "SendOrderEvent", "KafkaDefinition"] {
        assert!(
            operation.contains(&format!("global::Poolster.Models.{name}")),
            "missing selected symbol {name}"
        );
    }
}
#[test]
#[ignore = "requires .NET8 and pinned Kafka/JSON Schema dependencies"]
fn renamed_model_provider_compiles_with_runtime_and_operation_name_collisions() {
    let dir = tempfile::tempdir().unwrap();
    generate_renamed_models().write_to(dir.path()).unwrap();
    let probe = dir.path().join("probe");
    std::fs::create_dir_all(&probe).unwrap();
    std::fs::write(probe.join("Probe.csproj"), "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><TargetFramework>net8.0</TargetFramework><OutputType>Exe</OutputType><Nullable>enable</Nullable><ImplicitUsings>enable</ImplicitUsings></PropertyGroup><ItemGroup><ProjectReference Include=\"../sdk/Models.csproj\" /></ItemGroup></Project>").unwrap();
    std::fs::write(probe.join("Program.cs"), r#"
using System.Text.Json;
using var runtime = global::Poolster.Models.Events.KafkaDefaults.Connect();
var payload = new global::Poolster.Models.KafkaRuntime { Id = "order-1", Quantity = 0 };
var headers = new global::Poolster.Models.SendOrderEvent { Trace = "trace-1" };
var key = new global::Poolster.Models.KafkaDefinition("account-1");
try { global::Poolster.Models.Events.SendOrderEvent.SendAsync(runtime, payload, headers, key); throw new Exception("Missing validation"); }
catch (JsonException) { }
// The receive signature must also bind these selected model types.
Func<global::Poolster.Models.Events.KafkaRuntime, string?, global::Poolster.Models.Events.KafkaSubscription<global::Poolster.Models.KafkaRuntime, global::Poolster.Models.SendOrderEvent, global::Poolster.Models.KafkaDefinition>> subscribe = global::Poolster.Models.Events.ReceiveOrderEvent.Subscribe;
Console.WriteLine("renamed-symbols-ok");
"#).unwrap();
    let dotnet = std::env::var("POOLSTER_DOTNET").unwrap_or("dotnet".into());
    let result = std::process::Command::new(dotnet)
        .args(["run", "--project", "Probe.csproj"])
        .current_dir(&probe)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
