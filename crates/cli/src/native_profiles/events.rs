//! Broker clients bind their generated models to the same selected event contract.
use super::*;
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
            let source = provider::<EventOperations>(input, registry);
            let shapes = poolster_core::native::events::model_blocks(
                Some(source.handle()),
                input.path.to_string_lossy(),
            );
            let models = $target::shared_models(None).input_shapes(shapes.handle());
            let client = $target::asyncapi(Some(source.handle())).using_models(models.handle());
            let package = $target::package(path).common(common);
            let package = if let Some(name) = name {
                $target::PackageExt::name(package, name)
            } else {
                package
            };
            profiles.package(package.with(source).with(shapes).with(models).with(client))
        }};
    }
    Ok(match language {
        "typescript" => output!(ts),
        "go" => output!(go),
        "rust" => output!(rust),
        "python" => output!(python),
        "java" => output!(java),
        _ => bail!("unsupported Kafka event output {language}"),
    })
}
