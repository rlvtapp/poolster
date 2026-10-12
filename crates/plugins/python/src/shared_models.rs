//! Models-only output over Poolster-owned block contracts.
use crate::*;
use poolster_core::{
    blocks::{Blocks, ModelBlock},
    engine::{Handle, PluginContext},
    models::{ModelOutput, ModelPlan, ModelPlugin, ModelSymbol},
};

pub fn shared_models(provider: Option<Handle<Blocks<ModelBlock>>>) -> ModelPlugin<Python> {
    ModelPlugin::new(provider)
}

impl ModelOutput for Python {
    const MODEL_CONTRACT_NAME: &'static str = "poolster.python.shared-models.v1";
    fn emit_models(
        cx: &mut PluginContext<Self>,
        plan: &ModelPlan,
    ) -> Result<BTreeMap<String, ModelSymbol>> {
        let distribution = cx
            .settings
            .package_name
            .as_deref()
            .unwrap_or("poolster-models");
        let module = python_module_name(distribution);
        let root = format!("src/{module}");
        cx.workspace.shared_models = true;
        cx.files
            .emit(GeneratedFile::new(format!("{root}/py.typed"), "")?)?;
        cx.files.emit(GeneratedFile::new(
            format!("{root}/models/__init__.py"),
            render_models_init_for_count(plan.schemas.len()),
        )?)?;
        cx.files.emit(GeneratedFile::new(
            format!("{root}/models/_model_codec.py"),
            include_str!("../templates/model_codec.py"),
        )?)?;
        cx.files.emit(GeneratedFile::new(
            format!("{root}/models/chunks/__init__.py"),
            "",
        )?)?;
        for (index, chunk) in plan.schemas.chunks(100).enumerate() {
            cx.files.emit(GeneratedFile::new(
                format!("{root}/models/chunks/exports_{index:03}.py"),
                render_model_exports(chunk),
            )?)?;
        }
        let mut symbols = BTreeMap::new();
        for schema in &plan.schemas {
            supported(&schema.value)?;
            let path = format!("{root}/models/{}.py", schema_file_name(&schema.name));
            cx.files.emit(GeneratedFile::new(
                &path,
                validation::checked_model(schema, plan),
            )?)?;
            symbols.insert(
                schema.name.clone(),
                ModelSymbol {
                    name: python_type_name(&schema.name),
                    module: path.into(),
                },
            );
        }
        Ok(symbols)
    }
}

fn supported(value: &SchemaValue) -> Result<()> {
    anyhow::ensure!(
        value.default.is_none(),
        "shared models do not support schema defaults in this language yet"
    );
    let literal = |v: &serde_json::Value| {
        v.is_string()
            || v.is_boolean()
            || v.is_null()
            || v.as_i64().is_some()
            || v.as_u64().is_some()
    };
    anyhow::ensure!(
        value.enum_values.iter().all(literal) && value.const_value.as_ref().is_none_or(literal),
        "Python shared models require scalar Literal-compatible enum/constant values"
    );
    match &value.kind {
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            for field in fields {
                supported(&field.value)?;
            }
            if let AdditionalProperties::Schema { value } = additional_properties {
                supported(value)?;
            }
        }
        SchemaKind::Array { items } => supported(items)?,
        SchemaKind::OneOf { .. } | SchemaKind::AnyOf { .. } => {
            anyhow::bail!("Python shared models do not yet support union decoding")
        }
        SchemaKind::AllOf { .. } | SchemaKind::Not { .. } => {
            anyhow::bail!("shared models do not support intersection/negation declarations")
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests;

mod validation;
