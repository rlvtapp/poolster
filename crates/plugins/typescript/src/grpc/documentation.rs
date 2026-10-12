use poolster_core::native::rpc::RpcDefinitionMetadata;
pub(super) fn render(metadata: &RpcDefinitionMetadata) -> String {
    let text = metadata
        .detached_comments
        .iter()
        .map(String::as_str)
        .chain([
            metadata.leading_comments.as_str(),
            metadata.trailing_comments.as_str(),
        ])
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if text.trim().is_empty() && !metadata.deprecated {
        return String::new();
    }
    let mut out = String::from("/**\n");
    for line in text.replace("*/", "* /").lines() {
        out.push_str(&format!(" * {}\n", line.trim()));
    }
    if metadata.deprecated {
        out.push_str(" * @deprecated\n");
    }
    out.push_str(" */\n");
    out
}
