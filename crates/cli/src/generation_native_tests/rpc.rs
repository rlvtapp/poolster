use super::*;
use crate::generation_config::load_config_options;
fn rpc_recipe(root: &Path, packages: serde_json::Value) -> PathBuf {
    let path = root.join("poolster.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({
            "input":{"format":"protobuf","path":"service.proto"},
            "output":{"path":"generated"},"packages":packages
        }))
        .unwrap(),
    )
    .unwrap();
    path
}
#[test]
fn protobuf_python_and_typescript_need_no_external_generator() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("service.proto"), "syntax = \"proto3\"; package demo; message Request { optional string value = 1; } service Echo { rpc Call(Request) returns (Request); }").unwrap();
    let path = rpc_recipe(
        root.path(),
        serde_json::json!([
            {"language":"python","path":"python","name":"example-rpc","plugins":[{"name":"grpc"}]},
            {"language":"typescript","path":"typescript","name":"@example/rpc","plugins":[{"name":"grpc"}]}
        ]),
    );
    let options = load_config_options(&path, ColorChoice::Never, false, false).unwrap();
    assert!(options.native_output.module.is_none());
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    for file in ["python/pyproject.toml", "typescript/package.json"] {
        assert!(root.path().join("generated").join(file).exists());
    }
    generate_from_config(&path, ColorChoice::Never, true, false).unwrap();
    std::fs::write(
        root.path().join("generated/python/handwritten.py"),
        "# application implementation\n",
    )
    .unwrap();
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    assert!(root.path().join("generated/python/handwritten.py").exists());
}
#[test]
fn protobuf_recipe_rejects_foreign_language_options_before_loading_source() {
    let root = tempfile::tempdir().unwrap();
    for (language, options) in [
        ("python", serde_json::json!({"module":"example.test/api"})),
        (
            "typescript",
            serde_json::json!({"go_packages":{"service.proto":"example.test/api"}}),
        ),
        (
            "python",
            serde_json::json!({"toolchain":{"protoc":"protoc"}}),
        ),
        (
            "typescript",
            serde_json::json!({"toolchain":{"protoc":"protoc"}}),
        ),
    ] {
        let mut plugin = options;
        plugin["name"] = "grpc".into();
        let path = rpc_recipe(
            root.path(),
            serde_json::json!([{"language":language,"path":"sdk","plugins":[plugin]}]),
        );
        assert!(
            load_config_options(&path, ColorChoice::Never, false, false).is_err(),
            "{language}"
        );
        assert!(!root.path().join("generated").exists());
    }
    let path = rpc_recipe(
        root.path(),
        serde_json::json!([{"language":"rust","path":"rust","plugins":[{"name":"grpc"}]}]),
    );
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    assert!(!root.path().join("generated").exists());
}
#[test]
#[ignore = "requires the existing pinned Go toolchain in POOLSTER_GRPC_TOOLS"]
fn protobuf_three_language_recipe_generates_and_regenerates() {
    let root = tempfile::tempdir().unwrap();
    let tools = PathBuf::from(std::env::var("POOLSTER_GRPC_TOOLS").unwrap());
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../plugins/go/tests/fixtures/grpc");
    for file in ["service.proto", "common.proto"] {
        std::fs::copy(fixture.join(file), root.path().join(file)).unwrap();
    }
    let path = rpc_recipe(
        root.path(),
        serde_json::json!([
            {"language":"go","path":"go","plugins":[{"name":"grpc","module":"example.com/poolster/grpc","toolchain":{"protoc":tools.join("protoc"),"protoc_gen_go":tools.join("protoc-gen-go"),"protoc_gen_go_grpc":tools.join("protoc-gen-go-grpc")}}]},
            {"language":"python","path":"python","name":"example-rpc","plugins":[{"name":"grpc"}]},
            {"language":"typescript","path":"typescript","name":"@example/rpc","plugins":[{"name":"grpc"}]}
        ]),
    );
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    for file in [
        "go/storev1/service_grpc.pb.go",
        "python/pyproject.toml",
        "typescript/package.json",
    ] {
        assert!(root.path().join("generated").join(file).exists(), "{file}");
    }
    generate_from_config(&path, ColorChoice::Never, true, false).unwrap();
    std::fs::write(
        root.path().join("generated/python/handwritten.py"),
        "# application implementation\n",
    )
    .unwrap();
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    assert!(root.path().join("generated/python/handwritten.py").exists());
    std::fs::write(
        root.path().join("service.proto"),
        "syntax = \"proto3\"; message Changed { string value = 1; }",
    )
    .unwrap();
    assert!(generate_from_config(&path, ColorChoice::Never, true, false).is_err());
}
