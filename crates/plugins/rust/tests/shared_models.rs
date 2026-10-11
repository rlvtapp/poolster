use anyhow::Result;
use poolster_core::{
    blocks::*,
    engine::*,
    native::{ModelField, ModelKind, ModelType},
};
use poolster_plugin_rust as output;
use std::collections::BTreeSet;
struct Source {
    meta: Meta,
    name: &'static str,
}
fn scalar(nullable: bool) -> ModelType {
    ModelType {
        nullable,
        kind: ModelKind::Scalar("String".into()),
    }
}
fn field(name: &str, optional: bool, nullable: bool) -> ModelField {
    ModelField {
        name: name.into(),
        optional,
        default_value: None,
        ty: scalar(nullable),
    }
}
impl<L: Language> Plugin<L> for Source {
    fn kind(&self) -> &'static str {
        "shared-model-fixture"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn provides(&self) -> Vec<Provision> {
        vec![
            Provision::of::<Blocks<ModelBlock>>(),
            Provision::of::<Blocks<poolster_core::models::SchemaModelBlock>>(),
        ]
    }
    fn generate(&self, cx: &mut PluginContext<'_, L>) -> Result<()> {
        let mut fields = vec![
            field("name", false, false),
            field("note", true, true),
            field("nickname", true, false),
            field("requiredNull", false, true),
        ];
        fields.push(ModelField {
            name: "tags".into(),
            optional: true,
            default_value: None,
            ty: ModelType {
                nullable: false,
                kind: ModelKind::List(Box::new(scalar(false))),
            },
        });
        fields.push(ModelField {
            name: "child".into(),
            optional: true,
            default_value: None,
            ty: ModelType {
                nullable: true,
                kind: ModelKind::Named(self.name.into()),
            },
        });
        let mut model = ModelBlock {
            name: self.name.into(),
            ty: ModelType {
                nullable: false,
                kind: ModelKind::Object(fields),
            },
        };
        if self.name == "Literal" {
            model.ty.kind = ModelKind::Literal("only".into());
        }
        let extra = vec![
            model,
            ModelBlock {
                name: "Key".into(),
                ty: scalar(false),
            },
            ModelBlock {
                name: "Tags".into(),
                ty: ModelType {
                    nullable: false,
                    kind: ModelKind::List(Box::new(scalar(false))),
                },
            },
            ModelBlock {
                name: "Status".into(),
                ty: ModelType {
                    nullable: false,
                    kind: ModelKind::Enum(vec!["in-progress".into(), "done".into()]),
                },
            },
        ];
        let blocks = Blocks {
            parent: None,
            state: CollectionState::Complete,
            items: extra
                .into_iter()
                .map(|value| BuildingBlock {
                    metadata: BlockMetadata {
                        id: BlockId {
                            source: self.name.into(),
                            local: value.name.clone(),
                        },
                        parent: None,
                        capabilities: BTreeSet::from(["model".into()]),
                        references: vec![],
                        location: None,
                    },
                    value,
                })
                .collect(),
        };
        let plan = poolster_core::models::ModelPlan::new(&blocks)?;
        let shapes = plan
            .schemas
            .into_iter()
            .map(|mut schema| {
                if let poolster_core::SchemaKind::Object { fields, .. } = &mut schema.value.kind {
                    for field in fields {
                        if field.name == "note" {
                            field.value.kind = poolster_core::SchemaKind::Null;
                            field.value.nullable = true;
                        }
                    }
                }
                BuildingBlock {
                    metadata: BlockMetadata {
                        id: BlockId {
                            source: self.name.into(),
                            local: schema.name.clone(),
                        },
                        parent: None,
                        capabilities: BTreeSet::from(["model".into()]),
                        references: vec![],
                        location: None,
                    },
                    value: poolster_core::models::SchemaModelBlock {
                        name: schema.name,
                        value: schema.value,
                    },
                }
            })
            .collect();
        cx.publish(Blocks {
            parent: None,
            state: CollectionState::Complete,
            items: shapes,
        })?;
        cx.publish(blocks)
    }
}
fn generate() -> poolster_core::GeneratedTree {
    let selected = Source {
        meta: Meta::new(),
        name: "Person",
    };
    let ignored = Source {
        meta: Meta::new(),
        name: "Ignored",
    };
    let models = output::shared_models(Some(selected.meta.handle()));
    Packages::new()
        .package(
            output::package("sdk")
                .with(ignored)
                .with(models)
                .with(selected),
        )
        .generate_native()
        .unwrap()
}
fn generate_rich() -> poolster_core::GeneratedTree {
    let selected = Source {
        meta: Meta::new(),
        name: "Person",
    };
    let models = output::shared_models(None).input_shapes(selected.meta.handle());
    Packages::new()
        .package(output::package("sdk").with(models).with(selected))
        .generate_native()
        .unwrap()
}
#[test]
fn explicit_provider_selection_emits_only_selected_models_and_no_client() {
    let tree = generate();
    let source = tree.iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n");
    assert!(source.contains("Person"));
    assert!(!source.contains("Ignored"));
    assert!(!source.contains("Graphql"));
    assert!(!source.contains("HttpClient"));
    for (path, source) in tree.iter() {
        if matches!(
            path.extension().and_then(|v| v.to_str()),
            Some("rs" | "go" | "java" | "cs" | "swift")
        ) {
            assert!(source.len() <= 128 * 1024);
        }
    }
}
#[test]
#[ignore = "requires Cargo and cached serde dependencies"]
fn generated_models_compile_and_preserve_wire_presence() {
    native(false);
}
#[test]
#[ignore = "requires target native compiler and cached dependencies"]
fn rich_null_only_models_compile_and_preserve_wire_presence() {
    native(true);
}
fn native(rich: bool) {
    let dir = tempfile::tempdir().unwrap();
    let tree = if rich { generate_rich() } else { generate() };
    tree.write_to(dir.path()).unwrap();
    let sdk = dir.path().join("sdk");
    std::fs::create_dir_all(sdk.join("tests")).unwrap();
    std::fs::write(sdk.join("tests/wire.rs"),r#"use poolster_models::Person;
 #[test] fn wire(){for text in [r#PLACEHOLDER{"name":"Ada","requiredNull":null}ENDPLACEHOLDER,r#PLACEHOLDER{"name":"Ada","note":null,"requiredNull":null,"child":{"name":"Child","requiredNull":null}}ENDPLACEHOLDER]{let original:serde_json::Value=serde_json::from_str(text).unwrap();let p:Person=serde_json::from_value(original.clone()).unwrap();assert_eq!(serde_json::to_value(p).unwrap(),original);}for text in [r#PLACEHOLDER{"requiredNull":null}ENDPLACEHOLDER,r#PLACEHOLDER{"name":null,"requiredNull":null}ENDPLACEHOLDER,r#PLACEHOLDER{"name":"Ada","nickname":null,"requiredNull":null}ENDPLACEHOLDER,r#PLACEHOLDER{"name":"Ada"}ENDPLACEHOLDER]{assert!(serde_json::from_str::<Person>(text).is_err());}}
 #[test] fn aliases(){let key:poolster_models::Key=serde_json::from_str("\"account\"").unwrap();assert_eq!(serde_json::to_string(&key).unwrap(),"\"account\"");let tags:poolster_models::Tags=serde_json::from_str("[\"a\",\"b\"]").unwrap();assert_eq!(tags.len(),2);assert!(serde_json::from_str::<poolster_models::Tags>("[null]").is_err());let status:poolster_models::Status=serde_json::from_str("\"in-progress\"").unwrap();assert_eq!(serde_json::to_string(&status).unwrap(),"\"in-progress\"");assert!(serde_json::from_str::<poolster_models::Status>("\"future\"").is_err());}
"#.replace("r#PLACEHOLDER","r#\"").replace("ENDPLACEHOLDER","\"#")).unwrap();
    let result = std::process::Command::new("cargo")
        .args(["test", "--offline"])
        .current_dir(sdk)
        .env(
            "CARGO_TARGET_DIR",
            std::env::temp_dir().join("poolster-shared-models-rust-target"),
        )
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn unsupported_literal_model_is_a_clear_error() {
    let selected = Source {
        meta: Meta::new(),
        name: "Literal",
    };
    let models = output::shared_models(Some(selected.meta.handle()));
    let error = Packages::new()
        .package(output::package("sdk").with(models).with(selected))
        .generate_native()
        .expect_err("unsupported literal must fail");
    assert!(format!("{error:#}").contains("literal"), "{error:#}");
}
