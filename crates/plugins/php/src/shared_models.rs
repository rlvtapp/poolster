//! Models-only output over Poolster-owned block contracts.
use crate::*;
use poolster_core::{
    blocks::{Blocks, ModelBlock},
    engine::{Handle, PluginContext},
    models::{ModelOutput, ModelPlan, ModelPlugin, ModelSymbol},
};

pub fn shared_models(provider: Option<Handle<Blocks<ModelBlock>>>) -> ModelPlugin<Php> {
    ModelPlugin::new(provider)
}

impl ModelOutput for Php {
    const MODEL_CONTRACT_NAME: &'static str = "poolster.php.shared-models.v1";
    fn emit_models(
        cx: &mut PluginContext<Self>,
        plan: &ModelPlan,
    ) -> Result<BTreeMap<String, ModelSymbol>> {
        let package = cx
            .settings
            .package_name
            .as_deref()
            .unwrap_or("poolster/models");
        let namespace = namespace_for_package(package);
        let manifest = serde_json::json!({"name": package, "type":"library", "require":{"php":">=8.2"}, "autoload":{"psr-4":{format!("{namespace}\\"):"src/"}}});
        cx.files.emit(GeneratedFile::new(
            "composer.json",
            format!("{}\n", serde_json::to_string_pretty(&manifest)?),
        )?)?;
        let named = NamedTypes::from_schemas(&plan.schemas);
        let mut symbols = BTreeMap::new();
        for schema in &plan.schemas {
            supported(&schema.value)?;
            let name = type_name(&schema.name);
            let path = format!("src/Models/{name}.php");
            cx.files.emit(GeneratedFile::new(
                &path,
                validation::checked_model(schema, &namespace, &named, plan),
            )?)?;
            symbols.insert(
                schema.name.clone(),
                ModelSymbol {
                    name: format!("{namespace}\\Models\\{name}"),
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
    anyhow::ensure!(
        value.const_value.is_none(),
        "PHP shared models do not yet support literal declarations"
    );
    anyhow::ensure!(
        value.enum_values.is_empty()
            || value.enum_values.iter().all(serde_json::Value::is_string)
            || value.enum_values.iter().all(|v| v.as_i64().is_some()),
        "PHP shared models require homogeneous string/integer backed enums"
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
            anyhow::bail!(
                "PHP shared models do not yet support union declarations; refusing a mixed fallback"
            );
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
