use anyhow::{Result, ensure};
use poolster_core::{AdditionalProperties, SchemaKind, native::events::EventOperations};

pub(super) fn check(input: &EventOperations) -> Result<()> {
    ensure!(
        !input.broker.brokers.is_empty(),
        "Swift Kafka requires seed brokers"
    );
    ensure!(
        input
            .broker
            .brokers
            .iter()
            .all(|v| !v.is_empty() && !v.contains('\0')),
        "Swift Kafka broker addresses must be nonempty native strings"
    );
    if let Some(id) = &input.broker.client_id {
        ensure!(
            !id.is_empty() && !id.contains('\0'),
            "Swift Kafka client ID must be nonempty native string"
        );
    }
    for op in &input.operations {
        for value in [&op.group_id, &op.client_id].into_iter().flatten() {
            ensure!(
                !value.is_empty() && !value.contains('\0'),
                "Swift Kafka operation bindings must be nonempty native strings"
            );
        }
        ensure!(
            op.topic.len() <= 249
                && op.topic != "."
                && op.topic != ".."
                && op
                    .topic
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)),
            "Swift Kafka requires valid static Kafka topic names"
        );
        schema_supported(&op.message.payload_schema)?;
        if let Some(schema) = &op.message.headers_schema {
            schema_supported(schema)?;
        }
        if let Some(schema) = &op.message.key_schema {
            schema_supported(schema)?;
        }
        ensure!(
            !op.topic.is_empty() && !op.topic.contains('{'),
            "Swift Kafka requires a static topic for {}",
            op.name
        );
        if let Some(key) = &op.message.key {
            ensure!(
                matches!(key.kind, SchemaKind::String) && !key.nullable,
                "Swift Kafka supports nonnullable string keys only: {}",
                op.name
            );
        }
        if let Some(headers) = &op.message.headers {
            let SchemaKind::Object {
                fields,
                additional_properties,
            } = &headers.kind
            else {
                anyhow::bail!("Swift Kafka headers must be an object: {}", op.name)
            };
            ensure!(
                !headers.nullable
                    && matches!(additional_properties, AdditionalProperties::Forbidden),
                "Swift Kafka requires closed nonnullable header objects: {}",
                op.name
            );
            for field in fields {
                ensure!(
                    matches!(field.value.kind, SchemaKind::String) && !field.value.nullable,
                    "Swift Kafka headers require nonnullable strings: {}.{}",
                    op.name,
                    field.name
                );
            }
        }
        for (scope, binding, allowed) in [
            (
                "operation",
                &op.bindings,
                &["bindingVersion", "groupId", "clientId"][..],
            ),
            (
                "channel",
                &op.channel_bindings,
                &["bindingVersion", "topic"][..],
            ),
            (
                "message",
                &op.message.bindings,
                &["bindingVersion", "key"][..],
            ),
        ] {
            let binding = binding.get("kafka").unwrap_or(binding);
            if let Some(object) = binding.as_object() {
                for key in object.keys() {
                    ensure!(
                        allowed.contains(&key.as_str()),
                        "Swift Kafka unsupported {scope} binding {key}"
                    );
                }
            }
        }
    }
    if let Some(servers) = input.source.get("servers").and_then(|v| v.as_object()) {
        for server in servers.values() {
            ensure!(
                server
                    .get("security")
                    .is_none_or(|v| v.as_array().is_some_and(Vec::is_empty)),
                "Swift Kafka declared security is unsupported; configure an explicitly authenticated transport separately"
            );
        }
    }
    let binding = input
        .broker
        .bindings
        .get("kafka")
        .unwrap_or(&input.broker.bindings);
    ensure!(
        binding.is_null()
            || binding
                .as_object()
                .is_some_and(|o| o.keys().all(|k| k == "bindingVersion")),
        "Swift Kafka server bindings beyond bindingVersion are unsupported"
    );
    Ok(())
}

fn schema_supported(schema: &serde_json::Value) -> Result<()> {
    if schema.is_boolean() {
        return Ok(());
    }
    let object = schema
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Swift Kafka JSON Schema must be object or boolean"))?;
    for key in object.keys() {
        ensure!(
            [
                "$schema",
                "$id",
                "$comment",
                "title",
                "description",
                "examples",
                "readOnly",
                "writeOnly",
                "type",
                "enum",
                "const",
                "minimum",
                "maximum",
                "exclusiveMinimum",
                "exclusiveMaximum",
                "minLength",
                "maxLength",
                "minItems",
                "maxItems",
                "minProperties",
                "maxProperties",
                "required",
                "properties",
                "additionalProperties",
                "items"
            ]
            .contains(&key.as_str())
                || key.starts_with("x-"),
            "Swift Kafka unsupported JSON Schema keyword {key}"
        );
    }
    if let Some(value) = object.get("type") {
        let types = if let Some(values) = value.as_array() {
            values.clone()
        } else {
            vec![value.clone()]
        };
        ensure!(
            !types.is_empty()
                && types.iter().all(|v| v.as_str().is_some_and(|s| [
                    "null", "boolean", "object", "array", "number", "integer", "string"
                ]
                .contains(&s))),
            "Swift Kafka malformed schema type"
        );
    }
    if let Some(properties) = object.get("properties") {
        ensure!(
            properties.is_object(),
            "Swift Kafka properties must be an object"
        );
    }
    if let Some(required) = object.get("required") {
        ensure!(
            required
                .as_array()
                .is_some_and(|v| v.iter().all(serde_json::Value::is_string)),
            "Swift Kafka required must be string array"
        );
    }
    if let Some(values) = object.get("enum") {
        ensure!(
            values.as_array().is_some_and(|v| !v.is_empty()),
            "Swift Kafka enum must be nonempty array"
        );
    }
    for key in [
        "minLength",
        "maxLength",
        "minItems",
        "maxItems",
        "minProperties",
        "maxProperties",
    ] {
        if let Some(value) = object.get(key) {
            ensure!(
                value.as_u64().is_some_and(|v| v <= i64::MAX as u64),
                "Swift Kafka count constraint must be nonnegative representable integer"
            );
        }
    }
    if let Some(dialect) = object.get("$schema").and_then(|v| v.as_str()) {
        ensure!(
            dialect.contains("draft-07"),
            "Swift Kafka requires JSON Schema draft-07"
        );
    }
    if let Some(values) = object.get("properties").and_then(|v| v.as_object()) {
        for child in values.values() {
            schema_supported(child)?;
        }
    }
    for key in ["items", "additionalProperties"] {
        if let Some(child) = object.get(key) {
            schema_supported(child)?;
        }
    }
    for key in ["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum"] {
        if let Some(value) = object.get(key) {
            ensure!(
                value
                    .as_f64()
                    .is_some_and(|v| v.is_finite() && v.abs() <= 1e28),
                "Swift Kafka numeric schema bound exceeds supported decimal range"
            );
        }
    }
    Ok(())
}
