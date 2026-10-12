use super::*;
use super::{messages::file_stem, plan::*};
use poolster_core::native::rpc::*;
use std::fmt::Write;

pub(super) fn emit(
    cx: &mut PluginContext<'_, crate::Go>,
    file: &RpcWireFile,
    c: &plan::Context<'_>,
    options: &RpcGenerationOptions,
) -> Result<()> {
    if file.services.is_empty() || (!options.emit_clients && !options.emit_servers) {
        return Ok(());
    }
    let mut s = String::from("const _ = grpc.SupportPackageIsVersion9\n");
    for service in &file.services {
        render(&mut s, service, c, options)?;
    }
    c.emit(
        cx,
        &format!("{}_grpc.pb.go", file_stem(&file.name)),
        s,
        &[
            "context \"context\"",
            "grpc \"google.golang.org/grpc\"",
            "codes \"google.golang.org/grpc/codes\"",
            "status \"google.golang.org/grpc/status\"",
        ],
    )
}
fn render(
    s: &mut String,
    service: &RpcService,
    c: &plan::Context<'_>,
    options: &RpcGenerationOptions,
) -> Result<()> {
    let n = name(service.full_name.rsplit('.').next().unwrap());
    let concrete = format!("poolster{n}Client");
    for reserved in [
        format!("{n}Client"),
        format!("{n}Server"),
        format!("Unimplemented{n}Server"),
        format!("{n}_ServiceDesc"),
        format!("New{n}Client"),
        format!("Register{n}Server"),
    ] {
        ensure!(
            !c.plan
                .types
                .values()
                .any(|t| t.import == c.import && t.name == reserved),
            "Go RPC helper symbol collision: {reserved}"
        );
    }
    let mut methods = std::collections::BTreeSet::new();
    for method in &service.methods {
        ensure!(
            methods.insert(name(&method.name)),
            "Go RPC method name collision in {}: {}",
            service.full_name,
            method.name
        );
    }

    for m in &service.methods {
        writeln!(
            s,
            "const {n}_{}_FullMethodName={:?}",
            name(&m.name),
            format!("/{}/{}", service.full_name, m.name)
        )?;
    }
    if options.emit_clients {
        s.push_str(&super::documentation::comment(
            c,
            RpcDefinitionKind::Service,
            &service.full_name,
        )?);
        writeln!(s, "type {n}Client interface {{")?;
        for m in &service.methods {
            s.push_str(&super::documentation::comment(
                c,
                RpcDefinitionKind::Method,
                &format!("{}.{}", service.full_name, m.name),
            )?);
            writeln!(s, "{}{}", name(&m.name), client_signature(m, c)?)?;
        }
        s.push_str("}\n");
        writeln!(
            s,
            "type {concrete} struct {{ cc grpc.ClientConnInterface }}\nfunc New{n}Client(cc grpc.ClientConnInterface) {n}Client {{return &{concrete}{{cc:cc}}}}"
        )?;
        for m in &service.methods {
            let method = name(&m.name);
            let input = c.ty(&m.input_type)?;
            let output = c.ty(&m.output_type)?;
            writeln!(
                s,
                "func (c *{concrete}) {method}{} {{",
                client_signature(m, c)?
            )?;
            if m.streaming == RpcStreaming::Unary {
                writeln!(
                    s,
                    "out:=new({output});err:=c.cc.Invoke(ctx,{n}_{method}_FullMethodName,in,out,opts...);if err!=nil{{return nil,err}};return out,nil"
                )?;
            } else {
                writeln!(
                    s,
                    "stream,err:=c.cc.NewStream(ctx,&grpc.StreamDesc{{ServerStreams:{},ClientStreams:{}}},{n}_{method}_FullMethodName,opts...);if err!=nil{{return nil,err}}\nx:=&grpc.GenericClientStream[{input},{output}]{{ClientStream:stream}}",
                    matches!(
                        m.streaming,
                        RpcStreaming::Server | RpcStreaming::Bidirectional
                    ),
                    matches!(
                        m.streaming,
                        RpcStreaming::Client | RpcStreaming::Bidirectional
                    )
                )?;
                if m.streaming == RpcStreaming::Server {
                    writeln!(
                        s,
                        "if err=x.SendMsg(in);err!=nil{{return nil,err}};if err=x.CloseSend();err!=nil{{return nil,err}}"
                    )?;
                }
                s.push_str("return x,nil\n");
            }
            s.push_str("}\n");
            if m.streaming != RpcStreaming::Unary {
                writeln!(s, "type {n}_{method}Client = {}", client_stream(m, c)?)?;
            }
        }
    }
    if options.emit_servers {
        writeln!(s, "type {n}Server interface {{")?;
        for m in &service.methods {
            writeln!(s, "{}{}", name(&m.name), server_signature(m, c)?)?;
        }
        writeln!(
            s,
            "mustEmbedUnimplemented{n}Server()\n}}\ntype Unimplemented{n}Server struct {{}}"
        )?;
        for m in &service.methods {
            let method = name(&m.name);
            writeln!(
                s,
                "func (Unimplemented{n}Server) {method}{} {{",
                server_signature(m, c)?
            )?;
            writeln!(
                s,
                "return {}status.Error(codes.Unimplemented,{:?})\n}}",
                if m.streaming == RpcStreaming::Unary {
                    "nil,"
                } else {
                    ""
                },
                format!("method {} not implemented", m.name)
            )?;
        }
        writeln!(
            s,
            "func (Unimplemented{n}Server) mustEmbedUnimplemented{n}Server() {{}}\nfunc (Unimplemented{n}Server) testEmbeddedByValue() {{}}\ntype Unsafe{n}Server interface {{mustEmbedUnimplemented{n}Server()}}\nfunc Register{n}Server(s grpc.ServiceRegistrar,srv {n}Server) {{if t,ok:=srv.(interface{{testEmbeddedByValue()}});ok{{t.testEmbeddedByValue()}};s.RegisterService(&{n}_ServiceDesc,srv)}}"
        )?;
        for m in &service.methods {
            let method = name(&m.name);
            let input = c.ty(&m.input_type)?;
            let output = c.ty(&m.output_type)?;
            if m.streaming == RpcStreaming::Unary {
                writeln!(
                    s,
                    "func _{n}_{method}_Handler(srv any,ctx context.Context,dec func(any)error,interceptor grpc.UnaryServerInterceptor)(any,error){{\nin:=new({input});if err:=dec(in);err!=nil{{return nil,err}};if interceptor==nil{{return srv.({n}Server).{method}(ctx,in)}}\ninfo:=&grpc.UnaryServerInfo{{Server:srv,FullMethod:{n}_{method}_FullMethodName}}\nhandler:=func(ctx context.Context,req any)(any,error){{return srv.({n}Server).{method}(ctx,req.(*{input}))}}\nreturn interceptor(ctx,in,info,handler)\n}}"
                )?;
            } else {
                writeln!(
                    s,
                    "type {n}_{method}Server = {}\nfunc _{n}_{method}_Handler(srv any,stream grpc.ServerStream)error{{",
                    server_stream(m, c)?
                )?;
                if m.streaming == RpcStreaming::Server {
                    writeln!(
                        s,
                        "in:=new({input});if err:=stream.RecvMsg(in);err!=nil{{return err}};return srv.({n}Server).{method}(in,&grpc.GenericServerStream[{input},{output}]{{ServerStream:stream}})"
                    )?;
                } else {
                    writeln!(
                        s,
                        "return srv.({n}Server).{method}(&grpc.GenericServerStream[{input},{output}]{{ServerStream:stream}})"
                    )?;
                }
                s.push_str("}\n");
            }
        }
        writeln!(
            s,
            "var {n}_ServiceDesc=grpc.ServiceDesc{{ServiceName:{:?},HandlerType:(*{n}Server)(nil),Methods:[]grpc.MethodDesc{{",
            service.full_name
        )?;
        for m in &service.methods {
            if m.streaming == RpcStreaming::Unary {
                writeln!(
                    s,
                    "{{MethodName:{:?},Handler:_{n}_{}_Handler}},",
                    m.name,
                    name(&m.name)
                )?;
            }
        }
        s.push_str("},Streams:[]grpc.StreamDesc{\n");
        for m in &service.methods {
            if m.streaming != RpcStreaming::Unary {
                writeln!(
                    s,
                    "{{StreamName:{:?},Handler:_{n}_{}_Handler,ServerStreams:{},ClientStreams:{}}},",
                    m.name,
                    name(&m.name),
                    matches!(
                        m.streaming,
                        RpcStreaming::Server | RpcStreaming::Bidirectional
                    ),
                    matches!(
                        m.streaming,
                        RpcStreaming::Client | RpcStreaming::Bidirectional
                    )
                )?;
            }
        }
        writeln!(s, "}},Metadata:{:?}}}", service.file)?;
    }
    Ok(())
}
pub(super) fn client_stream(m: &RpcMethod, c: &plan::Context<'_>) -> Result<String> {
    let i = c.ty(&m.input_type)?;
    let o = c.ty(&m.output_type)?;
    Ok(match m.streaming {
        RpcStreaming::Server => format!("grpc.ServerStreamingClient[{o}]"),
        RpcStreaming::Client => format!("grpc.ClientStreamingClient[{i},{o}]"),
        _ => format!("grpc.BidiStreamingClient[{i},{o}]"),
    })
}
fn server_stream(m: &RpcMethod, c: &plan::Context<'_>) -> Result<String> {
    let i = c.ty(&m.input_type)?;
    let o = c.ty(&m.output_type)?;
    Ok(match m.streaming {
        RpcStreaming::Server => format!("grpc.ServerStreamingServer[{o}]"),
        RpcStreaming::Client => format!("grpc.ClientStreamingServer[{i},{o}]"),
        _ => format!("grpc.BidiStreamingServer[{i},{o}]"),
    })
}
pub(super) fn client_signature(m: &RpcMethod, c: &plan::Context<'_>) -> Result<String> {
    Ok(match m.streaming {
        RpcStreaming::Unary => format!(
            "(ctx context.Context,in *{},opts ...grpc.CallOption)(*{},error)",
            c.ty(&m.input_type)?,
            c.ty(&m.output_type)?
        ),
        RpcStreaming::Server => format!(
            "(ctx context.Context,in *{},opts ...grpc.CallOption)({},error)",
            c.ty(&m.input_type)?,
            client_stream(m, c)?
        ),
        _ => format!(
            "(ctx context.Context,opts ...grpc.CallOption)({},error)",
            client_stream(m, c)?
        ),
    })
}
fn server_signature(m: &RpcMethod, c: &plan::Context<'_>) -> Result<String> {
    Ok(match m.streaming {
        RpcStreaming::Unary => format!(
            "(context.Context,*{})(*{},error)",
            c.ty(&m.input_type)?,
            c.ty(&m.output_type)?
        ),
        RpcStreaming::Server => {
            format!("(*{}, {})error", c.ty(&m.input_type)?, server_stream(m, c)?)
        }
        _ => format!("({})error", server_stream(m, c)?),
    })
}
