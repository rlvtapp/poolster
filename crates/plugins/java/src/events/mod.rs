//! Kafka JSON event generation using shared-model symbols and official Kafka clients.
mod render;
mod validation;
use crate::Java;
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
    pub(crate) model_package: Option<String>,
    events: bool,
}
impl Workspace {
    pub(crate) fn finalize(cx: &mut FinalizeContext<'_, Java>) -> Result<()> {
        if let Some(package) = &cx.workspace.model_package {
            let mut pom = crate::docs::pom_xml(
                package,
                "poolster-models",
                cx.common.package_version.as_deref().unwrap_or("0.0.0"),
            );
            if cx.workspace.events {
                pom = pom.replace("  </dependencies>", "    <dependency><groupId>org.apache.kafka</groupId><artifactId>kafka-clients</artifactId><version>3.9.1</version></dependency>\n    <dependency><groupId>com.networknt</groupId><artifactId>json-schema-validator</artifactId><version>1.5.6</version></dependency>\n  </dependencies>");
            }
            cx.files.emit(GeneratedFile::new("pom.xml", pom)?)?;
        }
        Ok(())
    }
}
pub struct AsyncApi {
    meta: Meta,
    provider: Option<Handle<EventOperations>>,
    models: Option<Handle<GeneratedModels<Java>>>,
}
pub fn asyncapi(provider: Option<Handle<EventOperations>>) -> AsyncApi {
    AsyncApi {
        meta: Meta::new(),
        provider,
        models: None,
    }
}
impl AsyncApi {
    pub fn using_models(mut self, models: Handle<GeneratedModels<Java>>) -> Self {
        self.models = Some(models);
        self
    }
    pub fn models(self, models: Handle<GeneratedModels<Java>>) -> Self {
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
    const NAME: &'static str = "poolster.java.kafka-client.v1";
}
impl Plugin<Java> for AsyncApi {
    fn kind(&self) -> &'static str {
        "java-asyncapi-kafka"
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
    fn generate(&self, cx: &mut PluginContext<'_, Java>) -> Result<()> {
        let input = cx.inputs.get::<EventOperations>()?.clone();
        validation::check(&input)?;
        let selected = cx.inputs.reference::<EventOperations>()?.cloned();
        let source = selected
            .as_ref()
            .map(|v| v.instance.clone())
            .or_else(|| {
                self.models.and_then(|_| {
                    cx.inputs
                        .get::<GeneratedModels<Java>>()
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
            let models = cx.inputs.get::<GeneratedModels<Java>>()?;
            input.resolve_models_with_reference(models, selected.as_ref())?;
            ensure!(
                models.provenance.as_ref() == Some(&provenance),
                "Java event shared models belong to a different contract instance or revision"
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
                                anyhow::anyhow!("missing Java event model {}", id.local)
                            })?
                            .clone(),
                    ))
                })
                .collect::<Result<BTreeMap<_, _>>>()?
        } else {
            let models = Java::emit_models(cx, &plan)?;
            plan.entities
                .iter()
                .map(|(name, id)| (id.local.clone(), models[name].clone()))
                .collect()
        };
        let package = crate::naming::java_package_name(
            cx.settings
                .package_name
                .as_deref()
                .unwrap_or("io.poolster.models"),
        );
        ensure!(
            cx.workspace
                .model_package
                .as_ref()
                .is_none_or(|p| p == &package),
            "Java event shared models must be emitted in the same package"
        );
        cx.workspace.model_package = Some(package.clone());
        cx.workspace.events = true;
        let operations = render::emit(cx, &input, &symbols, &package)?;
        for (file, source) in [
            ("KafkaRuntime.java", include_str!("runtime.java.tmpl")),
            ("KafkaCodec.java", include_str!("codec.java.tmpl")),
            (
                "KafkaSubscription.java",
                include_str!("subscription.java.tmpl"),
            ),
        ] {
            cx.files.emit(GeneratedFile::new(
                format!("src/main/java/{}/events/{file}", package.replace('.', "/")),
                source.replace("__PACKAGE__", &package),
            )?)?;
        }
        cx.files.emit(GeneratedFile::new(
            "asyncapi.json",
            serde_json::to_string_pretty(&input.source)? + "\n",
        )?)?;
        cx.files.emit(GeneratedFile::new(
            "README.md",
            include_str!("readme.md.tmpl").replace("__PACKAGE__", &package),
        )?)?;
        cx.publish(KafkaClient {
            operations,
            provenance,
        })
    }
}
