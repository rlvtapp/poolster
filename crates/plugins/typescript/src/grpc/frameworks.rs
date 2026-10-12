use anyhow::Result;
use poolster_core::{GeneratedFile, native::rpc::*};
fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}
pub(super) fn generate(
    wire: &RpcWireSchema,
    options: &RpcGenerationOptions,
) -> Result<Vec<GeneratedFile>> {
    let mut out = vec![];
    for framework in &options.frameworks {
        if *framework == RpcFramework::Native {
            continue;
        }
        anyhow::ensure!(
            options.emit_clients || options.emit_servers,
            "Protobuf framework adapters require an RPC surface"
        );
        let nice = *framework == RpcFramework::NiceGrpc;
        let mut s = if nice {
            String::from(
                "import {createClient,type Channel,type ServiceImplementation,type DefaultCallOptions,type NormalizedServiceDefinition} from 'nice-grpc';\n",
            )
        } else {
            String::from(
                "import {ClientProxyFactory,Transport,GrpcMethod,GrpcStreamMethod,type ClientGrpc,type ClientProxy,type GrpcOptions} from '@nestjs/microservices';\nimport type {Observable} from 'rxjs';\nimport type {Metadata} from '@grpc/grpc-js';\n",
            )
        };
        let mut definitions = vec![];
        for file in &wire.files {
            for service in &file.services {
                let name = service.full_name.rsplit('.').next().unwrap();
                let alias = format!("Service_{}", super::render::symbol(&service.full_name));
                s.push_str(&format!(
                    "import {{{name}Service as {alias}}} from {};\n",
                    q(&format!(
                        "./{}.grpc.js",
                        super::layout::stem(&file.name, options)
                    ))
                ));
                definitions.push(format!("{}:{alias}", q(&service.full_name)));
                if nice {
                    s.push_str(&format!("export const {name}NiceGrpcDefinition={alias};\n"));
                    if options.emit_servers {
                        s.push_str(&format!("export type {name}NiceGrpcImplementation=ServiceImplementation<typeof {alias}>;\n"));
                    }
                    if options.emit_clients {
                        s.push_str(&format!("export function create{name}NiceGrpcClient(channel:Channel,options?:DefaultCallOptions<NormalizedServiceDefinition<typeof {alias}>>){{return createClient({alias},channel,options);}}\n"));
                    }
                } else {
                    s.push_str(&format!("export interface {name}NestClient {{\n"));
                    for method in &service.methods {
                        let input = format!(
                            "Parameters<typeof {alias}[{}]['requestSerialize']>[0]",
                            q(&super::layout::method(&method.name, options))
                        );
                        let output = format!(
                            "ReturnType<typeof {alias}[{}]['responseDeserialize']>",
                            q(&super::layout::method(&method.name, options))
                        );
                        let streamed = matches!(
                            method.streaming,
                            RpcStreaming::Client | RpcStreaming::Bidirectional
                        );
                        s.push_str(&format!(
                            "{}(input:{},metadata?:Metadata{}):Observable<{output}>;\n",
                            q(&super::layout::method(&method.name, options)),
                            if streamed {
                                format!("Observable<{input}>")
                            } else {
                                input
                            },
                            if options.nestjs_rest_parameter {
                                ",...rest:unknown[]"
                            } else {
                                ""
                            }
                        ));
                    }
                    s.push_str("}\n");
                    if options.emit_clients {
                        s.push_str(&format!("export function get{name}NestClient(client:ClientGrpc):{name}NestClient{{return client.getService<{name}NestClient>({});}}\n",q(name)));
                    }
                    if options.emit_servers {
                        s.push_str(&format!("export function decorate{name}NestController(target:{{prototype:Record<string,unknown>}}):void{{\n"));
                        for method in &service.methods {
                            let decorator = if matches!(
                                method.streaming,
                                RpcStreaming::Client | RpcStreaming::Bidirectional
                            ) {
                                "GrpcStreamMethod"
                            } else {
                                "GrpcMethod"
                            };
                            s.push_str(&format!("{decorator}({},{})(target.prototype,{},Object.getOwnPropertyDescriptor(target.prototype,{})!);\n",q(name),q(&method.name),q(&super::layout::method(&method.name,options)),q(&super::layout::method(&method.name,options))));
                        }
                        s.push_str("}\n");
                    }
                }
            }
        }
        if !nice {
            let packages = wire
                .files
                .iter()
                .filter(|f| !f.services.is_empty())
                .map(|f| f.package.clone())
                .collect::<std::collections::BTreeSet<_>>();
            s.push_str(&format!("export function nestGrpcOptions(url:string):GrpcOptions{{return{{transport:Transport.GRPC,options:{{url,package:{},packageDefinition:{{{}}}}}}};}}\n",serde_json::to_string(&packages)?,definitions.join(",")));
            if options.emit_clients {
                s.push_str("export function createNestGrpcClient(url:string):ClientGrpc&ClientProxy{return ClientProxyFactory.create(nestGrpcOptions(url)) as ClientGrpc&ClientProxy;}\n");
            }
        }
        out.push(GeneratedFile::new(
            if nice {
                "src/nice-grpc.ts"
            } else {
                "src/nestjs.ts"
            },
            s,
        )?);
    }
    Ok(out)
}
