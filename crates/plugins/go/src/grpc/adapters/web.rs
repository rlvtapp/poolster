use super::*;
use std::fmt::Write;
pub(super) fn emit(
    cx: &mut PluginContext<'_, crate::Go>,
    file: &RpcWireFile,
    c: &plan::Context<'_>,
    options: &RpcGenerationOptions,
) -> Result<()> {
    for service in &file.services {
        let n = name(service.full_name.rsplit('.').next().unwrap());
        let mut source = String::new();
        if options.emit_clients {
            writeln!(source, "type {n}GRPCWebClient struct {{")?;
            for m in &service.methods {
                if matches!(m.streaming, RpcStreaming::Unary | RpcStreaming::Server) {
                    writeln!(
                        source,
                        "operation{} *connect.Client[{},{}]",
                        name(&m.name),
                        c.ty(&m.input_type)?,
                        c.ty(&m.output_type)?
                    )?;
                }
            }
            source.push_str("}\n");
            writeln!(
                source,
                "func New{n}GRPCWebClient(httpClient connect.HTTPClient,baseURL string,options ...connect.ClientOption)*{n}GRPCWebClient{{\nbaseURL=strings.TrimRight(baseURL,\"/\")\nopts:=append([]connect.ClientOption{{}},options...)\nopts=append(opts,connect.WithGRPCWeb(),connect.WithCodec(poolsterWebCodec{{}}),connect.WithReadMaxBytes({}),connect.WithSendMaxBytes({}))\nreturn &{n}GRPCWebClient{{",
                options.max_message_bytes, options.max_message_bytes
            )?;
            for m in &service.methods {
                if matches!(m.streaming, RpcStreaming::Unary | RpcStreaming::Server) {
                    let method = name(&m.name);
                    writeln!(
                        source,
                        "operation{method}:connect.NewClient[{},{}](httpClient,baseURL+{n}_{method}_FullMethodName,opts...),",
                        c.ty(&m.input_type)?,
                        c.ty(&m.output_type)?
                    )?;
                }
            }
            source.push_str("}}\n");
            for m in &service.methods {
                let method = name(&m.name);
                let i = c.ty(&m.input_type)?;
                let o = c.ty(&m.output_type)?;
                match m.streaming {
                    RpcStreaming::Unary => writeln!(
                        source,
                        "func (client *{n}GRPCWebClient) {method}(ctx context.Context,request *connect.Request[{i}])(*connect.Response[{o}],error){{return client.operation{method}.CallUnary(ctx,request)}}"
                    )?,
                    RpcStreaming::Server => writeln!(
                        source,
                        "func (client *{n}GRPCWebClient) {method}(ctx context.Context,request *connect.Request[{i}])(*connect.ServerStreamForClient[{o}],error){{return client.operation{method}.CallServerStream(ctx,request)}}"
                    )?,
                    RpcStreaming::Client => writeln!(
                        source,
                        "func (client *{n}GRPCWebClient) {method}(ctx context.Context)(*connect.ClientStreamForClient[{i},{o}],error){{return nil,&RpcUnsupportedError{{Protocol:\"grpcWeb\",Method:{n}_{method}_FullMethodName,Reason:\"client streaming is unavailable\"}}}}"
                    )?,
                    RpcStreaming::Bidirectional => writeln!(
                        source,
                        "func (client *{n}GRPCWebClient) {method}(ctx context.Context)(*connect.BidiStreamForClient[{i},{o}],error){{return nil,&RpcUnsupportedError{{Protocol:\"grpcWeb\",Method:{n}_{method}_FullMethodName,Reason:\"bidirectional streaming is unavailable\"}}}}"
                    )?,
                }
            }
        }
        if options.emit_servers {
            writeln!(source, "type {n}GRPCWebHandler interface{{")?;
            for m in &service.methods {
                let method = name(&m.name);
                let i = c.ty(&m.input_type)?;
                let o = c.ty(&m.output_type)?;
                if m.streaming == RpcStreaming::Unary {
                    writeln!(
                        source,
                        "{method}(context.Context,*connect.Request[{i}])(*connect.Response[{o}],error)"
                    )?;
                } else if m.streaming == RpcStreaming::Server {
                    writeln!(
                        source,
                        "{method}(context.Context,*connect.Request[{i}],*connect.ServerStream[{o}])error"
                    )?;
                }
            }
            source.push_str("}\n");
            writeln!(
                source,
                "func New{n}GRPCWebHandler(implementation {n}GRPCWebHandler,options ...connect.HandlerOption)(string,http.Handler){{\nopts:=append([]connect.HandlerOption{{}},options...)\nopts=append(opts,connect.WithCodec(poolsterWebCodec{{}}),connect.WithReadMaxBytes({}),connect.WithSendMaxBytes({}))\nmux:=http.NewServeMux()",
                options.max_message_bytes, options.max_message_bytes
            )?;
            for m in &service.methods {
                let method = name(&m.name);
                let i = c.ty(&m.input_type)?;
                let o = c.ty(&m.output_type)?;
                let handler = match m.streaming {
                    RpcStreaming::Unary => format!(
                        "connect.NewUnaryHandler({n}_{method}_FullMethodName,implementation.{method},opts...)"
                    ),
                    RpcStreaming::Server => format!(
                        "connect.NewServerStreamHandler({n}_{method}_FullMethodName,implementation.{method},opts...)"
                    ),
                    RpcStreaming::Client => format!(
                        "connect.NewClientStreamHandler({n}_{method}_FullMethodName,func(context.Context,*connect.ClientStream[{i}])(*connect.Response[{o}],error){{return nil,connect.NewError(connect.CodeUnimplemented,&RpcUnsupportedError{{Protocol:\"grpcWeb\",Method:{n}_{method}_FullMethodName,Reason:\"client streaming is unavailable\"}})}},opts...)"
                    ),
                    RpcStreaming::Bidirectional => format!(
                        "connect.NewBidiStreamHandler({n}_{method}_FullMethodName,func(context.Context,*connect.BidiStream[{i},{o}])error{{return connect.NewError(connect.CodeUnimplemented,&RpcUnsupportedError{{Protocol:\"grpcWeb\",Method:{n}_{method}_FullMethodName,Reason:\"bidirectional streaming is unavailable\"}})}},opts...)"
                    ),
                };
                writeln!(source, "mux.Handle({n}_{method}_FullMethodName,{handler})")?;
            }
            writeln!(
                source,
                "return {:?},mux\n}}",
                format!("/{}/", service.full_name)
            )?;
        }
        if !source.is_empty() {
            c.emit(
                cx,
                &format!(
                    "{}_{}_grpcweb.pb.go",
                    super::super::messages::file_stem(&file.name),
                    n
                ),
                source,
                &[
                    "context \"context\"",
                    "http \"net/http\"",
                    "strings \"strings\"",
                    "connect \"connectrpc.com/connect\"",
                ],
            )?;
        }
    }
    Ok(())
}
