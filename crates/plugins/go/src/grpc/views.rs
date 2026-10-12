//! Opt-in read views retain native wire values behind defensive copies.
use super::{plan::*, *};
use poolster_core::native::rpc::*;
use std::fmt::Write;
pub(super) fn emit(
    cx: &mut PluginContext<'_, crate::Go>,
    file: &RpcWireFile,
    c: &plan::Context<'_>,
) -> Result<()> {
    for message in messages(file).into_iter().filter(|m| !m.map_entry) {
        let t = c.ty(&message.full_name)?;
        let view = format!("{t}View");
        ensure!(
            !c.plan
                .types
                .values()
                .any(|other| other.import == c.import && other.name == view),
            "Go immutable view name collision: {view}"
        );
        let mut source = format!(
            "// {view} owns a defensive snapshot. Accessors never expose its storage.\ntype {view} struct{{value *{t}}}\nfunc New{view}(value *{t}){view}{{if value==nil{{value=&{t}{{}}}};return {view}{{value:proto.Clone(value).(*{t})}}}}\nfunc(view {view})copy()*{t}{{if view.value==nil{{return &{t}{{}}}};return proto.Clone(view.value).(*{t})}}\n"
        );
        for field in &message.fields {
            let method = format!("Get{}", name(&field.name));
            let oneof = field.oneof_index.is_some() && !field.proto3_optional;
            let ty = if oneof {
                c.scalar(&field.kind)?
            } else {
                c.field(field)?
            };
            let value = if oneof {
                format!("view.copy().{method}()")
            } else {
                format!("view.copy().{}", super::messages::field_name(field))
            };
            writeln!(source, "func(view {view}){method}(){ty}{{return {value}}}")?;
            if oneof {
                writeln!(
                    source,
                    "func(view {view})Has{}()bool{{_,ok:=view.copy().{}.(*{t}_{});return ok}}",
                    name(&field.name),
                    name(&message.oneofs[field.oneof_index.unwrap()]),
                    super::messages::field_name(field)
                )?;
            }
        }
        c.emit(
            cx,
            &format!(
                "{}_{}_view.pb.go",
                super::messages::file_stem(&file.name),
                t
            ),
            source,
            &["proto \"google.golang.org/protobuf/proto\""],
        )?;
    }
    Ok(())
}
