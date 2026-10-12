//! Explicit request-owned queues over validated native unary mappings.
use super::{plan::*, *};
use anyhow::Context;
use poolster_core::native::rpc::*;
use std::fmt::Write;
pub(super) fn emit(
    cx: &mut PluginContext<'_, crate::Go>,
    schema: &RpcWireSchema,
    plan: &Plan,
    options: &RpcGenerationOptions,
) -> Result<()> {
    for mapping in &options.batching {
        let (file, service, single) = schema
            .files
            .iter()
            .find_map(|f| {
                f.services.iter().find_map(|s| {
                    s.methods
                        .iter()
                        .find(|m| m.full_name == mapping.single_method)
                        .map(|m| (f, s, m))
                })
            })
            .context("batch single method unavailable")?;
        let batch = schema
            .files
            .iter()
            .flat_map(|f| &f.services)
            .flat_map(|s| &s.methods)
            .find(|m| m.full_name == mapping.batch_method)
            .context("batch method unavailable")?;
        let c = plan.context(file)?;
        let request = c.ty(&single.input_type)?;
        let output = c.ty(&single.output_type)?;
        let batch_input = c.ty(&batch.input_type)?;
        let batch_output = c.ty(&batch.output_type)?;
        let input = &plan.wire_messages[&single.input_type];
        let field = input
            .fields
            .iter()
            .find(|f| f.name == mapping.input_field)
            .unwrap();
        let key = c.scalar(&field.kind)?;
        let n = format!(
            "{}{}",
            name(service.full_name.rsplit('.').next().unwrap()),
            name(&single.name)
        );
        let mut source = include_str!("batch_scope.go.tmpl")
            .replace("NAME", &n)
            .replace("BATCH_INPUT", &batch_input)
            .replace("BATCH_OUTPUT", &batch_output)
            .replace("INPUT_FIELD", &name(&mapping.batch_input_field))
            .replace("OUTPUT_FIELD", &name(&mapping.batch_output_field))
            .replace("GET_KEY", &format!("Get{}", name(&mapping.input_field)))
            .replace("REQUEST", &request)
            .replace("OUTPUT", &output)
            .replace("KEY", &key)
            .replace("MAX_BATCH", &mapping.max_batch_size.to_string())
            .replace("CACHE", if mapping.cache { "true" } else { "false" });
        let association = if let Some(field) = &mapping.output_key {
            format!(
                "byKey:=make(map[{key}]*{output},len(response.{}));for _,value:=range response.{}{{if value==nil{{continue}};key:=value.Get{}();if _,duplicate:=byKey[key];duplicate{{return scope.fail(group,fmt.Errorf(\"duplicate batch response key\"))}};byKey[key]=value}};var missing error;for _,item:=range group{{value,ok:=byKey[item.key];if !ok{{missing=fmt.Errorf(\"missing batch response key\");scope.finish(item,nil,missing);continue}};scope.finish(item,value,nil)}};return missing",
                name(&mapping.batch_output_field),
                name(&mapping.batch_output_field),
                name(field)
            )
        } else {
            format!(
                "if len(response.{})!=len(group){{return scope.fail(group,fmt.Errorf(\"batch response count mismatch\"))}};for index,item:=range group{{scope.finish(item,response.{}[index],nil)}}",
                name(&mapping.batch_output_field),
                name(&mapping.batch_output_field)
            )
        };
        source = source.replace("ASSOCIATE", &association);
        writeln!(
            source,
            "// Mapping: {} -> {}; cache is request-local and explicitly {}.",
            mapping.single_method, mapping.batch_method, mapping.cache
        )?;
        c.emit(
            cx,
            &format!("poolster_{}_batch.pb.go", n),
            source,
            &[
                "context \"context\"",
                "sync \"sync\"",
                "fmt \"fmt\"",
                "proto \"google.golang.org/protobuf/proto\"",
                "grpc \"google.golang.org/grpc\"",
            ],
        )?;
    }
    Ok(())
}
