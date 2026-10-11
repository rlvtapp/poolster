use super::{ModelPlan, SchemaModelBlock};
use crate::{
    GeneratedFile,
    blocks::{Blocks, ModelBlock},
    engine::{Contract, Handle, Language, Meta, Plugin, PluginContext, Provision, Requirement},
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, marker::PhantomData, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelSymbol {
    pub name: String,
    pub module: PathBuf,
}

/// Language outputs own their declarations, serializers and package assembly.
pub trait ModelOutput: Language {
    const MODEL_CONTRACT_NAME: &'static str;
    fn emit_models(
        cx: &mut PluginContext<'_, Self>,
        plan: &ModelPlan,
    ) -> Result<BTreeMap<String, ModelSymbol>>;
}

/// A normal typed output contract available to downstream plugins.
pub struct GeneratedModels<L: ModelOutput> {
    pub symbols: BTreeMap<String, ModelSymbol>,
    pub original_names: BTreeMap<String, String>,
    pub entities: BTreeMap<crate::blocks::BlockId, ModelSymbol>,
    pub provenance: Option<crate::blocks::ContractReference>,
    language: PhantomData<fn() -> L>,
}
impl<L: ModelOutput> Contract for GeneratedModels<L> {
    const NAME: &'static str = L::MODEL_CONTRACT_NAME;
}

enum Input {
    Native(Option<Handle<Blocks<ModelBlock>>>),
    Schemas(Handle<Blocks<SchemaModelBlock>>),
    ExistingSchemas(Handle<Blocks<crate::Schema>>),
}
pub struct ModelPlugin<L: ModelOutput> {
    meta: Meta,
    input: Input,
    language: PhantomData<fn() -> L>,
}
impl<L: ModelOutput> ModelPlugin<L> {
    pub fn new(provider: Option<Handle<Blocks<ModelBlock>>>) -> Self {
        Self {
            meta: Meta::new(),
            input: Input::Native(provider),
            language: PhantomData,
        }
    }
    /// Select a rich model collection explicitly, preserving JSON Schema details.
    pub fn input_shapes(mut self, provider: Handle<Blocks<SchemaModelBlock>>) -> Self {
        self.input = Input::Schemas(provider);
        self
    }
    /// Reuse existing rich schema blocks without constructing an HTTP API.
    pub fn input_schemas(mut self, provider: Handle<Blocks<crate::Schema>>) -> Self {
        self.input = Input::ExistingSchemas(provider);
        self
    }
    pub fn handle(&self) -> Handle<GeneratedModels<L>> {
        self.meta.handle()
    }
}
impl<L: ModelOutput> Plugin<L> for ModelPlugin<L> {
    fn kind(&self) -> &'static str {
        L::MODEL_CONTRACT_NAME
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![match self.input {
            Input::Native(provider) => Requirement::on(provider),
            Input::Schemas(provider) => Requirement::on(Some(provider)),
            Input::ExistingSchemas(provider) => Requirement::on(Some(provider)),
        }]
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<GeneratedModels<L>>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, L>) -> Result<()> {
        let plan = match self.input {
            Input::Native(_) => ModelPlan::new(cx.inputs.get::<Blocks<ModelBlock>>()?)?,
            Input::Schemas(_) => {
                ModelPlan::from_shapes(cx.inputs.get::<Blocks<SchemaModelBlock>>()?)?
            }
            Input::ExistingSchemas(_) => {
                ModelPlan::from_schemas(cx.inputs.get::<Blocks<crate::Schema>>()?)?
            }
        };
        let symbols = L::emit_models(cx, &plan)?;
        for schema in &plan.schemas {
            ensure!(
                symbols.contains_key(&schema.name),
                "{} did not publish a model symbol for {}",
                L::NAME,
                schema.name
            );
        }
        cx.files.emit(GeneratedFile::new(
            ".poolster/model-contract.json",
            serde_json::to_string_pretty(&serde_json::json!({
                "version": 1, "language": L::NAME, "schemas": plan.schemas,
                "originalNames": plan.names, "entities": plan.entities, "parent": plan.provenance,
                "runtimeSchemaValidation": "not implied by models-only generation"
            }))? + "\n",
        )?)?;
        cx.publish(GeneratedModels::<L> {
            entities: plan
                .entities
                .iter()
                .map(|(name, id)| (id.clone(), symbols[name].clone()))
                .collect(),
            provenance: plan.provenance,
            symbols,
            original_names: plan.names,
            language: PhantomData,
        })
    }
}
