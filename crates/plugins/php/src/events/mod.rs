//! Kafka JSON event packages backed by php-rdkafka and Opis JSON Schema.
mod render;
mod validation;
use crate::{Php, namespace_for_package};
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile, GeneratedTree,
    blocks::ContractReference,
    engine::{
        Contract, FinalizeContext, Handle, Meta, Plugin, PluginContext, Provision, Requirement,
    },
    models::{GeneratedModels, ModelOutput, ModelPlan, ModelSymbol},
    native::events::{EventAction, EventOperations},
};
use std::{collections::BTreeMap, path::PathBuf};
#[derive(Default)]
pub struct Workspace {
    pub(crate) models: bool,
    events: bool,
}
impl Workspace {
    pub(crate) fn finalize(cx: &mut FinalizeContext<'_, Php>) -> Result<()> {
        if cx.workspace.models {
            cx.files.emit(GeneratedFile::new(
                "composer.json",
                manifest(
                    cx.settings
                        .package_name
                        .as_deref()
                        .unwrap_or("poolster/models"),
                    cx.workspace.events,
                )?,
            )?)?;
        }
        Ok(())
    }
}
fn manifest(package: &str, events: bool) -> Result<String> {
    let namespace = namespace_for_package(package);
    let mut value = serde_json::json!({"name":package,"type":"library","require":{"php":">=8.2"},"autoload":{"psr-4":{format!("{namespace}\\"):"src/"}}});
    if events {
        value["require"]["ext-rdkafka"] = "6.0.5".into();
        value["require"]["opis/json-schema"] = "2.6.0".into();
    }
    Ok(serde_json::to_string_pretty(&value)? + "\n")
}
pub struct AsyncApi {
    meta: Meta,
    provider: Option<Handle<EventOperations>>,
    models: Option<Handle<GeneratedModels<Php>>>,
}
pub fn asyncapi(provider: Option<Handle<EventOperations>>) -> AsyncApi {
    AsyncApi {
        meta: Meta::new(),
        provider,
        models: None,
    }
}
impl AsyncApi {
    pub fn using_models(mut self, models: Handle<GeneratedModels<Php>>) -> Self {
        self.models = Some(models);
        self
    }
    pub fn models(self, models: Handle<GeneratedModels<Php>>) -> Self {
        self.using_models(models)
    }
    pub fn handle(&self) -> Handle<KafkaClient> {
        self.meta.handle()
    }
}
#[derive(Clone, Debug)]
pub struct KafkaOperationSymbols {
    pub method: String,
    pub module: PathBuf,
    pub payload: ModelSymbol,
    pub headers: Option<ModelSymbol>,
    pub key: Option<ModelSymbol>,
    pub action: EventAction,
}
#[derive(Clone, Debug)]
pub struct KafkaClient {
    pub operations: BTreeMap<String, KafkaOperationSymbols>,
    pub provenance: ContractReference,
}
impl Contract for KafkaClient {
    const NAME: &'static str = "poolster.php.kafka-client.v1";
}
impl Plugin<Php> for AsyncApi {
    fn kind(&self) -> &'static str {
        "php-asyncapi-kafka"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        let mut r = vec![Requirement::on(self.provider)];
        if let Some(m) = self.models {
            r.push(Requirement::on(Some(m)));
        }
        r
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<KafkaClient>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Php>) -> Result<()> {
        let input = cx.inputs.get::<EventOperations>()?.clone();
        validation::check(&input)?;
        let selected = cx.inputs.reference::<EventOperations>()?.cloned();
        let source = selected
            .as_ref()
            .map(|v| v.instance.clone())
            .or_else(|| {
                self.models.and_then(|_| {
                    cx.inputs
                        .get::<GeneratedModels<Php>>()
                        .ok()?
                        .provenance
                        .as_ref()
                        .map(|p| p.instance.clone())
                })
            })
            .unwrap_or("events".into());
        let mut blocks = input.shared_model_blocks(source);
        if let Some(r) = &selected {
            blocks = blocks.with_parent(r.clone());
        }
        let plan = ModelPlan::from_shapes(&blocks)?;
        let provenance = plan
            .provenance
            .clone()
            .expect("event blocks carry provenance");
        let symbols = if self.models.is_some() {
            let models = cx.inputs.get::<GeneratedModels<Php>>()?;
            input.resolve_models_with_reference(models, selected.as_ref())?;
            ensure!(
                models.provenance.as_ref() == Some(&provenance),
                "PHP event models belong to another contract instance or revision"
            );
            plan.entities
                .values()
                .map(|id| {
                    Ok((
                        id.local.clone(),
                        models
                            .entities
                            .get(id)
                            .ok_or_else(|| anyhow::anyhow!("missing PHP event model {}", id.local))?
                            .clone(),
                    ))
                })
                .collect::<Result<BTreeMap<_, _>>>()?
        } else {
            let emitted = Php::emit_models(cx, &plan)?;
            plan.entities
                .iter()
                .map(|(name, id)| (id.local.clone(), emitted[name].clone()))
                .collect()
        };
        let namespace = namespace_for_package(
            cx.settings
                .package_name
                .as_deref()
                .unwrap_or("poolster/models"),
        );
        cx.workspace.models = true;
        cx.workspace.events = true;
        let mut tree = GeneratedTree::default();
        let operations = render::emit(&mut tree, &input, &symbols, &namespace)?;
        cx.files.append(tree)?;
        cx.publish(KafkaClient {
            operations,
            provenance,
        })
    }
}
/// Standalone renderer used by integrations that delegate to the PHP package.
pub fn render_event_package(input: &EventOperations, package: &str) -> Result<GeneratedTree> {
    render_event_package_with_reference(input, package, None)
}
/// Delegate without losing the selected input instance and opaque revision.
pub fn render_event_package_with_reference(
    input: &EventOperations,
    package: &str,
    reference: Option<ContractReference>,
) -> Result<GeneratedTree> {
    use poolster_core::engine::Packages;
    struct Provider {
        meta: Meta,
        input: EventOperations,
        reference: Option<ContractReference>,
    }
    impl Plugin<Php> for Provider {
        fn kind(&self) -> &'static str {
            "php-events-delegated-input"
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
            if let Some(reference) = &self.reference {
                cx.publish_with_reference(self.input.clone(), reference.clone())
            } else {
                cx.publish(self.input.clone())
            }
        }
    }
    use crate::PackageExt;
    let source = reference
        .as_ref()
        .map(|r| r.instance.clone())
        .unwrap_or("events".into());
    let provider = Provider {
        meta: Meta::new(),
        input: input.clone(),
        reference,
    };
    let handle = provider.meta.handle();
    let shapes = poolster_core::native::events::model_blocks(Some(handle), source);
    let models = crate::shared_models(None).input_shapes(shapes.handle());
    let events = asyncapi(Some(handle)).using_models(models.handle());
    let generated = Packages::new()
        .package(
            crate::package("sdk")
                .name(package)
                .with(provider)
                .with(shapes)
                .with(models)
                .with(events),
        )
        .generate_native()?;
    let mut tree = GeneratedTree::default();
    for (file, _, _) in generated.into_owned_files() {
        let path = file.path.strip_prefix("sdk")?;
        tree.insert(GeneratedFile::new(path, file.contents)?)?;
    }
    Ok(tree)
}
#[cfg(test)]
mod tests;
