//! Native symbol references for companion plugins without reparsing Python.
use poolster_core::{blocks::ContractReference, engine::Contract, native::rpc::RpcWireSchema};
use std::collections::BTreeMap;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtobufMessageSymbol {
    pub module: String,
    pub class: String,
    pub source_path: Option<String>,
}
#[derive(Clone, Debug)]
pub struct ProtobufValueSymbol {
    pub module: String,
    pub type_name: String,
    pub to_native: String,
    pub from_native: String,
}
#[derive(Clone, Debug)]
pub struct ProtobufSymbols {
    pub value_models: BTreeMap<String, ProtobufValueSymbol>,
    pub messages: BTreeMap<String, ProtobufMessageSymbol>,
    pub source: Option<ContractReference>,
}
impl Contract for ProtobufSymbols {
    const NAME: &'static str = "poolster.python.protobuf-symbols.v1";
}
pub(super) fn build(
    schema: &RpcWireSchema,
    source: Option<ContractReference>,
    options: &poolster_core::native::rpc::RpcGenerationOptions,
    files: &[String],
) -> anyhow::Result<ProtobufSymbols> {
    fn visit(
        messages: &[poolster_core::native::rpc::RpcWireMessage],
        module: &str,
        path: &str,
        prefix: &str,
        output: &mut BTreeMap<String, ProtobufMessageSymbol>,
    ) {
        for message in messages {
            let class = if prefix.is_empty() {
                message.name.clone()
            } else {
                format!("{prefix}.{}", message.name)
            };
            if !message.map_entry {
                output.insert(
                    message.full_name.clone(),
                    ProtobufMessageSymbol {
                        module: module.into(),
                        class: class.clone(),
                        source_path: (!path.is_empty()).then(|| path.into()),
                    },
                );
            }
            visit(&message.messages, module, path, &class, output);
        }
    }
    let mut messages = BTreeMap::new();
    for file in &schema.files {
        visit(
            &file.messages,
            &super::presentation::module(&file.name, options),
            &if options.import_mappings.contains_key(&file.name)
                || file.name.starts_with("google/protobuf/")
            {
                String::new()
            } else {
                format!(
                    "src/{}.{}",
                    super::presentation::module(&file.name, options).replace('.', "/"),
                    if options.emit_types
                        && !options.emit_binary
                        && !options.emit_json
                        && !options.emit_clients
                        && !options.emit_servers
                        && !options.emit_reflection
                    {
                        "pyi"
                    } else {
                        "py"
                    }
                )
            },
            "",
            &mut messages,
        );
    }
    Ok(ProtobufSymbols {
        messages,
        source,
        value_models: super::values::references(schema, files, options)?,
    })
}
