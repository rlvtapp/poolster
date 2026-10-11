use poolster_core::{
    engine::Packages,
    input::{InputProvider, InputRegistry},
    native::events::{EventOperations, model_blocks},
};
use poolster_plugin_typescript as ts;
use std::sync::Arc;
fn generate(reverse: bool) -> poolster_core::GeneratedTree {
    let mut registry = InputRegistry::new();
    registry
        .register(poolster_input_asyncapi::AsyncApiInput)
        .unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../inputs/asyncapi/tests/fixtures/kafka-json.yaml");
    generate_path(reverse, registry, &path)
}
fn generate_path(
    reverse: bool,
    registry: InputRegistry,
    path: &std::path::Path,
) -> poolster_core::GeneratedTree {
    let input = InputProvider::<EventOperations>::new(Arc::new(registry), "asyncapi", path);
    let shapes = model_blocks(Some(input.handle()), path.to_string_lossy());
    let models = ts::shared_models(None).input_shapes(shapes.handle());
    let sdk = ts::asyncapi(Some(input.handle())).using_models(models.handle());
    let package = if reverse {
        ts::package("client")
            .with(input)
            .with(shapes)
            .with(models)
            .with(sdk)
    } else {
        ts::package("client")
            .with(sdk)
            .with(models)
            .with(shapes)
            .with(input)
    };
    Packages::new().package(package).generate_native().unwrap()
}
#[test]
fn event_shapes_are_referenced_once_and_registration_order_is_irrelevant() {
    let first = generate(false);
    let other = generate(true);
    let files = |tree: &poolster_core::GeneratedTree| {
        tree.iter()
            .map(|(p, s)| (p.to_owned(), s.to_owned()))
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    assert_eq!(files(&first), files(&other));
    let source = first.iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n");
    assert_eq!(
        source.matches("export type OrderCreatedPayload =").count(),
        1
    );
    assert!(source.contains("export type sendOrderPayload = SharedsendOrderPayload"));
    assert!(source.contains("export type receiveOrderPayload = SharedreceiveOrderPayload"));
    assert!(source.contains("SharedsendOrderKey"));
    assert!(
        first
            .get("client/package.json")
            .unwrap()
            .contains("kafkajs")
    );
}
#[test]
#[ignore = "requires pinned POOLSTER_KAFKA_NODE_MODULES; no broker required"]
fn shared_event_package_compiles_and_delegates_with_presence_preserved() {
    let directory = tempfile::tempdir().unwrap();
    generate(false).write_to(directory.path()).unwrap();
    let client = directory.path().join("client");
    std::os::unix::fs::symlink(
        std::env::var("POOLSTER_KAFKA_NODE_MODULES").unwrap(),
        client.join("node_modules"),
    )
    .unwrap();
    std::fs::write(client.join("check.ts"),r#"import {sendOrder,receiveOrder, type KafkaClient} from './index.js';
const message={payload:{id:'id',quantity:2,note:null},headers:{trace:'t'},key:'k'};
let sent: unknown;
const client={send:async (_:unknown,value:unknown)=>{sent=value;return [];},consume:async (_:unknown,handler:(v:unknown)=>void)=>{handler(message);return {};}} as unknown as KafkaClient;
await sendOrder(client,message);
if(sent!==message)throw Error('envelope changed');
await receiveOrder(client,value=>{if(value.payload.note!==null || value.key!=='k')throw Error('presence changed');});
// @ts-expect-error shared integer field remains typed
void sendOrder(client,{payload:{id:'id',quantity:'bad'},headers:{trace:'t'}});
"#).unwrap();
    let output = std::process::Command::new("node")
        .arg("node_modules/typescript/bin/tsc")
        .args(["-p", "tsconfig.json"])
        .current_dir(&client)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = std::process::Command::new("node")
        .arg("dist/check.js")
        .current_dir(&client)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires pinned POOLSTER_KAFKA_NODE_MODULES; no broker required"]
fn strict_utf8_is_checked_before_payload_key_and_header_validation() {
    let dir = tempfile::tempdir().unwrap();
    generate(false).write_to(dir.path()).unwrap();
    let client = dir.path().join("client");
    std::os::unix::fs::symlink(
        std::env::var("POOLSTER_KAFKA_NODE_MODULES").unwrap(),
        client.join("node_modules"),
    )
    .unwrap();
    let compiled = std::process::Command::new("node")
        .arg("node_modules/typescript/bin/tsc")
        .args(["-p", "tsconfig.json"])
        .current_dir(&client)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );
    std::fs::write(
        client.join("probe.cjs"),
        include_str!("fixtures/kafka_utf8_probe.cjs"),
    )
    .unwrap();
    let runtime = std::process::Command::new("node")
        .arg("probe.cjs")
        .current_dir(&client)
        .output()
        .unwrap();
    assert!(
        runtime.status.success(),
        "{}{}",
        String::from_utf8_lossy(&runtime.stdout),
        String::from_utf8_lossy(&runtime.stderr)
    );
}

#[test]
#[ignore = "requires pinned POOLSTER_KAFKA_NODE_MODULES and local POOLSTER_KAFKA_BROKER"]
fn shared_event_package_roundtrips_real_broker() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../inputs/asyncapi/tests/fixtures/kafka-json.yaml");
    let topic = format!("poolster-shared-typescript-{}", std::process::id());
    let path = dir.path().join("source.yaml");
    std::fs::write(
        &path,
        std::fs::read_to_string(fixture)
            .unwrap()
            .replace("poolster-orders", &topic)
            .replace("poolster-alpha2-orders", &format!("{topic}-group")),
    )
    .unwrap();
    let mut registry = InputRegistry::new();
    registry
        .register(poolster_input_asyncapi::AsyncApiInput)
        .unwrap();
    generate_path(false, registry, &path)
        .write_to(dir.path())
        .unwrap();
    let client = dir.path().join("client");
    std::os::unix::fs::symlink(
        std::env::var("POOLSTER_KAFKA_NODE_MODULES").unwrap(),
        client.join("node_modules"),
    )
    .unwrap();
    let compiled = std::process::Command::new("node")
        .arg("node_modules/typescript/bin/tsc")
        .args(["-p", "tsconfig.json"])
        .current_dir(&client)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );
    std::fs::write(
        client.join("probe.cjs"),
        include_str!("fixtures/kafka_probe.cjs").replace("poolster-orders", &topic),
    )
    .unwrap();
    let runtime = std::process::Command::new("node")
        .arg("probe.cjs")
        .env(
            "POOLSTER_KAFKA_BROKER",
            std::env::var("POOLSTER_KAFKA_BROKER").unwrap(),
        )
        .current_dir(&client)
        .output()
        .unwrap();
    assert!(
        runtime.status.success(),
        "{}{}",
        String::from_utf8_lossy(&runtime.stdout),
        String::from_utf8_lossy(&runtime.stderr)
    );
}
