//! Opt-in rich model extraction preserves native schema semantics and message context.
use crate::contracts::{EventMessage, MessageBlocks};
use anyhow::Result;
use poolster_core::{
    blocks::{Block, BlockReference, Blocks, BuildingBlock},
    engine::{Handle, Language, Meta, Plugin, PluginContext, Provision, Requirement},
    models::SchemaModelBlock,
};

pub struct SharedModelBlocks {
    meta: Meta,
    provider: Option<Handle<MessageBlocks>>,
}
pub fn shared_model_blocks(provider: Option<Handle<MessageBlocks>>) -> SharedModelBlocks {
    SharedModelBlocks {
        meta: Meta::new(),
        provider,
    }
}
impl SharedModelBlocks {
    pub fn handle(&self) -> Handle<Blocks<SchemaModelBlock>> {
        self.meta.handle()
    }
}
impl<L: Language> Plugin<L> for SharedModelBlocks {
    fn kind(&self) -> &'static str {
        "asyncapi-shared-model-blocks"
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
        let messages = cx.inputs.get::<MessageBlocks>()?;
        cx.publish(extract(messages))
    }
}

/// No lossy ModelType conversion: constraints, extensions and open properties survive.
pub fn extract(messages: &MessageBlocks) -> Blocks<SchemaModelBlock> {
    let mut items = Vec::new();
    for message in &messages.items {
        for (role, shape) in [
            ("Payload", Some(&message.value.payload)),
            ("Headers", message.value.headers.as_ref()),
            ("Key", message.value.key.as_ref()),
        ] {
            let Some(shape) = shape else { continue };
            let mut metadata = message.metadata.clone();
            metadata.references.push(BlockReference {
                contract: <EventMessage as Block>::CONTRACT_NAME.into(),
                id: metadata.id.clone(),
            });
            metadata.id.local = format!("{}/{}", metadata.id.local, role.to_lowercase());
            metadata.capabilities.insert("model.schema-value".into());
            items.push(BuildingBlock {
                metadata,
                value: SchemaModelBlock {
                    name: format!("{}{role}", message.value.name),
                    value: shape.clone(),
                },
            });
        }
    }
    items.sort_by(|a, b| a.metadata.id.cmp(&b.metadata.id));
    Blocks {
        parent: messages.parent.clone(),
        state: messages.state.clone(),
        items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rich_shapes_retain_constraints_and_message_provenance() {
        let document = crate::parse(include_str!("../tests/fixtures/kafka-json.yaml")).unwrap();
        let (messages, diagnostics) = crate::lower_message_blocks(&document, "fixture");
        assert!(diagnostics.is_empty());
        let original = messages.clone();
        let shapes = extract(&messages);
        assert_eq!(messages, original);
        assert_eq!(shapes.parent, messages.parent);
        assert_eq!(shapes.state, messages.state);
        assert_eq!(shapes.items.len(), 3);
        for block in &shapes.items {
            let reference = block.metadata.references.last().unwrap();
            assert_eq!(reference.id, messages.items[0].metadata.id);
            assert_eq!(block.metadata.parent, messages.items[0].metadata.parent);
        }
        let key = shapes
            .items
            .iter()
            .find(|b| b.value.name.ends_with("Key"))
            .unwrap();
        assert_eq!(
            key.value.value.constraints["minLength"],
            serde_json::json!(1)
        );
        assert_eq!(
            key.value.value,
            messages.items[0].value.key.clone().unwrap()
        );
    }
}
