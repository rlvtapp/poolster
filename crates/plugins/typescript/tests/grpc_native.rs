use poolster_core::{
    engine::{Meta, Package, Packages, Plugin, PluginContext, Provision},
    input::{InputProvider, InputRegistry},
    native::rpc::RpcContract,
};
use poolster_plugin_typescript::{TypeScript, grpc};
use std::{path::Path, process::Command, sync::Arc};
fn generate() -> anyhow::Result<poolster_core::GeneratedTree> {
    let mut registry = InputRegistry::new();
    registry.register(poolster_input_protobuf::ProtobufInput)?;
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grpc/service.proto");
    let input = InputProvider::<RpcContract>::new(Arc::new(registry), "protobuf", source);
    let plugin = grpc().input(input.handle());
    Packages::new()
        .package(Package::<TypeScript>::new("sdk").with(plugin).with(input))
        .generate_native()
}
struct ContractProvider {
    meta: Meta,
    contract: RpcContract,
}
impl Plugin<TypeScript> for ContractProvider {
    fn kind(&self) -> &'static str {
        "test-rpc-substitution"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<RpcContract>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> anyhow::Result<()> {
        cx.publish(self.contract.clone())
    }
}
fn substituted() -> anyhow::Result<poolster_core::GeneratedTree> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grpc/service.proto");
    let mut contract = poolster_input_protobuf::load(&source)?.rpc_contract();
    contract.services[0].methods[0].name = "MetadataDoesNotRewriteWire".into();
    let provider = ContractProvider {
        meta: Meta::new(),
        contract,
    };

    let output = grpc().input(provider.meta.handle());
    Packages::new()
        .package(
            Package::<TypeScript>::new("sdk")
                .with(output)
                .with(provider),
        )
        .generate_native()
}
#[test]
#[ignore = "requires POOLSTER_TS_GRPC_TOOLS pinned npm runtime/compiler cache and Node20+"]
fn generated_grpc_compiles_and_executes_every_stream_direction() {
    let tools = std::env::var("POOLSTER_TS_GRPC_TOOLS").expect("POOLSTER_TS_GRPC_TOOLS");
    let tools = Path::new(&tools);
    let tree = generate().unwrap();
    assert_eq!(tree, generate().unwrap());
    let replaced = substituted().unwrap();
    assert_eq!(
        tree.get("sdk/src/service.ts"),
        replaced.get("sdk/src/service.ts")
    );
    assert_eq!(
        tree.get("sdk/src/common.ts"),
        replaced.get("sdk/src/common.ts")
    );
    assert!(
        tree.get("sdk/src/service.ts")
            .unwrap()
            .contains("StoreServer")
    );
    assert!(tree.get("sdk/src/common.ts").unwrap().contains("case:"));
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let sdk = root.path().join("sdk");
    #[cfg(unix)]
    std::os::unix::fs::symlink(tools.join("node_modules"), sdk.join("node_modules")).unwrap();
    std::fs::write(
        sdk.join("src/server-shape.ts"),
        include_str!("fixtures/grpc/server-shape.ts"),
    )
    .unwrap();
    let compile = Command::new(tools.join("node_modules/.bin/tsc"))
        .args(["-p", "tsconfig.json"])
        .current_dir(&sdk)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr)
    );
    std::fs::write(
        sdk.join("runtime.mjs"),
        include_str!("fixtures/grpc/runtime.mjs"),
    )
    .unwrap();
    let run = Command::new("node")
        .arg("runtime.mjs")
        .current_dir(&sdk)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(String::from_utf8_lossy(&run.stdout).contains("all grpc checks passed"));
    tree.write_to(root.path()).unwrap();
    assert!(sdk.join("runtime.mjs").exists());
}

#[test]
#[ignore = "requires pinned TypeScript gRPC runtime/compiler cache"]
fn proto2_defaults_and_presence_compile_and_roundtrip() {
    let tools = std::env::var("POOLSTER_TS_GRPC_TOOLS").expect("POOLSTER_TS_GRPC_TOOLS");
    let tools = Path::new(&tools);
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grpc/proto2.proto");
    let provider = ContractProvider {
        meta: Meta::new(),
        contract: poolster_input_protobuf::load_with_includes(
            &source,
            &[Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../inputs/protobuf/tests/fixtures/upstream")],
        )
        .unwrap()
        .rpc_contract(),
    };
    let output = grpc().input(provider.meta.handle());
    let tree = Packages::new()
        .package(
            Package::<TypeScript>::new("sdk")
                .with(output)
                .with(provider),
        )
        .generate_native()
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let sdk = root.path().join("sdk");
    #[cfg(unix)]
    std::os::unix::fs::symlink(tools.join("node_modules"), sdk.join("node_modules")).unwrap();
    let result = Command::new(tools.join("node_modules/.bin/tsc"))
        .args(["-p", "tsconfig.json"])
        .current_dir(&sdk)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    std::fs::write(sdk.join("proto2.mjs"), "import assert from 'node:assert/strict'; import { LegacySchema } from './dist/proto2.js'; import {create,fromBinary,toBinary,isFieldSet} from '@bufbuild/protobuf'; const empty=fromBinary(LegacySchema,new Uint8Array());assert.equal(empty.title,'fallback');assert.equal(empty.number,42n);assert.equal(isFieldSet(empty,LegacySchema.field.title),false);assert.equal(isFieldSet(empty,LegacySchema.field.number),false);assert.equal(toBinary(LegacySchema,empty).length,0);const set=create(LegacySchema,{title:'fallback',number:42n});assert.equal(isFieldSet(set,LegacySchema.field.title),true);assert.equal(toBinary(LegacySchema,set).length,12);assert.deepEqual(fromBinary(LegacySchema,toBinary(LegacySchema,set)),set);").unwrap();
    let result = Command::new("node")
        .arg("proto2.mjs")
        .current_dir(sdk)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn owned_generation_uses_descriptor_projection_and_provider_substitution() {
    let tree = generate().unwrap();
    let replaced = substituted().unwrap();
    for path in [
        "sdk/src/common.ts",
        "sdk/src/service.ts",
        "sdk/src/google/protobuf/timestamp.ts",
        "sdk/src/poolster-rpc-runtime.ts",
        "sdk/package.json",
    ] {
        assert_eq!(tree.get(path), replaced.get(path));
        assert!(tree.get(path).is_some());
    }
    assert!(tree.get("sdk/src/common.ts").unwrap().contains("bigint"));
    assert!(tree.get("sdk/src/common.ts").unwrap().contains("case:"));
    assert!(
        tree.get("sdk/src/service.ts")
            .unwrap()
            .contains("ClientDuplexStream")
    );
    assert!(tree.get("sdk/proto/service.proto").is_some());
    assert_eq!(tree, generate().unwrap());
}
#[test]
fn malformed_documents_fail_before_rendering() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("bad.proto");
    std::fs::write(
        &path,
        "syntax=\"proto3\"; message Broken { string field=0; }",
    )
    .unwrap();
    assert!(poolster_input_protobuf::load(&path).is_err());
}

#[test]
fn unsupported_groups_fail_with_an_explicit_diagnostic() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("groups.proto");
    std::fs::write(&path,"syntax=\"proto2\"; package old; message Envelope { optional group LegacyGroup=1 { optional string name=2; } } service Legacy { rpc Read(Envelope) returns(Envelope); }").unwrap();
    let provider = ContractProvider {
        meta: Meta::new(),
        contract: poolster_input_protobuf::load(&path).unwrap().rpc_contract(),
    };
    let output = grpc().input(provider.meta.handle());
    let error = Packages::new()
        .package(
            Package::<TypeScript>::new("sdk")
                .with(output)
                .with(provider),
        )
        .generate_native()
        .unwrap_err();
    assert!(format!("{error:#}").contains("groups"));
}

#[test]
fn normalized_type_name_collisions_are_diagnostic() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("one.proto"),
        "syntax=\"proto3\"; package a_b; message C { string value=1; }",
    )
    .unwrap();
    std::fs::write(
        temp.path().join("two.proto"),
        "syntax=\"proto3\"; package aB; message C { string value=1; }",
    )
    .unwrap();
    let path = temp.path().join("api.proto");
    std::fs::write(&path,"syntax=\"proto3\"; import \"one.proto\"; import \"two.proto\"; service Store { rpc Read(a_b.C) returns(aB.C); }").unwrap();
    let provider = ContractProvider {
        meta: Meta::new(),
        contract: poolster_input_protobuf::load(&path).unwrap().rpc_contract(),
    };
    let output = grpc().input(provider.meta.handle());
    let error = Packages::new()
        .package(
            Package::<TypeScript>::new("sdk")
                .with(output)
                .with(provider),
        )
        .generate_native()
        .unwrap_err();
    assert!(format!("{error:#}").contains("collide"));
}
