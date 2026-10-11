use super::*;
use poolster_core::{
    blocks::{BlockId, BlockMetadata, BuildingBlock, CollectionState},
    engine::{Meta, Packages, Plugin, Provision},
    native::{ModelField, ModelKind, ModelType},
};
use std::process::Command;

struct Provider {
    meta: Meta,
    name: String,
}
impl Provider {
    fn new(name: &str) -> Self {
        Self {
            meta: Meta::new(),
            name: name.into(),
        }
    }
}
impl Plugin<Php> for Provider {
    fn kind(&self) -> &'static str {
        "replacement-model-provider"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<Blocks<ModelBlock>>()]
    }
    fn generate(&self, cx: &mut PluginContext<Php>) -> Result<()> {
        let scalar = |nullable| ModelType {
            nullable,
            kind: ModelKind::Scalar("String".into()),
        };
        let fields = vec![
            ModelField {
                name: "id".into(),
                ty: scalar(false),
                optional: false,
                default_value: None,
            },
            ModelField {
                name: "nickname".into(),
                ty: scalar(true),
                optional: true,
                default_value: None,
            },
            ModelField {
                name: "friend".into(),
                ty: ModelType {
                    nullable: true,
                    kind: ModelKind::Named(self.name.clone()),
                },
                optional: true,
                default_value: None,
            },
        ];
        cx.publish(Blocks {
            parent: None,
            state: CollectionState::Complete,
            items: vec![BuildingBlock {
                metadata: BlockMetadata {
                    id: BlockId {
                        source: "custom".into(),
                        local: self.name.clone(),
                    },
                    parent: None,
                    capabilities: Default::default(),
                    references: vec![],
                    location: None,
                },
                value: ModelBlock {
                    name: self.name.clone(),
                    ty: ModelType {
                        nullable: false,
                        kind: ModelKind::Object(fields),
                    },
                },
            }],
        })
    }
}
fn generated(name: &str) -> GeneratedTree {
    let input = Provider::new(name);
    let models = shared_models(Some(input.meta.handle()));
    Packages::new()
        .package(package("models").with(models).with(input))
        .generate_native()
        .unwrap()
}
#[test]
fn blocks_only_models_accept_provider_substitution_and_regenerate() {
    let first = generated("Person");
    let second = generated("Account");
    assert!(first.iter().any(|(_, s)| s.contains("Person")));
    assert!(second.iter().any(|(_, s)| s.contains("Account")));
    assert!(!second.iter().any(|(_, s)| s.contains("Person")));
    assert!(
        !first
            .iter()
            .any(|(_, s)| s.contains("http://") || s.contains("GraphqlResponse"))
    );
    assert_eq!(
        first
            .iter()
            .map(|(p, s)| (p.to_owned(), s.to_owned()))
            .collect::<Vec<_>>(),
        generated("Person")
            .iter()
            .map(|(p, s)| (p.to_owned(), s.to_owned()))
            .collect::<Vec<_>>()
    );
}
#[test]
#[ignore = "requires local PHP toolchain"]
fn native_models_compile_and_preserve_presence_and_recursive_references() {
    let directory = tempfile::tempdir().unwrap();
    generated("Person").write_to(directory.path()).unwrap();
    let root = directory.path().join("models");
    let probe = r#"<?php
spl_autoload_register(function($class) { require __DIR__.'/src/'.str_replace('\\', '/', str_replace('Poolster\\Models\\', '', $class)).'.php'; });
$p=Poolster\Models\Models\Person::fromArray(['id'=>'1','friend'=>['id'=>'2'],'nickname'=>null]);
if ($p->friend->id !== '2' || !array_key_exists('nickname',(array)$p->jsonSerialize())) { throw new Exception('presence/recursive decode'); }
try { Poolster\Models\Models\Person::fromArray(['id'=>null]); throw new Exception('nonnull'); } catch (InvalidArgumentException $expected) {}
$q=Poolster\Models\Models\Person::fromArray(['id'=>'1']);
if (array_key_exists('nickname',(array)$q->jsonSerialize())) { throw new Exception('omission'); }
"#;
    std::fs::write(root.join("probe.php"), probe).unwrap();
    let out = Command::new(std::env::var("POOLSTER_TEST_PHP").unwrap_or("php".into()))
        .arg(root.join("probe.php"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn defaults_are_explicitly_unsupported_instead_of_dropped() {
    let mut value = SchemaValue::new(SchemaKind::String);
    value.default = Some(serde_json::json!("default"));
    assert!(
        supported(&value)
            .unwrap_err()
            .to_string()
            .contains("defaults")
    );
}

#[test]
fn unions_are_diagnosed_instead_of_erased_to_mixed() {
    let value = SchemaValue::new(SchemaKind::OneOf {
        variants: vec![
            SchemaValue::new(SchemaKind::String),
            SchemaValue::new(SchemaKind::Integer),
        ],
    });
    assert!(supported(&value).unwrap_err().to_string().contains("union"));
}

#[test]
fn intersections_are_diagnosed() {
    let value = SchemaValue::new(SchemaKind::AllOf {
        variants: vec![SchemaValue::new(SchemaKind::String)],
    });
    assert!(
        supported(&value)
            .unwrap_err()
            .to_string()
            .contains("intersection")
    );
}

#[test]
fn unsupported_union_and_literal_are_diagnosed() {
    let union = SchemaValue::new(SchemaKind::OneOf {
        variants: vec![
            SchemaValue::new(SchemaKind::String),
            SchemaValue::new(SchemaKind::Integer),
        ],
    });
    assert!(supported(&union).is_err());
    let mut literal = SchemaValue::new(SchemaKind::String);
    literal.const_value = Some(serde_json::json!("only"));
    assert!(supported(&literal).is_err());
}
