use anyhow::Result;
use poolster_core::GeneratedFile;
use poolster_core::native::rpc::*;
use std::collections::BTreeMap;
fn quote(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}
pub(super) fn render(
    f: &RpcWireFile,
    names: &BTreeMap<String, String>,
    options: &RpcGenerationOptions,
    metadata: Option<&RpcFileMetadata>,
) -> String {
    let documentation = |kind, full: &str| {
        if !options.comments {
            return String::new();
        }
        metadata
            .and_then(|m| m.definition(kind, full))
            .map(super::documentation::render)
            .unwrap_or_default()
    };
    let mut s = String::new();
    for service in &f.services {
        let name = service.full_name.rsplit('.').next().unwrap();
        s.push_str(&documentation(
            RpcDefinitionKind::Service,
            &service.full_name,
        ));
        s.push_str(&format!("export const {name}Service={{\n"));
        for method in &service.methods {
            s.push_str(&documentation(RpcDefinitionKind::Method, &method.full_name));
            let input = &names[&method.input_type];
            let output = &names[&method.output_type];
            let cs = matches!(
                method.streaming,
                RpcStreaming::Client | RpcStreaming::Bidirectional
            );
            let ss = matches!(
                method.streaming,
                RpcStreaming::Server | RpcStreaming::Bidirectional
            );
            s.push_str(&format!("{}:{{path:{},requestStream:{cs},responseStream:{ss},requestSerialize:(x:{input})=>Buffer.from(encode({input}Schema,x)),requestDeserialize:(x:Buffer)=>decode({input}Schema,x),responseSerialize:(x:{output})=>Buffer.from(encode({output}Schema,x)),responseDeserialize:(x:Buffer)=>decode({output}Schema,x)}},\n",quote(&super::layout::method(&method.name,options)),quote(&format!("/{}/{}",service.full_name,method.name))));
        }
        s.push_str("} as const;\n");
        if options.emit_servers {
            s.push_str(&format!(
                "export interface {name}Server extends UntypedServiceImplementation {{\n"
            ));
            for m in &service.methods {
                s.push_str(&documentation(RpcDefinitionKind::Method, &m.full_name));
                let i = &names[&m.input_type];
                let o = &names[&m.output_type];
                let handle = match m.streaming {
                    RpcStreaming::Unary => "handleUnaryCall",
                    RpcStreaming::Client => "handleClientStreamingCall",
                    RpcStreaming::Server => "handleServerStreamingCall",
                    RpcStreaming::Bidirectional => "handleBidiStreamingCall",
                };
                s.push_str(&format!(
                    "{}:{handle}<{i},{o}>;\n",
                    quote(&super::layout::method(&m.name, options))
                ));
            }
            s.push_str("}\n");
        }
        if options.emit_clients {
            s.push_str(&format!(
                "export interface {name}Client extends Client {{\n"
            ));
            for m in &service.methods {
                s.push_str(&documentation(RpcDefinitionKind::Method, &m.full_name));
                let i = &names[&m.input_type];
                let o = &names[&m.output_type];
                let n = super::layout::method(&m.name, options);
                let cb = format!("callback:(error:ServiceError|null,response:{o})=>void");
                s.push_str(&super::client::signatures(&n, i, o, &cb, m.streaming));
            }
            s.push_str("}\n");
            s.push_str(&format!("export const {name}Client=makeGenericClientConstructor({name}Service,{}) as unknown as {{new(address:string,credentials:ChannelCredentials,options?:ClientOptions):{name}Client}};\n",quote(&service.full_name)));
        }
    }
    s
}

pub(super) fn module(
    file: &RpcWireFile,
    names: &BTreeMap<String, String>,
    owners: &BTreeMap<String, String>,
    options: &RpcGenerationOptions,
    metadata: Option<&RpcFileMetadata>,
) -> Result<GeneratedFile> {
    let mut out = String::from(
        "import {Client,makeGenericClientConstructor,type ClientOptions,type ChannelCredentials,type Metadata,type CallOptions,type ClientUnaryCall,type ClientReadableStream,type ClientWritableStream,type ClientDuplexStream,type ServiceError,type UntypedServiceImplementation,type handleUnaryCall,type handleClientStreamingCall,type handleServerStreamingCall,type handleBidiStreamingCall} from '@grpc/grpc-js';\nimport type {GenMessage} from '@bufbuild/protobuf/codegenv2';\n",
    );
    out.push_str(&format!(
        "import {{registry,encode,decode}} from {};\n",
        quote(&super::render::relative(
            &file.name,
            "poolster-rpc-runtime.proto"
        ))
    ));
    let types = file
        .services
        .iter()
        .flat_map(|s| {
            s.methods
                .iter()
                .flat_map(|m| [&m.input_type, &m.output_type])
        })
        .collect::<std::collections::BTreeSet<_>>();
    for full in types {
        let name = &names[full];
        out.push_str(&format!("import type {{{name}}} from {};\nconst {name}Schema=registry.getMessage({})! as GenMessage<{name}>;\n",quote(&super::layout::import(&file.name,&owners[full],options,"")),quote(full)));
    }
    out.push_str(&render(file, names, options, metadata));
    GeneratedFile::new(
        format!("src/{}.grpc.ts", super::layout::stem(&file.name, options)),
        out,
    )
}
