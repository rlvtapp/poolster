//! JSON event packages over explicit Kafka and shared-model contracts.
use crate::Rust;
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile,
    engine::{Contract, Handle, Meta, Plugin, PluginContext, Provision, Requirement},
    models::{GeneratedModels, ModelOutput, ModelPlan, ModelSymbol},
    native::events::{EventAction, EventModelSymbols, EventOperations},
};
use std::{collections::BTreeMap, path::PathBuf};
pub(crate) mod assembly;
mod render;
mod validation;

pub struct AsyncApi {
    meta: Meta,
    provider: Option<Handle<EventOperations>>,
    models: Option<Handle<GeneratedModels<Rust>>>,
}
pub fn asyncapi(provider: Option<Handle<EventOperations>>) -> AsyncApi {
    AsyncApi {
        meta: Meta::new(),
        provider,
        models: None,
    }
}
#[derive(Clone, Debug)]
pub struct KafkaOperationSymbols {
    pub function: String,
    pub module: PathBuf,
    pub payload: ModelSymbol,
    pub headers: Option<ModelSymbol>,
    pub key: Option<ModelSymbol>,
    pub action: EventAction,
}
#[derive(Clone, Debug)]
pub struct KafkaClient {
    pub operations: BTreeMap<String, KafkaOperationSymbols>,
    pub runtime_module: PathBuf,
}
impl Contract for KafkaClient {
    const NAME: &'static str = "poolster.rust.kafka-client.v1";
}
impl AsyncApi {
    pub fn using_models(mut self, provider: Handle<GeneratedModels<Rust>>) -> Self {
        self.models = Some(provider);
        self
    }
    pub fn handle(&self) -> Handle<KafkaClient> {
        self.meta.handle()
    }
}
impl Plugin<Rust> for AsyncApi {
    fn kind(&self) -> &'static str {
        "rust-asyncapi-kafka"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        let mut requires = vec![Requirement::on(self.provider)];
        if let Some(models) = self.models {
            requires.push(Requirement::on(Some(models)));
        }
        requires
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<KafkaClient>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Rust>) -> Result<()> {
        let events = cx.inputs.get::<EventOperations>()?.clone();
        validation::check(&events)?;
        let symbols = if self.models.is_some() {
            let models = cx.inputs.get::<GeneratedModels<Rust>>()?;
            if let Some(reference) = cx.inputs.reference::<EventOperations>()? {
                ensure!(
                    models.provenance.as_ref() == Some(reference),
                    "shared Kafka models belong to another selected event provider/revision"
                );
            }
            events
                .resolve_models_with_reference(models, cx.inputs.reference::<EventOperations>()?)?
        } else {
            let source = cx
                .inputs
                .reference::<EventOperations>()?
                .map(|r| r.instance.as_str())
                .unwrap_or("rust.events")
                .to_owned();
            let blocks = events.shared_model_blocks(source);
            let plan = ModelPlan::from_shapes(&blocks)?;
            let models = Rust::emit_models(cx, &plan)?;
            let symbol = |location: &str, role: &str| -> Result<ModelSymbol> {
                let suffix = format!("{location}/{role}");
                let name = plan
                    .entities
                    .iter()
                    .find(|(_, id)| id.local == suffix)
                    .map(|(name, _)| name)
                    .ok_or_else(|| anyhow::anyhow!("missing event model role {suffix}"))?;
                Ok(models[name].clone())
            };
            events
                .operations
                .iter()
                .map(|op| {
                    Ok((
                        op.message.location.clone(),
                        EventModelSymbols {
                            payload: symbol(&op.message.location, "payload")?,
                            headers: op
                                .message
                                .headers
                                .as_ref()
                                .map(|_| symbol(&op.message.location, "headers"))
                                .transpose()?,
                            key: op
                                .message
                                .key
                                .as_ref()
                                .map(|_| symbol(&op.message.location, "key"))
                                .transpose()?,
                        },
                    ))
                })
                .collect::<Result<BTreeMap<_, _>>>()?
        };
        let operations = render::emit(cx, &events, &symbols)?;
        ensure!(
            !cx.workspace.event_package,
            "only one Rust event transport may assemble a package"
        );
        cx.workspace.event_package = true;
        cx.publish(KafkaClient {
            operations,
            runtime_module: "src/events/runtime".into(),
        })
    }
}
#[cfg(test)]
mod tests;
