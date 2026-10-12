use anyhow::{ensure, Result};
use poolster_core::native::rpc::*;
use std::collections::{BTreeMap, BTreeSet};
pub(super) fn render(
    file: &RpcWireFile,
    names: &BTreeMap<String, String>,
    metadata: Option<&RpcFileMetadata>,
) -> Result<String> {
    let mut fields = Vec::new();
    fn messages<'a>(items: &'a [RpcWireMessage], out: &mut Vec<(String, &'a RpcWireField)>) {
        for m in items {
            out.extend(
                m.extensions
                    .iter()
                    .map(|f| (format!("{}.{}", m.full_name, f.name), f)),
            );
            messages(&m.messages, out);
        }
    }
    fields.extend(file.extensions.iter().map(|f| {
        (
            if file.package.is_empty() {
                f.name.clone()
            } else {
                format!("{}.{}", file.package, f.name)
            },
            f,
        )
    }));
    messages(&file.messages, &mut fields);
    let mut reserved = names.values().cloned().collect::<BTreeSet<_>>();
    let mut out = String::new();
    for (full, field) in fields {
        let name = format!("Ext_{}", super::render::symbol(&full));
        ensure!(
            reserved.insert(name.clone()),
            "Protobuf extension symbols collide: {full}"
        );
        let extendee = field
            .extendee
            .as_ref()
            .and_then(|v| names.get(v))
            .ok_or_else(|| anyhow::anyhow!("unknown extension target {full}"))?;
        let mut ty = super::render::field_ty(
            &field.kind,
            names,
            metadata
                .and_then(|m| m.definition(RpcDefinitionKind::Extension, &full))
                .and_then(|m| m.javascript_type),
        )?;
        if field.label == RpcFieldLabel::Repeated {
            ty.push_str("[]");
        }
        out.push_str(&format!(
            "export const {name}=registry.getExtension({})! as GenExtension<{extendee},{ty}>;\n",
            serde_json::to_string(&full)?
        ));
    }
    Ok(out)
}
