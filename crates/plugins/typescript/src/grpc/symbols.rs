//! Typed artifact references for downstream plugins, without source reparsing.
use poolster_core::{blocks::ContractReference, engine::Contract, native::rpc::*};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProtobufMessageSymbols {
    pub module: String,
    pub type_name: Option<String>,
    pub schema: Option<String>,
    pub binary: Option<String>,
    pub json: Option<String>,
    pub create: Option<String>,
    pub value_type: Option<String>,
    pub value_codec: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProtobufSymbols {
    pub messages: BTreeMap<String, ProtobufMessageSymbols>,
    pub registry_module: Option<String>,
    pub source: Option<ContractReference>,
}
impl Contract for ProtobufSymbols {
    const NAME: &'static str = "poolster.typescript.protobuf-symbols.v1";
}
pub(super) fn build(
    wire: &RpcWireSchema,
    options: &RpcGenerationOptions,
    source: Option<ContractReference>,
) -> ProtobufSymbols {
    fn collect(
        messages: &[RpcWireMessage],
        module: &str,
        package: &str,
        options: &RpcGenerationOptions,
        out: &mut BTreeMap<String, ProtobufMessageSymbols>,
    ) {
        for m in messages {
            if !m.map_entry {
                let name = super::layout::short(&m.full_name, package, options);
                out.insert(
                    m.full_name.clone(),
                    ProtobufMessageSymbols {
                        module: module.into(),
                        value_type: (!options.representations.is_native() && options.emit_types)
                            .then(|| format!("{name}Value")),
                        value_codec: (!options.representations.is_native()
                            && (options.emit_binary
                                || options.emit_json
                                || options.emit_clients
                                || options.emit_servers
                                || options.emit_reflection))
                            .then(|| format!("{name}ValueCodec")),
                        type_name: options.emit_types.then(|| name.clone()),
                        schema: options.emit_reflection.then(|| format!("{name}Schema")),
                        binary: (options.emit_binary
                            && super::helpers::has_binary(&m.full_name, options))
                        .then(|| format!("{name}Binary")),
                        json: options.emit_json.then(|| format!("{name}Json")),
                        create: (options.emit_binary
                            || options.emit_json
                            || options.emit_reflection)
                            .then(|| format!("{name}Create")),
                    },
                );
            }
            collect(&m.messages, module, package, options, out);
        }
    }
    let mut messages = BTreeMap::new();
    for file in &wire.files {
        collect(
            &file.messages,
            &options
                .import_mappings
                .get(&file.name)
                .cloned()
                .unwrap_or_else(|| format!("src/{}.ts", super::layout::stem(&file.name, options))),
            &file.package,
            options,
            &mut messages,
        );
    }
    ProtobufSymbols {
        messages,
        registry_module: options
            .emit_reflection
            .then(|| "src/poolster-rpc-runtime.ts".into()),
        source,
    }
}
