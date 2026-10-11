//! Native model-only packages consume explicit building blocks, never protocol SDKs.
use super::*;
use poolster_core::{
    blocks::{Blocks, ModelBlock},
    input::{InputOptions, InputProvider},
};
use std::sync::Arc;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    format: String,
    source: String,
    provider: String,
    #[serde(default)]
    operation_files: Vec<String>,
    #[serde(default)]
    import_roots: Vec<String>,
    packages: Vec<SdkPackage>,
}
pub struct GenerateModels {
    request: String,
}
impl Task for GenerateModels {
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
    if !["graphql", "asyncapi"].contains(&request.format.as_str()) {
        bail!("models mode supports native GraphQL and AsyncAPI inputs");
    }
    if request.source.trim().is_empty() || request.provider.trim().is_empty() {
        bail!("models input requires source and provider");
    }
    if request.format == "asyncapi"
        && (!request.operation_files.is_empty() || !request.import_roots.is_empty())
    {
        bail!("AsyncAPI model input does not support GraphQL operationFiles/importRoots");
    }
    let registry = Arc::new(default_registry()?);
    let mut profiles = ProfileSet::new(".");
    macro_rules! append {
        ($language:ident,$package:ident) => {{
            let common = Common {
                package_version: $package.version.clone(),
                source_quality: $package.source_quality.clone(),
                ..Default::default()
            };
            let mut target = $language::package(&$package.path).common(common);
            if let Some(name) = &$package.name {
                target = target.name(name);
            }
            if request.format == "graphql" {
                let input = InputProvider::<Blocks<ModelBlock>>::new(
                    registry.clone(),
                    "graphql",
                    &request.source,
                )
                .using(&request.provider)
                .with_options(InputOptions {
                    operation_files: request.operation_files.iter().map(Into::into).collect(),
                    import_roots: request.import_roots.iter().map(Into::into).collect(),
                    ..Default::default()
                });
                let output = $language::shared_models(Some(input.handle()));
                profiles = profiles.package(target.with(output).with(input));
            } else {
                let input =
                    InputProvider::<poolster_inputs::asyncapi::contracts::MessageBlocks>::new(
                        registry.clone(),
                        "asyncapi",
                        &request.source,
                    )
                    .using(&request.provider);
                let shapes = poolster_inputs::asyncapi::shared_model_blocks(Some(input.handle()));
                let output = $language::shared_models(None).input_shapes(shapes.handle());
                profiles = profiles.package(target.with(output).with(shapes).with(input));
            }
        }};
    }
    for package in request.packages {
        sdk_package::validate_models(&package)?;
        match package.language.as_str() {
            "typescript" => append!(ts, package),
            "rust" => append!(rust, package),
            "go" => append!(go, package),
            "python" => append!(python, package),
            "php" => append!(php, package),
            "java" => append!(java, package),
            "csharp" => append!(csharp, package),
            "ruby" => append!(ruby, package),
            "swift" => append!(swift, package),
            "elixir" => append!(elixir, package),
            _ => bail!("unsupported models output language {}", package.language),
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
pub fn generate_models(request: String) -> AsyncTask<GenerateModels> {
    AsyncTask::new(GenerateModels { request })
}
