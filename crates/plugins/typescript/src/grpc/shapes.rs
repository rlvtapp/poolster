use anyhow::{Result, ensure};
use poolster_core::native::rpc::*;
use std::collections::BTreeMap;
pub(super) fn render(
    f: &RpcWireFile,
    names: &BTreeMap<String, String>,
    maps: &BTreeMap<String, &RpcWireMessage>,
    options: &RpcGenerationOptions,
    metadata: Option<&RpcFileMetadata>,
) -> Result<String> {
    let runtime = options.emit_binary
        || options.emit_json
        || options.emit_clients
        || options.emit_servers
        || options.emit_reflection;
    let documentation = |kind, full: &str| {
        if !options.comments {
            return String::new();
        }
        metadata
            .and_then(|m| m.definition(kind, full))
            .map(super::documentation::render)
            .unwrap_or_default()
    };
    let field_type = |field: &RpcWireField, parent: &str| {
        super::render::field_ty(
            &field.kind,
            names,
            metadata
                .and_then(|m| {
                    m.definition(
                        RpcDefinitionKind::Field,
                        &format!("{parent}.{}", field.name),
                    )
                })
                .and_then(|m| m.javascript_type),
        )
    };
    let mut s = String::new();
    let mut es = f.enums.iter().collect::<Vec<_>>();
    super::render::enums(&f.messages, &mut es);
    for e in es {
        let n = &names[&e.full_name];
        s.push_str(&documentation(RpcDefinitionKind::Enum, &e.full_name));
        let mut values = Vec::new();
        let mut members = std::collections::BTreeSet::new();
        let prefix = format!("{}_", e.name.to_ascii_uppercase());
        for v in &e.values {
            let member = if options.remove_enum_prefix {
                v.name.strip_prefix(&prefix).unwrap_or(&v.name)
            } else {
                &v.name
            };
            ensure!(
                members.insert(member.to_owned()),
                "Protobuf enum members collide after prefix removal: {}.{member}",
                e.full_name
            );
            values.push((
                member,
                v.number,
                documentation(
                    RpcDefinitionKind::EnumValue,
                    &format!("{}.{}", e.full_name, v.name),
                ),
            ));
        }
        match options.enum_style {
            RpcEnumStyle::Literal => {
                if runtime {
                    s.push_str(&format!("export const {n}={{\n"));
                    for (member, number, doc) in &values {
                        s.push_str(&format!(
                            "{doc}{}:{number},\n",
                            super::render::quote(member)
                        ));
                    }
                    s.push_str("} as const;\n");
                } else {
                    s.push_str(&format!("export declare const {n}:{{\n"));
                    for (member, number, doc) in &values {
                        s.push_str(&format!(
                            "{doc}readonly {}:{number};\n",
                            super::render::quote(member)
                        ));
                    }
                    s.push_str("};\n");
                }
                s.push_str(&format!(
                    "export type {n}=(typeof {n})[keyof typeof {n}]|number;\n"
                ));
            }
            style => {
                s.push_str(&format!(
                    "export {}{}enum {n} {{\n",
                    if runtime { "" } else { "declare " },
                    if style == RpcEnumStyle::Const {
                        "const "
                    } else {
                        ""
                    }
                ));
                for (member, number, doc) in &values {
                    s.push_str(&format!(
                        "{doc}{}={number},\n",
                        super::render::quote(member)
                    ));
                }
                s.push_str("}\n");
            }
        }
        if runtime {
            s.push_str(&format!(
                "export function {n}Name(value:number):string{{switch(value){{"
            ));
            for (member, number, _) in &values {
                s.push_str(&format!(
                    "case {number}:return {};",
                    super::render::quote(member)
                ));
            }
            s.push_str(&format!(
                "default:return {};}}}}\n",
                super::render::quote(&options.unknown_enum_name)
            ));
        }
        if runtime && options.emit_reflection {
            s.push_str(&format!(
                "export const {n}Schema=registry.getEnum({})!;\n",
                super::render::quote(&e.full_name)
            ));
        }
    }
    let mut ms = vec![];
    super::render::fields(&f.messages, &mut ms);
    for m in ms {
        let n = &names[&m.full_name];
        s.push_str(&documentation(RpcDefinitionKind::Message, &m.full_name));
        s.push_str(&format!(
            "export type {n}=Message<{}>&{{\n",
            super::render::quote(&m.full_name)
        ));
        for field in &m.fields {
            s.push_str(&documentation(
                RpcDefinitionKind::Field,
                &format!("{}.{}", m.full_name, field.name),
            ));
            if field.oneof_index.is_some() && !field.proto3_optional {
                continue;
            }
            let mut t = if matches!(&field.kind,RpcFieldKind::Message(target) if maps.contains_key(target))
            {
                String::new()
            } else {
                field_type(field, &m.full_name)?
            };
            if let RpcFieldKind::Message(ref target) = field.kind {
                if let Some(map) = maps.get(target) {
                    ensure!(map.fields.len() == 2, "malformed Protobuf map entry");
                    let k = field_type(&map.fields[0], &map.full_name)?;
                    let v = field_type(&map.fields[1], &map.full_name)?;
                    t = format!(
                        "{{[key:{}]:{v}}}",
                        if k == "number" { "number" } else { "string" }
                    );
                } else if field.label == RpcFieldLabel::Repeated {
                    t.push_str("[]");
                }
            } else if field.label == RpcFieldLabel::Repeated {
                t.push_str("[]");
            }
            let optional = field.proto3_optional
                || (field.label != RpcFieldLabel::Repeated
                    && matches!(
                        field.kind,
                        RpcFieldKind::Message(_) | RpcFieldKind::Group(_)
                    ))
                || (f.syntax == "proto2" && field.label == RpcFieldLabel::Optional);
            s.push_str(&format!(
                "{}{}:{t};\n",
                super::render::quote(&super::render::property(&field.name)),
                if optional { "?" } else { "" }
            ));
        }
        for (i, oneof) in m.oneofs.iter().enumerate() {
            let fs = m
                .fields
                .iter()
                .filter(|x| x.oneof_index == Some(i) && !x.proto3_optional)
                .collect::<Vec<_>>();
            if fs.is_empty() {
                continue;
            }
            s.push_str(&format!(
                "{}:",
                super::render::quote(&super::render::property(oneof))
            ));
            for field in fs {
                s.push_str(&format!(
                    "{{case:{};value:{}}}|",
                    super::render::quote(&super::render::ident(&field.name)),
                    field_type(field, &m.full_name)?
                ));
            }
            s.push_str("{case:undefined;value?:undefined};\n");
        }
        s.push_str("};\n");
        if runtime {
            s.push_str(&format!(
                "{}const {n}Schema=registry.getMessage({})! as GenMessage<{n}>;\n",
                if options.emit_reflection {
                    "export "
                } else {
                    ""
                },
                super::render::quote(&m.full_name)
            ));
            s.push_str(&super::helpers::message(n, &m.full_name, options));
        }
    }
    Ok(s)
}
