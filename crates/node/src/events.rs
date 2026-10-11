//! Explicit Kafka event SDK packages reuse event-owned shared model artifacts.
use super::*;
use poolster_core::{
    input::{InputOptions, InputProvider},
    native::events::{EventOperations, model_blocks},
};
use std::sync::Arc;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    source: String,
    provider: String,
    #[serde(default)]
    broker: Option<serde_json::Value>,
    packages: Vec<SdkPackage>,
}
pub struct GenerateEvents {
    request: String,
}
impl Task for GenerateEvents {
    type Output = String;
    type JsValue = String;
    fn compute(&mut self) -> Result<String> {
        generate(&self.request).map_err(napi_anyhow)
    }
    fn resolve(&mut self, _: Env, output: String) -> Result<String> {
        Ok(output)
    }
}
fn generate(request: &str) -> AnyResult<String> {
    let request: Request = serde_json::from_str(request)?;
    if request.source.trim().is_empty() || request.provider.trim().is_empty() {
        bail!("event input requires source and provider");
    }
    let registry = Arc::new(default_registry()?);
    let mut profiles = ProfileSet::new(".");
    macro_rules! append {
        ($language:ident, $package:ident) => {{
            let input = InputProvider::<EventOperations>::new(
                registry.clone(),
                "asyncapi",
                &request.source,
            )
            .using(&request.provider)
            .with_options(InputOptions {
                broker: request.broker.clone(),
                ..Default::default()
            });
            let shapes = model_blocks(Some(input.handle()), &request.source);
            let models = $language::shared_models(None).input_shapes(shapes.handle());
            let output = $language::asyncapi(Some(input.handle())).using_models(models.handle());
            let common = Common {
                package_version: $package.version.clone(),
                source_quality: $package.source_quality.clone(),
                ..Default::default()
            };
            let mut target = $language::package(&$package.path).common(common);
            if let Some(name) = &$package.name {
                target = target.name(name);
            }
            profiles = profiles.package(target.with(output).with(models).with(shapes).with(input));
        }};
    }
    for package in request.packages {
        if package.mode != sdk_package::PackageMode::Sdk {
            bail!("event SDK entry point requires mode sdk");
        }
        if package.style.is_some()
            || package.raw.is_some()
            || package.transport.is_some()
            || package.client_name.is_some()
            || package.jobs.is_some()
            || !package.scalars.is_empty()
            || !package.groups.is_empty()
            || !package.contracts.is_empty()
            || !package.plugins.is_empty()
            || package.endpoint.is_some()
            || package.command_name.is_some()
            || package.subscriptions.is_some()
        {
            bail!(
                "Kafka event SDK packages support package metadata and sourceQuality only; configure broker on input"
            );
        }
        if package.path.trim().is_empty() {
            bail!("SDK package path must not be empty");
        }
        match package.language.as_str() {
            "typescript" => append!(ts, package),
            "go" => append!(go, package),
            "rust" => append!(rust, package),
            _ => bail!(
                "unsupported Kafka event output language {}",
                package.language
            ),
        }
    }
    let tree = poolster::generate_native(profiles)?;
    let files = tree
        .into_owned_files()
        .map(|(file, preserve_existing, owner)| OutputFile {
            path: file.path.to_string_lossy().into_owned(),
            contents: file.contents,
            preserve_existing,
            owner,
        })
        .collect::<Vec<_>>();
    Ok(serde_json::to_string(&files)?)
}
#[napi]
pub fn generate_events(request: String) -> AsyncTask<GenerateEvents> {
    AsyncTask::new(GenerateEvents { request })
}
