//! Deliberately narrow Kafka/JSON capabilities, checked before output.
use super::*;
use poolster_core::{AdditionalProperties, SchemaKind, SchemaValue};

pub(super) fn check(events: &EventOperations) -> Result<()> {
    ensure!(
        !events.operations.is_empty(),
        "Rust Kafka generation requires event operations"
    );
    ensure!(
        !events.broker.brokers.is_empty()
            && events.broker.brokers.iter().all(|s| !s.contains("://")
                && s.rsplit_once(':').is_some_and(
                    |(host, port)| !host.is_empty() && port.parse::<u16>().is_ok_and(|p| p > 0)
                )),
        "Rust Kafka needs explicit plaintext host:port broker addresses"
    );
    bindings(&events.broker.bindings, &["bindingVersion"])?;
    if let Some(servers) = events
        .source
        .get("servers")
        .and_then(serde_json::Value::as_object)
    {
        for server in servers.values() {
            ensure!(
                server
                    .get("protocol")
                    .and_then(serde_json::Value::as_str)
                    .is_none_or(|p| p == "kafka"),
                "Rust Kafka supports plaintext Kafka only"
            );
            ensure!(
                server
                    .get("security")
                    .is_none_or(|s| s.as_array().is_some_and(Vec::is_empty)),
                "Rust Kafka authentication is unsupported"
            );
        }
    }
    for op in &events.operations {
        ensure!(
            !op.topic.is_empty()
                && op
                    .topic
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')),
            "Rust Kafka requires static topic names"
        );
        identifier(&op.name)?;
        ensure!(
            !op.message.location.is_empty(),
            "event message location is required for model identity"
        );
        bindings(&op.bindings, &["bindingVersion", "groupId", "clientId"])?;
        bindings(&op.channel_bindings, &["bindingVersion", "topic"])?;
        bindings(&op.message.bindings, &["bindingVersion", "key"])?;
        ensure!(
            op.message.headers.is_some() == op.message.headers_schema.is_some()
                && op.message.key.is_some() == op.message.key_schema.is_some(),
            "event model and schema roles must agree"
        );
        shape(&op.message.payload)?;
        if let Some(headers) = &op.message.headers {
            let SchemaKind::Object {
                fields,
                additional_properties,
            } = &headers.kind
            else {
                anyhow::bail!("Kafka headers must be a nonnullable string object");
            };
            ensure!(
                !headers.nullable
                    && fields
                        .iter()
                        .all(|f| matches!(f.value.kind, SchemaKind::String) && !f.value.nullable)
                    && matches!(
                        additional_properties,
                        AdditionalProperties::Forbidden
                            | AdditionalProperties::Unspecified
                            | AdditionalProperties::Any
                    ),
                "Kafka header values must be nonnullable strings"
            );
            shape(headers)?;
        }
        if let Some(key) = &op.message.key {
            ensure!(
                matches!(key.kind, SchemaKind::String) && !key.nullable,
                "Kafka key supports only nonnullable strings"
            );
            shape(key)?;
        }
        for schema in [
            Some(&op.message.payload_schema),
            op.message.headers_schema.as_ref(),
            op.message.key_schema.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            schema_keywords(schema)?;
        }
    }
    Ok(())
}
fn bindings(value: &serde_json::Value, allowed: &[&str]) -> Result<()> {
    if let Some(kafka) = value.get("kafka") {
        ensure!(
            value.as_object().is_some_and(|v| v.len() == 1),
            "other broker bindings cannot accompany Kafka"
        );
        return bindings(kafka, allowed);
    }
    ensure!(
        value.is_null()
            || value
                .as_object()
                .is_some_and(|v| v.keys().all(|k| allowed.contains(&k.as_str()))),
        "unsupported Rust Kafka broker binding"
    );
    Ok(())
}
fn shape(value: &SchemaValue) -> Result<()> {
    ensure!(
        value.default.is_none() && value.const_value.is_none(),
        "Rust Kafka typed models do not support defaults/literal constraints"
    );
    match &value.kind {
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            for field in fields {
                shape(&field.value)?;
            }
            if let AdditionalProperties::Schema { value } = additional_properties {
                shape(value)?;
            }
        }
        SchemaKind::Array { items } => shape(items)?,
        SchemaKind::OneOf { .. }
        | SchemaKind::AnyOf { .. }
        | SchemaKind::AllOf { .. }
        | SchemaKind::Not { .. }
        | SchemaKind::Reference { .. } => anyhow::bail!(
            "Rust Kafka models require resolved acyclic shapes without schema compositions"
        ),
        _ => {}
    }
    Ok(())
}
fn schema_keywords(value: &serde_json::Value) -> Result<()> {
    let Some(object) = value.as_object() else {
        anyhow::bail!("Kafka JSON schemas must be resolved objects");
    };
    if let Some(dialect) = object.get("$schema").and_then(serde_json::Value::as_str) {
        ensure!(
            dialect.trim_end_matches('#').ends_with("draft-07/schema"),
            "Rust Kafka runtime requires JSON Schema Draft 7"
        );
    }
    if let Some(format) = object.get("format").and_then(serde_json::Value::as_str) {
        ensure!(
            [
                "date",
                "date-time",
                "time",
                "email",
                "hostname",
                "ipv4",
                "ipv6",
                "uuid",
                "uri",
                "uri-reference",
                "regex",
                "json-pointer",
                "relative-json-pointer"
            ]
            .contains(&format),
            "unsupported Rust Kafka schema format {format}"
        );
    }
    for (key, value) in object {
        ensure!(
            [
                "type",
                "properties",
                "required",
                "items",
                "additionalProperties",
                "enum",
                "minimum",
                "maximum",
                "exclusiveMinimum",
                "exclusiveMaximum",
                "multipleOf",
                "minLength",
                "maxLength",
                "pattern",
                "format",
                "minItems",
                "maxItems",
                "uniqueItems",
                "minProperties",
                "maxProperties",
                "title",
                "description",
                "$schema"
            ]
            .contains(&key.as_str())
                || key.starts_with("x-"),
            "unsupported Rust Kafka JSON Schema keyword {key}"
        );
        match key.as_str() {
            "properties" => {
                let fields = value
                    .as_object()
                    .ok_or_else(|| anyhow::anyhow!("invalid schema properties"))?;
                for field in fields.values() {
                    schema_keywords(field)?;
                }
            }
            "items" => schema_keywords(value)?,
            "additionalProperties" if value.is_object() => schema_keywords(value)?,
            _ => {}
        }
    }
    Ok(())
}
pub(super) fn identifier(value: &str) -> Result<String> {
    ensure!(
        !value.is_empty()
            && value
                .chars()
                .enumerate()
                .all(|(i, c)| c.is_ascii_alphabetic() || c == '_' || (i > 0 && c.is_ascii_digit())),
        "unsupported Rust event identifier {value}"
    );
    ensure!(
        value.len() <= 120,
        "Rust event operation exceeds filename budget"
    );
    let name = crate::render::rust_field_name(value);
    ensure!(
        ![
            "new",
            "subscribe",
            "publish",
            "receive",
            "ack",
            "close",
            "default"
        ]
        .contains(&name.as_str()),
        "reserved Rust event operation {value}"
    );
    Ok(name)
}
