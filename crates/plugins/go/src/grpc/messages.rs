use super::plan::*;
use super::*;
use anyhow::Context as _;
use poolster_core::native::rpc::*;
use std::fmt::Write;

pub(super) fn wire(kind: &RpcFieldKind) -> &'static str {
    match kind {
        RpcFieldKind::Double | RpcFieldKind::Fixed64 | RpcFieldKind::Sfixed64 => "fixed64",
        RpcFieldKind::Float | RpcFieldKind::Fixed32 | RpcFieldKind::Sfixed32 => "fixed32",
        RpcFieldKind::Sint32 => "zigzag32",
        RpcFieldKind::Sint64 => "zigzag64",
        RpcFieldKind::String | RpcFieldKind::Bytes | RpcFieldKind::Message(_) => "bytes",
        RpcFieldKind::Group(_) => "group",
        _ => "varint",
    }
}
pub(super) fn tag(f: &RpcWireField, c: &plan::Context<'_>, oneof: bool) -> String {
    let label = match f.label {
        RpcFieldLabel::Optional => "opt",
        RpcFieldLabel::Required => "req",
        RpcFieldLabel::Repeated => "rep",
    };
    let mut t = format!("{},{},{label},name={}", wire(&f.kind), f.number, f.name);
    if f.json_name != f.name {
        t.push_str(&format!(",json={}", f.json_name));
    }
    if c.file.syntax == "proto3" {
        t.push_str(",proto3");
    }
    if let RpcFieldKind::Enum(n) = &f.kind {
        t.push_str(&format!(",enum={}", n));
    }
    if oneof {
        t.push_str(",oneof");
    }
    if let Some(default) = &f.default_value {
        let value = if let RpcFieldKind::Enum(n) = &f.kind {
            c.plan
                .enum_value(n, default)
                .map(|v| v.to_string())
                .unwrap_or(default.clone())
        } else {
            default.clone()
        };
        t.push_str(&format!(",def={value}"));
    }
    t
}
pub(super) fn field_name(f: &RpcWireField) -> String {
    let n = name(&f.name);
    if [
        "Reset",
        "String",
        "ProtoMessage",
        "ProtoReflect",
        "Descriptor",
        "MarshalJSON",
        "UnmarshalJSON",
        "MarshalJSONWithOptions",
        "UnmarshalJSONWithOptions",
        "MarshalBinary",
        "UnmarshalBinary",
    ]
    .contains(&n.as_str())
        || n.starts_with("Get")
    {
        format!("{n}_")
    } else {
        n
    }
}
pub(super) fn emit(
    cx: &mut PluginContext<'_, crate::Go>,
    file: &RpcWireFile,
    c: &plan::Context<'_>,
) -> Result<()> {
    for (index, e) in enums(file).iter().enumerate() {
        super::enums::emit(cx, e, index, c)?;
    }
    for (index, m) in messages(file).iter().enumerate() {
        if !m.map_entry {
            emit_message(cx, m, index, c)?;
        }
    }
    Ok(())
}
fn emit_message(
    cx: &mut PluginContext<'_, crate::Go>,
    m: &RpcWireMessage,
    index: usize,
    c: &plan::Context<'_>,
) -> Result<()> {
    let t = c.ty(&m.full_name)?;
    let mut symbols = std::collections::BTreeSet::new();
    for field in &m.fields {
        anyhow::ensure!(
            symbols.insert(format!("Get{}", name(&field.name))),
            "Go getter name collision in {}: {}",
            m.full_name,
            field.name
        );
    }
    for field in &m.fields {
        if field.oneof_index.is_none() || field.proto3_optional {
            anyhow::ensure!(
                symbols.insert(field_name(field)),
                "Go field name collision in {}: {}",
                m.full_name,
                field.name
            );
        }
    }
    for (index, oneof) in m.oneofs.iter().enumerate() {
        if m.fields
            .iter()
            .any(|f| f.oneof_index == Some(index) && !f.proto3_optional)
        {
            anyhow::ensure!(
                symbols.insert(name(oneof)),
                "Go oneof name collision in {}: {}",
                m.full_name,
                oneof
            );
        }
    }
    let mut s = super::documentation::comment(c, RpcDefinitionKind::Message, &m.full_name)?;
    s.push_str(&format!(
        "type {t} struct {{\nstate protoimpl.MessageState\n"
    ));
    for f in &m.fields {
        if f.oneof_index.is_some() && !f.proto3_optional {
            continue;
        }
        let n = field_name(f);
        let ty = c.field(f)?;
        let mut tags = format!(
            "protobuf:{:?} json:{:?}",
            tag(f, c, f.proto3_optional),
            format!("{},omitempty", f.name)
        );
        if let RpcFieldKind::Message(n) = &f.kind {
            if c.plan.types[n].map_entry {
                let entry = &c.plan.wire_messages[n];
                tags.push_str(&format!(
                    " protobuf_key:{:?} protobuf_val:{:?}",
                    tag(&entry.fields[0], c, false),
                    tag(&entry.fields[1], c, false)
                ));
            }
        }
        s.push_str(&super::documentation::comment(
            c,
            RpcDefinitionKind::Field,
            &format!("{}.{}", m.full_name, f.name),
        )?);
        writeln!(s, "{n} {ty} `{tags}`")?;
    }
    for (i, oneof) in m.oneofs.iter().enumerate() {
        if m.fields
            .iter()
            .any(|f| f.oneof_index == Some(i) && !f.proto3_optional)
        {
            writeln!(
                s,
                "{} is{t}_{} `protobuf_oneof:{:?}`",
                name(oneof),
                name(oneof),
                oneof
            )?;
        }
    }
    s.push_str("unknownFields protoimpl.UnknownFields\nsizeCache protoimpl.SizeCache\nextensionFields protoimpl.ExtensionFields\n}\n");
    writeln!(
        s,
        "func (x *{t}) Reset() {{ *x={t}{{}};mi:=&{}_msgTypes[{index}];ms:=protoimpl.X.MessageStateOf(protoimpl.Pointer(x));ms.StoreMessageInfo(mi) }}\nfunc (x *{t}) String() string {{ return protoimpl.X.MessageStringOf(x) }}\nfunc (*{t}) ProtoMessage() {{}}\nfunc (x *{t}) ProtoReflect() protoreflect.Message {{ mi:=&{}_msgTypes[{index}];if x!=nil{{ms:=protoimpl.X.MessageStateOf(protoimpl.Pointer(x));if ms.LoadMessageInfo()==nil{{ms.StoreMessageInfo(mi)}};return ms}};return mi.MessageOf(x) }}",
        c.prefix, c.prefix
    )?;
    if let Some(public) = t.strip_prefix("poolster") {
        writeln!(s, "func New{public}()*{t}{{return &{t}{{}}}}")?;
    }
    for f in &m.fields {
        let n = field_name(f);
        let is_oneof = f.oneof_index.is_some() && !f.proto3_optional;
        let ty = if is_oneof {
            c.scalar(&f.kind)?
        } else {
            let ty = c.field(f)?;
            if c.pointer(f) { ty[1..].into() } else { ty }
        };
        let getter = format!("Get{}", name(&f.name));
        let fallback = default_value(f, c)?;
        if is_oneof {
            let oneof = &m.oneofs[f.oneof_index.unwrap()];
            writeln!(
                s,
                "func (x *{t}) {getter}() {ty} {{ if x!=nil {{ if value,ok:=x.{}.(*{t}_{n});ok{{return value.{n}}} }};return {fallback} }}",
                name(oneof)
            )?;
        } else if c.pointer(f) {
            writeln!(
                s,
                "func (x *{t}) {getter}() {ty} {{ if x!=nil && x.{n}!=nil {{ return *x.{n} }};return {fallback} }}"
            )?;
        } else {
            writeln!(
                s,
                "func (x *{t}) {getter}() {ty} {{ if x!=nil {{ return x.{n} }};return {fallback} }}"
            )?;
        }
    }
    for (i, oneof) in m.oneofs.iter().enumerate() {
        let fields = m
            .fields
            .iter()
            .filter(|f| f.oneof_index == Some(i) && !f.proto3_optional)
            .collect::<Vec<_>>();
        if fields.is_empty() {
            continue;
        }
        let oneof = name(oneof);
        writeln!(
            s,
            "type is{t}_{oneof} interface {{ is{t}_{oneof}() }}\nfunc (x *{t}) Get{oneof}() is{t}_{oneof} {{ if x!=nil {{return x.{oneof}}};return nil }}"
        )?;
        for f in fields {
            let n = field_name(f);
            writeln!(
                s,
                "type {t}_{n} struct {{ {n} {} `protobuf:{:?}` }}\nfunc (*{t}_{n}) is{t}_{oneof}() {{}}",
                c.scalar(&f.kind)?,
                tag(f, c, true)
            )?;
        }
    }
    c.emit(
        cx,
        &format!("{}_{}.pb.go", file_stem(&c.file.name), t),
        s,
        &[
            "protoreflect \"google.golang.org/protobuf/reflect/protoreflect\"",
            "protoimpl \"google.golang.org/protobuf/runtime/protoimpl\"",
        ],
    )
}
fn default_value(f: &RpcWireField, c: &plan::Context<'_>) -> Result<String> {
    if f.label == RpcFieldLabel::Repeated
        || matches!(f.kind, RpcFieldKind::Message(_) | RpcFieldKind::Group(_))
    {
        return Ok("nil".into());
    }
    if let Some(value) = &f.default_value {
        return Ok(match &f.kind {
            RpcFieldKind::String => serde_json::to_string(value)?,
            RpcFieldKind::Bytes => format!(
                "[]byte{{{}}}",
                super::documentation::bytes_default(value)?
                    .iter()
                    .map(u8::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            RpcFieldKind::Enum(n) => format!(
                "{}({})",
                c.ty(n)?,
                c.plan
                    .enum_value(n, value)
                    .context("enum default missing")?
            ),
            RpcFieldKind::Float | RpcFieldKind::Double
                if ["inf", "-inf", "nan"].contains(&value.as_str()) =>
            {
                format!(
                    "{}(math.{})",
                    c.scalar(&f.kind)?,
                    match value.as_str() {
                        "inf" => "Inf(1)",
                        "-inf" => "Inf(-1)",
                        _ => "NaN()",
                    }
                )
            }
            _ => value.clone(),
        });
    }
    Ok(match &f.kind {
        RpcFieldKind::String => "\"\"".into(),
        RpcFieldKind::Bytes => "nil".into(),
        RpcFieldKind::Bool => "false".into(),
        RpcFieldKind::Enum(n) => format!("{}({})", c.ty(n)?, c.plan.enum_first(n)?),
        _ => "0".into(),
    })
}
pub(super) fn file_stem(file: &str) -> String {
    poolster_core::files::source_file_stem(
        file.rsplit('/').next().unwrap().trim_end_matches(".proto"),
    )
}
