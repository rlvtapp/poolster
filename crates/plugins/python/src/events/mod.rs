//! Kafka JSON events over authoritative native event and shared-model contracts.
mod render;
#[cfg(test)]
mod tests;
mod validation;
use crate::{Python, python_module_name, render_pyproject};
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile,
    blocks::ContractReference,
    engine::{
        Contract, FinalizeContext, Handle, Meta, Plugin, PluginContext, Provision, Requirement,
    },
    models::{GeneratedModels, ModelOutput, ModelPlan, ModelSymbol},
    native::events::EventOperations,
};
use std::collections::BTreeMap;
#[derive(Default)]
pub struct Workspace {
    pub(crate) shared_models: bool,
    events: bool,
}
pub(crate) fn finalize(cx: &mut FinalizeContext<Python>) -> Result<()> {
    if !cx.workspace.shared_models {
        return Ok(());
    }
    let distribution = cx
        .settings
        .package_name
        .as_deref()
        .unwrap_or("poolster-models");
    let module = python_module_name(distribution);
    let mut manifest = render_pyproject(
        distribution,
        cx.common.package_version.as_deref().unwrap_or("0.1.0"),
    );
    if cx.workspace.events {
        manifest = manifest.replace("requires-python = \">=3.10\"", "requires-python = \">=3.10\"\ndependencies = [\"kafka-python==3.0.11\", \"jsonschema==4.26.0\"]");
    }
    cx.files
        .emit(GeneratedFile::new("pyproject.toml", manifest)?)?;
    let init = if cx.workspace.events {
        "from .models import *\nfrom .events import KafkaClient\n"
    } else {
        "from .models import *\n"
    };
    cx.files.emit(GeneratedFile::new(
        format!("src/{module}/__init__.py"),
        init,
    )?)?;
    Ok(())
}
pub struct AsyncApi {
    meta: Meta,
    provider: Option<Handle<EventOperations>>,
    models: Option<Handle<GeneratedModels<Python>>>,
}
pub fn asyncapi(provider: Option<Handle<EventOperations>>) -> AsyncApi {
    AsyncApi {
        meta: Meta::new(),
        provider,
        models: None,
    }
}
impl AsyncApi {
    pub fn using_models(mut self, models: Handle<GeneratedModels<Python>>) -> Self {
        self.models = Some(models);
        self
    }
    pub fn models(self, models: Handle<GeneratedModels<Python>>) -> Self {
        self.using_models(models)
    }
    pub fn handle(&self) -> Handle<KafkaClient> {
        self.meta.handle()
    }
}
#[derive(Clone, Debug)]
pub struct KafkaClient {
    pub operations: BTreeMap<String, ModelSymbol>,
    pub provenance: ContractReference,
}
impl Contract for KafkaClient {
    const NAME: &'static str = "poolster.python.kafka-client.v1";
}
impl Plugin<Python> for AsyncApi {
    fn kind(&self) -> &'static str {
        "python-asyncapi-kafka"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        let mut r = vec![Requirement::on(self.provider)];
        if let Some(m) = self.models {
            r.push(Requirement::on(Some(m)));
        }
        r
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<KafkaClient>()]
    }
    fn generate(&self, cx: &mut PluginContext<Python>) -> Result<()> {
        let input = cx.inputs.get::<EventOperations>()?.clone();
        validation::check(&input)?;
        let reference = cx.inputs.reference::<EventOperations>()?.cloned();
        let source = reference
            .as_ref()
            .map(|r| r.instance.clone())
            .or_else(|| {
                self.models
                    .and_then(|_| cx.inputs.get::<GeneratedModels<Python>>().ok())
                    .and_then(|m| m.provenance.as_ref().map(|p| p.instance.clone()))
            })
            .unwrap_or("events".into());
        let mut blocks = input.shared_model_blocks(source);
        if let Some(r) = reference {
            blocks = blocks.with_parent(r);
        }
        let plan = ModelPlan::from_shapes(&blocks)?;
        let provenance = plan.provenance.clone().expect("event provenance");
        let symbols = if self.models.is_some() {
            let models = cx.inputs.get::<GeneratedModels<Python>>()?;
            input
                .resolve_models_with_reference(models, cx.inputs.reference::<EventOperations>()?)?;
            ensure!(
                models.provenance.as_ref() == Some(&provenance),
                "Python event shared models belong to a different contract instance or revision"
            );
            plan.entities
                .values()
                .map(|id| {
                    Ok((
                        id.local.clone(),
                        models
                            .entities
                            .get(id)
                            .ok_or_else(|| {
                                anyhow::anyhow!("missing Python event model {}", id.local)
                            })?
                            .clone(),
                    ))
                })
                .collect::<Result<BTreeMap<_, _>>>()?
        } else {
            let symbols = Python::emit_models(cx, &plan)?;
            plan.entities
                .iter()
                .map(|(name, id)| (id.local.clone(), symbols[name].clone()))
                .collect()
        };
        cx.workspace.events = true;
        let operations = render::emit(cx, &input, &symbols)?;
        cx.files.emit(GeneratedFile::new(
            "asyncapi.json",
            serde_json::to_string_pretty(&input.source)? + "\n",
        )?)?;
        cx.files.emit(GeneratedFile::new(".poolster/event-contract.json",serde_json::to_string_pretty(&serde_json::json!({"version":1,"parent":provenance,"transport":"kafka-json","client":"kafka-python@3.0.11","operations":input.operations}))?+"\n")?)?;
        cx.publish(KafkaClient {
            operations,
            provenance,
        })
    }
}
