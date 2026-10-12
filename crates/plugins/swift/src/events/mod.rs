//! Kafka JSON event generation using shared-model symbols and official Kafka clients.
mod render;
mod validation;
use crate::Swift;
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile,
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
    pub(crate) events: bool,
    pub(crate) native_api: bool,
}
impl Workspace {
    pub(crate) fn finalize(cx: &mut FinalizeContext<'_, Swift>) -> Result<()> {
        if cx.workspace.models {
            let module = crate::http::type_name(
                cx.settings
                    .package_name
                    .as_deref()
                    .unwrap_or("PoolsterModels"),
            );
            let manifest = if cx.workspace.events {
                include_str!("package.swift.tmpl").replace("__MODULE__", &module)
            } else {
                crate::http::package_manifest(&module, &module)
            };
            cx.files
                .emit(GeneratedFile::new("Package.swift", manifest)?)?;
        }
        Ok(())
    }
}
pub struct AsyncApi {
    meta: Meta,
    provider: Option<Handle<EventOperations>>,
    models: Option<Handle<GeneratedModels<Swift>>>,
}
pub fn asyncapi(provider: Option<Handle<EventOperations>>) -> AsyncApi {
    AsyncApi {
        meta: Meta::new(),
        provider,
        models: None,
    }
}
impl AsyncApi {
    pub fn using_models(mut self, models: Handle<GeneratedModels<Swift>>) -> Self {
        self.models = Some(models);
        self
    }
    pub fn models(self, models: Handle<GeneratedModels<Swift>>) -> Self {
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
    const NAME: &'static str = "poolster.swift.kafka-client.v1";
}
impl Plugin<Swift> for AsyncApi {
    fn kind(&self) -> &'static str {
        "swift-asyncapi-kafka"
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
    fn generate(&self, cx: &mut PluginContext<'_, Swift>) -> Result<()> {
        ensure!(
            !cx.workspace.native_api,
            "Swift Kafka and HTTP/GraphQL require separate output packages"
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
                        .get::<GeneratedModels<Swift>>()
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
            let models = cx.inputs.get::<GeneratedModels<Swift>>()?;
            input.resolve_models_with_reference(models, selected.as_ref())?;
            ensure!(
                models.provenance.as_ref() == Some(&provenance),
                "Swift event shared models belong to a different contract instance or revision"
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
                                anyhow::anyhow!("missing Swift event model {}", id.local)
                            })?
                            .clone(),
                    ))
                })
                .collect::<Result<BTreeMap<_, _>>>()?
        } else {
            let models = Swift::emit_models(cx, &plan)?;
            plan.entities
                .iter()
                .map(|(name, id)| (id.local.clone(), models[name].clone()))
                .collect()
        };
        let package = crate::http::type_name(
            cx.settings
                .package_name
                .as_deref()
                .unwrap_or("PoolsterModels"),
        );
        cx.workspace.models = true;
        cx.workspace.events = true;
        let operations = render::emit(cx, &input, &symbols, &package)?;
        for (file, source) in [
            ("KafkaRuntime.swift", include_str!("runtime.swift.tmpl")),
            ("KafkaCodec.swift", include_str!("codec.swift.tmpl")),
            ("KafkaSchema.swift", include_str!("schema.swift.tmpl")),
            (
                "KafkaSubscription.swift",
                include_str!("subscription.swift.tmpl"),
            ),
        ] {
            cx.files.emit(GeneratedFile::new(
                format!("Sources/{package}/{file}"),
                source,
            )?)?;
        }
        for (file, source) in [
            (
                "Sources/CPoolsterKafka/module.modulemap",
                include_str!("module.modulemap.tmpl"),
            ),
            (
                "Sources/CPoolsterKafka/shim.h",
                include_str!("system.h.tmpl"),
            ),
            (
                "Sources/PoolsterKafkaShim/include/poolster_kafka.h",
                include_str!("shim.h.tmpl"),
            ),
            (
                "Sources/PoolsterKafkaShim/poolster_kafka.c",
                include_str!("shim.c.tmpl"),
            ),
        ] {
            cx.files.emit(GeneratedFile::new(file, source)?)?;
        }
        cx.files.emit(GeneratedFile::new(
            "asyncapi.json",
            serde_json::to_string_pretty(&input.source)? + "\n",
        )?)?;
        cx.files.emit(GeneratedFile::new(
            "README.md",
            include_str!("readme.md.tmpl").replace("__NAMESPACE__", &package),
        )?)?;
        cx.publish(KafkaClient {
            operations,
            provenance,
        })
    }
}
