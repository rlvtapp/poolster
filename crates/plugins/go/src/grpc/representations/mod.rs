//! Typed alternate models convert explicitly while retaining native wire ABI.
mod conversion;
mod types;
use super::plan::Context as ModelContext;
use super::{plan::*, *};
use poolster_core::native::rpc::*;
use std::{collections::BTreeSet, fmt::Write};
pub(super) fn emit(
    cx: &mut PluginContext<'_, crate::Go>,
    schema: &RpcWireSchema,
    plan: &Plan,
    generation: &[String],
) -> Result<()> {
    let mut packages = BTreeSet::new();
    for file in &schema.files {
        if !generation.contains(&file.name) {
            continue;
        }
        let c = plan.context(file)?;
        if packages.insert(c.import.clone()) {
            for symbol in ["RpcEnumName", "RpcEntry"] {
                ensure!(
                    !plan
                        .types
                        .values()
                        .any(|t| t.import == c.import && t.name == symbol),
                    "Go representation helper name collision: {symbol}"
                );
            }
            c.emit(
                cx,
                "poolster_representations.pb.go",
                include_str!("helpers.go.tmpl").into(),
                &[
                    "sort \"sort\"",
                    "fmt \"fmt\"",
                    "durationpb \"google.golang.org/protobuf/types/known/durationpb\"",
                ],
            )?;
        }
        for message in messages(file).into_iter().filter(|m| !m.map_entry) {
            render(cx, message, &c)?;
        }
    }
    Ok(())
}
fn render(
    cx: &mut PluginContext<'_, crate::Go>,
    message: &RpcWireMessage,
    c: &ModelContext<'_>,
) -> Result<()> {
    let n = c.ty(&message.full_name)?;
    let values = format!("{n}Values");
    ensure!(
        !c.plan
            .types
            .values()
            .any(|t| t.import == c.import && t.name == values),
        "Go representation type collision: {values}"
    );
    let mut source = format!(
        "// {values} is an explicit alternate value model. Native identity remains unchanged.\ntype {values} struct{{\nnative *{n}\n"
    );
    for field in &message.fields {
        if field.oneof_index.is_some() && !field.proto3_optional {
            continue;
        }
        writeln!(
            source,
            "{} {}",
            super::messages::field_name(field),
            types::field(c, field)?
        )?;
    }
    let oneofs = message
        .oneofs
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            message
                .fields
                .iter()
                .any(|f| f.oneof_index == Some(*index) && !f.proto3_optional)
        })
        .collect::<Vec<_>>();
    for (_, oneof) in &oneofs {
        writeln!(source, "{} is{values}_{}", name(oneof), name(oneof))?;
    }
    source.push_str("}\n");
    for (index, oneof) in &oneofs {
        let oneof = name(oneof);
        writeln!(
            source,
            "type is{values}_{oneof} interface{{is{values}_{oneof}()}}"
        )?;
        for field in message
            .fields
            .iter()
            .filter(|f| f.oneof_index == Some(*index) && !f.proto3_optional)
        {
            let f = super::messages::field_name(field);
            writeln!(
                source,
                "type {values}_{f} struct{{{f} {}}}\nfunc(*{values}_{f})is{values}_{oneof}(){{}}",
                types::scalar(c, &field.kind)?
            )?;
        }
    }
    writeln!(
        source,
        "func {values}FromNative(input *{n})(*{values},error){{\nif input==nil{{return nil,nil}}\nif err:=poolsterValueCheckDepth(input,{},map[proto.Message]bool{{}});err!=nil{{return nil,err}}\nif proto.Size(input)>{}{{return nil,fmt.Errorf(\"representation exceeds byte limit\")}}\noutput:=&{values}{{native:proto.Clone(input).(*{n})}}",
        c.plan.options.max_depth, c.plan.options.max_message_bytes
    )?;
    fields(&mut source, message, c, false)?;
    for (index, oneof) in &oneofs {
        let o = name(oneof);
        writeln!(source, "switch value:=input.{o}.(type){{")?;
        for field in message
            .fields
            .iter()
            .filter(|f| f.oneof_index == Some(*index) && !f.proto3_optional)
        {
            let f = super::messages::field_name(field);
            let callback = conversion::callback(c, &field.kind, false)?;
            writeln!(
                source,
                "case *{n}_{f}:\nif value==nil{{break}}\nconverted,err:=({callback})(value.{f});if err!=nil{{return nil,err}};output.{o}=&{values}_{f}{{{f}:converted}}"
            )?;
        }
        source.push_str("}\n");
    }
    source.push_str("return output,nil\n}\n");
    writeln!(
        source,
        "func(input *{values})ToNative()(*{n},error){{return input.ToNativeDepth({},map[any]bool{{}})}}\nfunc(input *{values})ToNativeDepth(depth int,active map[any]bool)(*{n},error){{\nif input==nil{{return nil,nil}}\nif active==nil{{active=map[any]bool{{}}}};if depth>{}{{depth={}}};if depth<=0||active[input]{{return nil,fmt.Errorf(\"cyclic or over-depth value model\")}}\nactive[input]=true;defer delete(active,input)\noutput:=&{n}{{}};if input.native!=nil{{output=proto.Clone(input.native).(*{n})}}",
        c.plan.options.max_depth, c.plan.options.max_depth, c.plan.options.max_depth
    )?;
    fields(&mut source, message, c, true)?;
    for (index, oneof) in &oneofs {
        let o = name(oneof);
        writeln!(source, "output.{o}=nil;switch value:=input.{o}.(type){{")?;
        for field in message
            .fields
            .iter()
            .filter(|f| f.oneof_index == Some(*index) && !f.proto3_optional)
        {
            let f = super::messages::field_name(field);
            let callback = conversion::callback(c, &field.kind, true)?;
            writeln!(
                source,
                "case *{values}_{f}:\nif value==nil{{break}}\nconverted,err:=({callback})(value.{f});if err!=nil{{return nil,err}};output.{o}=&{n}_{f}{{{f}:converted}}"
            )?;
        }
        source.push_str("}\n");
    }
    writeln!(
        source,
        "if err:=proto.CheckInitialized(output);err!=nil{{return nil,err}};if proto.Size(output)>{}{{return nil,fmt.Errorf(\"representation exceeds byte limit\")}};return output,nil\n}}",
        c.plan.options.max_message_bytes
    )?;
    c.emit(
        cx,
        &format!(
            "{}_{}_values.pb.go",
            super::messages::file_stem(&c.file.name),
            n
        ),
        source,
        &[
            "fmt \"fmt\"",
            "time \"time\"",
            "proto \"google.golang.org/protobuf/proto\"",
        ],
    )
}
fn fields(
    source: &mut String,
    message: &RpcWireMessage,
    c: &ModelContext<'_>,
    to_native: bool,
) -> Result<()> {
    for field in &message.fields {
        if field.oneof_index.is_some() && !field.proto3_optional {
            continue;
        }
        let f = super::messages::field_name(field);
        let expression = conversion::field(c, field, to_native, &format!("input.{f}"))?;
        writeln!(
            source,
            "{{converted,err:={expression};if err!=nil{{return nil,fmt.Errorf(\"{}: %w\",err)}};output.{f}=converted}}",
            field.name
        )?;
    }
    Ok(())
}
