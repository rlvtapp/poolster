//! Models-only packages select block collections, without protocol client generation.
use super::*;
use poolster_core::blocks::{Blocks, ModelBlock};

pub(super) fn compatible(format: &str, package: &PackageConfig) -> bool {
    ["graphql", "asyncapi"].contains(&format)
        && [
            "typescript",
            "rust",
            "go",
            "python",
            "php",
            "java",
            "csharp",
            "ruby",
            "swift",
            "elixir",
        ]
        .contains(&package.language.as_str())
        && package.plugins.len() == 1
        && package.plugins[0].name == "shared-models"
}

pub(super) fn validate_options(raw: &serde_json::Value, package: &PackageConfig) -> Result<()> {
    ensure!(
        package.client_style.is_none() && !package.sdk_raw,
        "models-only packages do not accept client style or raw options"
    );
    for key in raw.as_object().context("package must be an object")?.keys() {
        ensure!(
            [
                "language",
                "path",
                "name",
                "version",
                "plugins",
                "source_quality"
            ]
            .contains(&key.as_str()),
            "models-only package option {key:?} is unsupported"
        );
    }
    let plugin = raw["plugins"][0]
        .as_object()
        .context("plugin must be an object")?;
    ensure!(
        plugin.len() == 1 && plugin.contains_key("name"),
        "shared-models does not accept client, transport or scalar options"
    );
    Ok(())
}

pub(super) fn append(
    profiles: ProfileSet,
    input: &NativeInputConfig,
    registry: Arc<InputRegistry>,
    language: &str,
    path: &str,
    name: Option<&str>,
    common: Common,
) -> Result<ProfileSet> {
    macro_rules! output {
        ($target:ident) => {{
            let package = $target::package(path).common(common);
            let package = if let Some(name) = name {
                $target::PackageExt::name(package, name)
            } else {
                package
            };
            match input.format.as_str() {
                "graphql" => {
                    let source = provider::<Blocks<ModelBlock>>(input, registry);
                    let output = $target::shared_models(Some(source.handle()));
                    profiles.package(package.with(source).with(output))
                }
                "asyncapi" => {
                    let source = provider::<poolster_inputs::asyncapi::contracts::MessageBlocks>(
                        input, registry,
                    );
                    let shapes =
                        poolster_inputs::asyncapi::shared_model_blocks(Some(source.handle()));
                    let output = $target::shared_models(None).input_shapes(shapes.handle());
                    profiles.package(package.with(source).with(shapes).with(output))
                }
                _ => bail!("models-only generation requires GraphQL or AsyncAPI model blocks"),
            }
        }};
    }
    Ok(match language {
        "typescript" => output!(ts),
        "rust" => output!(rust),
        "go" => output!(go),
        "python" => output!(python),
        "php" => output!(php),
        "java" => output!(java),
        "csharp" => output!(csharp),
        "ruby" => output!(ruby),
        "swift" => output!(swift),
        "elixir" => output!(elixir),
        _ => bail!("unsupported models-only language {language:?}"),
    })
}
