use super::*;
fn portable_name(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.contains(['\\', ':', '\0', '\r', '\n', ' ', '='])
        && value.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        })
}
#[cfg(test)]
pub(super) fn validate(
    contract: &RpcContract,
    module: &str,
    mappings: &BTreeMap<String, String>,
) -> Result<Vec<String>> {
    validate_options(contract, module, mappings, &Default::default())
}
pub(super) fn validate_options(
    contract: &RpcContract,
    module: &str,
    mappings: &BTreeMap<String, String>,
    options: &poolster_core::native::rpc::RpcGenerationOptions,
) -> Result<Vec<String>> {
    ensure!(
        portable_name(module),
        "gRPC Go module must be a portable import path"
    );
    ensure!(
        !contract.root_files.is_empty(),
        "gRPC input has no root files"
    );
    ensure!(
        !contract.descriptor_set.is_empty(),
        "gRPC input has no descriptor set"
    );
    let mut generation = Vec::new();
    for root in &contract.root_files {
        ensure!(
            contract.files.iter().any(|f| &f.name == root),
            "missing root descriptor {root}"
        );
    }
    for name in mappings.keys() {
        ensure!(
            contract.files.iter().any(|f| &f.name == name),
            "Go package mapping refers to unknown file {name}"
        );
    }
    let mut package_names = BTreeMap::new();
    for file in &contract.files {
        ensure!(
            portable_name(&file.name) && file.name.ends_with(".proto"),
            "unsafe Protobuf file name {:?}",
            file.name
        );
        // Google's built-ins come from the pinned protobuf runtime, not this module.
        if file.name.starts_with("google/protobuf/") && file.source.is_none() {
            ensure!(
                !mappings.contains_key(&file.name),
                "overriding built-in Go package mappings is unsupported: {}",
                file.name
            );
            continue;
        }
        let mapping = mappings
            .get(&file.name)
            .or(file.go_package.as_ref())
            .with_context(|| {
                format!(
                    "{} needs option go_package or an explicit Go package mapping",
                    file.name
                )
            })?;
        let mut parts = mapping.split(';');
        let import = parts.next().unwrap();
        let package = parts.next();
        ensure!(
            parts.next().is_none() && portable_name(import),
            "invalid Go package mapping for {}",
            file.name
        );
        if let Some(package) = package {
            ensure!(
                !package.is_empty()
                    && package.chars().enumerate().all(|(i, c)| c == '_'
                        || c.is_ascii_alphabetic()
                        || (i > 0 && c.is_ascii_digit())),
                "invalid Go package name for {}",
                file.name
            );
        }
        if options.import_mappings.contains_key(&file.name) {
            continue;
        }
        ensure!(
            options.emit_imported_files || contract.root_files.contains(&file.name),
            "Go emitImportedFiles=false requires an explicit external importMappings entry for {}",
            file.name
        );
        ensure!(
            import == module || import.starts_with(&format!("{module}/")),
            "Go package for {} ({import}) lies outside module {module}",
            file.name
        );
        if let Some(package) = package {
            if let Some(old) = package_names.insert(import, package) {
                ensure!(old == package, "conflicting Go package names for {import}");
            }
        }
        generation.push(file.name.clone());
    }
    generation.sort();
    generation.dedup();
    ensure!(
        !generation.is_empty(),
        "gRPC input has no local files to generate"
    );
    Ok(generation)
}
