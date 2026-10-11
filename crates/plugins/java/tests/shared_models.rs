use anyhow::Result;
use poolster_core::{
    blocks::*,
    engine::*,
    native::{ModelField, ModelKind, ModelType},
};
use poolster_plugin_java as output;
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
#[ignore = "requires Java17 and Maven with cached Jackson2.18.3"]
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
    let tests = sdk.join("src/test/java");
    std::fs::create_dir_all(&tests).unwrap();
    std::fs::write(tests.join("Wire.java"),r#"import io.poolster.models.model.*;import com.fasterxml.jackson.databind.*;
 public class Wire {public static void main(String[]args)throws Exception{var mapper=new ObjectMapper();for(var text:new String[]{"{\"name\":\"Ada\",\"requiredNull\":null}","{\"name\":\"Ada\",\"note\":null,\"requiredNull\":null,\"child\":{\"name\":\"Child\",\"requiredNull\":null}}"}){var original=mapper.readTree(text);var p=mapper.readValue(text,Person.class);if(!original.equals(mapper.readTree(mapper.writeValueAsString(p))))throw new AssertionError("wire");}for(var text:new String[]{"{\"requiredNull\":null}","{\"name\":null,\"requiredNull\":null}","{\"name\":\"Ada\",\"nickname\":null,\"requiredNull\":null}","{\"name\":\"Ada\"}"}){try{mapper.readValue(text,Person.class);throw new AssertionError("accepted "+text);}catch(com.fasterxml.jackson.core.JsonProcessingException expected){}}var key=mapper.readValue("\"account\"",Key.class);if(!mapper.writeValueAsString(key).equals("\"account\""))throw new AssertionError("key");var tags=mapper.readValue("[\"a\",\"b\"]",Tags.class);if(tags.value().size()!=2)throw new AssertionError("tags");try{mapper.readValue("[null]",Tags.class);throw new AssertionError("nullitem");}catch(com.fasterxml.jackson.core.JsonProcessingException expected){}var status=mapper.readValue("\"in-progress\"",Status.class);if(!mapper.writeValueAsString(status).equals("\"in-progress\""))throw new AssertionError("enum");try{mapper.readValue("\"future\"",Status.class);throw new AssertionError("future");}catch(com.fasterxml.jackson.core.JsonProcessingException expected){}}}
 "#).unwrap();
    let mvn = std::env::var("POOLSTER_MAVEN").unwrap_or("mvn".into());
    let result = std::process::Command::new(mvn)
        .args(["-o", "test-compile"])
        .current_dir(&sdk)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let jackson = std::env::var("POOLSTER_JACKSON_CP").unwrap_or_else(|_| {
        let home=std::env::var("HOME").expect("set POOLSTER_JACKSON_CP for custom Maven caches");
        ["jackson-annotations", "jackson-core", "jackson-databind"].iter().map(|artifact| format!("{home}/.m2/repository/com/fasterxml/jackson/core/{artifact}/2.18.3/{artifact}-2.18.3.jar")).collect::<Vec<_>>().join(":")
    });
    let java = std::env::var("JAVA_HOME")
        .map(|home| format!("{home}/bin/java"))
        .unwrap_or("java".into());
    let cp = format!("target/classes:target/test-classes:{jackson}");
    let result = std::process::Command::new(java)
        .args(["-cp", &cp, "Wire"])
        .current_dir(&sdk)
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
