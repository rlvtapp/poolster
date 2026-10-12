use super::*;
pub(super) fn callback(
    c: &ModelContext<'_>,
    kind: &RpcFieldKind,
    to_native: bool,
) -> Result<String> {
    let native = c.scalar(kind)?;
    let value = types::scalar(c, kind)?;
    let (input, output) = if to_native {
        (&value, &native)
    } else {
        (&native, &value)
    };
    let r = &c.plan.options.representations;
    let body = match kind {
        RpcFieldKind::Int64
        | RpcFieldKind::Uint64
        | RpcFieldKind::Sint64
        | RpcFieldKind::Fixed64
        | RpcFieldKind::Sfixed64 => {
            let unsigned = matches!(kind, RpcFieldKind::Uint64 | RpcFieldKind::Fixed64);
            let base = if unsigned { "Uint64" } else { "Int64" };
            match (r.int64, to_native) {
                (RpcInt64Representation::Native, _) => "return value,nil".into(),
                (RpcInt64Representation::String, false) => {
                    format!("return {base}String(value),nil")
                }
                (RpcInt64Representation::String, true) => {
                    format!("return Parse{base}String(value)")
                }
                (RpcInt64Representation::NumberExact, false) => {
                    format!("return {base}Float64(value)")
                }
                (RpcInt64Representation::NumberExact, true) => {
                    format!("return Float64{base}(value)")
                }
            }
        }
        RpcFieldKind::Bytes => {
            if r.bytes == RpcBytesRepresentation::Base64 {
                if to_native {
                    "return ParseBytesBase64(value)".into()
                } else {
                    "return BytesBase64(value),nil".into()
                }
            } else {
                "return append([]byte(nil),value...),nil".into()
            }
        }
        RpcFieldKind::Enum(full) if r.enums == RpcEnumsRepresentation::Name => {
            let ty = c.ty(full)?;
            if to_native {
                format!(
                    "if value.Name!=\"\"{{number,ok:={ty}_value[value.Name];if !ok{{return 0,fmt.Errorf(\"unknown enum name %s\",value.Name)}};if number!=value.Number{{return 0,fmt.Errorf(\"enum name/number mismatch\")}};return {ty}(number),nil}};return {ty}(value.Number),nil"
                )
            } else {
                format!(
                    "label,known:={ty}_name[int32(value)];display:=label;if !known{{display={}}};return RpcEnumName{{Name:label,Number:int32(value),DisplayName:display}},nil",
                    serde_json::to_string(&c.plan.options.unknown_enum_name)?
                )
            }
        }
        RpcFieldKind::Message(full) | RpcFieldKind::Group(full) => match full.as_str() {
            "google.protobuf.Timestamp" if r.timestamp != RpcTimestampRepresentation::Native => {
                if to_native {
                    if r.timestamp == RpcTimestampRepresentation::DateTime {
                        "if value==nil{return nil,nil};return TimeTimestamp(*value)".into()
                    } else {
                        "if value==nil{return nil,nil};parsed,err:=time.Parse(time.RFC3339Nano,*value);if err!=nil{return nil,err};return TimeTimestamp(parsed)".into()
                    }
                } else {
                    if r.timestamp == RpcTimestampRepresentation::DateTime {
                        "if value==nil{return nil,nil};converted,err:=TimestampTime(value);return &converted,err".into()
                    } else {
                        "if value==nil{return nil,nil};converted,err:=TimestampTime(value);if err!=nil{return nil,err};text:=converted.UTC().Format(time.RFC3339Nano);return &text,nil".into()
                    }
                }
            }
            "google.protobuf.Duration" if r.duration != RpcDurationRepresentation::Native => {
                if to_native {
                    if r.duration == RpcDurationRepresentation::TimeDelta {
                        "if value==nil{return nil,nil};return TimeDuration(*value)".into()
                    } else {
                        "if value==nil{return nil,nil};return poolsterSecondsDuration(*value)"
                            .into()
                    }
                } else {
                    if r.duration == RpcDurationRepresentation::TimeDelta {
                        "if value==nil{return nil,nil};converted,err:=DurationTime(value);return &converted,err".into()
                    } else {
                        "if value==nil{return nil,nil};converted,err:=poolsterDurationSeconds(value);return &converted,err".into()
                    }
                }
            }
            _ if types::local_message(c, full) => {
                if to_native {
                    "if value==nil{return nil,nil};return value.ToNativeDepth(depth-1,active)"
                        .into()
                } else {
                    let ty = c.ty(full)?;
                    let (module, name) = ty
                        .rsplit_once('.')
                        .map(|(m, n)| (format!("{m}."), n))
                        .unwrap_or((String::new(), ty.as_str()));
                    format!("return {module}{name}ValuesFromNative(value)")
                }
            }
            _ => {
                format!(
                    "if value==nil{{return nil,nil}};if err:=poolsterValueCheckDepth(value,{},map[proto.Message]bool{{}});err!=nil{{return nil,err}};return proto.Clone(value).({native}),nil",
                    c.plan.options.max_depth
                )
            }
        },
        _ => "return value,nil".into(),
    };
    Ok(format!("func(value {input})({output},error){{{body}}}"))
}
pub(super) fn field(
    c: &ModelContext<'_>,
    f: &RpcWireField,
    to_native: bool,
    input: &str,
) -> Result<String> {
    if let RpcFieldKind::Message(full) = &f.kind {
        if c.plan.types[full].map_entry {
            let m = &c.plan.wire_messages[full];
            let key = callback(c, &m.fields[0].kind, to_native)?;
            let value = callback(c, &m.fields[1].kind, to_native)?;
            let helper = if c.plan.options.representations.maps == RpcMapsRepresentation::Entries {
                if to_native {
                    "poolsterEntriesMap"
                } else {
                    "poolsterMapEntries"
                }
            } else {
                "poolsterConvertMap"
            };
            return Ok(format!("{helper}({input},{key},{value})"));
        }
    }
    let convert = callback(c, &f.kind, to_native)?;
    let helper = if f.label == RpcFieldLabel::Repeated {
        Some("poolsterConvertList")
    } else if c.pointer(f) {
        Some("poolsterConvertOptional")
    } else {
        None
    };
    Ok(if let Some(helper) = helper {
        format!("{helper}({input},{convert})")
    } else {
        format!("({convert})({input})")
    })
}
