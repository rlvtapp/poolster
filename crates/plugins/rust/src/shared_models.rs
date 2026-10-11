//! Standalone shared data models; no HTTP or GraphQL execution contract.
use crate::Rust;
use anyhow::{Result, ensure};
use poolster_core::blocks::{Blocks, ModelBlock};
use poolster_core::engine::{Handle, PluginContext};
use poolster_core::models::{ModelOutput, ModelPlan, ModelPlugin, ModelSymbol};
use poolster_core::{GeneratedFile, SchemaKind, SchemaValue};
use std::{collections::BTreeMap, path::PathBuf};

pub fn shared_models(provider: Option<Handle<Blocks<ModelBlock>>>) -> ModelPlugin<Rust> {
    ModelPlugin::new(provider)
}

impl ModelOutput for Rust {
    const MODEL_CONTRACT_NAME: &'static str = "poolster.rust.shared-models.v1";
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
    cx: &mut PluginContext<'_, Rust>,
    plan: &ModelPlan,
) -> Result<BTreeMap<String, ModelSymbol>> {
    let mut symbols = BTreeMap::new();
    let mut index = String::from("use serde::{Deserialize, Serialize};\n");
    index.push_str(RUNTIME);
    for schema in &plan.schemas {
        ensure!(
            schema.name.len() <= 160,
            "shared model name exceeds portable filename budget"
        );
        check(&schema.value, true)?;
        let mut schema = schema.clone();
        box_references(&mut schema.value);
        let mut source = if !schema.value.enum_values.is_empty() {
            let mut source = format!(
                "#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]\npub enum {} {{\n",
                schema.name
            );
            for (i, value) in schema.value.enum_values.iter().enumerate() {
                ensure!(value.is_string(), "shared Rust enums require string values");
                source.push_str(&format!("    #[serde(rename = {value})]\n    Value{i},\n"));
            }
            source + "}\n"
        } else {
            crate::render::render_model(&schema, false, false)
        };
        if let SchemaKind::Object { fields, .. } = &schema.value.kind {
            let names =
                crate::native_names::field_names(fields, crate::render::rust_field_name, &[]);
            for field in fields {
                if !field.required && matches!(field.value.kind, SchemaKind::Null) {
                    let old = "    #[serde(default, deserialize_with = \"crate::models::poolster_deserialize_optional_nullable\")]\n";
                    let declaration = format!("    pub {}:", names[&field.name]);
                    source = source.replace(&format!("{old}{declaration}"), &format!("    #[serde(default, deserialize_with = \"crate::models::null_presence\")]\n{declaration}"));
                }
                let decoder = if field.required
                    && field.value.nullable
                    && !matches!(field.value.kind, SchemaKind::Null)
                {
                    Some("required_nullable")
                } else if !field.required && !field.value.nullable {
                    Some("optional_nonnull")
                } else {
                    None
                };
                if let Some(decoder) = decoder {
                    let declaration = format!("    pub {}:", names[&field.name]);
                    let attr = if field.required { "" } else { "default, " };
                    source = source.replace(&declaration, &format!("    #[serde({attr}deserialize_with = \"crate::models::{decoder}\")]\n{declaration}"));
                }
            }
        }
        let module = format!("model_{}", schema.name.to_ascii_lowercase());
        let path = PathBuf::from(format!("src/models/{module}.rs"));
        index.push_str(&format!("mod {module};\npub use {module}::*;\n"));
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
    cx.files
        .emit(GeneratedFile::new("src/models/mod.rs", index)?)?;
    cx.files.emit(GeneratedFile::new(
        "src/lib.rs",
        "pub mod models;\npub use models::*;\n",
    )?)?;
    let package = cx
        .settings
        .package_name
        .as_deref()
        .unwrap_or("poolster-models");
    let version = cx.common.package_version.as_deref().unwrap_or("0.0.0");
    cx.files.emit(GeneratedFile::new("Cargo.toml", format!("[package]\nname = {package:?}\nversion = {version:?}\nedition = \"2024\"\n\n[dependencies]\nserde = {{ version = \"1\", features = [\"derive\"] }}\nserde_json = \"1\"\n"))?)?;
    Ok(symbols)
}
fn box_references(value: &mut SchemaValue) {
    match &mut value.kind {
        SchemaKind::Reference { .. } => value.format = Some("poolster-boxed-reference".into()),
        SchemaKind::Object { fields, .. } => {
            for field in fields {
                box_references(&mut field.value);
            }
        }
        SchemaKind::Array { items } => box_references(items),
        _ => {}
    }
}
const RUNTIME: &str = r#"
pub(crate) fn null_presence<'de, D>(deserializer: D) -> Result<Option<()>, D::Error>
where D: serde::Deserializer<'de> { <() as Deserialize>::deserialize(deserializer).map(Some) }

pub(crate) fn poolster_deserialize_optional_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where D: serde::Deserializer<'de>, T: serde::Deserialize<'de> {
    <Option<T> as Deserialize>::deserialize(deserializer).map(Some)
}
pub(crate) fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where D: serde::Deserializer<'de>, T: serde::Deserialize<'de> {
    <Option<T> as Deserialize>::deserialize(deserializer)
}
pub(crate) fn optional_nonnull<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where D: serde::Deserializer<'de>, T: serde::Deserialize<'de> {
    T::deserialize(deserializer).map(Some)
}
"#;
