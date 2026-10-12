use super::*;
pub(super) fn local_message(c: &ModelContext<'_>, full: &str) -> bool {
    let ty = &c.plan.types[full];
    !c.plan
        .options
        .import_mappings
        .keys()
        .any(|file| c.plan.packages[file].0 == ty.import)
        && (ty.import == c.plan.module || ty.import.starts_with(&format!("{}/", c.plan.module)))
}
pub(super) fn scalar(c: &ModelContext<'_>, kind: &RpcFieldKind) -> Result<String> {
    let r = &c.plan.options.representations;
    Ok(match kind {
        RpcFieldKind::Int64
        | RpcFieldKind::Uint64
        | RpcFieldKind::Sint64
        | RpcFieldKind::Fixed64
        | RpcFieldKind::Sfixed64 => match r.int64 {
            RpcInt64Representation::Native => c.scalar(kind)?,
            RpcInt64Representation::String => "string".into(),
            RpcInt64Representation::NumberExact => "float64".into(),
        },
        RpcFieldKind::Bytes if r.bytes == RpcBytesRepresentation::Base64 => "string".into(),
        RpcFieldKind::Enum(_) if r.enums == RpcEnumsRepresentation::Name => "RpcEnumName".into(),
        RpcFieldKind::Message(full) | RpcFieldKind::Group(full) => match full.as_str() {
            "google.protobuf.Timestamp" if r.timestamp != RpcTimestampRepresentation::Native => {
                match r.timestamp {
                    RpcTimestampRepresentation::DateTime => "*time.Time".into(),
                    _ => "*string".into(),
                }
            }
            "google.protobuf.Duration" if r.duration != RpcDurationRepresentation::Native => {
                match r.duration {
                    RpcDurationRepresentation::TimeDelta => "*time.Duration".into(),
                    _ => "*float64".into(),
                }
            }
            _ if local_message(c, full) => format!("*{}Values", c.ty(full)?),
            _ => c.scalar(kind)?,
        },
        _ => c.scalar(kind)?,
    })
}
pub(super) fn field(c: &ModelContext<'_>, field: &RpcWireField) -> Result<String> {
    if let RpcFieldKind::Message(full) = &field.kind {
        if c.plan.types[full].map_entry {
            let m = &c.plan.wire_messages[full];
            let key = scalar(c, &m.fields[0].kind)?;
            let value = scalar(c, &m.fields[1].kind)?;
            return Ok(
                if c.plan.options.representations.maps == RpcMapsRepresentation::Entries {
                    format!("[]RpcEntry[{key},{value}]")
                } else {
                    format!("map[{key}]{value}")
                },
            );
        }
    }
    let ty = scalar(c, &field.kind)?;
    Ok(if field.label == RpcFieldLabel::Repeated {
        format!("[]{ty}")
    } else if c.pointer(field) {
        format!("*{ty}")
    } else {
        ty
    })
}
