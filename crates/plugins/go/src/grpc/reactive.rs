//! Bounded opt-in channels reuse native client calls and cancellation contexts.
use super::{plan::*, *};
use poolster_core::native::rpc::*;
use std::{collections::BTreeSet, fmt::Write};
pub(super) fn emit(
    cx: &mut PluginContext<'_, crate::Go>,
    schema: &RpcWireSchema,
    plan: &Plan,
) -> Result<()> {
    let mut packages = BTreeSet::new();
    for file in &schema.files {
        if plan.options.import_mappings.contains_key(&file.name) {
            continue;
        }
        if file.services.is_empty() {
            continue;
        }
        let import = &plan.packages[&file.name].0;
        if !(import == &plan.module || import.starts_with(&format!("{}/", plan.module))) {
            continue;
        }
        let c = plan.context(file)?;
        if packages.insert(c.import.clone()) {
            ensure!(
                !plan
                    .types
                    .values()
                    .any(|t| t.import == c.import && t.name == "RpcSubscription"),
                "Go reactive symbol collision: RpcSubscription"
            );
            c.emit(
                cx,
                "poolster_reactive.pb.go",
                include_str!("reactive.go.tmpl").into(),
                &[
                    "context \"context\"",
                    "io \"io\"",
                    "errors \"errors\"",
                    "sync \"sync\"",
                    "metadata \"google.golang.org/grpc/metadata\"",
                ],
            )?;
        }
        for service in &file.services {
            let n = name(service.full_name.rsplit('.').next().unwrap());
            for method in &service.methods {
                let m = name(&method.name);
                let i = c.ty(&method.input_type)?;
                let o = c.ty(&method.output_type)?;
                let input = if matches!(
                    method.streaming,
                    RpcStreaming::Client | RpcStreaming::Bidirectional
                ) {
                    format!("input <-chan *{i}")
                } else {
                    format!("input *{i}")
                };
                let mut source = format!(
                    "func Subscribe{n}{m}(ctx context.Context,client {n}Client,{input},options ...grpc.CallOption)*RpcSubscription[{o}]{{return poolsterSubscribe(ctx,func(ctx context.Context,subscription *RpcSubscription[{o}])error{{\n"
                );
                match method.streaming {
                    RpcStreaming::Unary => writeln!(
                        source,
                        "var header,trailer metadata.MD;opts:=append([]grpc.CallOption{{}},options...);opts=append(opts,grpc.Header(&header),grpc.Trailer(&trailer));value,err:=client.{m}(ctx,input,opts...);subscription.setMetadata(header,trailer);if err!=nil{{return err}};return subscription.publish(ctx,value)"
                    )?,
                    RpcStreaming::Server => writeln!(
                        source,
                        "stream,err:=client.{m}(ctx,input,options...);if err!=nil{{return err}};header,err:=stream.Header();if err!=nil{{return err}};subscription.setMetadata(header,nil);defer func(){{subscription.setMetadata(header,stream.Trailer())}}();for{{value,err:=stream.Recv();if err!=nil{{return err}};if err=subscription.publish(ctx,value);err!=nil{{return err}}}}"
                    )?,
                    RpcStreaming::Client => writeln!(
                        source,
                        "stream,err:=client.{m}(ctx,options...);if err!=nil{{return err}};for{{select{{case <-ctx.Done():return ctx.Err();case value,ok:=<-input:if !ok{{reply,err:=stream.CloseAndRecv();header,_:=stream.Header();subscription.setMetadata(header,stream.Trailer());if err!=nil{{return err}};return subscription.publish(ctx,reply)}};if err=stream.Send(value);err!=nil{{return err}}}}}}"
                    )?,
                    RpcStreaming::Bidirectional => writeln!(
                        source,
                        "stream,err:=client.{m}(ctx,options...);if err!=nil{{return err}};sent:=make(chan error,1);go func(){{for{{select{{case <-ctx.Done():sent<-ctx.Err();return;case value,ok:=<-input:if !ok{{sent<-stream.CloseSend();return}};if err:=stream.Send(value);err!=nil{{sent<-err;return}}}}}}}}();header,err:=stream.Header();if err!=nil{{return err}};subscription.setMetadata(header,nil);defer func(){{subscription.setMetadata(header,stream.Trailer())}}();for{{value,err:=stream.Recv();if err!=nil{{select{{case sendErr:=<-sent:if sendErr!=nil{{return sendErr}};default:}};return err}};if err=subscription.publish(ctx,value);err!=nil{{return err}}}}"
                    )?,
                }
                source.push_str("})}\n");
                c.emit(
                    cx,
                    &format!("poolster_{n}_{m}_subscription.pb.go"),
                    source,
                    &[
                        "context \"context\"",
                        "grpc \"google.golang.org/grpc\"",
                        "metadata \"google.golang.org/grpc/metadata\"",
                    ],
                )?;
            }
        }
    }
    Ok(())
}
