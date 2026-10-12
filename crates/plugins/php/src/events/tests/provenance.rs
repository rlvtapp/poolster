use super::*;
struct Source {
    meta: Meta,
    input: EventOperations,
    revision: &'static str,
}
impl Source {
    fn new(revision: &'static str) -> Self {
        Self {
            meta: Meta::new(),
            input: input(),
            revision,
        }
    }
    fn handle(&self) -> Handle<EventOperations> {
        self.meta.handle()
    }
}
impl Plugin<Php> for Source {
    fn kind(&self) -> &'static str {
        "php-events-test-source"
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
    fn generate(&self, cx: &mut PluginContext<Php>) -> Result<()> {
        let mut reference = ContractReference::from_bytes(
            EventOperations::NAME,
            "events",
            &serde_json::to_vec(&self.input)?,
        );
        reference.revision = self.revision.into();
        cx.publish_with_reference(self.input.clone(), reference)
    }
}
#[test]
fn opaque_revision_selected_input_and_stale_models_are_checked() {
    use crate::PackageExt;
    use poolster_core::engine::Packages;
    use poolster_core::native::events::model_blocks;
    let original = Source::new("opaque-original");
    let selected = Source::new("opaque-selected");
    let shapes = model_blocks(Some(selected.handle()), "events");
    let models = crate::shared_models(None).input_shapes(shapes.handle());
    let events = asyncapi(Some(selected.handle())).using_models(models.handle());
    let tree = Packages::new()
        .package(
            crate::package("sdk")
                .name("acme/events")
                .with(original)
                .with(selected)
                .with(shapes)
                .with(models)
                .with(events),
        )
        .generate_native()
        .unwrap();
    assert!(
        tree.get("sdk/.poolster/model-contract.json")
            .unwrap()
            .contains("opaque-selected")
    );
    let original = Source::new("old");
    let changed = Source::new("new");
    let shapes = model_blocks(Some(original.handle()), "events");
    let models = crate::shared_models(None).input_shapes(shapes.handle());
    let events = asyncapi(Some(changed.handle())).using_models(models.handle());
    let error = Packages::new()
        .package(
            crate::package("sdk")
                .name("acme/events")
                .with(original)
                .with(changed)
                .with(shapes)
                .with(models)
                .with(events),
        )
        .generate_native()
        .unwrap_err();
    assert!(format!("{error:#}").contains("revision"));
}
#[test]
fn large_operation_set_stays_split_and_bounded() {
    let mut input = input();
    let original = input.operations[0].clone();
    input.operations.clear();
    for n in 0..250 {
        let mut op = original.clone();
        op.name = format!("sendUser{n}");
        input.operations.push(op);
    }
    let tree = render_event_package(&input, "acme/events").unwrap();
    assert_eq!(
        tree.iter()
            .filter(|(path, _)| path.to_string_lossy().contains("UserPayload.php"))
            .count(),
        1
    );
    assert_eq!(
        tree.iter()
            .filter(|(path, _)| path.to_string_lossy().contains("src/Events/SendUser"))
            .count(),
        250
    );
    for (_, source) in tree.iter() {
        assert!(source.len() < 128 * 1024);
    }
}
struct RenamedShapes {
    meta: Meta,
    source: Handle<poolster_core::blocks::Blocks<poolster_core::models::SchemaModelBlock>>,
}
impl Plugin<Php> for RenamedShapes {
    fn kind(&self) -> &'static str {
        "php-renamed-event-shapes"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(Some(self.source))]
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<
            poolster_core::blocks::Blocks<poolster_core::models::SchemaModelBlock>,
        >()]
    }
    fn generate(&self, cx: &mut PluginContext<Php>) -> Result<()> {
        let mut shapes = cx
            .inputs
            .get::<poolster_core::blocks::Blocks<poolster_core::models::SchemaModelBlock>>()?
            .clone();
        for item in &mut shapes.items {
            item.value.name = format!("Renamed{}", item.value.name);
        }
        cx.publish(shapes)
    }
}
#[test]
fn event_operation_uses_actual_substituted_model_symbols() {
    use crate::PackageExt;
    use poolster_core::engine::Packages;
    let source = Source::new("opaque-symbols");
    let blocks = poolster_core::native::events::model_blocks(Some(source.handle()), "events");
    let renamed = RenamedShapes {
        meta: Meta::new(),
        source: blocks.handle(),
    };
    let handle = renamed.meta.handle();
    let models = crate::shared_models(None).input_shapes(handle);
    let events = asyncapi(Some(source.handle())).using_models(models.handle());
    let tree = Packages::new()
        .package(
            crate::package("sdk")
                .name("acme/events")
                .with(source)
                .with(blocks)
                .with(renamed)
                .with(models)
                .with(events),
        )
        .generate_native()
        .unwrap();
    let operation = tree.get("sdk/src/Events/SendUserEvent.php").unwrap();
    assert!(operation.contains("\\Acme\\Events\\Models\\RenamedUserPayload $payload"));
    assert!(operation.contains("RenamedUserKey"));
    assert!(tree.get("sdk/src/Models/RenamedUserPayload.php").is_some());
}
