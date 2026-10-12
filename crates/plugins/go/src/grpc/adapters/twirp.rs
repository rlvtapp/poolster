use super::*;
use std::fmt::Write;
pub(super) fn runtime(
    cx: &mut PluginContext<'_, crate::Go>,
    c: &plan::Context<'_>,
    options: &RpcGenerationOptions,
) -> Result<()> {
    let source = include_str!("twirp.go.tmpl")
        .replace("MAX_BYTES", &options.max_message_bytes.to_string())
        .replace("MAX_DEPTH", &options.max_depth.to_string())
        .replace(
            "DISCARD_UNKNOWN",
            if options.unknown_fields == RpcUnknownFields::Discard {
                "true"
            } else {
                "false"
            },
        );
    c.emit(
        cx,
        "poolster_twirp_runtime.pb.go",
        source,
        &[
            "context \"context\"",
            "http \"net/http\"",
            "bytes \"bytes\"",
            "io \"io\"",
            "json \"encoding/json\"",
            "fmt \"fmt\"",
            "proto \"google.golang.org/protobuf/proto\"",
            "protojson \"google.golang.org/protobuf/encoding/protojson\"",
            "status \"google.golang.org/grpc/status\"",
            "codes \"google.golang.org/grpc/codes\"",
        ],
    )
}
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
            writeln!(
                source,
                "type {n}TwirpClient struct {{ HTTPClient TwirpHTTPClient; BaseURL string; JSON bool }}\nfunc New{n}TwirpClient(client TwirpHTTPClient,url string)*{n}TwirpClient{{return &{n}TwirpClient{{HTTPClient:client,BaseURL:strings.TrimRight(url,\"/\")}}}}"
            )?;
            for m in &service.methods {
                let method = name(&m.name);
                let i = c.ty(&m.input_type)?;
                let o = c.ty(&m.output_type)?;
                writeln!(
                    source,
                    "func(client *{n}TwirpClient){method}(ctx context.Context,request *{i},headers http.Header)(*{o},http.Header,error){{"
                )?;
                if m.streaming != RpcStreaming::Unary {
                    writeln!(
                        source,
                        "return nil,nil,&RpcUnsupportedError{{Protocol:\"twirp\",Method:{n}_{method}_FullMethodName,Reason:\"Twirp supports unary methods only\"}}\n}}"
                    )?;
                } else {
                    writeln!(
                        source,
                        "response:=new({o});metadata,err:=poolsterTwirpCall(ctx,client.HTTPClient,client.BaseURL+{:?},request,response,headers,client.JSON);if err!=nil{{return nil,metadata,err}};return response,metadata,nil\n}}",
                        format!("/twirp/{}/{}", service.full_name, m.name)
                    )?;
                }
            }
        }
        if options.emit_servers {
            writeln!(source, "type {n}TwirpHandler interface{{")?;
            for m in &service.methods {
                if m.streaming == RpcStreaming::Unary {
                    writeln!(
                        source,
                        "{}(context.Context,*{})(*{},error)",
                        name(&m.name),
                        c.ty(&m.input_type)?,
                        c.ty(&m.output_type)?
                    )?;
                }
            }
            source.push_str("}\n");
            writeln!(
                source,
                "func New{n}TwirpHandler(implementation {n}TwirpHandler)(string,http.Handler){{mux:=http.NewServeMux()"
            )?;
            for m in &service.methods {
                let path = format!("/twirp/{}/{}", service.full_name, m.name);
                if m.streaming != RpcStreaming::Unary {
                    writeln!(
                        source,
                        "mux.HandleFunc({path:?},func(w http.ResponseWriter,r *http.Request){{poolsterTwirpError(w,\"unimplemented\",\"Twirp supports unary methods only\",501)}})"
                    )?;
                } else {
                    let i = c.ty(&m.input_type)?;
                    let method = name(&m.name);
                    writeln!(
                        source,
                        "mux.HandleFunc({path:?},func(w http.ResponseWriter,r *http.Request){{request:=new({i});if !poolsterTwirpRead(w,r,request){{return}};response,err:=implementation.{method}(r.Context(),request);if err!=nil{{poolsterTwirpStatus(w,err);return}};poolsterTwirpWrite(w,r,response)}})"
                    )?;
                }
            }
            writeln!(
                source,
                "return {:?},mux\n}}",
                format!("/twirp/{}/", service.full_name)
            )?;
        }
        if !source.is_empty() {
            c.emit(
                cx,
                &format!(
                    "{}_{}_twirp.pb.go",
                    super::super::messages::file_stem(&file.name),
                    n
                ),
                source,
                &[
                    "context \"context\"",
                    "http \"net/http\"",
                    "strings \"strings\"",
                ],
            )?;
        }
    }
    Ok(())
}
