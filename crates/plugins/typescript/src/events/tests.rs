use super::*;
use poolster_core::engine::Packages;
struct Source {
    meta: Meta,
    input: EventOperations,
}
impl Plugin<TypeScript> for Source {
    fn kind(&self) -> &'static str {
        "event-layout-fixture"
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
    fn generate(&self, cx: &mut PluginContext<'_, TypeScript>) -> Result<()> {
        cx.publish(self.input.clone())
    }
}
fn generate(reverse: bool) -> poolster_core::GeneratedTree {
    let doc = poolster_input_asyncapi::parse(include_str!(
        "../../../../inputs/asyncapi/tests/fixtures/kafka-json.yaml"
    ))
    .unwrap();
    let mut input = poolster_input_asyncapi::lower_operations(&doc, &Default::default()).unwrap();
    let original = input.operations[0].clone();
    input.operations = (0..300)
        .map(|i| {
            let mut op = original.clone();
            op.name = format!("sendOrder{i}");
            op
        })
        .collect();
    if reverse {
        input.operations.reverse();
    }
    let source = Source {
        meta: Meta::new(),
        input,
    };
    let output = asyncapi(Some(source.meta.handle()));
    Packages::new()
        .package(crate::package("client").with(output).with(source))
        .generate_native()
        .unwrap()
}
#[test]
fn hundreds_of_events_have_bounded_deterministic_models_and_operations() {
    let first = generate(false);
    let second = generate(true);
    let normalize = |tree: &poolster_core::GeneratedTree| {
        tree.iter()
            .map(|(p, s)| (p.to_owned(), s.to_owned()))
            .collect::<BTreeMap<_, _>>()
    };
    assert_eq!(normalize(&first), normalize(&second));
    assert_eq!(
        first
            .iter()
            .filter(|(p, _)| p.to_string_lossy().contains("events/models/"))
            .count(),
        300
    );
    assert_eq!(
        first
            .iter()
            .filter(|(p, _)| p.to_string_lossy().contains("events/operations/"))
            .count(),
        300
    );
    for (path, source) in first.iter() {
        assert!(source.len() < 128 * 1024, "{} oversized", path.display());
    }
}
