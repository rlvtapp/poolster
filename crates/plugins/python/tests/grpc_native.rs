use poolster_core::{
    engine::{Package, Packages},
    input::{InputProvider, InputRegistry},
    native::rpc::RpcContract,
};
use poolster_plugin_python::{Python, grpc};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};
fn generate(source: PathBuf) -> anyhow::Result<poolster_core::GeneratedTree> {
    let mut registry = InputRegistry::new();
    registry.register(poolster_input_protobuf::ProtobufInput)?;
    let input = InputProvider::<RpcContract>::new(Arc::new(registry), "protobuf", source);
    let plugin = grpc("poolster-rpc-test").input(input.handle());
    Packages::new()
        .package(Package::<Python>::new("sdk").with(plugin).with(input))
        .generate_native()
}
#[test]
#[ignore = "requires Python 3.10+ with pinned grpcio/protobuf in POOLSTER_PYTHON_GRPC"]
fn poolster_python_grpc_compiles_installs_and_runs_all_streams() {
    let python =
        PathBuf::from(std::env::var("POOLSTER_PYTHON_GRPC").expect("POOLSTER_PYTHON_GRPC"));
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grpc/service.proto");
    let tree = generate(source.clone()).unwrap();
    assert!(
        tree.get("sdk/src/service_pb2_grpc.py")
            .unwrap()
            .contains("class StoreStub")
    );
    assert!(
        tree.get("sdk/src/common_pb2.pyi")
            .unwrap()
            .contains("class Item")
    );
    assert!(tree.get("sdk/proto/common.proto").is_some());
    assert_eq!(tree, generate(source.clone()).unwrap());
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let handwritten = root.path().join("sdk/server.py");
    std::fs::write(&handwritten, "# user-owned implementation\n").unwrap();
    tree.write_to(root.path()).unwrap();
    assert_eq!(
        std::fs::read_to_string(handwritten).unwrap(),
        "# user-owned implementation\n"
    );
    for args in [
        vec!["-m", "compileall", "-q", "src"],
        vec![
            "-m",
            "pip",
            "wheel",
            "--no-deps",
            "--no-build-isolation",
            "--wheel-dir",
            "dist",
            ".",
        ],
    ] {
        let result = Command::new(&python)
            .args(args)
            .current_dir(root.path().join("sdk"))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let wheel = std::fs::read_dir(root.path().join("sdk/dist"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let installed = root.path().join("installed");
    let result = Command::new(&python)
        .args(["-m", "pip", "install", "--no-deps", "--target"])
        .arg(&installed)
        .arg(wheel)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        installed.join("common_pb2.pyi").is_file(),
        "wheel must retain message typing declarations"
    );
    assert!(
        installed.join("service_pb2_grpc.pyi").is_file(),
        "wheel must retain service typing declarations"
    );
    let result = Command::new(&python)
        .arg("-c")
        .arg(format!(
            "{}\n{}",
            include_str!("fixtures/grpc/runtime.py"),
            include_str!("fixtures/grpc/transport.py")
        ))
        .env("PYTHONPATH", &installed)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    tree.write_to(root.path()).unwrap();
}

#[test]
fn substituted_owned_rpc_provider_uses_authoritative_descriptors() {
    use poolster_core::engine::{Meta, Plugin, PluginContext, Provision};
    struct Provider {
        meta: Meta,
        contract: RpcContract,
    }
    impl Plugin<Python> for Provider {
        fn kind(&self) -> &'static str {
            "replacement-rpc"
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
        fn generate(&self, cx: &mut PluginContext<Python>) -> anyhow::Result<()> {
            cx.publish(self.contract.clone())
        }
    }
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grpc/service.proto");
    let contract = poolster_input_protobuf::load(&source)
        .unwrap()
        .rpc_contract();
    let provider = Provider {
        meta: Meta::new(),
        contract,
    };
    let handle = provider.meta.handle::<RpcContract>();
    let plugin = grpc("poolster-rpc-test").input(handle);
    let replacement = Packages::new()
        .package(Package::<Python>::new("sdk").with(plugin).with(provider))
        .generate_native()
        .unwrap();
    assert_eq!(replacement, generate(source).unwrap());
}

#[test]
fn protobuf_generation_requires_no_python_or_codegen_configuration() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grpc/service.proto");
    let tree = generate(source).unwrap();
    assert!(
        tree.get("sdk/src/service_pb2_grpc.py")
            .unwrap()
            .contains("# Generated by Poolster.")
    );
    assert!(
        !tree
            .get("sdk/pyproject.toml")
            .unwrap()
            .contains("grpcio-tools")
    );
}

#[test]
#[ignore = "requires Python with pinned grpcio/protobuf in POOLSTER_PYTHON_GRPC"]
fn pinned_upstream_proto2_public_imports_compile_and_preserve_presence() {
    let python =
        PathBuf::from(std::env::var("POOLSTER_PYTHON_GRPC").expect("POOLSTER_PYTHON_GRPC"));
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../inputs/protobuf/tests/fixtures/upstream/google/protobuf");
    let source = tempfile::tempdir().unwrap();
    std::fs::create_dir(source.path().join("upstream")).unwrap();
    let public = std::fs::read_to_string(corpus.join("unittest_import_public.proto")).unwrap();
    std::fs::write(source.path().join("upstream/public.proto"), public).unwrap();
    let imported = std::fs::read_to_string(corpus.join("unittest_import.proto"))
        .unwrap()
        .replace(
            "google/protobuf/unittest_import_public.proto",
            "upstream/public.proto",
        );
    std::fs::write(source.path().join("upstream/imported.proto"), imported).unwrap();
    std::fs::write(source.path().join("service.proto"),"syntax=\"proto2\"; package poolster.upstream; import \"upstream/imported.proto\"; service Imported { rpc Get(protobuf_unittest_import.ImportMessage) returns (protobuf_unittest_import.ImportMessage); }").unwrap();
    let tree = generate(source.path().join("service.proto")).unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let code = "import ast,pathlib; from upstream import imported_pb2 as m; import service_pb2_grpc; value=m.ImportMessage(); assert value.d==0 and not value.HasField('d'); value.d=0; assert value.HasField('d'); assert m.ImportMessage.FromString(value.SerializeToString()).HasField('d'); assert m.PublicImportMessage(e=2).e==2; [ast.parse(p.read_text()) for p in pathlib.Path('src').rglob('*.pyi')]";
    let result = Command::new(python)
        .args(["-c", code])
        .env("PYTHONPATH", root.path().join("sdk/src"))
        .current_dir(root.path().join("sdk"))
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
fn messages_only_generation_has_message_typing_without_services() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("messages.proto");
    std::fs::write(
        &source,
        "syntax=\"proto3\"; message Payload {string value=1;}",
    )
    .unwrap();
    let tree = generate(source).unwrap();
    assert!(
        tree.get("sdk/src/messages_pb2.pyi")
            .unwrap()
            .contains("class Payload")
    );
    assert!(tree.get("sdk/src/messages_pb2_grpc.py").is_none());
}

#[test]
#[ignore = "requires pinned mypy/types-grpcio/types-protobuf in POOLSTER_PYTHON_GRPC"]
fn generated_message_and_sync_async_consumers_are_statically_typed() {
    let python =
        PathBuf::from(std::env::var("POOLSTER_PYTHON_GRPC").expect("POOLSTER_PYTHON_GRPC"));
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grpc/service.proto");
    let tree = generate(source).unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    for (name, code, valid) in [
        (
            "typing_valid.py",
            include_str!("fixtures/grpc/typing_valid.py"),
            true,
        ),
        (
            "typing_invalid.py",
            include_str!("fixtures/grpc/typing_invalid.py"),
            false,
        ),
    ] {
        let consumer = root.path().join("sdk").join(name);
        std::fs::write(&consumer, code).unwrap();
        let result = Command::new(&python)
            .args(["-m", "mypy", "--strict", "--no-incremental"])
            .arg(consumer)
            .env("MYPYPATH", root.path().join("sdk/src"))
            .output()
            .unwrap();
        let output = format!(
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        if valid {
            assert!(result.status.success(), "{output}");
        } else {
            assert!(!result.status.success(), "invalidconsumeraccepted");
            assert!(output.matches("error:").count() >= 5, "{output}");
            assert!(output.contains("typing_invalid.py"), "{output}");
        }
    }
}

#[test]
#[ignore = "requires pinned Python runtimes in POOLSTER_PYTHON_GRPC"]
fn native_wire_protojson_and_well_known_conformance() {
    let python =
        PathBuf::from(std::env::var("POOLSTER_PYTHON_GRPC").expect("POOLSTER_PYTHON_GRPC"));
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../inputs/protobuf/tests/fixtures/conformance/portable.proto");
    let tree = generate(source).unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let result = Command::new(&python)
        .args(["-c", include_str!("fixtures/grpc/semantics.py")])
        .env("PYTHONPATH", root.path().join("sdk/src"))
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
#[ignore = "requires pinned Python runtimes/build tools in POOLSTER_PYTHON_GRPC"]
fn messages_only_package_installs_with_controls_and_bounded_helpers() {
    use poolster_core::native::rpc::{RpcGenerationOptions, RpcUnknownFields};
    let python =
        PathBuf::from(std::env::var("POOLSTER_PYTHON_GRPC").expect("POOLSTER_PYTHON_GRPC"));
    let source = tempfile::tempdir().unwrap();
    let schema = source.path().join("models.proto");
    std::fs::write(
        &schema,
        "syntax=\"proto3\"; package bounded; message Payload {string value=1; Payload child=2;}",
    )
    .unwrap();
    let mut registry = InputRegistry::new();
    registry
        .register(poolster_input_protobuf::ProtobufInput)
        .unwrap();
    let input = InputProvider::<RpcContract>::new(Arc::new(registry), "protobuf", schema);
    let plugin = grpc("bounded-models")
        .input(input.handle())
        .options(RpcGenerationOptions {
            emit_clients: false,
            emit_servers: false,
            emit_types: false,
            emit_json: false,
            emit_reflection: false,
            unknown_fields: RpcUnknownFields::Discard,
            max_message_bytes: 32,
            max_depth: 2,
            ..Default::default()
        });
    let tree = Packages::new()
        .package(Package::<Python>::new("sdk").with(plugin).with(input))
        .generate_native()
        .unwrap();
    assert!(tree.get("sdk/src/models_pb2.pyi").is_none());
    assert!(tree.get("sdk/src/models_pb2_grpc.py").is_none());
    assert!(!tree.get("sdk/pyproject.toml").unwrap().contains("grpcio=="));
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let installed = root.path().join("installed");
    let result = Command::new(&python)
        .args([
            "-m",
            "pip",
            "install",
            "--no-deps",
            "--no-build-isolation",
            "--target",
        ])
        .arg(&installed)
        .arg(root.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let code = r#"import models_pb2 as m; import _poolster_codecs_bounded_models as c
assert c.decode(m.Payload,c.encode(m.Payload(value='test'))).value=='test'
assert not hasattr(c,'to_json') and not hasattr(c,'message_type')
raw=m.Payload(value='a').SerializeToString()+b'\xa2\x06\x01x'
assert b'\xa2\x06\x01x' not in c.encode(c.decode(m.Payload,raw))
for invalid in [b'x'*33]:
    try:c.decode(m.Payload,invalid)
    except ValueError:pass
    else:raise AssertionError('size limit ignored')
value=m.Payload();value.child.child.child.value='deep'
for action in [lambda:c.encode(value),lambda:c.decode(m.Payload,value.SerializeToString()),lambda:c.encode(m.Payload(value='x'*33))]:
    try:action()
    except ValueError:pass
    else:raise AssertionError('depth/size limit ignored')
"#;
    let result = Command::new(&python)
        .args(["-c", code])
        .env("PYTHONPATH", &installed)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
