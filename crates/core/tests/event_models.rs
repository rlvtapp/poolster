use anyhow::Result;
use poolster_core::{
    SchemaKind, SchemaValue, blocks::ContractReference, engine::*, models::*, native::events::*,
};
use std::collections::BTreeMap;
struct Lang;
impl Language for Lang {
    const NAME: &'static str = "event-model-test";
    type Settings = ();
    type Workspace = ();
}
impl ModelOutput for Lang {
    const MODEL_CONTRACT_NAME: &'static str = "example.event-models.v1";
    fn emit_models(
        _: &mut PluginContext<'_, Self>,
        plan: &ModelPlan,
    ) -> Result<BTreeMap<String, ModelSymbol>> {
        Ok(plan
            .schemas
            .iter()
            .map(|schema| {
                (
                    schema.name.clone(),
                    ModelSymbol {
                        name: schema.name.clone(),
                        module: "models".into(),
                    },
                )
            })
            .collect())
    }
}
fn events() -> EventOperations {
    EventOperations {
        source: serde_json::json!({"fixture":true}),
        broker: KafkaBroker {
            brokers: vec!["localhost:9092".into()],
            client_id: None,
            bindings: serde_json::Value::Null,
        },
        operations: vec![EventOperation {
            name: "sendItem".into(),
            action: EventAction::Send,
            channel: "items".into(),
            topic: "items".into(),
            message: EventMessage {
                name: "Item".into(),
                location: "#/channels/items/messages/item".into(),
                payload: SchemaValue::new(SchemaKind::String),
                payload_schema: serde_json::json!({"type":"string"}),
                headers: None,
                headers_schema: None,
                key: Some(SchemaValue::new(SchemaKind::String)),
                key_schema: Some(serde_json::json!({"type":"string"})),
                bindings: serde_json::Value::Null,
            },
            group_id: None,
            client_id: None,
            bindings: serde_json::Value::Null,
            channel_bindings: serde_json::Value::Null,
        }],
    }
}
struct Source {
    meta: Meta,
    opaque: bool,
}
impl Plugin<Lang> for Source {
    fn kind(&self) -> &'static str {
        "event-source"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<EventOperations>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Lang>) -> Result<()> {
        let value = events();
        let mut reference = ContractReference::from_bytes(
            EventOperations::NAME,
            "fixture",
            &serde_json::to_vec(&value)?,
        );
        if self.opaque {
            reference.revision = "provider-revision-42".into();
        }
        cx.publish_with_reference(value, reference)
    }
}
#[test]
fn event_models_bind_by_stable_ids_and_reject_changed_revision() {
    let source = Source {
        meta: Meta::new(),
        opaque: false,
    };
    let blocks = model_blocks(Some(source.meta.handle()), "fixture");
    let models = ModelPlugin::<Lang>::new(None).input_shapes(blocks.handle());
    let check = hooks::<Lang>().on_contract(Some(models.handle()), |models, _| {
        let original = events();
        let resolved = original.resolve_models(models)?;
        let symbols = &resolved[&original.operations[0].message.location];
        assert_eq!(symbols.payload.name, "ItemPayload");
        assert_eq!(symbols.key.as_ref().unwrap().name, "ItemKey");
        assert!(symbols.headers.is_none());
        let mut changed = original;
        changed.operations[0].topic = "changed".into();
        assert!(
            changed
                .resolve_models(models)
                .unwrap_err()
                .to_string()
                .contains("revision mismatch")
        );
        Ok(())
    });
    Packages::new()
        .package(
            Package::<Lang>::new("output")
                .with(check)
                .with(models)
                .with(blocks)
                .with(source),
        )
        .generate_native()
        .unwrap();
}
#[test]
fn extraction_rejects_wrong_contract_instance_before_emission() {
    let source = Source {
        meta: Meta::new(),
        opaque: false,
    };
    let blocks = model_blocks(Some(source.meta.handle()), "other");
    let error = Packages::new()
        .package(Package::<Lang>::new("output").with(blocks).with(source))
        .generate_native()
        .unwrap_err();
    assert!(format!("{error:#}").contains("selected contract instance"));
}

#[test]
fn published_event_revisions_remain_opaque() {
    let source = Source {
        meta: Meta::new(),
        opaque: true,
    };
    let blocks = model_blocks(Some(source.meta.handle()), "fixture");
    let models = ModelPlugin::<Lang>::new(None).input_shapes(blocks.handle());
    let check = hooks::<Lang>().on_contract(Some(models.handle()), |models, _| {
        let reference = ContractReference {
            contract: EventOperations::NAME.into(),
            instance: "fixture".into(),
            revision: "provider-revision-42".into(),
        };
        assert!(
            events()
                .resolve_models_with_reference(models, Some(&reference))
                .is_ok()
        );
        let mut stale = reference;
        stale.revision = "provider-revision-41".into();
        assert!(
            events()
                .resolve_models_with_reference(models, Some(&stale))
                .is_err()
        );
        Ok(())
    });
    Packages::new()
        .package(
            Package::<Lang>::new("output")
                .with(check)
                .with(models)
                .with(blocks)
                .with(source),
        )
        .generate_native()
        .unwrap();
}
