//! Native enum ABI, optional aliases and unknown display labels.
use super::{plan::*, *};
use poolster_core::native::rpc::*;
use std::fmt::Write;
pub(super) fn emit(
    cx: &mut PluginContext<'_, crate::Go>,
    e: &RpcWireEnum,
    index: usize,
    c: &plan::Context<'_>,
) -> Result<()> {
    let t = c.ty(&e.full_name)?;
    let mut s = super::documentation::comment(c, RpcDefinitionKind::Enum, &e.full_name)?;
    s.push_str(&format!("type {t} int32\nconst (\n"));
    let prefix = e
        .full_name
        .rsplit_once('.')
        .map(|(parent, _)| parent)
        .unwrap_or("");
    let prefix = if prefix == c.file.package {
        t.clone()
    } else {
        c.ty(prefix)?
    };
    for v in &e.values {
        s.push_str(&super::documentation::comment(
            c,
            RpcDefinitionKind::EnumValue,
            &format!("{}.{}", e.full_name, v.name),
        )?);
        writeln!(s, "{prefix}_{} {t} = {}", v.name, v.number)?;
        if c.plan.options.remove_enum_prefix {
            let alias = name(&v.name);
            ensure!(
                !c.plan
                    .types
                    .values()
                    .any(|other| other.import == c.import && other.name == alias),
                "Go enum alias collides with a type: {alias}"
            );
            let count = c
                .plan
                .wire_enums
                .values()
                .filter(|other| c.plan.types[&other.full_name].import == c.import)
                .flat_map(|other| &other.values)
                .filter(|other| name(&other.name) == alias)
                .count();
            ensure!(
                count == 1,
                "Go enum prefix removal creates ambiguous alias: {alias}"
            );
            writeln!(s, "{alias} {t} = {prefix}_{}", v.name)?;
        }
    }
    s.push_str(")\n");
    writeln!(s, "var {t}_name = map[int32]string{{")?;
    let mut numbers = std::collections::BTreeSet::new();
    for v in &e.values {
        if numbers.insert(v.number) {
            writeln!(s, "{}: {:?},", v.number, v.name)?;
        }
    }
    s.push_str("}\n");
    writeln!(s, "var {t}_value = map[string]int32{{")?;
    for v in &e.values {
        writeln!(s, "{:?}: {},", v.name, v.number)?;
    }
    s.push_str("}\n");
    writeln!(
        s,
        "func (x {t}) Enum() *{t} {{ p:=new({t});*p=x;return p }}\nfunc (x {t}) String() string {{ return protoimpl.X.EnumStringOf(x.Descriptor(),protoreflect.EnumNumber(x)) }}\nfunc ({t}) Descriptor() protoreflect.EnumDescriptor {{ return {}_enumTypes[{index}].Descriptor() }}\nfunc ({t}) Type() protoreflect.EnumType {{ return &{}_enumTypes[{index}] }}\nfunc (x {t}) Number() protoreflect.EnumNumber {{ return protoreflect.EnumNumber(x) }}",
        c.prefix, c.prefix
    )?;
    writeln!(
        s,
        "func(x {t})DisplayName()string{{if label,known:={t}_name[int32(x)];known{{return label}};return {}}}",
        serde_json::to_string(&c.plan.options.unknown_enum_name)?
    )?;
    c.emit(
        cx,
        &format!("{}_{}.pb.go", super::messages::file_stem(&c.file.name), t),
        s,
        &[
            "protoreflect \"google.golang.org/protobuf/reflect/protoreflect\"",
            "protoimpl \"google.golang.org/protobuf/runtime/protoimpl\"",
        ],
    )
}
