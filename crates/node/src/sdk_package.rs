//! Native SDK package settings shared across protocol entry points.
use super::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SdkPackage {
    #[serde(default)]
    pub(super) mode: PackageMode,
    #[serde(default)]
    pub(super) source_quality: Option<poolster_core::SourceQuality>,
    pub(super) language: String,
    #[serde(default)]
    pub(super) contract_options: super::rpc::ContractOptions,
    #[serde(default)]
    pub(super) command_name: Option<String>,
    #[serde(default)]
    pub(super) endpoint: Option<String>,
    pub(super) path: String,
    #[serde(default)]
    pub(super) name: Option<String>,
    #[serde(default)]
    pub(super) version: Option<String>,
    #[serde(default)]
    pub(super) style: Option<String>,
    #[serde(default)]
    pub(super) scalars: std::collections::BTreeMap<String, ts::GraphqlScalarMapping>,
    #[serde(default)]
    pub(super) groups:
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    #[serde(default)]
    pub(super) contracts: std::collections::BTreeMap<String, output_options::ContractOptions>,
    #[serde(default)]
    pub(super) transport: Option<String>,
    #[serde(default)]
    pub(super) client_name: Option<String>,
    #[serde(default)]
    pub(super) raw: Option<bool>,
    #[serde(default)]
    pub(super) subscriptions: Option<bool>,
    #[serde(default)]
    pub(super) jobs: Option<usize>,
    #[serde(default)]
    pub(super) plugins: Vec<NativePlugin>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct NativePlugin {
    pub(super) name: String,
    #[serde(default)]
    pub(super) query_options: Option<QueryOptions>,
    #[serde(default)]
    pub(super) fixture_options: Option<ts::FixtureOptions>,
    #[serde(default)]
    pub(super) cypress_options: Option<ts::CypressOptions>,
    #[serde(default)]
    pub(super) output: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct QueryOptions {
    pub(super) include_operations: Option<Vec<String>>,
    #[serde(default)]
    pub(super) operation_kinds: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub(super) operation_names: std::collections::BTreeMap<String, String>,
    pub(super) max_operations_per_file: Option<usize>,
}

impl QueryOptions {
    pub(super) fn apply(
        &self,
        mut query: ts::composition::Query,
    ) -> AnyResult<ts::composition::Query> {
        if let Some(ids) = &self.include_operations {
            query = query.include_operations(ids.clone());
        }
        if let Some(count) = self.max_operations_per_file {
            query = query.max_operations_per_file(count);
        }
        for (operation, kind) in &self.operation_kinds {
            query = query.operation_kind(
                operation,
                match kind.as_str() {
                    "query" => ts::composition::QueryKind::Query,
                    "mutation" => ts::composition::QueryKind::Mutation,
                    _ => bail!("operation kind must be query or mutation"),
                },
            );
        }
        for (operation, name) in &self.operation_names {
            query = query.operation_name(operation, name);
        }
        Ok(query)
    }
}

pub(super) fn package_common(package: &SdkPackage) -> AnyResult<Common> {
    let mut common = Common {
        source_quality: package.source_quality.clone(),
        ..Default::default()
    };
    if let Some(style) = package.style.as_deref() {
        common = common.client_style(match style {
            "flat" => SdkClientStyle::Flat,
            "namespaced" => SdkClientStyle::Namespaced,
            _ => bail!("SDK style must be flat or namespaced"),
        });
    }
    if let Some(version) = &package.version {
        common = common.package_version(version);
    }
    Ok(common)
}

pub(super) fn validate_options(package: &SdkPackage) -> AnyResult<()> {
    if package.contract_options.protobuf.is_some() {
        bail!("contractOptions.protobuf requires Protobuf input");
    }
    if package.command_name.is_some() || package.endpoint.is_some() {
        bail!("commandName and endpoint require GraphQL tool output");
    }
    if !package.scalars.is_empty() || !package.groups.is_empty() {
        bail!("scalar mappings and groups require GraphQL input");
    }
    if package.path.is_empty() {
        bail!("SDK package path must not be empty");
    }
    if package.language != "typescript"
        && (package.transport.is_some()
            || package.client_name.is_some()
            || package.raw.unwrap_or(false))
    {
        bail!("transport, clientName and raw are TypeScript-only SDK options");
    }
    if package.language != "go" && package.jobs.is_some() {
        bail!("jobs is a Go-only SDK option");
    }
    if package.jobs == Some(0) {
        bail!("jobs must be at least 1");
    }
    if package.language != "typescript" && !package.plugins.is_empty() {
        bail!("registered native auxiliaries currently require a TypeScript package");
    }
    Ok(())
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum PackageMode {
    #[default]
    Sdk,
    Models,
}
pub(super) fn validate_models(package: &SdkPackage) -> AnyResult<()> {
    if package.mode != PackageMode::Models {
        bail!("generateModels requires mode models");
    }
    if package.path.trim().is_empty() {
        bail!("models package path must not be empty");
    }
    if package.contract_options.protobuf.is_some()
        || package.style.is_some()
        || package.transport.is_some()
        || package.client_name.is_some()
        || package.raw.is_some()
        || package.subscriptions.is_some()
        || package.jobs.is_some()
        || package.endpoint.is_some()
        || package.command_name.is_some()
        || !package.scalars.is_empty()
        || !package.groups.is_empty()
        || !package.contracts.is_empty()
        || !package.plugins.is_empty()
    {
        bail!(
            "models mode supports only language, path, name, version and sourceQuality; SDK styles, transports, scalar mappings, groups and auxiliaries require mode sdk"
        );
    }
    Ok(())
}
