//! Field-specific immutable view declarations backed by deep native snapshots.
use anyhow::{Result, ensure};
use poolster_core::native::rpc::{
    RpcFieldKind, RpcFieldLabel, RpcGenerationOptions, RpcWireMessage, RpcWireSchema,
};
use std::collections::{BTreeMap, BTreeSet};
pub(super) fn render(
    schema: &RpcWireSchema,
    options: &RpcGenerationOptions,
) -> Result<(String, String)> {
    fn visit<'a>(
        messages: &'a [RpcWireMessage],
        output: &mut BTreeMap<String, &'a RpcWireMessage>,
    ) {
        for message in messages {
            output.insert(message.full_name.clone(), message);
            visit(&message.messages, output);
        }
    }
    let mut messages = BTreeMap::new();
    for file in &schema.files {
        visit(&file.messages, &mut messages);
    }
    let mut names = BTreeMap::new();
    let mut unique = BTreeSet::new();
    for message in messages.values().filter(|m| !m.map_entry) {
        let name = format!("Frozen_{}", message.full_name.replace('.', "_"));
        ensure!(
            unique.insert(name.clone()),
            "Python immutable view class collision: {name}"
        );
        names.insert(message.full_name.clone(), name);
    }
    fn kind(kind: &RpcFieldKind, names: &BTreeMap<String, String>) -> Result<String> {
        Ok(match kind {
            RpcFieldKind::Message(name) | RpcFieldKind::Group(name) => names
                .get(name.trim_start_matches('.'))
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Missing immutable message {name}"))?,
            RpcFieldKind::String => "str".into(),
            RpcFieldKind::Bytes => "bytes".into(),
            RpcFieldKind::Bool => "bool".into(),
            RpcFieldKind::Double | RpcFieldKind::Float => "float".into(),
            _ => "int".into(),
        })
    }
    let types = super::typing::Types::configured("", schema, options);
    let mut code = String::from("\n_FROZEN_MODELS = {}\n");
    let mut annotations = types.imports();
    for (identity, name) in &names {
        let message = messages[identity];
        let native = types.reference(identity)?;
        code.push_str(&format!(
            "\nclass {name}(FrozenMessage):\n    pass\n_FROZEN_MODELS[{}] = {name}\n",
            super::render::literal(identity)
        ));
        annotations.push_str(&format!("\nclass {name}(FrozenMessage[{native}]):\n"));
        if message.fields.is_empty() {
            annotations.push_str("    pass\n");
        }
        for field in &message.fields {
            let mut ty = kind(&field.kind, &names);
            if field.label == RpcFieldLabel::Repeated {
                if let RpcFieldKind::Message(target) = &field.kind {
                    let entry = messages[target.trim_start_matches('.')];
                    if entry.map_entry {
                        ty = Ok(format!(
                            "_typing.Mapping[{}, {}]",
                            kind(&entry.fields[0].kind, &names)?,
                            kind(&entry.fields[1].kind, &names)?
                        ));
                    } else {
                        ty = Ok(format!("tuple[{}, ...]", ty?));
                    }
                } else {
                    ty = Ok(format!("tuple[{}, ...]", ty?));
                }
            }
            annotations.push_str(&format!(
                "    @property\n    def {}(self) -> {}: ...\n",
                field.name, ty?
            ));
        }
    }
    code.push_str("\ndef freeze(value):\n    return _FROZEN_MODELS.get(value.DESCRIPTOR.full_name, FrozenMessage)(value)\n");
    for (identity, name) in &names {
        if names.len() > 1 {
            annotations.push_str("\n@_typing.overload\n");
        }
        annotations.push_str(&format!(
            "def freeze(value: {}) -> {name}: ...\n",
            types.reference(identity)?
        ));
    }
    Ok((code, annotations))
}
