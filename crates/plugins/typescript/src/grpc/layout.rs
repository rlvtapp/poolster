use poolster_core::native::rpc::*;
pub(super) fn type_name(full: &str, options: &RpcGenerationOptions) -> String {
    let name = if options.snake_type_names {
        full.replace('.', "_")
    } else {
        super::render::symbol(full)
    };
    format!("{}{}{}", options.type_prefix, name, options.type_suffix)
}
pub(super) fn owned(full: &str, options: &RpcGenerationOptions) -> String {
    format!("Pb_{}", type_name(full, options))
}
pub(super) fn short(full: &str, package: &str, options: &RpcGenerationOptions) -> String {
    type_name(
        full.strip_prefix(&format!("{package}.")).unwrap_or(full),
        options,
    )
}
pub(super) fn stem(proto: &str, options: &RpcGenerationOptions) -> String {
    format!(
        "{}{}",
        proto.trim_end_matches(".proto"),
        options.file_suffix
    )
}
pub(super) fn method(name: &str, options: &RpcGenerationOptions) -> String {
    if options.lower_case_service_methods {
        format!("{}{}", name[..1].to_ascii_lowercase(), &name[1..])
    } else {
        name.into()
    }
}
pub(super) fn import(from: &str, to: &str, options: &RpcGenerationOptions, flavor: &str) -> String {
    if let Some(mapped) = options.import_mappings.get(to) {
        return mapped.clone();
    }
    let target = format!("{}{flavor}.proto", stem(to, options));
    let origin = format!("{}.proto", stem(from, options));
    super::render::relative(&origin, &target)
}
