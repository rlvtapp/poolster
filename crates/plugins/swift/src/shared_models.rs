//! Standalone shared data models; no HTTP or GraphQL execution contract.
use crate::Swift;
use anyhow::{Result, ensure};
use poolster_core::blocks::{Blocks, ModelBlock};
use poolster_core::engine::{Handle, PluginContext};
use poolster_core::models::{ModelOutput, ModelPlan, ModelPlugin, ModelSymbol};
use poolster_core::{GeneratedFile, SchemaKind, SchemaValue};
use std::{collections::BTreeMap, path::PathBuf};

pub fn shared_models(provider: Option<Handle<Blocks<ModelBlock>>>) -> ModelPlugin<Swift> {
    ModelPlugin::new(provider)
}

impl ModelOutput for Swift {
    const MODEL_CONTRACT_NAME: &'static str = "poolster.swift.shared-models.v1";
    fn emit_models(
        cx: &mut PluginContext<'_, Self>,
        plan: &ModelPlan,
    ) -> Result<BTreeMap<String, ModelSymbol>> {
        emit(cx, plan)
    }
}

fn check(value: &SchemaValue, root: bool) -> Result<()> {
    ensure!(
        value.const_value.is_none(),
        "shared literal constraints are not supported by this backend"
    );
    ensure!(
        !value.nullable || !root,
        "nullable named root models are not supported by this backend"
    );
    ensure!(
        root || value.enum_values.is_empty(),
        "inline enum models must be named before generation"
    );
    match &value.kind {
        SchemaKind::Object { fields, .. } => {
            ensure!(root, "inline object models must be named before generation");
            for field in fields {
                check(&field.value, false)?;
            }
        }
        SchemaKind::Array { items } => check(items, false)?,
        SchemaKind::OneOf { .. }
        | SchemaKind::AnyOf { .. }
        | SchemaKind::AllOf { .. }
        | SchemaKind::Not { .. } => {
            anyhow::bail!("shared model unions and intersections are unsupported by this backend")
        }
        _ => {}
    }
    Ok(())
}
fn emit(
    cx: &mut PluginContext<'_, Swift>,
    plan: &ModelPlan,
) -> Result<BTreeMap<String, ModelSymbol>> {
    let module = crate::http::type_name(
        cx.settings
            .package_name
            .as_deref()
            .unwrap_or("PoolsterModels"),
    );
    let mut symbols = BTreeMap::new();
    for schema in &plan.schemas {
        ensure!(
            schema.name.len() <= 160,
            "shared model name exceeds portable filename budget"
        );
        check(&schema.value, true)?;
        let mut source = if let SchemaKind::Object {
            fields,
            additional_properties,
        } = &schema.value.kind
        {
            crate::http::render_object_kind(&schema.name, fields, additional_properties, true)
        } else {
            crate::http::render_model(schema)
        };
        if let SchemaKind::Object { fields, .. } = &schema.value.kind {
            for field in fields.iter().filter(|f| !f.required && !f.value.nullable) {
                let key = format!("PoolsterCodingKey({:?})", field.name);
                let marker = "        self.";
                let guard = format!(
                    "        if try container.contains({key}) && container.decodeNil(forKey: {key}) {{ throw DecodingError.valueNotFound(String.self, .init(codingPath: decoder.codingPath, debugDescription: \"null nonnullable field\")) }}\n"
                );
                // One guard per selected field, inside decoder before assigning any field.
                let start = source.find("public init(from decoder: Decoder)").unwrap();
                let pos = source[start..].find(marker).unwrap() + start;
                source.insert_str(pos, &guard);
            }
        }
        if let SchemaKind::Object { fields, .. } = &schema.value.kind {
            let names = crate::native_names::field_names(
                fields,
                crate::http::identifier,
                &["encode", "init", "self"],
            );
            for field in fields
                .iter()
                .filter(|field| matches!(field.value.kind, SchemaKind::Null))
            {
                let key = format!("PoolsterCodingKey({:?})", field.name);
                let start = source.find("public init(from decoder: Decoder)").unwrap();
                let pos = source[start..].find("        self.").unwrap() + start;
                source.insert_str(pos, &format!("        if try container.contains({key}) && !container.decodeNil(forKey: {key}) {{ throw DecodingError.valueNotFound(String.self, .init(codingPath: decoder.codingPath, debugDescription: \"expected null-only field\")) }}\n"));
                let start = source
                    .find("public func encode(to encoder: Encoder)")
                    .unwrap();
                let pos = source[start..]
                    .find("        if let")
                    .map(|pos| pos + start)
                    .unwrap_or_else(|| {
                        source[start..].find("        try container").unwrap() + start
                    });
                source.insert_str(pos, &format!("        if let value = self.{} {{ guard value == .null else {{ throw EncodingError.invalidValue(value, .init(codingPath: encoder.codingPath, debugDescription: \"expected null-only field\")) }} }}\n", names[&field.name]));
            }
        }
        let path = PathBuf::from(format!("Sources/{module}/{}.swift", schema.name));
        ensure!(
            source.len() <= 128 * 1024,
            "shared model declaration exceeds 128 KiB source budget"
        );
        cx.files.emit(GeneratedFile::new(&path, source)?)?;
        symbols.insert(
            schema.name.clone(),
            ModelSymbol {
                name: schema.name.clone(),
                module: path,
            },
        );
    }
    cx.files.emit(GeneratedFile::new(
        format!("Sources/{module}/ModelRuntime.swift"),
        crate::http::json_value(),
    )?)?;
    cx.workspace.models = true;
    Ok(symbols)
}
