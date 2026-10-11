//! Shared event models are bound to stable message identities and event revisions.
use super::*;
use crate::{blocks::*, engine::*, models::*};
use anyhow::{Context, Result, ensure};
use std::collections::BTreeMap;
#[derive(Clone, Debug)]
pub struct EventModelSymbols {
    pub payload: ModelSymbol,
    pub headers: Option<ModelSymbol>,
    pub key: Option<ModelSymbol>,
}
impl EventOperations {
    pub fn shared_model_blocks(&self, source: impl Into<String>) -> Blocks<SchemaModelBlock> {
        let messages = self.message_blocks(source);
        let mut items = Vec::new();
        for message in messages.items {
            for (role, suffix, value) in [
                ("payload", "Payload", Some(&message.value.payload)),
                ("headers", "Headers", message.value.headers.as_ref()),
                ("key", "Key", message.value.key.as_ref()),
            ] {
                let Some(value) = value else { continue };
                let mut metadata = message.metadata.clone();
                metadata.references.push(BlockReference {
                    contract: EventMessage::CONTRACT_NAME.into(),
                    id: metadata.id.clone(),
                });
                metadata.id.local = format!("{}/{role}", metadata.id.local);
                metadata.capabilities.insert("model.schema-value".into());
                items.push(BuildingBlock {
                    metadata,
                    value: SchemaModelBlock {
                        name: format!("{}{suffix}", message.value.name),
                        value: value.clone(),
                    },
                });
            }
        }
        Blocks {
            parent: messages.parent,
            state: messages.state,
            items,
        }
    }
    pub fn resolve_models<L: ModelOutput>(
        &self,
        models: &GeneratedModels<L>,
    ) -> Result<BTreeMap<String, EventModelSymbols>> {
        self.resolve_models_with_reference(models, None)
    }
    /// Published revisions are opaque; only fallback providers use content hashes.
    pub fn resolve_models_with_reference<L: ModelOutput>(
        &self,
        models: &GeneratedModels<L>,
        selected: Option<&ContractReference>,
    ) -> Result<BTreeMap<String, EventModelSymbols>> {
        let parent = models
            .provenance
            .as_ref()
            .context("event models require contract provenance")?;
        parent.validate()?;
        let expected = match selected {
            Some(reference) => reference.clone(),
            None => ContractReference::from_bytes(
                Self::NAME,
                &parent.instance,
                &serde_json::to_vec(self)?,
            ),
        };
        expected.validate()?;
        ensure!(
            expected.contract == Self::NAME,
            "selected event provenance has the wrong contract identity"
        );
        ensure!(
            parent == &expected,
            "event model revision mismatch; derive models from the selected EventOperations contract"
        );
        let mut result = BTreeMap::new();
        for operation in &self.operations {
            let message = &operation.message;
            let symbol = |role: &str| -> Result<ModelSymbol> {
                let id = BlockId {
                    source: parent.instance.clone(),
                    local: format!("{}/{role}", message.location),
                };
                models.entities.get(&id).cloned().with_context(|| {
                    format!("missing shared event model {}:{role}", message.location)
                })
            };
            result.insert(
                message.location.clone(),
                EventModelSymbols {
                    payload: symbol("payload")?,
                    headers: message
                        .headers
                        .as_ref()
                        .map(|_| symbol("headers"))
                        .transpose()?,
                    key: message.key.as_ref().map(|_| symbol("key")).transpose()?,
                },
            );
        }
        Ok(result)
    }
}
pub struct EventModelBlocks {
    meta: Meta,
    provider: Option<Handle<EventOperations>>,
    source: String,
}
pub fn model_blocks(
    provider: Option<Handle<EventOperations>>,
    source: impl Into<String>,
) -> EventModelBlocks {
    EventModelBlocks {
        meta: Meta::new(),
        provider,
        source: source.into(),
    }
}
impl EventModelBlocks {
    pub fn handle(&self) -> Handle<Blocks<SchemaModelBlock>> {
        self.meta.handle()
    }
}
impl<L: Language> Plugin<L> for EventModelBlocks {
    fn kind(&self) -> &'static str {
        "event-shared-model-blocks"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.provider)]
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<Blocks<SchemaModelBlock>>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, L>) -> Result<()> {
        ensure!(
            !self.source.trim().is_empty(),
            "event model source identity cannot be empty"
        );
        let input = cx.inputs.get::<EventOperations>()?;
        let blocks = input.shared_model_blocks(&self.source);
        let blocks = if let Some(reference) = cx.inputs.reference::<EventOperations>()? {
            ensure!(
                reference.contract == EventOperations::NAME && reference.instance == self.source,
                "event model extraction must use the selected contract instance and revision"
            );
            blocks.with_parent(reference.clone())
        } else {
            blocks
        };
        cx.publish(blocks)
    }
}
