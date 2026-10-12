use anyhow::{ensure, Result};
use poolster_core::{native::rpc::*, GeneratedFile};
use std::collections::BTreeMap;
pub(super) fn quote(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}
pub(super) fn ident(s: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in s.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}
pub(super) fn property(s: &str) -> String {
    let name = ident(s);
    if matches!(
        name.as_str(),
        "constructor" | "toString" | "toJSON" | "valueOf"
    ) {
        format!("{name}$")
    } else {
        name
    }
}
pub(super) fn symbol(s: &str) -> String {
    s.split('.').map(ident).collect::<Vec<_>>().join("_")
}
pub(super) fn fields<'a>(messages: &'a [RpcWireMessage], result: &mut Vec<&'a RpcWireMessage>) {
    for m in messages {
        if !m.map_entry {
            result.push(m);
        }
        fields(&m.messages, result);
    }
}
pub(super) fn enums<'a>(messages: &'a [RpcWireMessage], result: &mut Vec<&'a RpcWireEnum>) {
    for m in messages {
        result.extend(&m.enums);
        enums(&m.messages, result);
    }
}
pub(super) fn relative(from: &str, to: &str) -> String {
    let mut a = from.split('/').collect::<Vec<_>>();
    a.pop();
    let b = to.split('/').collect::<Vec<_>>();
    let common = a.iter().zip(&b).take_while(|(a, b)| a == b).count();
    let mut parts = vec![".."; a.len() - common];
    parts.extend(&b[common..]);
    let s = parts.join("/");
    let s = s.strip_suffix(".proto").unwrap_or(&s);
    format!("{}{}.js", if s.starts_with("..") { "" } else { "./" }, s)
}
pub(super) fn ty(kind: &RpcFieldKind, names: &BTreeMap<String, String>) -> Result<String> {
    Ok(match kind {
        RpcFieldKind::Enum(n) => format!(
            "({}|number)",
            names
                .get(n)
                .ok_or_else(|| anyhow::anyhow!("unknown Protobuf enum {n}"))?
        ),
        RpcFieldKind::Message(n) | RpcFieldKind::Group(n) => names
            .get(n)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unknown Protobuf type {n}"))?,
        RpcFieldKind::String => "string".into(),
        RpcFieldKind::Bytes => "Uint8Array".into(),
        RpcFieldKind::Bool => "boolean".into(),
        RpcFieldKind::Int64
        | RpcFieldKind::Uint64
        | RpcFieldKind::Sint64
        | RpcFieldKind::Fixed64
        | RpcFieldKind::Sfixed64 => "bigint".into(),
        _ => "number".into(),
    })
}
pub(super) fn field_ty(
    kind: &RpcFieldKind,
    names: &BTreeMap<String, String>,
    javascript_type: Option<RpcJavaScriptType>,
) -> Result<String> {
    let native = ty(kind, names)?;
    Ok(
        if native == "bigint" && javascript_type == Some(RpcJavaScriptType::String) {
            "string".into()
        } else {
            native
        },
    )
}
pub fn generate(
    wire: &RpcWireSchema,
    descriptors: &[u8],
    options: &RpcGenerationOptions,
) -> Result<Vec<GeneratedFile>> {
    let metadata = wire.metadata()?;
    let runtime = options.emit_binary
        || options.emit_json
        || options.emit_clients
        || options.emit_servers
        || options.emit_reflection;
    let rpc = options.emit_clients || options.emit_servers;
    let mut message_names = std::collections::BTreeSet::new();
    let mut names = BTreeMap::new();
    let mut symbols = std::collections::BTreeSet::new();
    let mut owners = BTreeMap::new();
    let mut maps = BTreeMap::new();
    fn collect_maps<'a>(ms: &'a [RpcWireMessage], out: &mut BTreeMap<String, &'a RpcWireMessage>) {
        for m in ms {
            if m.map_entry {
                out.insert(m.full_name.clone(), m);
            }
            collect_maps(&m.messages, out);
        }
    }
    for f in &wire.files {
        ensure!(
            !(options.emit_clients
                && matches!(
                    f.name.as_str(),
                    "client.proto" | "poolster-rpc-facade.proto"
                )),
            "Protobuf file collides with generated client module"
        );
        ensure!(
            f.name != "poolster-rpc-runtime.proto",
            "Protobuf file collides with Poolster runtime module"
        );
        ensure!(
            f.name.ends_with(".proto")
                && !f.name.starts_with('/')
                && !f.name.contains(['\\', ':'])
                && f.name
                    .split('/')
                    .all(|p| !p.is_empty() && p != ".." && p != "."),
            "unsafe Protobuf path"
        );
        let mut ms = vec![];
        fields(&f.messages, &mut ms);
        collect_maps(&f.messages, &mut maps);
        message_names.extend(ms.iter().map(|m| m.full_name.clone()));
        let mut es = f.enums.iter().collect::<Vec<_>>();
        enums(&f.messages, &mut es);
        for (full, name) in ms
            .iter()
            .map(|m| (&m.full_name, &m.name))
            .chain(es.iter().map(|e| (&e.full_name, &e.name)))
        {
            let _ = name;
            ensure!(
                symbols.insert(super::layout::owned(full, options)),
                "Protobuf type names collide after TypeScript normalization: {full}"
            );
            names.insert(full.clone(), super::layout::owned(full, options));
            owners.insert(full.clone(), f.name.clone());
        }
    }
    super::naming::validate(wire, &names, &owners, options)?;
    let mut out = Vec::new();
    if runtime {
        let mut combined = String::new();
        for (index, bytes) in descriptors.chunks(8192).enumerate() {
            let filename = format!("poolster-rpc-descriptor-{index}");
            ensure!(
                !wire
                    .files
                    .iter()
                    .any(|f| f.name == format!("{filename}.proto")),
                "Protobuf source collides with descriptor chunk"
            );
            out.push(GeneratedFile::new(
                format!("src/{filename}.ts"),
                format!(
                    "export const bytes=Uint8Array.from({});\n",
                    serde_json::to_string(bytes)?
                ),
            )?);
            combined.push_str(&format!(
                "import {{bytes as chunk{index}}} from './{filename}.js';\n"
            ));
        }
        combined.push_str(&format!(
            "export const descriptorBytes=new Uint8Array({});\n",
            descriptors.len()
        ));
        let mut offset = 0;
        for (index, bytes) in descriptors.chunks(8192).enumerate() {
            combined.push_str(&format!("descriptorBytes.set(chunk{index},{offset});\n"));
            offset += bytes.len();
        }
        ensure!(
            !wire
                .files
                .iter()
                .any(|f| f.name == "poolster-rpc-descriptors.proto"),
            "Protobuf source collides with descriptor registry"
        );
        out.push(GeneratedFile::new(
            "src/poolster-rpc-descriptors.ts",
            combined,
        )?);
        out.push(GeneratedFile::new(
            "src/poolster-rpc-runtime.ts",
            super::helpers::runtime(options),
        )?);
    }
    for f in &wire.files {
        if options.import_mappings.contains_key(&f.name) {
            continue;
        }
        let mut s = if options.annotate_files_with_version {
            String::from(
                "// Generated by Poolster 0.5.0-alpha.1. Native Protobuf descriptors remain authoritative.\n",
            )
        } else {
            String::new()
        };
        if runtime {
            s.push_str("import {create,type Message,type MessageInitShape,type JsonValue,type JsonReadOptions,type JsonWriteOptions} from '@bufbuild/protobuf';\nimport type {GenMessage,GenExtension} from '@bufbuild/protobuf/codegenv2';\n");
            s.push_str(&format!(
                "import {{registry,encode,decode,parseJson,stringifyJson}} from {};\n",
                quote(&relative(&f.name, "poolster-rpc-runtime.proto"))
            ));
        } else {
            s.push_str("type Message<N extends string>={readonly $typeName:N};\n");
        }
        for other in &wire.files {
            if other.name == f.name {
                continue;
            }
            let exports = names
                .iter()
                .filter(|(n, _)| owners[*n] == other.name)
                .map(|(_, n)| n.clone())
                .collect::<Vec<_>>();
            if !exports.is_empty() {
                s.push_str(&format!(
                    "import type {{{}}} from {};\n",
                    exports
                        .iter()
                        .cloned()
                        .chain(if options.representations.is_native() {
                            vec![]
                        } else {
                            names
                                .iter()
                                .filter(|(full, _)| {
                                    owners[*full] == other.name && message_names.contains(*full)
                                })
                                .map(|(_, n)| format!("{n}Value"))
                                .collect::<Vec<_>>()
                        })
                        .collect::<Vec<_>>()
                        .join(","),
                    quote(&super::layout::import(&f.name, &other.name, options, ""))
                ));
            }
        }
        if rpc {
            for (full, n) in &names {
                if owners[full] != f.name && message_names.contains(full) {
                    s.push_str(&format!(
                        "const {n}Schema=registry.getMessage({})! as GenMessage<{n}>;\n",
                        quote(full)
                    ));
                }
            }
        }
        s.push_str(&super::shapes::render(
            f,
            &names,
            &maps,
            options,
            metadata.iter().find(|m| m.file == f.name),
        )?);
        s.push_str(&super::value_shapes::render(
            f,
            &names,
            &maps,
            options,
            metadata.iter().find(|m| m.file == f.name),
        )?);
        if runtime && options.emit_reflection {
            s.push_str(&super::extensions::render(
                f,
                &names,
                metadata.iter().find(|m| m.file == f.name),
            )?);
        }
        if (options.emit_clients || options.emit_servers) && !f.services.is_empty() {
            out.push(super::services::module(
                f,
                &names,
                &owners,
                options,
                metadata.iter().find(|m| m.file == f.name),
            )?);
        }
        for (full, n) in &names {
            if owners[full] == f.name {
                let short = super::layout::short(full, &f.package, options);
                if short != *n {
                    if options.emit_types {
                        s.push_str(&format!(
                            "export {}{{ {n} as {short} }};\n",
                            if runtime { "" } else { "type " }
                        ));
                    }
                    if runtime && options.emit_reflection {
                        s.push_str(&format!("export {{ {n}Schema as {short}Schema }};\n"));
                    }
                    if !message_names.contains(full) && runtime {
                        s.push_str(&format!("export {{ {n}Name as {short}Name }};\n"));
                    }
                    if message_names.contains(full) {
                        for (enabled, suffix) in [
                            (
                                options.emit_binary && super::helpers::has_binary(full, options),
                                "Binary",
                            ),
                            (options.emit_json, "Json"),
                            (
                                runtime
                                    && (options.emit_binary
                                        || options.emit_json
                                        || options.emit_reflection),
                                "Create",
                            ),
                        ] {
                            if enabled {
                                s.push_str(&format!(
                                    "export {{ {n}{suffix} as {short}{suffix} }};\n"
                                ));
                            }
                        }
                    }
                }
            }
        }
        let stem = super::layout::stem(&f.name, options);
        if rpc && !f.services.is_empty() {
            out.push(GeneratedFile::new(format!("src/{stem}.messages.ts"), s)?);
            let leaf = stem.rsplit('/').next().unwrap();
            out.push(GeneratedFile::new(
                format!("src/{stem}.ts"),
                format!(
                    "export * from './{leaf}.messages.js';\nexport * from './{leaf}.grpc.js';\n"
                ),
            )?);
        } else {
            out.push(GeneratedFile::new(format!("src/{stem}.ts"), s)?);
        }
    }
    Ok(out)
}
