//! Kafka JSON event generation using shared-model symbols and official Kafka clients.
mod render;
mod validation;
mod workspace;
use crate::Ruby;
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile,
    blocks::ContractReference,
    engine::{Contract, Handle, Meta, Plugin, PluginContext, Provision, Requirement},
    models::{GeneratedModels, ModelOutput, ModelPlan, ModelSymbol},
    native::events::{EventAction, EventOperations},
};
use std::{collections::BTreeMap, path::PathBuf};
pub(crate) use workspace::Workspace;
pub struct AsyncApi {
    meta: Meta,
    provider: Option<Handle<EventOperations>>,
    models: Option<Handle<GeneratedModels<Ruby>>>,
}
pub fn asyncapi(provider: Option<Handle<EventOperations>>) -> AsyncApi {
    AsyncApi {
        meta: Meta::new(),
        provider,
        models: None,
    }
}
impl AsyncApi {
    pub fn using_models(mut self, models: Handle<GeneratedModels<Ruby>>) -> Self {
        self.models = Some(models);
        self
    }
    pub fn models(self, models: Handle<GeneratedModels<Ruby>>) -> Self {
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
    const NAME: &'static str = "poolster.ruby.kafka-client.v1";
}
impl Plugin<Ruby> for AsyncApi {
    fn kind(&self) -> &'static str {
        "ruby-asyncapi-kafka"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        let mut r = vec![Requirement::on(self.provider)];
        if let Some(models) = self.models {
            r.push(Requirement::on(Some(models)));
        }
        r
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<KafkaClient>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Ruby>) -> Result<()> {
        ensure!(
            !cx.workspace.native_api,
            "Ruby Kafka and HTTP/GraphQL require separate output packages"
        );
        let input = cx.inputs.get::<EventOperations>()?.clone();
        validation::check(&input)?;
        let selected = cx.inputs.reference::<EventOperations>()?.cloned();
        let source = selected
            .as_ref()
            .map(|v| v.instance.clone())
            .or_else(|| {
                self.models.and_then(|_| {
                    cx.inputs
                        .get::<GeneratedModels<Ruby>>()
                        .ok()?
                        .provenance
                        .as_ref()
                        .map(|p| p.instance.clone())
                })
            })
            .unwrap_or("events".into());
        let mut blocks = input.shared_model_blocks(source);
        if let Some(reference) = &selected {
            blocks = blocks.with_parent(reference.clone());
        }
        let plan = ModelPlan::from_shapes(&blocks)?;
        let provenance = plan.provenance.clone().expect("event blocks have parent");
        let symbols = if self.models.is_some() {
            let models = cx.inputs.get::<GeneratedModels<Ruby>>()?;
            input.resolve_models_with_reference(models, selected.as_ref())?;
            ensure!(
                models.provenance.as_ref() == Some(&provenance),
                "Ruby event shared models belong to a different contract instance or revision"
            );
            plan.entities
                .values()
                .map(|id| {
                    Ok((
                        id.local.clone(),
                        models
                            .entities
                            .get(id)
                            .ok_or_else(|| {
                                anyhow::anyhow!("missing Ruby event model {}", id.local)
                            })?
                            .clone(),
                    ))
                })
                .collect::<Result<BTreeMap<_, _>>>()?
        } else {
            let models = Ruby::emit_models(cx, &plan)?;
            plan.entities
                .iter()
                .map(|(name, id)| (id.local.clone(), models[name].clone()))
                .collect()
        };
        let package = cx
            .settings
            .package_name
            .as_deref()
            .unwrap_or("poolster_models")
            .to_owned();
        ensure!(
            !package.is_empty()
                && package
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
            "ruby Kafka package name must contain only letters, digits, hyphens and underscores"
        );
        cx.workspace.models = true;
        cx.workspace.events = true;
        ensure!(
            crate::pascal_case(&package)
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_uppercase),
            "event package namespace must start with a letter"
        );
        let operations = render::emit(cx, &input, &symbols, &package)?;
        cx.files.emit(GeneratedFile::new(
            "asyncapi.json",
            serde_json::to_string_pretty(&input.source)? + "\n",
        )?)?;
        cx.files.emit(GeneratedFile::new(
            "README.md",
            include_str!("readme.md.tmpl")
                .replace("__NAMESPACE__", &crate::pascal_case(&package))
                .replace("__LIBRARY__", &crate::snake_case(&package)),
        )?)?;
        cx.publish(KafkaClient {
            operations,
            provenance,
        })
    }
}
