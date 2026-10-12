//! Private descriptor decoding and validation.
use super::*;
use anyhow::{Context, Result, bail, ensure};
use prost_types::{DescriptorProto, EnumDescriptorProto, FieldDescriptorProto};
use std::collections::{BTreeMap, BTreeSet};
pub(super) fn name(v: Option<&str>) -> Result<String> {
    let v = v.context("descriptor name missing")?;
    ensure!(
        !v.is_empty()
            && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            && !v.as_bytes()[0].is_ascii_digit(),
        "invalid descriptor identifier {v}"
    );
    Ok(v.into())
}
pub(super) fn full(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.into()
    } else {
        format!("{parent}.{name}")
    }
}
pub(super) fn enumeration(e: &EnumDescriptorProto, parent: &str) -> Result<RpcWireEnum> {
    let n = name(e.name.as_deref())?;
    let mut seen = BTreeSet::new();
    let values = e
        .value
        .iter()
        .map(|v| {
            let n = name(v.name.as_deref())?;
            ensure!(seen.insert(n.clone()), "duplicate enum value {n}");
            Ok(RpcEnumValue {
                name: n,
                number: v.number.context("enum value number missing")?,
            })
        })
        .collect::<Result<_>>()?;
    Ok(RpcWireEnum {
        name: n.clone(),
        full_name: full(parent, &n),
        values,
    })
}
pub(super) fn field(f: &FieldDescriptorProto, oneofs: usize) -> Result<RpcWireField> {
    let n = name(f.name.as_deref())?;
    let number = f.number.context("field number missing")?;
    ensure!(
        number > 0 && number <= 536870911 && !(19000..=19999).contains(&number),
        "invalid field number {number}"
    );
    let label = match f.label {
        Some(1) => RpcFieldLabel::Optional,
        Some(2) => RpcFieldLabel::Required,
        Some(3) => RpcFieldLabel::Repeated,
        _ => bail!("invalid field label for {n}"),
    };
    let reference = || -> Result<String> {
        let t = f.type_name.as_deref().context("field reference missing")?;
        ensure!(
            t.starts_with('.') && t.len() > 1,
            "field type must be fully qualified: {t}"
        );
        Ok(t.trim_start_matches('.').into())
    };
    let kind = match f.r#type {
        Some(1) => RpcFieldKind::Double,
        Some(2) => RpcFieldKind::Float,
        Some(3) => RpcFieldKind::Int64,
        Some(4) => RpcFieldKind::Uint64,
        Some(5) => RpcFieldKind::Int32,
        Some(6) => RpcFieldKind::Fixed64,
        Some(7) => RpcFieldKind::Fixed32,
        Some(8) => RpcFieldKind::Bool,
        Some(9) => RpcFieldKind::String,
        Some(10) => RpcFieldKind::Group(reference()?),
        Some(11) => RpcFieldKind::Message(reference()?),
        Some(12) => RpcFieldKind::Bytes,
        Some(13) => RpcFieldKind::Uint32,
        Some(14) => RpcFieldKind::Enum(reference()?),
        Some(15) => RpcFieldKind::Sfixed32,
        Some(16) => RpcFieldKind::Sfixed64,
        Some(17) => RpcFieldKind::Sint32,
        Some(18) => RpcFieldKind::Sint64,
        _ => bail!("unknown field type for {n}"),
    };
    let oneof_index = f
        .oneof_index
        .map(|i| {
            ensure!(
                i >= 0 && (i as usize) < oneofs,
                "invalid oneof index for {n}"
            );
            ensure!(
                label == RpcFieldLabel::Optional,
                "oneof field must be optional"
            );
            Ok(i as usize)
        })
        .transpose()?;
    ensure!(
        !f.proto3_optional.unwrap_or(false) || oneof_index.is_some(),
        "proto3 optional field missing synthetic oneof"
    );
    Ok(RpcWireField {
        name: n.clone(),
        json_name: f.json_name.clone().unwrap_or_else(|| json_name(&n)),
        number,
        label,
        kind,
        oneof_index,
        proto3_optional: f.proto3_optional.unwrap_or(false),
        default_value: f.default_value.clone(),
        extendee: f
            .extendee
            .as_ref()
            .map(|s| s.trim_start_matches('.').into()),
    })
}
pub(super) fn fields(fs: &[FieldDescriptorProto], oneofs: usize) -> Result<Vec<RpcWireField>> {
    let mut names = BTreeSet::new();
    let mut numbers = BTreeSet::new();
    fs.iter()
        .map(|f| {
            let v = field(f, oneofs)?;
            ensure!(
                names.insert(v.name.clone()) && numbers.insert(v.number),
                "duplicate message field {}",
                v.name
            );
            Ok(v)
        })
        .collect()
}
pub(super) fn message(m: &DescriptorProto, parent: &str) -> Result<RpcWireMessage> {
    let n = name(m.name.as_deref())?;
    let qualified = full(parent, &n);
    let oneofs = m
        .oneof_decl
        .iter()
        .map(|o| name(o.name.as_deref()))
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        oneofs.iter().collect::<BTreeSet<_>>().len() == oneofs.len(),
        "duplicate oneof name"
    );
    Ok(RpcWireMessage {
        name: n,
        full_name: qualified.clone(),
        fields: fields(&m.field, oneofs.len())?,
        oneofs,
        messages: m
            .nested_type
            .iter()
            .map(|m| message(m, &qualified))
            .collect::<Result<_>>()?,
        enums: m
            .enum_type
            .iter()
            .map(|e| enumeration(e, &qualified))
            .collect::<Result<_>>()?,
        map_entry: m
            .options
            .as_ref()
            .and_then(|o| o.map_entry)
            .unwrap_or(false),
        extensions: fields(&m.extension, 0)?,
    })
}
pub(super) fn descriptor_files(mut bytes: &[u8]) -> Result<Vec<Vec<u8>>> {
    fn varint(b: &mut &[u8]) -> Result<u64> {
        let mut v = 0u64;
        for shift in (0..70).step_by(7) {
            let first = *b.first().context("truncated descriptor varint")?;
            *b = &b[1..];
            ensure!(shift < 63 || first <= 1, "descriptor varint overflow");
            v |= u64::from(first & 127) << shift;
            if first < 128 {
                return Ok(v);
            }
        }
        bail!("descriptor varint overflow")
    }
    let mut files = Vec::new();
    while !bytes.is_empty() {
        ensure!(
            varint(&mut bytes)? == 10,
            "unexpected FileDescriptorSet field"
        );
        let len = usize::try_from(varint(&mut bytes)?)?;
        ensure!(bytes.len() >= len, "truncated descriptor file");
        files.push(bytes[..len].to_vec());
        bytes = &bytes[len..];
    }
    Ok(files)
}
pub(super) fn symbols(m: &RpcWireMessage, seen: &mut BTreeMap<String, bool>) -> Result<()> {
    ensure!(
        seen.insert(m.full_name.clone(), true).is_none(),
        "duplicate descriptor symbol {}",
        m.full_name
    );
    for e in &m.enums {
        ensure!(
            seen.insert(e.full_name.clone(), false).is_none(),
            "duplicate descriptor symbol {}",
            e.full_name
        );
    }
    for child in &m.messages {
        symbols(child, seen)?;
    }
    Ok(())
}
pub(super) fn validate_fields(fs: &[RpcWireField], seen: &BTreeMap<String, bool>) -> Result<()> {
    for f in fs {
        let target = match &f.kind {
            RpcFieldKind::Message(n) | RpcFieldKind::Group(n) => Some((n, true)),
            RpcFieldKind::Enum(n) => Some((n, false)),
            _ => None,
        };
        if let Some((n, message)) = target {
            ensure!(
                seen.get(n) == Some(&message),
                "unknown or incompatible field reference {n}"
            );
        }
        if let Some(n) = &f.extendee {
            ensure!(seen.get(n) == Some(&true), "unknown extension target {n}");
        }
    }
    Ok(())
}
pub(super) fn validate_message(m: &RpcWireMessage, seen: &BTreeMap<String, bool>) -> Result<()> {
    validate_fields(&m.fields, seen)?;
    validate_fields(&m.extensions, seen)?;
    for child in &m.messages {
        validate_message(child, seen)?;
    }
    Ok(())
}

pub(super) fn validate_syntax(m: &RpcWireMessage, syntax: &str) -> Result<()> {
    for f in &m.fields {
        ensure!(
            syntax != "proto3" || f.label != RpcFieldLabel::Required,
            "proto3 field cannot be required"
        );
        ensure!(
            syntax != "proto3" || f.default_value.is_none(),
            "proto3 field cannot declare a default"
        );
        ensure!(
            syntax == "proto3" || !f.proto3_optional,
            "proto3 optional flag requires proto3 syntax"
        );
        if let Some(value) = &f.default_value {
            ensure!(
                f.label != RpcFieldLabel::Repeated && f.oneof_index.is_none(),
                "repeated and oneof fields cannot declare defaults"
            );
            let valid = match &f.kind {
                RpcFieldKind::Bool => ["true", "false"].contains(&value.as_str()),
                RpcFieldKind::Int32 | RpcFieldKind::Sint32 | RpcFieldKind::Sfixed32 => {
                    value.parse::<i32>().is_ok()
                }
                RpcFieldKind::Int64 | RpcFieldKind::Sint64 | RpcFieldKind::Sfixed64 => {
                    value.parse::<i64>().is_ok()
                }
                RpcFieldKind::Uint32 | RpcFieldKind::Fixed32 => value.parse::<u32>().is_ok(),
                RpcFieldKind::Uint64 | RpcFieldKind::Fixed64 => value.parse::<u64>().is_ok(),
                RpcFieldKind::Float | RpcFieldKind::Double => {
                    ["inf", "-inf", "nan"].contains(&value.as_str()) || value.parse::<f64>().is_ok()
                }
                RpcFieldKind::Message(_) | RpcFieldKind::Group(_) => false,
                _ => true,
            };
            ensure!(valid, "invalid default value for field {}", f.name);
        }
    }
    for child in &m.messages {
        validate_syntax(child, syntax)?;
    }
    Ok(())
}

fn json_name(name: &str) -> String {
    let mut result = String::new();
    let mut upper = false;
    for c in name.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            result.push(c.to_ascii_uppercase());
            upper = false;
        } else {
            result.push(c);
        }
    }
    result
}
