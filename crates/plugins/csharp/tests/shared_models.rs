use anyhow::Result;
use poolster_core::{
    blocks::*,
    engine::*,
    native::{ModelField, ModelKind, ModelType},
};
use poolster_plugin_csharp as output;
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
#[ignore = "requires .NET8"]
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
    std::fs::write(sdk.join("Models.csproj"),"<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><TargetFramework>net8.0</TargetFramework><OutputType>Exe</OutputType><Nullable>enable</Nullable><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>").unwrap();
    std::fs::write(sdk.join("Program.cs"),r#"using Poolster.Models;using System.Text.Json;using System.Text.Json.Nodes;
 foreach(var text in new[]{"{\"name\":\"Ada\",\"requiredNull\":null}","{\"name\":\"Ada\",\"note\":null,\"requiredNull\":null,\"child\":{\"name\":\"Child\",\"requiredNull\":null}}"}){var p=JsonSerializer.Deserialize<Person>(text)!;if(!JsonNode.DeepEquals(JsonNode.Parse(text),JsonNode.Parse(JsonSerializer.Serialize(p))))throw new Exception("wire");}
 foreach(var text in new[]{"{\"requiredNull\":null}","{\"name\":null,\"requiredNull\":null}","{\"name\":\"Ada\",\"nickname\":null,\"requiredNull\":null}","{\"name\":\"Ada\"}"}){try{JsonSerializer.Deserialize<Person>(text);throw new Exception("accepted "+text);}catch(JsonException){}}
 var key=JsonSerializer.Deserialize<Key>("\"account\"")!;if(JsonSerializer.Serialize(key)!="\"account\"")throw new Exception("key");var tags=JsonSerializer.Deserialize<Tags>("[\"a\",\"b\"]")!;if(tags.Value.Count!=2)throw new Exception("tags");try{JsonSerializer.Deserialize<Tags>("[null]");throw new Exception("nullitem");}catch(JsonException){}var status=JsonSerializer.Deserialize<Status>("\"in-progress\"")!;if(JsonSerializer.Serialize(status)!="\"in-progress\"")throw new Exception("enum");try{JsonSerializer.Deserialize<Status>("\"future\"");throw new Exception("future");}catch(JsonException){}
"#).unwrap();
    let dotnet = std::env::var("POOLSTER_DOTNET").unwrap_or("dotnet".into());
    let result = std::process::Command::new(dotnet)
        .arg("run")
        .current_dir(sdk)
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
