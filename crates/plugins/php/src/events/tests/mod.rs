use super::*;
use poolster_core::{
    AdditionalProperties, SchemaKind, SchemaValue,
    native::events::{EventMessage, EventOperation, KafkaBroker},
};
fn input() -> EventOperations {
    let string = SchemaValue::new(SchemaKind::String);
    let object = SchemaValue::new(SchemaKind::Object {
        fields: vec![poolster_core::Field {
            name: "id".into(),
            required: true,
            value: string.clone(),
            annotations: Default::default(),
        }],
        additional_properties: AdditionalProperties::Forbidden,
    });
    let message = EventMessage {
        name: "User".into(),
        location: "/channels/users/messages/user".into(),
        payload: object,
        payload_schema: serde_json::json!({"type":"object","properties":{"id":{"type":"string","minLength":1}},"required":["id"],"additionalProperties":false}),
        headers: Some(SchemaValue::new(SchemaKind::Object {
            fields: vec![poolster_core::Field {
                name: "trace".into(),
                value: string.clone(),
                required: false,
                annotations: Default::default(),
            }],
            additional_properties: AdditionalProperties::Forbidden,
        })),
        headers_schema: Some(
            serde_json::json!({"type":"object","properties":{"trace":{"type":"string"}},"additionalProperties":false}),
        ),
        key: Some(string),
        key_schema: Some(serde_json::json!({"type":"string"})),
        bindings: serde_json::Value::Null,
    };
    EventOperations {
        source: serde_json::json!({"asyncapi":"3.0.0"}),
        broker: KafkaBroker {
            brokers: vec!["127.0.0.1:29092".into()],
            client_id: Some("poolster-php-tests".into()),
            bindings: serde_json::Value::Null,
        },
        operations: vec![
            EventOperation {
                name: "sendUser".into(),
                action: EventAction::Send,
                channel: "users".into(),
                topic: "users".into(),
                message: message.clone(),
                group_id: None,
                client_id: Some("poolster-php-send".into()),
                bindings: serde_json::Value::Null,
                channel_bindings: serde_json::Value::Null,
            },
            EventOperation {
                name: "receiveUser".into(),
                action: EventAction::Receive,
                channel: "users".into(),
                topic: "users".into(),
                message,
                group_id: None,
                client_id: Some("poolster-php-receive".into()),
                bindings: serde_json::Value::Null,
                channel_bindings: serde_json::Value::Null,
            },
        ],
    }
}
#[test]
fn events_share_models_and_keep_package_manifest_ownership() {
    let tree = render_event_package(&input(), "acme/events").unwrap();
    assert!(
        tree.get("src/Events/SendUserEvent.php")
            .unwrap()
            .contains("\\Acme\\Events\\Models\\UserPayload")
    );
    assert!(tree.get("composer.json").unwrap().contains("2.6.0"));
    assert!(tree.get("asyncapi.json").is_some());
    assert_eq!(
        tree.iter()
            .filter(|(p, _)| p.to_string_lossy().contains("UserPayload"))
            .count(),
        1
    );
    let repeat = render_event_package(&input(), "acme/events").unwrap();
    assert_eq!(tree, repeat);
}
#[test]
fn unsupported_features_fail_before_emission() {
    let mut input = input();
    input.operations[0].topic = "users/{id}".into();
    assert!(
        format!(
            "{:#}",
            render_event_package(&input, "acme/events").unwrap_err()
        )
        .contains("static")
    );
    let mut input = self::input();
    input.operations[0].message.payload_schema["format"] = "uri".into();
    assert!(
        format!(
            "{:#}",
            render_event_package(&input, "acme/events").unwrap_err()
        )
        .contains("format")
    );
}

mod native;
mod provenance;
