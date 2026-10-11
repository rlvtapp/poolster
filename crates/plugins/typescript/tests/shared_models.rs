use poolster_core::{
    engine::Packages,
    input::{InputProvider, InputRegistry},
};
use poolster_input_asyncapi::contracts::MessageBlocks;
use poolster_plugin_typescript as ts;
use std::sync::Arc;

fn model_package(reverse: bool) -> poolster_core::engine::Package<ts::TypeScript> {
    let mut registry = InputRegistry::new();
    registry
        .register(poolster_input_asyncapi::AsyncApiInput)
        .unwrap();
    let registry = Arc::new(registry);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../inputs/asyncapi/tests/fixtures/kafka-json.yaml");
    let input = InputProvider::<MessageBlocks>::new(registry.clone(), "asyncapi", &path);
    let shapes = poolster_input_asyncapi::shared_model_blocks(Some(input.handle()));
    let output = ts::shared_models(None).input_shapes(shapes.handle());
    if reverse {
        ts::package("client").with(input).with(shapes).with(output)
    } else {
        ts::package("client").with(output).with(shapes).with(input)
    }
}
fn package(reverse: bool, kafka: bool) -> poolster_core::GeneratedTree {
    let package = model_package(reverse);
    let package = if kafka {
        let mut registry = InputRegistry::new();
        registry
            .register(poolster_input_asyncapi::AsyncApiInput)
            .unwrap();
        let registry = Arc::new(registry);
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../inputs/asyncapi/tests/fixtures/kafka-json.yaml");
        let input = InputProvider::<poolster_core::native::events::EventOperations>::new(
            registry, "asyncapi", &path,
        );
        let sdk = ts::asyncapi(Some(input.handle()));
        package.with(sdk).with(input)
    } else {
        package
    };
    Packages::new().package(package).generate_native().unwrap()
}
#[test]
fn rich_models_are_standalone_and_deterministic_without_broker_codegen() {
    let a = package(false, false);
    let b = package(true, false);
    let normalize = |tree: &poolster_core::GeneratedTree| {
        tree.iter()
            .map(|(p, s)| (p.to_owned(), s.to_owned()))
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    assert_eq!(normalize(&a), normalize(&b));
    assert!(
        a.get("client/package.json")
            .unwrap()
            .contains("poolster-shared-models")
    );
    assert!(a.get("client/tsconfig.json").is_some());
    assert!(a.get("client/events.ts").is_none());
    let manifest = a.get("client/shared-models/schema-manifest.json").unwrap();
    assert!(manifest.contains("minLength"));
    assert!(
        a.iter()
            .any(|(p, s)| p.extension().is_some_and(|e| e == "ts")
                && s.contains("export type OrderCreatedPayload"))
    );
}
#[test]
fn explicit_sdk_manifest_wins_over_model_defaults_in_both_registration_orders() {
    for reverse in [false, true] {
        let tree = package(reverse, true);
        let manifest = tree.get("client/package.json").unwrap();
        assert!(manifest.contains("poolster-kafka-client"));
        assert!(manifest.contains("kafkajs"));
        assert!(
            tree.get("client/shared-models/schema-manifest.json")
                .is_some()
        );
    }
}
#[test]
#[ignore = "requires pinned POOLSTER_KAFKA_NODE_MODULES; no broker required"]
fn standalone_shared_models_compile_and_typecheck_presence() {
    let modules = std::env::var("POOLSTER_KAFKA_NODE_MODULES").unwrap();
    let dir = tempfile::tempdir().unwrap();
    package(false, false).write_to(dir.path()).unwrap();
    std::os::unix::fs::symlink(modules, dir.path().join("client/node_modules")).unwrap();
    std::fs::write(dir.path().join("client/probe.ts"),"import type {OrderCreatedPayload} from './index.js';\nconst value:OrderCreatedPayload={id:'1',quantity:2};\nvoid value;\n").unwrap();
    let out = std::process::Command::new("node")
        .args(["node_modules/typescript/bin/tsc", "-p", "tsconfig.json"])
        .current_dir(dir.path().join("client"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn http_and_graphql_metadata_override_shared_model_defaults() {
    for reverse in [false, true] {
        let api = poolster_core::Api {
            name: "HTTP Fixture".into(),
            version: "1.0".into(),
            ..Default::default()
        };
        let tree = Packages::new()
            .package(model_package(reverse).with(ts::sdk()))
            .generate(&api, None)
            .unwrap();
        assert!(
            !tree
                .get("client/package.json")
                .unwrap()
                .contains("poolster-shared-models")
        );
        assert!(
            tree.get("client/shared-models/schema-manifest.json")
                .is_some()
        );
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("schema.graphql"),
            "type Query { hello: String! }",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("operations.graphql"),
            "query Read { hello }",
        )
        .unwrap();
        let mut registry = InputRegistry::new();
        registry
            .register(poolster_input_graphql::GraphqlInput)
            .unwrap();
        let input = InputProvider::<poolster_core::native::GraphqlOperations>::new(
            Arc::new(registry),
            "graphql",
            dir.path().join("schema.graphql"),
        )
        .with_options(poolster_core::input::InputOptions {
            operation_files: vec![dir.path().join("operations.graphql")],
            ..Default::default()
        });
        let output = ts::graphql(Some(input.handle()));
        let tree = Packages::new()
            .package(model_package(reverse).with(output).with(input))
            .generate_native()
            .unwrap();
        assert!(
            !tree
                .get("client/package.json")
                .unwrap()
                .contains("poolster-shared-models")
        );
        assert!(
            tree.get("client/shared-models/schema-manifest.json")
                .is_some()
        );
    }
}

#[test]
fn shared_output_binds_explicitly_selected_rich_provider_and_regenerates() {
    use poolster_core::{
        blocks::Blocks,
        engine::{Meta, Plugin, PluginContext, Provision},
        models::SchemaModelBlock,
    };
    struct Source {
        meta: Meta,
        name: &'static str,
    }
    impl Plugin<ts::TypeScript> for Source {
        fn kind(&self) -> &'static str {
            "shared-shape-substitution"
        }
        fn meta(&self) -> &Meta {
            &self.meta
        }
        fn supports_native_input(&self) -> bool {
            true
        }
        fn provides(&self) -> Vec<Provision> {
            vec![Provision::of::<Blocks<SchemaModelBlock>>()]
        }
        fn generate(&self, cx: &mut PluginContext<'_, ts::TypeScript>) -> anyhow::Result<()> {
            let document = poolster_input_asyncapi::parse(include_str!(
                "../../../inputs/asyncapi/tests/fixtures/kafka-json.yaml"
            ))?;
            let (messages, _) =
                poolster_input_asyncapi::lower_message_blocks(&document, "substitution");
            let mut blocks = poolster_input_asyncapi::shared_models::extract(&messages);
            blocks.items.truncate(1);
            blocks.items[0].value.name = self.name.into();
            cx.publish(blocks)
        }
    }
    for name in ["Selected", "Replacement"] {
        let source = Source {
            meta: Meta::new(),
            name,
        };
        let ignored = Source {
            meta: Meta::new(),
            name: "Ignored",
        };
        let output = ts::shared_models(None).input_shapes(source.meta.handle());
        let tree = Packages::new()
            .package(
                ts::package("client")
                    .with(ignored)
                    .with(output)
                    .with(source),
            )
            .generate_native()
            .unwrap();
        let content = tree.iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n");
        assert!(content.contains(&format!("export type {name}")));
        assert!(!content.contains("export type Ignored"));
    }
}
