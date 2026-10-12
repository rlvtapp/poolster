use anyhow::{Result, ensure};
use poolster_core::{GeneratedFile, native::rpc::*};
use std::collections::{BTreeMap, BTreeSet};
fn quote(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}
fn local(s: &str) -> String {
    format!("{}{}", s[..1].to_ascii_lowercase(), &s[1..])
}
fn contains(messages: &[RpcWireMessage], name: &str) -> bool {
    messages
        .iter()
        .any(|m| m.full_name == name || contains(&m.messages, name))
}
pub(super) fn generate(
    wire: &RpcWireSchema,
    options: &RpcGenerationOptions,
) -> Result<Vec<GeneratedFile>> {
    if !options.emit_clients {
        return Ok(vec![]);
    }
    let services = wire
        .files
        .iter()
        .flat_map(|f| f.services.iter().map(move |s| (f, s)))
        .collect::<Vec<_>>();
    if services.is_empty() {
        return Ok(vec![]);
    }
    let mut groups = BTreeSet::new();
    let mut methods = BTreeMap::<String, usize>::new();
    for (_, s) in &services {
        let group = local(s.full_name.rsplit('.').next().unwrap());
        ensure!(
            groups.insert(group.clone()),
            "Protobuf service groups collide: {group}"
        );
        for m in &s.methods {
            *methods
                .entry(super::layout::method(&m.name, options))
                .or_default() += 1;
        }
    }
    let mut source = String::from(
        "import type {ChannelCredentials,ClientOptions} from '@grpc/grpc-js';\nimport {unary,stream,upload,duplex,type RpcContext} from './poolster-rpc-facade.js';\nexport type {RpcContext,RpcUnary,RpcStream} from './poolster-rpc-facade.js';\n",
    );
    let mut reserved = BTreeSet::new();
    for (f, s) in &services {
        let name = s.full_name.rsplit('.').next().unwrap();
        let alias = format!("Native_{}", super::render::symbol(&s.full_name));
        source.push_str(&format!(
            "import {{{name}Client as {alias}}} from {};\n",
            quote(&super::layout::import(
                "client.proto",
                &f.name,
                options,
                ".grpc"
            ))
        ));
    }
    source.push_str("export interface RpcClientConfiguration {channelOptions?:ClientOptions;transport?:RawClient;}\nexport function createRawClient(address:string,credentials:ChannelCredentials,configuration:RpcClientConfiguration={}):RawClient {if(configuration.transport)return configuration.transport;return {\n");
    for (_, s) in &services {
        let group = local(s.full_name.rsplit('.').next().unwrap());
        let alias = format!("Native_{}", super::render::symbol(&s.full_name));
        source.push_str(&format!(
            "{}:new {alias}(address,credentials,configuration.channelOptions),\n",
            quote(&group)
        ));
    }
    source.push_str("};}\nexport type RawClient={\n");
    for (_, s) in &services {
        let group = local(s.full_name.rsplit('.').next().unwrap());
        let alias = format!("Native_{}", super::render::symbol(&s.full_name));
        source.push_str(&format!("{}:{alias};\n", quote(&group)));
    }
    source.push_str("};\n");
    let mut grouped = String::new();
    let mut flat = String::new();
    for (_, s) in &services {
        let group = local(s.full_name.rsplit('.').next().unwrap());
        grouped.push_str(&format!("{}:{{\n", quote(&group)));
        for m in &s.methods {
            let name = super::layout::method(&m.name, options);
            let flatname = if methods[&name] > 1 {
                format!("{group}{}{}", name[..1].to_ascii_uppercase(), &name[1..])
            } else {
                name.clone()
            };
            ensure!(
                reserved.insert(flatname.clone()) && flatname != "close" && flatname != "raw",
                "Protobuf flat client methods collide: {flatname}"
            );
            let raw = format!("raw[{}][{}]", quote(&group), quote(&name));
            let i = format!("Parameters<typeof {raw}>[0]");
            let function = match m.streaming {
                RpcStreaming::Unary => format!(
                    "(input:{i},context?:RpcContext)=>unary({raw}.bind(raw[{}]),input,context)",
                    quote(&group)
                ),
                RpcStreaming::Server => format!(
                    "(input:{i},context?:RpcContext)=>stream({raw}(input,context?.metadata??new Metadata(),context?.options??{{}}),context)"
                ),
                RpcStreaming::Client => format!(
                    "(input:AsyncIterable<{}>|Iterable<{}>,context?:RpcContext)=>upload({raw}.bind(raw[{}]),input,context)",
                    super::layout::owned(&m.input_type, options),
                    super::layout::owned(&m.input_type, options),
                    quote(&group)
                ),
                RpcStreaming::Bidirectional => format!(
                    "(input:AsyncIterable<{}>|Iterable<{}>,context?:RpcContext)=>duplex({raw}(context?.metadata??new Metadata(),context?.options??{{}}),input,context)",
                    super::layout::owned(&m.input_type, options),
                    super::layout::owned(&m.input_type, options)
                ),
            };
            grouped.push_str(&format!("{}:{function},\n", quote(&name)));
            flat.push_str(&format!("{}:{function},\n", quote(&flatname)));
        }
        grouped.push_str("},\n");
    }
    source = source.replace(
        "import type {ChannelCredentials",
        "import {Metadata,type ChannelCredentials",
    );
    // Input types for streaming use the same owned fully-qualified symbols.
    for file in &wire.files {
        let symbols = services
            .iter()
            .flat_map(|(_, s)| s.methods.iter())
            .filter(|m| {
                matches!(
                    m.streaming,
                    RpcStreaming::Client | RpcStreaming::Bidirectional
                )
            })
            .filter(|m| contains(&file.messages, &m.input_type))
            .map(|m| super::layout::owned(&m.input_type, options))
            .collect::<BTreeSet<_>>();
        if !symbols.is_empty() {
            source.push_str(&format!(
                "import type {{{}}} from {};\n",
                symbols.into_iter().collect::<Vec<_>>().join(","),
                quote(&super::layout::import(
                    "client.proto",
                    &file.name,
                    options,
                    ""
                ))
            ));
        }
    }
    let close = groups
        .iter()
        .map(|g| format!("raw[{}].close();", quote(g)))
        .collect::<String>();
    for (name, body) in [
        ("createFlatClient", flat),
        ("createIdiomaticClient", grouped),
    ] {
        source.push_str(&format!("export function {name}(address:string,credentials:ChannelCredentials,configuration:RpcClientConfiguration={{}}){{const raw=createRawClient(address,credentials,configuration);return {{raw,close:()=>{{{close}}},{body}}};}}\n"));
    }
    source.push_str("export type FlatClient=ReturnType<typeof createFlatClient>;\nexport type IdiomaticClient=ReturnType<typeof createIdiomaticClient>;\n");
    let selected = match options.client_style {
        RpcClientStyle::Raw => "createRawClient",
        RpcClientStyle::Flat => "createFlatClient",
        RpcClientStyle::Idiomatic => "createIdiomaticClient",
    };
    source.push_str(&format!("export const createClient={selected};\nexport type Client=ReturnType<typeof createClient>;\n"));
    Ok(vec![
        GeneratedFile::new("src/client.ts", source)?,
        GeneratedFile::new("src/poolster-rpc-facade.ts", include_str!("facade.ts.tmpl"))?,
    ])
}
