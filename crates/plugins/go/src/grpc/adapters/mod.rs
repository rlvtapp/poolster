//! Additional wire protocols have their own contracts and stream boundaries.
mod twirp;
mod web;
use super::plan::*;
use super::*;
use poolster_core::native::rpc::*;
use std::collections::BTreeSet;
pub(super) fn emit(
    cx: &mut PluginContext<'_, crate::Go>,
    schema: &RpcWireSchema,
    plan: &Plan,
    options: &RpcGenerationOptions,
) -> Result<()> {
    let web = options.transports.contains(&RpcTransport::GrpcWeb);
    let twirp = options.transports.contains(&RpcTransport::Twirp);
    if !web && !twirp {
        return Ok(());
    }
    let mut packages = BTreeSet::new();
    for file in &schema.files {
        if plan.options.import_mappings.contains_key(&file.name) {
            continue;
        }
        let import = &plan.packages[&file.name].0;
        if !(import == &plan.module || import.starts_with(&format!("{}/", plan.module))) {
            continue;
        }
        let c = plan.context(file)?;
        if file.services.is_empty() {
            continue;
        }
        if packages.insert(c.import.clone()) {
            for symbol in ["RpcUnsupportedError", "TwirpError", "TwirpHTTPClient"] {
                ensure!(
                    !plan
                        .types
                        .values()
                        .any(|t| t.import == c.import && t.name == symbol),
                    "Go transport adapter symbol collision: {symbol}"
                );
            }
            let mut support = String::from(
                "type RpcUnsupportedError struct { Protocol string; Method string; Reason string }\nfunc (error *RpcUnsupportedError) Error() string {return error.Protocol+\" does not support \"+error.Method+\": \"+error.Reason}\n",
            );
            support.push_str(
                &include_str!("../depth.go.tmpl").replace("{{PREFIX}}", "poolsterAdapter"),
            );
            if web {
                support.push_str(&format!("type poolsterWebCodec struct{{}}\nfunc(poolsterWebCodec)Name()string{{return \"proto\"}}\nfunc(poolsterWebCodec)Marshal(value any)([]byte,error){{message,ok:=value.(proto.Message);if !ok{{return nil,fmt.Errorf(\"expected native Protobuf message\")}};if err:=poolsterAdapterCheckDepth(message,{},map[proto.Message]bool{{}});err!=nil{{return nil,err}};return proto.Marshal(message)}}\nfunc(poolsterWebCodec)Unmarshal(data []byte,value any)error{{message,ok:=value.(proto.Message);if !ok{{return fmt.Errorf(\"expected native Protobuf message\")}};return(proto.UnmarshalOptions{{RecursionLimit:{},DiscardUnknown:{}}}).Unmarshal(data,message)}}\n", options.max_depth,options.max_depth,options.unknown_fields==RpcUnknownFields::Discard));
            }
            c.emit(
                cx,
                "poolster_adapter_support.pb.go",
                support,
                &[
                    "proto \"google.golang.org/protobuf/proto\"",
                    "protoreflect \"google.golang.org/protobuf/reflect/protoreflect\"",
                    "fmt \"fmt\"",
                ],
            )?;
            if twirp {
                twirp::runtime(cx, &c, options)?;
            }
        }
        if web {
            web::emit(cx, file, &c, options)?;
        }
        if twirp {
            twirp::emit(cx, file, &c, options)?;
        }
    }
    let capabilities=schema.files.iter().flat_map(|f|&f.services).flat_map(|s|s.methods.iter().map(move |m|serde_json::json!({"method":format!("/{}/{}",s.full_name,m.name),"nativeGrpc":true,"grpcWeb":web&&matches!(m.streaming,RpcStreaming::Unary|RpcStreaming::Server),"twirp":twirp&&m.streaming==RpcStreaming::Unary,"streaming":m.streaming}))).collect::<Vec<_>>();
    cx.files.emit(GeneratedFile::new(".poolster/rpc-capabilities.json",serde_json::to_string_pretty(&serde_json::json!({"version":1,"methods":capabilities,"unsupportedAdapterMethods":"typed RpcUnsupportedError before network I/O","grpcWebRuntime":if web{Some("connectrpc.com/connect@v1.18.1")}else{None},"twirpRuntime":"Poolster net/http adapter, Protobuf and ProtoJSON native codecs"}))?+"\n")?)?;
    Ok(())
}
