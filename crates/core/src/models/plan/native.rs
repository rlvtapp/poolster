use crate::{
    AdditionalProperties, Field, SchemaKind, SchemaValue,
    native::{ModelKind, ModelType},
};
use anyhow::{Result, bail, ensure};
pub(super) fn schema(ty: &ModelType) -> Result<SchemaValue> {
    let mut value = SchemaValue::new(match &ty.kind {
        ModelKind::Scalar(name) => match name.as_str() {
            "String" | "ID" | "string" => SchemaKind::String,
            "Int" | "integer" => SchemaKind::Integer,
            "Float" | "number" => SchemaKind::Number,
            "Boolean" | "boolean" => SchemaKind::Boolean,
            "json" => SchemaKind::Any,
            _ => bail!("shared models require a supported scalar mapping; unknown scalar {name:?}"),
        },
        ModelKind::Enum(values) => {
            ensure!(!values.is_empty(), "shared model enum cannot be empty");
            SchemaKind::String
        }
        ModelKind::Literal(_) => SchemaKind::String,
        ModelKind::Named(name) => SchemaKind::Reference {
            reference: name.clone(),
        },
        ModelKind::List(item) => SchemaKind::Array {
            items: Box::new(schema(item)?),
        },
        ModelKind::Object(fields) => {
            let mut seen = std::collections::BTreeSet::new();
            let fields = fields.iter().map(|field| {
                ensure!(seen.insert(&field.name), "duplicate shared model field {:?}", field.name);
                ensure!(field.default_value.is_none(), "shared model field {:?} has a default without a protocol-neutral literal representation", field.name);
                Ok(Field { name: field.name.clone(), value: schema(&field.ty)?, required: !field.optional, annotations: Default::default() })
            }).collect::<Result<_>>()?;
            SchemaKind::Object {
                fields,
                additional_properties: AdditionalProperties::Forbidden,
            }
        }
        ModelKind::Union(variants) => {
            ensure!(!variants.is_empty(), "shared model union cannot be empty");
            SchemaKind::AnyOf {
                variants: variants.iter().map(schema).collect::<Result<_>>()?,
            }
        }
    });
    value.nullable = ty.nullable;
    if matches!(&ty.kind, ModelKind::Scalar(name) if name == "Int") {
        value.format = Some("int32".into());
        value
            .constraints
            .insert("minimum".into(), serde_json::json!(i32::MIN));
        value
            .constraints
            .insert("maximum".into(), serde_json::json!(i32::MAX));
    }
    match &ty.kind {
        ModelKind::Enum(values) => {
            value.enum_values = values
                .iter()
                .cloned()
                .map(serde_json::Value::String)
                .collect()
        }
        ModelKind::Literal(literal) => {
            value.const_value = Some(serde_json::Value::String(literal.clone()))
        }
        _ => {}
    }
    Ok(value)
}

/// Only used for the native-model route, whose supported vocabulary is closed.
pub(super) fn model(value: &SchemaValue) -> ModelType {
    let kind = if let Some(literal) = value
        .const_value
        .as_ref()
        .and_then(serde_json::Value::as_str)
    {
        ModelKind::Literal(literal.into())
    } else if !value.enum_values.is_empty() {
        ModelKind::Enum(
            value
                .enum_values
                .iter()
                .map(|v| v.as_str().expect("native enums contain strings").into())
                .collect(),
        )
    } else {
        match &value.kind {
            SchemaKind::String => ModelKind::Scalar("string".into()),
            SchemaKind::Boolean => ModelKind::Scalar("boolean".into()),
            SchemaKind::Integer => ModelKind::Scalar(
                if value.format.as_deref() == Some("int32") {
                    "Int"
                } else {
                    "integer"
                }
                .into(),
            ),
            SchemaKind::Number => ModelKind::Scalar("number".into()),
            SchemaKind::Any => ModelKind::Scalar("json".into()),
            SchemaKind::Reference { reference } => {
                ModelKind::Named(reference.rsplit('/').next().unwrap().into())
            }
            SchemaKind::Array { items } => ModelKind::List(Box::new(model(items))),
            SchemaKind::Object { fields, .. } => ModelKind::Object(
                fields
                    .iter()
                    .map(|field| crate::native::ModelField {
                        name: field.name.clone(),
                        ty: model(&field.value),
                        optional: !field.required,
                        default_value: None,
                    })
                    .collect(),
            ),
            SchemaKind::AnyOf { variants } => {
                ModelKind::Union(variants.iter().map(model).collect())
            }
            _ => unreachable!(
                "native ModelType normalization only creates its supported structural vocabulary"
            ),
        }
    };
    ModelType {
        nullable: value.nullable,
        kind,
    }
}
