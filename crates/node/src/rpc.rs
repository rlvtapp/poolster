//! Protobuf packages delegate wire and RPC generation to pinned upstream tools.
use super::*;
use poolster_core::{
    input::{InputOptions, InputProvider},
    native::rpc::RpcContract,
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ContractOptions {
    pub(super) protobuf: Option<ProtobufOptions>,
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ProtobufOptions {
    module: Option<String>,
    #[serde(default)]
    package_name: Option<String>,
    #[serde(default)]
    go_packages: BTreeMap<String, String>,
    #[serde(default)]
    toolchain: Tools,
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Tools {
    protoc: Option<PathBuf>,
    protoc_gen_go: Option<PathBuf>,
    protoc_gen_go_grpc: Option<PathBuf>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    source: String,
    provider: String,
    #[serde(default)]
    import_roots: Vec<PathBuf>,
    packages: Vec<SdkPackage>,
}
pub struct GenerateRpc {
    request: String,
}
impl Task for GenerateRpc {
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
        bail!("Protobuf input requires source and provider");
    }
    let registry = Arc::new(default_registry()?);
    let mut profiles = ProfileSet::new(".");
    for package in request.packages {
        if package.mode != sdk_package::PackageMode::Sdk
            || package.style.is_some()
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
                "Protobuf SDK packages support metadata, sourceQuality and contractOptions.protobuf; HTTP/GraphQL styles are incompatible"
            );
        }
        if package.path.trim().is_empty() {
            bail!("SDK package path must not be empty");
        }
        let options = package.contract_options.protobuf.unwrap_or_default();
        let input =
            InputProvider::<RpcContract>::new(registry.clone(), "protobuf", &request.source)
                .using(&request.provider)
                .with_options(InputOptions {
                    import_roots: request.import_roots.clone(),
                    ..Default::default()
                });
        let common = Common {
            package_version: package.version,
            source_quality: package.source_quality,
            ..Default::default()
        };
        match package.language.as_str() {
            "go" => {
                if options.package_name.is_some() {
                    bail!("Go Protobuf options cannot configure Python or TypeScript tools");
                }
                let mut tools = go::GrpcToolchain::default();
                if let Some(v) = options.toolchain.protoc {
                    tools.protoc = v;
                }
                if let Some(v) = options.toolchain.protoc_gen_go {
                    tools.protoc_gen_go = v;
                }
                if let Some(v) = options.toolchain.protoc_gen_go_grpc {
                    tools.protoc_gen_go_grpc = v;
                }
                let mut output = go::grpc(
                    options
                        .module
                        .context("Go Protobuf requires contractOptions.protobuf.module")?,
                )
                .input(input.handle())
                .toolchain(tools);
                for (file, mapping) in options.go_packages {
                    output = output.go_package(file, mapping);
                }
                let mut target = go::package(&package.path).common(common);
                if let Some(name) = package.name {
                    target = target.name(name);
                }
                profiles = profiles.package(target.with(output).with(input));
            }
            "python" => {
                if options.module.is_some()
                    || !options.go_packages.is_empty()
                    || options.toolchain.protoc.is_some()
                    || options.toolchain.protoc_gen_go.is_some()
                    || options.toolchain.protoc_gen_go_grpc.is_some()
                {
                    bail!(
                        "Python Protobuf supports packageName only; upstream codegen toolchains are not used"
                    );
                }
                let output = python::grpc(
                    options
                        .package_name
                        .unwrap_or_else(|| "poolster-rpc".into()),
                )
                .input(input.handle());
                let mut target = python::package(&package.path).common(common);
                if let Some(name) = package.name {
                    target = target.name(name);
                }
                profiles = profiles.package(target.with(output).with(input));
            }
            "typescript" => {
                if options.module.is_some()
                    || options.package_name.is_some()
                    || !options.go_packages.is_empty()
                    || options.toolchain.protoc.is_some()
                    || options.toolchain.protoc_gen_go.is_some()
                    || options.toolchain.protoc_gen_go_grpc.is_some()
                {
                    bail!(
                        "TypeScript Protobuf supports package metadata only; upstream codegen toolchains are not used"
                    );
                }
                let output = ts::grpc().input(input.handle());
                let mut target = ts::package(&package.path).common(common);
                if let Some(name) = package.name {
                    target = target.name(name);
                }
                profiles = profiles.package(target.with(output).with(input));
            }
            _ => bail!("unsupported Protobuf SDK language {}", package.language),
        }
    }
    let files = poolster::generate_native(profiles)?
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
pub fn generate_rpc(request: String) -> AsyncTask<GenerateRpc> {
    AsyncTask::new(GenerateRpc { request })
}
