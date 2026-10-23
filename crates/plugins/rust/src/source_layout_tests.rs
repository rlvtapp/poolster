fn fixture() -> poolster_core::Api {
    let mut api = poolster_core::Api {
        name: "Layout boundary".into(),
        version: "1.0.0".into(),
        ..Default::default()
    };
    api.operations = (0..48)
        .map(|index| poolster_core::Operation {
            id: format!("listItems{index}{}", "y".repeat(1024)),
            method: poolster_core::HttpMethod::Get,
            path: format!("/items/{index}"),
            parameters: (0..48)
                .map(|field| poolster_core::OperationParameter {
                    name: format!("field{field}{}", "x".repeat(112)),
                    location: "query".into(),
                    required: false,
                    schema: Some(poolster_core::SchemaValue::new(
                        poolster_core::SchemaKind::String,
                    )),
                    description: None,
                    annotations: Default::default(),
                })
                .collect(),
            annotations: std::collections::BTreeMap::from([(
                "tags".into(),
                serde_json::json!(["Items"]),
            )]),
            ..Default::default()
        })
        .collect();
    api
}
#[test]
fn byte_budget_splits_verbose_single_resource_before_count_limit() {
    let tree =
        crate::render::generate_sdk(&fixture(), &crate::render::RenderOptions::default()).unwrap();
    let chunks = tree
        .iter()
        .filter(|(path, _)| {
            path.to_string_lossy().starts_with("src/client/operations")
                && path.extension().and_then(|extension| extension.to_str()) == Some("rs")
        })
        .collect::<Vec<_>>();
    assert!(chunks.len() > 2);
    for (path, source) in chunks {
        assert!(
            source.len() <= 128 * 1024,
            "{}: {}",
            path.display(),
            source.len()
        );
    }
    let resources = tree
        .iter()
        .filter(|(path, _)| {
            path.to_string_lossy()
                .starts_with("src/client/resources/items_1/chunk_")
                && path.extension().and_then(|value| value.to_str()) == Some("rs")
        })
        .collect::<Vec<_>>();
    assert!(resources.len() > 1);
    for (path, source) in resources {
        assert!(
            source.len() <= 128 * 1024,
            "{} {}",
            path.display(),
            source.len()
        );
    }
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let path = tree
        .iter()
        .find(|(path, _)| {
            path.to_string_lossy().starts_with("src/client/operations")
                && path.extension().and_then(|extension| extension.to_str()) == Some("rs")
        })
        .unwrap()
        .0;
    std::fs::write(root.path().join(path), "customer edit").unwrap();
    assert!(
        tree.write_to(root.path())
            .unwrap_err()
            .to_string()
            .contains("locally modified generated file")
    );
}
#[test]
#[ignore = "requires native SDK toolchain and cached dependencies"]
fn native_byte_grouped_sdk_compiles() {
    let tree =
        crate::render::generate_sdk(&fixture(), &crate::render::RenderOptions::default()).unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let result = crate::native_cargo()
        .args(["check", "--quiet"])
        .env("CARGO_NET_OFFLINE", "true")
        .current_dir(root.path())
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
fn oversized_atomic_model_keeps_native_fields_and_reports_exact_source() {
    let api = poolster_core::Api {
        schemas: vec![poolster_core::Schema::new(
            "Large",
            poolster_core::SchemaValue::new(poolster_core::SchemaKind::Object {
                fields: (0..1000)
                    .map(|index| poolster_core::Field {
                        name: format!("field{index}{}", "z".repeat(128)),
                        value: poolster_core::SchemaValue::new(poolster_core::SchemaKind::String),
                        required: true,
                        annotations: Default::default(),
                    })
                    .collect(),
                additional_properties: poolster_core::AdditionalProperties::Forbidden,
            }),
        )],
        ..Default::default()
    };
    let files = crate::render::RustModels
        .generate(&api, &crate::render::RenderOptions::default())
        .unwrap();
    let model = files
        .iter()
        .find(|file| file.contents.contains("pub struct Large"))
        .unwrap();
    assert!(model.contents.len() > 128 * 1024);
    assert_eq!(model.contents.matches(": String,").count(), 1000);
    let diagnostics = files
        .iter()
        .find(|file| {
            file.path.to_string_lossy() == ".poolster/source-layout-model-diagnostics.json"
        })
        .unwrap();
    let entries: serde_json::Value = serde_json::from_str(&diagnostics.contents).unwrap();
    assert_eq!(entries[0]["bytes"], model.contents.len());
    assert_eq!(entries[0]["path"], model.path.to_string_lossy().as_ref());
}

fn method_model_fixture() -> poolster_core::Api {
    let mut api = poolster_core::Api {
        name: "Method schema".into(),
        version: "1.0.0".into(),
        ..Default::default()
    };
    api.schemas.push(poolster_core::Schema::new(
        "Method",
        poolster_core::SchemaValue::new(poolster_core::SchemaKind::String),
    ));
    for (name, method) in [
        ("getMethod", poolster_core::HttpMethod::Get),
        ("queryMethod", poolster_core::HttpMethod::Query),
    ] {
        api.operations.push(poolster_core::Operation {
            id: name.into(),
            method,
            path: "/method".into(),
            responses: vec![poolster_core::OperationResponse::json(
                "200",
                poolster_core::SchemaValue::reference("#/components/schemas/Method"),
            )],
            ..Default::default()
        });
    }
    api
}

#[test]
fn http_method_does_not_shadow_a_schema_named_method() {
    let tree = crate::render::generate_sdk(
        &method_model_fixture(),
        &crate::render::RenderOptions::default(),
    )
    .unwrap();
    let source = tree
        .iter()
        .map(|(_, contents)| contents)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(source.contains("pub type Method = String;"));
    assert!(source.contains("reqwest::Method::GET"));
    assert!(source.contains("reqwest::Method::from_bytes"));
    assert!(!source.contains("use reqwest::Method;"));
}

#[test]
#[ignore = "requires native Cargo dependencies; models/runtime glob collision regression"]
fn native_method_schema_and_http_methods_compile_together() {
    let tree = crate::render::generate_sdk(
        &method_model_fixture(),
        &crate::render::RenderOptions::default(),
    )
    .unwrap();
    let root = tempfile::tempdir().unwrap();
    tree.write_to(root.path()).unwrap();
    let output = crate::native_cargo()
        .args(["check", "--quiet"])
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
