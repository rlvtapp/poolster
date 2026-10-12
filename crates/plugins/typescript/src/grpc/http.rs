use anyhow::Result;
use poolster_core::{GeneratedFile, native::rpc::*};
fn quote(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}
pub(super) fn generate(
    wire: &RpcWireSchema,
    options: &RpcGenerationOptions,
) -> Result<Vec<GeneratedFile>> {
    if !options.emit_clients {
        return Ok(vec![]);
    }
    let transports = options
        .transports
        .iter()
        .filter(|t| !matches!(t, RpcTransport::Grpc))
        .collect::<Vec<_>>();
    if transports.is_empty() {
        return Ok(vec![]);
    }
    let mut out = vec![GeneratedFile::new(
        "src/poolster-rpc-http.ts",
        include_str!("http.ts.tmpl").replace("MAX_BYTES", &options.max_message_bytes.to_string()),
    )?];
    for transport in transports {
        let (name, kind) = match transport {
            RpcTransport::GrpcWeb => ("GrpcWeb", "grpcWeb"),
            RpcTransport::Twirp => ("Twirp", "twirp"),
            _ => unreachable!(),
        };
        let mut s = String::from(
            "import {registry,encode,decode} from './poolster-rpc-runtime.js';\nimport type {GenMessage} from '@bufbuild/protobuf/codegenv2';\nimport {grpcWeb,twirp,type RpcHttpOptions,RpcHttpError} from './poolster-rpc-http.js';\nexport {RpcHttpError};export type {RpcHttpOptions};\n",
        );
        for f in &wire.files {
            for service in &f.services {
                let service_name = service.full_name.rsplit('.').next().unwrap();
                s.push_str(&format!("export function create{service_name}{name}Client(baseUrl:string,defaults:RpcHttpOptions={{}}){{return {{\n"));
                for method in &service.methods {
                    let field = super::layout::method(&method.name, options);
                    let type_ref = |full: &str| {
                        let owner = wire
                            .files
                            .iter()
                            .find(|f| contains(&f.messages, full))
                            .unwrap();
                        let suffix = if !owner.services.is_empty() {
                            ".messages"
                        } else {
                            ""
                        };
                        format!(
                            "import({}).Pb_{}",
                            quote(&super::layout::import(
                                "client.proto",
                                &owner.name,
                                options,
                                suffix
                            )),
                            super::layout::type_name(full, options)
                        )
                    };
                    let input = type_ref(&method.input_type);
                    let output = type_ref(&method.output_type);
                    let supported = matches!(method.streaming, RpcStreaming::Unary)
                        || (*transport == RpcTransport::GrpcWeb
                            && matches!(method.streaming, RpcStreaming::Server));
                    if !supported {
                        s.push_str(&format!("{field}(_input:{input},_options:RpcHttpOptions={{}}):never{{throw new RpcHttpError('unimplemented',{});}},\n",quote(&format!("{name} does not support {:?} streaming",method.streaming))));
                        continue;
                    }
                    let path = if *transport == RpcTransport::Twirp {
                        format!("/twirp/{}/{}", service.full_name, method.name)
                    } else {
                        format!("/{}/{}", service.full_name, method.name)
                    };
                    let schemas = format!(
                        "const request=registry.getMessage({})! as GenMessage<{input}>;const response=registry.getMessage({})! as GenMessage<{output}>;const settings={{...defaults,...options}};const url=baseUrl.replace(/\\/$/,'')+{};",
                        quote(&method.input_type),
                        quote(&method.output_type),
                        quote(&path)
                    );
                    if matches!(method.streaming, RpcStreaming::Server) {
                        s.push_str(&format!("async *{field}(input:{input},options:RpcHttpOptions={{}}):AsyncGenerator<{output}>{{{schemas}for await(const bytes of grpcWeb(url,encode(request,input),settings))yield decode(response,bytes);}},\n"));
                    } else if kind == "twirp" {
                        s.push_str(&format!("async {field}(input:{input},options:RpcHttpOptions={{}}):Promise<{output}>{{{schemas}return decode(response,await twirp(url,encode(request,input),settings));}},\n"));
                    } else {
                        s.push_str(&format!("async {field}(input:{input},options:RpcHttpOptions={{}}):Promise<{output}>{{{schemas}let result:{output}|undefined;for await(const bytes of grpcWeb(url,encode(request,input),settings)){{if(result)throw new RpcHttpError('internal','Multiple unary responses');result=decode(response,bytes);}}if(!result)throw new RpcHttpError('internal','Missing unary response');return result;}},\n"));
                    }
                }
                s.push_str("};}\n");
            }
        }
        out.push(GeneratedFile::new(format!("src/{kind}.ts"), s)?);
    }
    Ok(out)
}
fn contains(messages: &[RpcWireMessage], name: &str) -> bool {
    messages
        .iter()
        .any(|m| m.full_name == name || contains(&m.messages, name))
}
