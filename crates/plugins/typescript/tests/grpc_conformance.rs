use poolster_core::{
    engine::{Package, Packages},
    input::{InputProvider, InputRegistry},
    native::rpc::{RpcContract, RpcGenerationOptions},
};
use poolster_plugin_typescript::{grpc, TypeScript};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};
fn generate(file: &str, options: RpcGenerationOptions) -> poolster_core::GeneratedTree {
    try_generate(file, options).unwrap()
}
fn try_generate(
    file: &str,
    options: RpcGenerationOptions,
) -> anyhow::Result<poolster_core::GeneratedTree> {
    let mut registry = InputRegistry::new();
    registry.register(poolster_input_protobuf::ProtobufInput)?;
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/grpc/conformance")
        .join(file);
    let input = InputProvider::<RpcContract>::new(Arc::new(registry), "protobuf", source);
    let output = grpc().input(input.handle()).options(options);
    Packages::new()
        .package(Package::<TypeScript>::new("sdk").with(output).with(input))
        .generate_native()
}
fn runtime(root: &Path, cache: &Path, script: &str) {
    let sdk = root.join("sdk");
    #[cfg(unix)]
    std::os::unix::fs::symlink(cache.join("node_modules"), sdk.join("node_modules")).unwrap();
    let compile = Command::new(cache.join("node_modules/.bin/tsc"))
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
    std::fs::write(sdk.join("conformance.mjs"), script).unwrap();
    let run = Command::new("node")
        .arg("conformance.mjs")
        .current_dir(sdk)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
}
#[test]
fn exported_surfaces_follow_independent_emission_controls() {
    let options = RpcGenerationOptions {
        emit_binary: false,
        emit_json: false,
        emit_clients: false,
        emit_servers: false,
        emit_reflection: false,
        ..Default::default()
    };
    let tree = generate("wire.proto", options);
    let src = tree.get("sdk/src/wire.ts").unwrap();
    assert!(!src.contains("registry"));
    assert!(!src.contains("Schema="));
    assert!(!src.contains("from '@bufbuild"));
    assert!(src.contains("export type {"));
    assert!(tree.get("sdk/src/poolster-rpc-runtime.ts").is_none());
    let manifest: serde_json::Value =
        serde_json::from_str(tree.get("sdk/package.json").unwrap()).unwrap();
    assert!(manifest["dependencies"].as_object().unwrap().is_empty());
    let options = RpcGenerationOptions {
        emit_types: false,
        emit_clients: false,
        emit_servers: false,
        emit_reflection: false,
        ..Default::default()
    };
    let tree = generate("wire.proto", options);
    let src = tree.get("sdk/src/wire.ts").unwrap();
    assert!(src.contains("ScalarsBinary"));
    assert!(src.contains("ScalarsJson"));
    assert!(!src.contains("as Scalars }"));
    assert!(!src.contains("as ScalarsSchema"));
}
#[test]
#[ignore = "requires POOLSTER_TS_GRPC_TOOLS pinned runtime/compiler cache"]
fn messages_json_wire_wkt_presence_and_bounds_execute() {
    let cache = PathBuf::from(std::env::var("POOLSTER_TS_GRPC_TOOLS").unwrap());
    let root = tempfile::tempdir().unwrap();
    generate(
        "wire.proto",
        RpcGenerationOptions {
            max_message_bytes: 4096,
            max_depth: 8,
            ..Default::default()
        },
    )
    .write_to(root.path())
    .unwrap();
    std::fs::write(root.path().join("sdk/src/representation-proof.ts"), "import {RepresentationCreate,ScalarsCreate} from './wire.js'; const input=RepresentationCreate({text:'9007199254740993',texts:['18446744073709551615'],numeric:9007199254740993n,choice:{case:'key',value:'-42'}});const text:string=input.text;const numeric:bigint=input.numeric;ScalarsCreate({kind:987});void text;void numeric;\n// @ts-expect-error descriptor string values cannot be bigint\nRepresentationCreate({text:42n});\n// @ts-expect-error JS_NUMBER does not change Buf lossless bigint representation\nRepresentationCreate({numeric:42});\n").unwrap();
    runtime(
        root.path(),
        &cache,
        include_str!("fixtures/grpc/conformance/runtime.mjs"),
    );
    let root = tempfile::tempdir().unwrap();
    generate("legacy.proto", Default::default())
        .write_to(root.path())
        .unwrap();
    runtime(
        root.path(),
        &cache,
        include_str!("fixtures/grpc/conformance/legacy.mjs"),
    );
}
#[test]
#[ignore = "requires POOLSTER_TS_GRPC_TOOLS pinned runtime/compiler cache"]
fn independent_emission_configurations_compile_as_clean_consumers() {
    let cache = PathBuf::from(std::env::var("POOLSTER_TS_GRPC_TOOLS").unwrap());
    for options in [
        RpcGenerationOptions {
            emit_binary: false,
            emit_json: false,
            emit_clients: false,
            emit_servers: false,
            emit_reflection: false,
            ..Default::default()
        },
        RpcGenerationOptions {
            emit_types: false,
            emit_json: false,
            emit_clients: false,
            emit_servers: false,
            emit_reflection: false,
            ..Default::default()
        },
        RpcGenerationOptions {
            emit_types: false,
            emit_binary: false,
            emit_clients: false,
            emit_servers: false,
            emit_reflection: false,
            ..Default::default()
        },
    ] {
        let root = tempfile::tempdir().unwrap();
        generate("wire.proto", options)
            .write_to(root.path())
            .unwrap();
        runtime(
            root.path(),
            &cache,
            "console.log('emission config compiled');",
        );
    }
}

#[test]
#[ignore = "requires POOLSTER_TS_GRPC_TOOLS pinned runtime/compiler cache"]
fn explicit_batch_scopes_compile_and_execute() {
    let cache = PathBuf::from(std::env::var("POOLSTER_TS_GRPC_TOOLS").unwrap());
    let root = tempfile::tempdir().unwrap();
    generate(
        "batching.proto",
        RpcGenerationOptions {
            batching: vec![poolster_core::native::rpc::RpcBatchMapping {
                single_method: "poolster.conformance.batch.Reader.Read".into(),
                batch_method: "poolster.conformance.batch.Reader.ReadMany".into(),
                input_field: "id".into(),
                batch_input_field: "ids".into(),
                batch_output_field: "entities".into(),
                output_key: Some("id".into()),
                cache: true,
                max_batch_size: 2,
            }],
            ..Default::default()
        },
    )
    .write_to(root.path())
    .unwrap();
    runtime(
        root.path(),
        &cache,
        include_str!("fixtures/grpc/conformance/batching.mjs"),
    );
}

#[test]
#[ignore = "requires POOLSTER_TS_GRPC_TOOLS pinned runtime/compiler cache"]
fn discarded_unknown_fields_and_decode_depth_are_explicit() {
    let cache = PathBuf::from(std::env::var("POOLSTER_TS_GRPC_TOOLS").unwrap());
    let root = tempfile::tempdir().unwrap();
    generate(
        "wire.proto",
        RpcGenerationOptions {
            unknown_fields: poolster_core::native::rpc::RpcUnknownFields::Discard,
            max_depth: 2,
            ..Default::default()
        },
    )
    .write_to(root.path())
    .unwrap();
    runtime(
        root.path(),
        &cache,
        "import assert from 'node:assert/strict';import {ScalarsBinary,NodeBinary} from './dist/wire.js';const input=Uint8Array.from([160,6,42]);assert.equal(ScalarsBinary.encode(ScalarsBinary.decode(input)).length,0);assert.throws(()=>NodeBinary.decode(Uint8Array.from([18,6,18,4,18,2,18,0])),/depth|recursion/i);",
    );
}

#[test]
#[ignore = "requires POOLSTER_TS_GRPC_TOOLS pinned runtime/compiler cache"]
fn portable_layout_and_selective_codec_controls_compile() {
    let cache = PathBuf::from(std::env::var("POOLSTER_TS_GRPC_TOOLS").unwrap());
    let root = tempfile::tempdir().unwrap();
    let tree = generate(
        "wire.proto",
        RpcGenerationOptions {
            type_prefix: "X".into(),
            type_suffix: "Y".into(),
            snake_type_names: true,
            file_suffix: ".generated".into(),
            comments: false,
            annotate_files_with_version: false,
            encode_include_types: vec!["conformance.Scalars".into()],
            decode_include_types: vec!["conformance.Node".into()],
            ..Default::default()
        },
    );
    let source = tree.get("sdk/src/wire.generated.ts").unwrap();
    assert!(source.contains("Pb_Xconformance_ScalarsY"));
    assert!(source.contains("XScalarsYBinary"));
    assert!(!source.contains("Generated by Poolster"));
    assert!(tree
        .get("sdk/src/index.ts")
        .unwrap()
        .contains("wire.generated.js"));
    tree.write_to(root.path()).unwrap();
    std::fs::write(root.path().join("sdk/src/selected-proof.ts"),"import {XScalarsYBinary,XNodeYBinary} from './wire.generated.js';void XScalarsYBinary.encode;void XNodeYBinary.decode;\n// @ts-expect-error explicitly filtered decode surface\nvoid XScalarsYBinary.decode;\n// @ts-expect-error explicitly filtered encode surface\nvoid XNodeYBinary.encode;\n").unwrap();
    runtime(
        root.path(),
        &cache,
        "import assert from 'node:assert/strict';import {XScalarsYBinary,XNodeYBinary} from './dist/wire.generated.js';assert.equal(typeof XScalarsYBinary.encode,'function');assert.equal(XScalarsYBinary.decode,undefined);assert.equal(typeof XNodeYBinary.decode,'function');assert.equal(XNodeYBinary.encode,undefined);",
    );
}

#[test]
#[ignore = "requires POOLSTER_TS_GRPC_TOOLS pinned runtime/compiler cache"]
fn typed_value_representations_preserve_wire_and_reject_precision_loss() {
    use poolster_core::native::rpc::*;
    let cache = PathBuf::from(std::env::var("POOLSTER_TS_GRPC_TOOLS").unwrap());
    let root = tempfile::tempdir().unwrap();
    generate(
        "wire.proto",
        RpcGenerationOptions {
            representations: RpcRepresentations {
                int64: RpcInt64Representation::String,
                bytes: RpcBytesRepresentation::Base64,
                maps: RpcMapsRepresentation::Map,
                enums: RpcEnumsRepresentation::Name,
                optional: RpcOptionalRepresentation::Nullable,
                timestamp: RpcTimestampRepresentation::Rfc3339,
                duration: RpcDurationRepresentation::SecondsExact,
            },
            ..Default::default()
        },
    )
    .write_to(root.path())
    .unwrap();
    runtime(
        root.path(),
        &cache,
        include_str!("fixtures/grpc/conformance/values.mjs"),
    );
}

#[test]
#[ignore = "requires POOLSTER_TS_GRPC_TOOLS pinned runtime/compiler cache"]
fn enum_declaration_policies_compile_and_preserve_wire_identity() {
    use poolster_core::native::rpc::RpcEnumStyle;
    let cache = PathBuf::from(std::env::var("POOLSTER_TS_GRPC_TOOLS").unwrap());
    for enum_style in [
        RpcEnumStyle::Native,
        RpcEnumStyle::Const,
        RpcEnumStyle::Literal,
    ] {
        let root = tempfile::tempdir().unwrap();
        generate(
            "enum_styles.proto",
            RpcGenerationOptions {
                enum_style,
                remove_enum_prefix: true,
                unknown_enum_name: "FUTURE".into(),
                ..Default::default()
            },
        )
        .write_to(root.path())
        .unwrap();
        runtime(
            root.path(),
            &cache,
            "import assert from 'node:assert/strict';import {EnumModelCreate,EnumModelBinary,StateName} from './dist/enum_styles.js';assert.equal(StateName(1),'READY');assert.equal(StateName(99),'FUTURE');const value=EnumModelCreate({state:1});assert.equal(EnumModelBinary.decode(EnumModelBinary.encode(value)).state,1);",
        );
    }
}

#[test]
#[ignore = "requires POOLSTER_TS_GRPC_TOOLS pinned runtime/compiler cache"]
fn browser_package_entry_does_not_load_native_grpc() {
    use poolster_core::native::rpc::RpcTransport;
    let cache = PathBuf::from(std::env::var("POOLSTER_TS_GRPC_TOOLS").unwrap());
    let root = tempfile::tempdir().unwrap();
    generate(
        "batching.proto",
        RpcGenerationOptions {
            transports: vec![RpcTransport::GrpcWeb, RpcTransport::Twirp],
            ..Default::default()
        },
    )
    .write_to(root.path())
    .unwrap();
    runtime(
        root.path(),
        &cache,
        "import assert from 'node:assert/strict';import {registerHooks} from 'node:module';import {readFileSync} from 'node:fs';registerHooks({resolve(specifier,context,next){if(specifier.includes('@grpc/grpc-js')||specifier.endsWith('.grpc.js'))throw new Error('Browser entry imported native grpc');return next(specifier,context);}});const manifest=JSON.parse(readFileSync('./package.json'));const browser=await import(manifest.name+'/browser');assert.equal(typeof browser.createReaderGrpcWebClient,'function');assert.equal(typeof browser.createReaderTwirpClient,'function');",
    );
}

#[test]
fn alternate_model_names_are_reserved_before_output() {
    use poolster_core::native::rpc::{RpcBytesRepresentation, RpcRepresentations};
    let error = try_generate(
        "value_collision.proto",
        RpcGenerationOptions {
            representations: RpcRepresentations {
                bytes: RpcBytesRepresentation::Base64,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(format!("{error:#}").contains("collide"));
}

#[test]
fn external_native_modules_reject_alternate_value_models_before_emission() {
    let mut options = RpcGenerationOptions::default();
    options.representations.bytes = poolster_core::native::rpc::RpcBytesRepresentation::Base64;
    options
        .import_mappings
        .insert("wire.proto".into(), "external-wire".into());
    let error =
        try_generate("wire.proto", options).expect_err("unsupported external DTO composition");
    assert!(format!("{error:#}").contains("alternate representations cannot use importMappings"));
}
