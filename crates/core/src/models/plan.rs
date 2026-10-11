use crate::{
    Schema, SchemaValue,
    blocks::{Block, BlockId, Blocks, ModelBlock},
    native::ModelType,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
mod native;
mod normalize;

/// Rich model shapes retain validation keywords, extension data and dictionaries.
/// This is additive: existing ModelBlock and protocol contracts keep their identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaModelBlock {
    pub name: String,
    pub value: SchemaValue,
}
impl Block for SchemaModelBlock {
    const CONTRACT_NAME: &'static str = "poolster.schema-model-blocks.v1";
}

pub struct ModelPlan {
    pub schemas: Vec<Schema>,
    pub blocks: Blocks<ModelBlock>,
    pub names: BTreeMap<String, String>,
    pub types: BTreeMap<String, ModelType>,
    pub entities: BTreeMap<String, BlockId>,
    pub provenance: Option<crate::blocks::ContractReference>,
}
impl ModelPlan {
    pub fn from_schemas(blocks: &Blocks<Schema>) -> Result<Self> {
        blocks.validate()?;
        let shapes = Blocks {
            parent: blocks.parent.clone(),
            state: blocks.state.clone(),
            items: blocks
                .items
                .iter()
                .map(|item| crate::blocks::BuildingBlock {
                    metadata: item.metadata.clone(),
                    value: SchemaModelBlock {
                        name: item.value.name.clone(),
                        value: item.value.value.clone(),
                    },
                })
                .collect(),
        };
        Self::from_shapes(&shapes)
    }
    pub fn new(blocks: &Blocks<ModelBlock>) -> Result<Self> {
        blocks.validate()?;
        blocks.require_complete()?;
        let entries = blocks
            .items
            .iter()
            .map(|item| {
                Ok((
                    item.metadata.id.clone(),
                    Schema::new(&item.value.name, native::schema(&item.value.ty)?),
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let (schemas, names, entities) = normalize::normalize(entries)?;
        let types = schemas
            .iter()
            .map(|schema| (schema.name.clone(), native::model(&schema.value)))
            .collect();
        Ok(Self {
            schemas,
            blocks: blocks.clone(),
            names,
            types,
            entities,
            provenance: blocks.parent.clone(),
        })
    }
    pub fn from_shapes(blocks: &Blocks<SchemaModelBlock>) -> Result<Self> {
        blocks.validate()?;
        blocks.require_complete()?;
        let entries = blocks
            .items
            .iter()
            .map(|item| {
                (
                    item.metadata.id.clone(),
                    Schema::new(&item.value.name, item.value.value.clone()),
                )
            })
            .collect();
        let (schemas, names, entities) = normalize::normalize(entries)?;
        Ok(Self {
            schemas,
            names,
            entities,
            provenance: blocks.parent.clone(),
            types: BTreeMap::new(),
            blocks: Blocks {
                parent: blocks.parent.clone(),
                state: blocks.state.clone(),
                items: vec![],
            },
        })
    }
}
