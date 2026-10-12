//! Native RPC generators retain authoritative descriptor bytes and native APIs.
use super::*;
#[allow(clippy::too_many_arguments)]
pub(super) fn append(
    profiles: ProfileSet,
    input: &NativeInputConfig,
    registry: Arc<InputRegistry>,
    language: &str,
    path: &str,
    name: Option<&str>,
    common: Common,
    options: NativeOutputConfig,
) -> Result<ProfileSet> {
    let source = provider::<RpcContract>(input, registry);
    Ok(match language {
        "go" => {
            let module = options.module.context("Protobuf Go generation requires a module import path: grpc plugin.module or --module")?;
            let mut client = go::grpc(module)
                .input(source.handle())
                .toolchain(options.toolchain.into_tools());
            for (file, mapping) in options.go_packages {
                client = client.go_package(file, mapping);
            }
            profiles.package(go::package(path).common(common).with(source).with(client))
        }
        "typescript" => {
            let client = ts::grpc().input(source.handle());
            let package = ts::package(path).common(common);
            let package = if let Some(name) = name {
                package.name(name)
            } else {
                package
            };
            profiles.package(package.with(source).with(client))
        }
        "python" => {
            let client = python::grpc(name.unwrap_or("poolster-rpc")).input(source.handle());
            let package = python::package(path).common(common);
            let package = if let Some(name) = name {
                package.name(name)
            } else {
                package
            };
            profiles.package(package.with(source).with(client))
        }
        _ => bail!("unsupported Protobuf RPC output {language}"),
    })
}
