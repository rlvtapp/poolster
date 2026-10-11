use super::*;
use crate::{AdditionalProperties, Field, SchemaKind, SchemaValue, blocks::*, native::*};
use std::collections::BTreeMap;

#[test]
fn portable_suffixes_protect_natural_names_and_runtime_identifiers() {
    let blocks = shapes(vec![
        ("first", "person", SchemaValue::new(SchemaKind::String)),
        ("second", "Person", SchemaValue::new(SchemaKind::String)),
        (
            "third",
            "PersonModel2",
            SchemaValue::new(SchemaKind::String),
        ),
        ("fourth", "String", SchemaValue::new(SchemaKind::String)),
    ]);
    let plan = ModelPlan::from_shapes(&blocks).unwrap();
    let names = plan
        .names
        .values()
        .map(|name| name.to_lowercase())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(names.len(), 4);
    assert_eq!(plan.names["PersonModel2"], "PersonModel2");
    assert_ne!(plan.names["String"], "String");
    assert!(
        plan.names
            .values()
            .all(|name| name.chars().all(|ch| ch.is_ascii_alphanumeric()))
    );
}

fn metadata(local: &str) -> BlockMetadata {
    BlockMetadata {
        id: BlockId {
            source: "fixture".into(),
            local: local.into(),
        },
        parent: None,
        capabilities: Default::default(),
        references: vec![],
        location: None,
    }
}
fn shapes(entries: Vec<(&str, &str, SchemaValue)>) -> Blocks<SchemaModelBlock> {
    Blocks {
        parent: None,
        state: CollectionState::Complete,
        items: entries
            .into_iter()
            .map(|(id, name, value)| BuildingBlock {
                metadata: metadata(id),
                value: SchemaModelBlock {
                    name: name.into(),
                    value,
                },
            })
            .collect(),
    }
}
fn native(ty: ModelType) -> Blocks<ModelBlock> {
    Blocks {
        parent: None,
        state: CollectionState::Complete,
        items: vec![BuildingBlock {
            metadata: metadata("model"),
            value: ModelBlock {
                name: "Document".into(),
                ty,
            },
        }],
    }
}
fn string() -> ModelType {
    ModelType {
        nullable: false,
        kind: ModelKind::Scalar("String".into()),
    }
}

#[test]
fn collision_resolution_and_inline_identity_do_not_depend_on_input_order_or_revision() {
    let object = SchemaValue::new(SchemaKind::Object {
        fields: vec![Field {
            name: "item".into(),
            required: false,
            value: SchemaValue::new(SchemaKind::Object {
                fields: vec![],
                additional_properties: AdditionalProperties::Forbidden,
            }),
            annotations: BTreeMap::new(),
        }],
        additional_properties: AdditionalProperties::Forbidden,
    });
    let blocks = shapes(vec![
        ("first", "user", object),
        ("second", "User", SchemaValue::new(SchemaKind::String)),
    ]);
    let first = ModelPlan::from_shapes(&blocks).unwrap();
    let mut reordered = blocks.clone();
    reordered.items.reverse();
    let second = ModelPlan::from_shapes(&reordered).unwrap();
    assert_eq!(first.schemas, second.schemas);
    assert_eq!(first.entities, second.entities);
    assert_ne!(
        first.names["user"].to_lowercase(),
        first.names["User"].to_lowercase()
    );
    assert_eq!(first.schemas.len(), 3);
    let parent = ContractReference::from_bytes("custom.v1", "fixture", b"updated");
    let updated = ModelPlan::from_shapes(&reordered.with_parent(parent.clone())).unwrap();
    assert_eq!(first.entities, updated.entities);
    assert_eq!(updated.provenance, Some(parent));
}

#[test]
fn hoisting_preserves_presence_nullability_constraints_and_typed_dictionaries() {
    let mut child = SchemaValue::new(SchemaKind::Object {
        fields: vec![],
        additional_properties: AdditionalProperties::Schema {
            value: Box::new(SchemaValue::new(SchemaKind::Integer)),
        },
    });
    child.nullable = true;
    child
        .constraints
        .insert("minProperties".into(), serde_json::json!(2));
    child
        .extensions
        .insert("x-source".into(), serde_json::json!("native"));
    let root = SchemaValue::new(SchemaKind::Object {
        fields: vec![Field {
            name: "child".into(),
            value: child,
            required: false,
            annotations: BTreeMap::new(),
        }],
        additional_properties: AdditionalProperties::Forbidden,
    });
    let plan = ModelPlan::from_shapes(&shapes(vec![("root", "Root", root)])).unwrap();
    let root = plan
        .schemas
        .iter()
        .find(|s| s.name == plan.names["Root"])
        .unwrap();
    let SchemaKind::Object { fields, .. } = &root.value.kind else {
        panic!()
    };
    assert!(!fields[0].required);
    assert!(fields[0].value.nullable);
    let target = fields[0].value.kind.reference_name().unwrap();
    let nested = plan.schemas.iter().find(|s| s.name == target).unwrap();
    assert!(!nested.value.nullable);
    assert_eq!(
        nested.value.constraints["minProperties"],
        serde_json::json!(2)
    );
    assert_eq!(
        nested.value.extensions["x-source"],
        serde_json::json!("native")
    );
    assert!(matches!(
        nested.value.kind,
        SchemaKind::Object {
            additional_properties: AdditionalProperties::Schema { .. },
            ..
        }
    ));
}

#[test]
fn closed_recursive_references_are_valid_but_missing_dependencies_are_not() {
    let recursive = SchemaValue::new(SchemaKind::Array {
        items: Box::new(SchemaValue::reference("#/components/schemas/node")),
    });
    let plan = ModelPlan::from_shapes(&shapes(vec![("node", "node", recursive)])).unwrap();
    let SchemaKind::Array { items } = &plan.schemas[0].value.kind else {
        panic!()
    };
    assert_eq!(
        items.kind.reference_name(),
        Some(plan.names["node"].as_str())
    );
    let invalid = shapes(vec![("node", "node", SchemaValue::reference("missing"))]);
    assert!(
        ModelPlan::from_shapes(&invalid)
            .err()
            .unwrap()
            .to_string()
            .contains("dependency closure")
    );
}

#[test]
fn native_models_reject_undefined_defaults_unknown_scalars_and_partial_collections() {
    let mut ty = string();
    ty.kind = ModelKind::Scalar("CustomTimestamp".into());
    assert!(
        ModelPlan::new(&native(ty))
            .err()
            .unwrap()
            .to_string()
            .contains("scalar mapping")
    );
    let ty = ModelType {
        nullable: false,
        kind: ModelKind::Object(vec![ModelField {
            name: "name".into(),
            ty: string(),
            optional: true,
            default_value: Some("hello".into()),
        }]),
    };
    assert!(
        ModelPlan::new(&native(ty))
            .err()
            .unwrap()
            .to_string()
            .contains("literal representation")
    );
    let mut blocks = native(string());
    blocks.state = CollectionState::Partial {
        diagnostics: vec!["unresolved reference".into()],
    };
    assert!(
        ModelPlan::new(&blocks)
            .err()
            .unwrap()
            .to_string()
            .contains("requires complete")
    );
}

#[test]
fn empty_complete_models_and_enum_literal_wrappers_are_preserved() {
    assert!(
        ModelPlan::from_shapes(&shapes(vec![]))
            .unwrap()
            .schemas
            .is_empty()
    );
    let mut enum_type = string();
    enum_type.kind = ModelKind::Enum(vec!["A".into(), "B".into()]);
    enum_type.nullable = true;
    let plan = ModelPlan::new(&native(enum_type)).unwrap();
    assert!(plan.schemas[0].value.nullable);
    assert_eq!(
        plan.schemas[0].value.enum_values,
        vec![serde_json::json!("A"), serde_json::json!("B")]
    );
    let mut literal = string();
    literal.kind = ModelKind::Literal("fixed".into());
    assert_eq!(
        ModelPlan::new(&native(literal)).unwrap().schemas[0]
            .value
            .const_value,
        Some(serde_json::json!("fixed"))
    );
}
