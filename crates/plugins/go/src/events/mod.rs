//! Kafka JSON event packages, independent of the HTTP execution model.
mod render;
mod runtime;
mod validation;

use crate::Go;
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile,
    blocks::ContractReference,
    engine::{Contract, Handle, Meta, Plugin, PluginContext, Provision, Requirement},
    models::{GeneratedModels, ModelOutput, ModelPlan, ModelSymbol},
    native::events::{EventAction, EventOperations},
};
use std::{collections::BTreeMap, path::PathBuf};

pub struct AsyncApi {
    meta: Meta,
    provider: Option<Handle<EventOperations>>,
    models: Option<Handle<GeneratedModels<Go>>>,
}
/// Generate a Kafka JSON client from the selected event contract.
pub fn asyncapi(provider: Option<Handle<EventOperations>>) -> AsyncApi {
    AsyncApi {
        meta: Meta::new(),
        provider,
        models: None,
    }
}
impl AsyncApi {
    /// Reuse models derived from this exact event contract revision.
    pub fn models(mut self, models: Handle<GeneratedModels<Go>>) -> Self {
        self.models = Some(models);
        self
    }
    /// Explicit shared-model selection, matching other native generators.
    pub fn using_models(self, models: Handle<GeneratedModels<Go>>) -> Self {
        self.models(models)
    }
    pub fn handle(&self) -> Handle<KafkaClient> {
        self.meta.handle()
    }
}
#[derive(Clone, Debug)]
pub struct KafkaOperationSymbols {
    pub method: String,
    pub module: PathBuf,
    pub payload: ModelSymbol,
    pub headers: Option<ModelSymbol>,
    pub key: Option<ModelSymbol>,
    pub action: EventAction,
}
#[derive(Clone, Debug)]
pub struct KafkaClient {
    pub operations: BTreeMap<String, KafkaOperationSymbols>,
    pub provenance: ContractReference,
}
impl Contract for KafkaClient {
    const NAME: &'static str = "poolster.go.kafka-client.v1";
}
impl Plugin<Go> for AsyncApi {
    fn kind(&self) -> &'static str {
        "go-asyncapi-kafka"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        let mut requirements = vec![Requirement::on(self.provider)];
        if let Some(models) = self.models {
            requirements.push(Requirement::on(Some(models)));
        }
        requirements
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<KafkaClient>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Go>) -> Result<()> {
        let input = cx.inputs.get::<EventOperations>()?.clone();
        validation::check(&input)?;
        let reference = cx.inputs.reference::<EventOperations>()?.cloned();
        let source = match &reference {
            Some(reference) => reference.instance.clone(),
            None if self.models.is_some() => cx
                .inputs
                .get::<GeneratedModels<Go>>()?
                .provenance
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Go event shared models require provenance"))?
                .instance
                .clone(),
            None => "events".to_owned(),
        };
        let mut blocks = input.shared_model_blocks(source);
        if let Some(reference) = reference {
            blocks = blocks.with_parent(reference);
        }
        let plan = ModelPlan::from_shapes(&blocks)?;
        let provenance = plan
            .provenance
            .clone()
            .expect("event model blocks have provenance");
        let role_symbols = if self.models.is_some() {
            let models = cx.inputs.get::<GeneratedModels<Go>>()?;
            input
                .resolve_models_with_reference(models, cx.inputs.reference::<EventOperations>()?)?;
            ensure!(
                models.provenance.as_ref() == Some(&provenance),
                "Go event shared models belong to a different contract instance or revision"
            );
            let mut symbols = BTreeMap::new();
            for id in plan.entities.values() {
                let symbol = models.entities.get(id).ok_or_else(|| {
                    anyhow::anyhow!("Go event shared models omit entity {}", id.local)
                })?;

                ensure!(
                    symbol
                        .module
                        .parent()
                        .is_none_or(|p| p.as_os_str().is_empty()),
                    "Go event shared models must be in the same package"
                );
                symbols.insert(id.local.clone(), symbol.clone());
            }
            symbols
        } else {
            let symbols = Go::emit_models(cx, &plan)?;
            plan.entities
                .iter()
                .map(|(name, id)| (id.local.clone(), symbols[name].clone()))
                .collect()
        };
        let module = cx
            .settings
            .package_name
            .as_deref()
            .unwrap_or("example.com/poolster-models")
            .to_owned();
        cx.workspace.native_module = Some(module);
        cx.workspace
            .native_dependencies
            .insert("github.com/twmb/franz-go".into(), "v1.18.1".into());
        cx.workspace.native_dependencies.insert(
            "github.com/santhosh-tekuri/jsonschema/v5".into(),
            "v5.3.1".into(),
        );
        let operations = render::emit(cx, &input, &role_symbols)?;
        cx.files
            .emit(GeneratedFile::new("kafka_runtime.go", runtime::SOURCE)?)?;
        cx.files.emit(GeneratedFile::new(
            "asyncapi.json",
            serde_json::to_string_pretty(&input.source)? + "\n",
        )?)?;
        cx.files.emit(GeneratedFile::new(".poolster/event-contract.json", serde_json::to_string_pretty(&serde_json::json!({"version":1,"parent":provenance,"operations":input.operations,"transport":"kafka-json","client":"franz-go@v1.18.1","headerEncoding":"utf8-string","keyEncoding":"utf8-string"}))? + "\n")?)?;
        cx.publish(KafkaClient {
            operations,
            provenance,
        })
    }
}
