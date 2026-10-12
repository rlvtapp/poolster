//! Explicit batch mappings; no method-name or idempotency inference.
use super::*;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpcBatchMapping {
    pub single_method: String,
    pub batch_method: String,
    pub input_field: String,
    pub batch_input_field: String,
    pub batch_output_field: String,
    /// None associates by position. A field name associates by a typed key.
    #[serde(default)]
    pub output_key: Option<String>,
    /// Cache successes only, within the explicitly created request scope.
    #[serde(default)]
    pub cache: bool,
    #[serde(default = "batch_limit")]
    pub max_batch_size: u32,
}
fn batch_limit() -> u32 {
    100
}
fn find_message<'a>(wire: &'a RpcWireSchema, name: &str) -> Result<&'a RpcWireMessage> {
    fn find<'a>(messages: &'a [RpcWireMessage], name: &str) -> Option<&'a RpcWireMessage> {
        for message in messages {
            if message.full_name == name {
                return Some(message);
            }
            if let Some(value) = find(&message.messages, name) {
                return Some(value);
            }
        }
        None
    }
    wire.files
        .iter()
        .find_map(|file| find(&file.messages, name))
        .with_context(|| format!("Protobuf batch mapping references unknown message {name}"))
}
fn field<'a>(message: &'a RpcWireMessage, name: &str) -> Result<&'a RpcWireField> {
    message
        .fields
        .iter()
        .find(|f| f.name == name)
        .with_context(|| {
            format!(
                "Protobuf batch mapping references unknown field {}.{name}",
                message.full_name
            )
        })
}
impl RpcBatchMapping {
    pub fn validate(&self, wire: &RpcWireSchema) -> Result<()> {
        ensure!(
            self.max_batch_size > 0 && self.max_batch_size <= 10_000,
            "Protobuf maxBatchSize must be between 1 and 10000"
        );
        ensure!(
            self.single_method != self.batch_method,
            "Protobuf batch and single methods must differ"
        );
        let method = |name: &str| {
            wire.files
                .iter()
                .flat_map(|file| &file.services)
                .flat_map(|service| &service.methods)
                .find(|method| method.full_name == name)
                .with_context(|| format!("Protobuf batch mapping references unknown method {name}"))
        };
        let single = method(&self.single_method)?;
        let batch = method(&self.batch_method)?;
        ensure!(
            single.streaming == RpcStreaming::Unary && batch.streaming == RpcStreaming::Unary,
            "Protobuf batching requires explicitly mapped unary methods"
        );
        let input = find_message(wire, &single.input_type)?;
        let batch_input = find_message(wire, &batch.input_type)?;
        let batch_output = find_message(wire, &batch.output_type)?;
        let key = field(input, &self.input_field)?;
        let keys = field(batch_input, &self.batch_input_field)?;
        let values = field(batch_output, &self.batch_output_field)?;
        ensure!(
            input.fields.len() == 1 && batch_input.fields.len() == 1,
            "Protobuf batch mapping must account for every input field"
        );
        ensure!(
            key.label != RpcFieldLabel::Repeated && key.oneof_index.is_none(),
            "Protobuf batch key must be a singular field outside a oneof"
        );
        ensure!(
            !matches!(
                key.kind,
                RpcFieldKind::Message(_)
                    | RpcFieldKind::Group(_)
                    | RpcFieldKind::Float
                    | RpcFieldKind::Double
                    | RpcFieldKind::Bytes
            ),
            "Protobuf batch keys must be stable scalar values (string, integer, boolean or enum)"
        );
        ensure!(
            keys.label == RpcFieldLabel::Repeated && keys.kind == key.kind,
            "Protobuf batch input collection must preserve the single request key type"
        );
        ensure!(
            values.label == RpcFieldLabel::Repeated
                && values.kind == RpcFieldKind::Message(single.output_type.clone()),
            "Protobuf batch output collection must contain the single method response type"
        );
        if let Some(name) = &self.output_key {
            let response = find_message(wire, &single.output_type)?;
            let output = field(response, name)?;
            ensure!(
                output.label != RpcFieldLabel::Repeated
                    && output.oneof_index.is_none()
                    && output.kind == key.kind,
                "Protobuf batch response association key must preserve the request key type"
            );
        }
        Ok(())
    }
}
