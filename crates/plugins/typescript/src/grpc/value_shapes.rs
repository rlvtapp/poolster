use anyhow::Result;
use poolster_core::native::rpc::*;
use std::collections::BTreeMap;
fn scalar(
    kind: &RpcFieldKind,
    names: &BTreeMap<String, String>,
    options: &RpcGenerationOptions,
) -> Result<String> {
    let r = &options.representations;
    Ok(match kind {
        RpcFieldKind::Message(name) | RpcFieldKind::Group(name) => format!(
            "{}Value",
            names
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("Missing Protobuf value-model symbol: {name}"))?
        ),
        RpcFieldKind::Enum(_) => {
            if r.enums == RpcEnumsRepresentation::Name {
                "{name:string}|{number:number;displayName?:string}".into()
            } else {
                super::render::ty(kind, names)?
            }
        }
        RpcFieldKind::Bytes if r.bytes == RpcBytesRepresentation::Base64 => "string".into(),
        RpcFieldKind::Int64
        | RpcFieldKind::Uint64
        | RpcFieldKind::Sint64
        | RpcFieldKind::Fixed64
        | RpcFieldKind::Sfixed64 => match r.int64 {
            RpcInt64Representation::String => "string".into(),
            RpcInt64Representation::NumberExact => "number".into(),
            _ => "bigint".into(),
        },
        _ => super::render::ty(kind, names)?,
    })
}
pub(super) fn render(
    file: &RpcWireFile,
    names: &BTreeMap<String, String>,
    maps: &BTreeMap<String, &RpcWireMessage>,
    options: &RpcGenerationOptions,
    metadata: Option<&RpcFileMetadata>,
) -> Result<String> {
    let r = &options.representations;
    if r.is_native() {
        return Ok(String::new());
    }
    let runtime = options.emit_binary
        || options.emit_json
        || options.emit_clients
        || options.emit_servers
        || options.emit_reflection;
    let mut s = String::new();
    if runtime {
        s.push_str(&format!(
            "import {{valueCodec}} from {};\n",
            super::render::quote(&super::render::relative(
                &file.name,
                "poolster-rpc-values.proto"
            ))
        ));
    }
    let mut messages = vec![];
    super::render::fields(&file.messages, &mut messages);
    for message in messages {
        let name = &names[&message.full_name];
        let alias = super::layout::short(&message.full_name, &file.package, options);
        let special = if message.full_name == "google.protobuf.Timestamp"
            && r.timestamp != RpcTimestampRepresentation::Native
        {
            Some(if r.timestamp == RpcTimestampRepresentation::DateTime {
                "Date"
            } else {
                "string"
            })
        } else if message.full_name == "google.protobuf.Duration"
            && r.duration != RpcDurationRepresentation::Native
        {
            Some("number")
        } else {
            None
        };
        if let Some(ty) = special {
            s.push_str(&format!("export type {name}Value={ty};\n"));
        } else {
            s.push_str(&format!(
                "export type {name}Value={{$presence?:readonly string[];\n"
            ));
            let field_type = |field: &RpcWireField| -> Result<String> {
                let mut ty = scalar(&field.kind, names, options)?;
                if r.int64 == RpcInt64Representation::Native
                    && ty == "bigint"
                    && metadata
                        .and_then(|m| {
                            m.definition(
                                RpcDefinitionKind::Field,
                                &format!("{}.{}", message.full_name, field.name),
                            )
                        })
                        .and_then(|m| m.javascript_type)
                        == Some(RpcJavaScriptType::String)
                {
                    ty = "string".into();
                }
                Ok(ty)
            };
            for field in &message.fields {
                if field.oneof_index.is_some() && !field.proto3_optional {
                    continue;
                }
                let map = match &field.kind {
                    RpcFieldKind::Message(target) => maps.get(target),
                    _ => None,
                };
                // Synthetic map-entry messages are not exported value models.
                let mut ty = if map.is_some() {
                    String::new()
                } else {
                    field_type(field)?
                };
                if let RpcFieldKind::Message(_) = &field.kind {
                    if let Some(map) = map {
                        let key = scalar(&map.fields[0].kind, names, options)?;
                        let value = scalar(&map.fields[1].kind, names, options)?;
                        ty = match r.maps {
                            RpcMapsRepresentation::Map => format!("Map<{key},{value}>"),
                            RpcMapsRepresentation::Entries => format!("[{key},{value}][]"),
                            _ => format!(
                                "{{[key:{}]:{value}}}",
                                if key == "number" { "number" } else { "string" }
                            ),
                        };
                    } else if field.label == RpcFieldLabel::Repeated {
                        ty = format!("({ty})[]");
                    }
                } else if field.label == RpcFieldLabel::Repeated {
                    ty = format!("({ty})[]");
                }
                let optional = field.proto3_optional
                    || (field.label != RpcFieldLabel::Repeated
                        && matches!(
                            field.kind,
                            RpcFieldKind::Message(_) | RpcFieldKind::Group(_)
                        ))
                    || (file.syntax == "proto2" && field.label == RpcFieldLabel::Optional);
                if optional && r.optional == RpcOptionalRepresentation::Nullable {
                    ty = format!("({ty})|null");
                }
                s.push_str(&format!(
                    "{}{}:{ty};\n",
                    super::render::quote(&super::render::property(&field.name)),
                    if optional && r.optional != RpcOptionalRepresentation::Nullable {
                        "?"
                    } else {
                        ""
                    }
                ));
            }
            for (index, oneof) in message.oneofs.iter().enumerate() {
                let fields = message
                    .fields
                    .iter()
                    .filter(|f| f.oneof_index == Some(index) && !f.proto3_optional)
                    .collect::<Vec<_>>();
                if fields.is_empty() {
                    continue;
                }
                s.push_str(&format!(
                    "{}:",
                    super::render::quote(&super::render::property(oneof))
                ));
                for field in fields {
                    s.push_str(&format!(
                        "{{case:{};value:{}}}|",
                        super::render::quote(&super::render::ident(&field.name)),
                        field_type(field)?
                    ));
                }
                s.push_str("{case:undefined;value?:undefined};\n");
            }
            s.push_str("};\n");
        }
        if runtime {
            s.push_str(&format!(
                "export const {name}ValueCodec=valueCodec<{name},{name}Value>({name}Schema);\n"
            ));
        }
        if alias != *name {
            s.push_str(&format!("export type {{{name}Value as {alias}Value}};\n"));
            if runtime {
                s.push_str(&format!(
                    "export {{{name}ValueCodec as {alias}ValueCodec}};\n"
                ));
            }
        }
    }
    Ok(s)
}
