use super::*;
use poolster_core::native::rpc::{RpcEnumStyle, RpcUnknownFields};
use std::collections::BTreeMap;
fn execute(path: &std::path::Path) -> std::process::Output {
    Command::new("go")
        .args(["test", "-timeout=30s", "-mod=readonly", "./..."])
        .env("GOCACHE", "/private/tmp/kaji-go-cache")
        .current_dir(path)
        .output()
        .unwrap()
}
fn success(output: std::process::Output) {
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
#[ignore = "requires Go toolchain and pinned runtime cache"]
fn external_import_composition_uses_existing_packages_without_reemitting() {
    let baseline = generate(Default::default());
    let dir = tempfile::tempdir().unwrap();
    let dependencies = tempfile::tempdir().unwrap();
    baseline.write_to(dependencies.path()).unwrap();
    let tree = super::harness::generate_with_source(
        RpcGenerationOptions {
            representations: poolster_core::native::rpc::RpcRepresentations {
                int64: poolster_core::native::rpc::RpcInt64Representation::String,
                ..Default::default()
            },
            emit_imported_files: false,
            import_mappings: BTreeMap::from([
                (
                    "portable.proto".into(),
                    "example.com/poolster/conformance/wire;wire".into(),
                ),
                (
                    "legacy.proto".into(),
                    "example.com/poolster/conformance/legacy;legacy".into(),
                ),
                (
                    "batching.proto".into(),
                    "example.com/poolster/conformance/batch;batch".into(),
                ),
            ]),
            ..Default::default()
        },
        "message Imported { poolster.conformance.ScalarValues value = 1; }",
    )
    .unwrap();
    assert!(tree.get("sdk/wire/portable_ScalarValues.pb.go").is_none());
    assert!(tree.get("sdk/legacy/legacy_RequiredValues.pb.go").is_none());
    assert!(tree.get("sdk/bundle/bundle.pb.go").is_some());
    tree.write_to(dir.path()).unwrap();
    // Imported packages are independently supplied, outside this generation ownership.
    for package in ["wire", "legacy", "batch"] {
        copy_directory(
            &dependencies.path().join("sdk").join(package),
            &dir.path().join("sdk").join(package),
        );
    }
    success(execute(&dir.path().join("sdk")));
}
#[test]
#[ignore = "requires Go toolchain and pinned runtime cache"]
fn explicit_discard_enum_aliases_and_exact_partial_consumers_execute() {
    let tree = generate(RpcGenerationOptions {
        unknown_fields: RpcUnknownFields::Discard,
        remove_enum_prefix: true,
        enum_style: RpcEnumStyle::Literal,
        unknown_enum_name: "FUTURE".into(),
        ..Default::default()
    });
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    let root = dir.path().join("sdk");
    std::fs::write(root.join("policies_test.go"),r#"package conformance_test
import("testing";pb "example.com/poolster/conformance/wire")
func TestPolicies(t *testing.T){partial:=&pb.ScalarValues{State:pb.READY};if partial.GetState()!=pb.ScalarValues_READY{t.Fatal("native enum alias")};if pb.ScalarValues_State(99).DisplayName()!="FUTURE"{t.Fatal("display label invented enum member")};if err:=partial.UnmarshalBinary([]byte{0xc0,0x0c,0x2a});err!=nil{t.Fatal(err)};if len(partial.ProtoReflect().GetUnknown())!=0{t.Fatal("explicit discard ignored")}}
"#).unwrap();
    success(execute(&root));
    for source in [
        "package conformance_test\nimport pb \"example.com/poolster/conformance/wire\"\nvar invalid = &pb.ScalarValues{UnknownField:1}\n",
        "package conformance_test\nimport pb \"example.com/poolster/conformance/wire\"\nvar invalid = &pb.ScalarValues{State:\"READY\"}\n",
    ] {
        std::fs::write(root.join("invalid_test.go"), source).unwrap();
        assert!(
            !execute(&root).status.success(),
            "invalid typed consumer compiled"
        );
    }
    std::fs::remove_file(root.join("invalid_test.go")).unwrap();
}

fn copy_directory(source: &std::path::Path, target: &std::path::Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination = target.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_directory(&entry.path(), &destination);
        } else {
            std::fs::copy(entry.path(), destination).unwrap();
        }
    }
}

#[test]
fn incompatible_helper_profiles_fail_before_emission() {
    use poolster_core::native::rpc::{RpcInt64Representation, RpcRepresentations};
    let options = RpcGenerationOptions {
        emit_types: false,
        emit_servers: false,
        representations: RpcRepresentations {
            int64: RpcInt64Representation::String,
            ..Default::default()
        },
        ..Default::default()
    };
    let error = super::harness::generate_result(options)
        .expect_err("incompatible representations accepted");
    assert!(format!("{error:#}").contains("representations require"));
}
#[test]
#[ignore = "requires Go toolchain and pinned runtime cache"]
fn types_only_representation_profiles_compile_without_codec_helpers() {
    use poolster_core::native::rpc::{RpcInt64Representation, RpcRepresentations};
    let tree = generate(RpcGenerationOptions {
        emit_binary: false,
        emit_json: false,
        emit_clients: false,
        emit_servers: false,
        representations: RpcRepresentations {
            int64: RpcInt64Representation::String,
            ..Default::default()
        },
        ..Default::default()
    });
    let dir = tempfile::tempdir().unwrap();
    tree.write_to(dir.path()).unwrap();
    success(execute(&dir.path().join("sdk")));
}
