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
        .arg(include_str!("fixtures/grpc/runtime.py"))
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
