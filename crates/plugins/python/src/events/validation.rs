use anyhow::{Result, ensure};
use poolster_core::{AdditionalProperties, SchemaKind, native::events::EventOperations};

pub(super) fn check(input: &EventOperations) -> Result<()> {
    ensure!(
        !input.broker.brokers.is_empty(),
        "Python Kafka requires seed brokers"
    );
    for address in &input.broker.brokers {
        let (host, port) = address.rsplit_once(':').ok_or_else(|| {
            anyhow::anyhow!("Python Kafka requires plaintext host:port broker addresses")
        })?;
        ensure!(
            !host.is_empty()
                && !address.contains("://")
                && !address.chars().any(char::is_whitespace)
                && port.parse::<u16>().is_ok_and(|port| port > 0),
            "Python Kafka requires plaintext host:port broker addresses"
        );
    }
    for op in &input.operations {
        schema_supported(&op.message.payload_schema)?;
        if let Some(schema) = &op.message.headers_schema {
            schema_supported(schema)?;
        }
        if let Some(schema) = &op.message.key_schema {
            schema_supported(schema)?;
        }
        ensure!(
            !op.topic.is_empty() && !op.topic.contains('{'),
            "Python Kafka requires a static topic for {}",
            op.name
        );
        if let Some(key) = &op.message.key {
            ensure!(
                matches!(key.kind, SchemaKind::String) && !key.nullable,
                "Python Kafka supports nonnullable string keys only: {}",
                op.name
            );
        }
        if let Some(headers) = &op.message.headers {
            let SchemaKind::Object {
                fields,
                additional_properties,
            } = &headers.kind
            else {
                anyhow::bail!("Python Kafka headers must be an object: {}", op.name)
            };
            ensure!(
                !headers.nullable
                    && matches!(additional_properties, AdditionalProperties::Forbidden),
                "Python Kafka requires closed nonnullable header objects: {}",
                op.name
            );
            for field in fields {
                ensure!(
                    matches!(field.value.kind, SchemaKind::String) && !field.value.nullable,
                    "Python Kafka headers require nonnullable strings: {}.{}",
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
                        "Python Kafka unsupported {scope} binding {key}"
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
                "Python Kafka declared security is unsupported; configure an explicitly authenticated transport separately"
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
        "Python Kafka server bindings beyond bindingVersion are unsupported"
    );
    Ok(())
}

fn schema_supported(schema: &serde_json::Value) -> Result<()> {
    if schema.is_boolean() {
        return Ok(());
    }
    let Some(object) = schema.as_object() else {
        anyhow::bail!("Kafka JSON Schema must be an object or boolean")
    };
    for key in object.keys() {
        ensure!(
            [
                "$schema",
                "$id",
                "$ref",
                "$comment",
                "title",
                "description",
                "default",
                "examples",
                "readOnly",
                "writeOnly",
                "definitions",
                "type",
                "enum",
                "const",
                "multipleOf",
                "maximum",
                "exclusiveMaximum",
                "minimum",
                "exclusiveMinimum",
                "maxLength",
                "minLength",
                "pattern",
                "items",
                "additionalItems",
                "maxItems",
                "minItems",
                "uniqueItems",
                "contains",
                "maxProperties",
                "minProperties",
                "required",
                "properties",
                "patternProperties",
                "additionalProperties",
                "dependencies",
                "propertyNames",
                "allOf",
                "anyOf",
                "oneOf",
                "not",
                "if",
                "then",
                "else"
            ]
            .contains(&key.as_str())
                || key.starts_with("x-"),
            "Python Kafka unsupported JSON Schema keyword {key}"
        );
    }
    if let Some(dialect) = object.get("$schema").and_then(|v| v.as_str()) {
        ensure!(
            dialect.contains("draft-07"),
            "Python Kafka requires JSON Schema draft-07"
        );
    }
    if let Some(reference) = object.get("$ref").and_then(|v| v.as_str()) {
        ensure!(
            reference.starts_with('#'),
            "Python Kafka external schema references are unsupported"
        );
    }
    for key in ["properties", "patternProperties", "definitions"] {
        if let Some(values) = object.get(key).and_then(|v| v.as_object()) {
            for child in values.values() {
                schema_supported(child)?;
            }
        }
    }
    for key in [
        "items",
        "additionalItems",
        "contains",
        "additionalProperties",
        "propertyNames",
        "not",
        "if",
        "then",
        "else",
    ] {
        if let Some(child) = object.get(key) {
            if let Some(array) = child.as_array() {
                for child in array {
                    schema_supported(child)?;
                }
            } else {
                schema_supported(child)?;
            }
        }
    }
    if let Some(values) = object.get("dependencies").and_then(|v| v.as_object()) {
        for value in values.values() {
            if !value.is_array() {
                schema_supported(value)?;
            }
        }
    }
    for key in ["allOf", "anyOf", "oneOf"] {
        if let Some(values) = object.get(key).and_then(|v| v.as_array()) {
            for child in values {
                schema_supported(child)?;
            }
        }
    }
    Ok(())
}
