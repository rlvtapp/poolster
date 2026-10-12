//! Safe presentation aliases leave descriptor names and enum numbers unchanged.
use anyhow::{Result, ensure};
use poolster_core::native::rpc::{RpcGenerationOptions, RpcWireEnum, RpcWireFile, RpcWireMessage};
use std::collections::BTreeSet;
pub(super) fn alias(enumeration: &str, value: &str) -> String {
    let prefix = format!(
        "{}_",
        crate::python_identifier(enumeration).to_ascii_uppercase()
    );
    value.strip_prefix(&prefix).unwrap_or(value).to_owned()
}
pub(super) fn render(file: &RpcWireFile, options: &RpcGenerationOptions) -> Result<String> {
    if !options.remove_enum_prefix {
        return Ok(String::new());
    }
    fn scope(
        messages: &[RpcWireMessage],
        enums: &[RpcWireEnum],
        reserved: Vec<String>,
        prefix: &str,
        options: &RpcGenerationOptions,
        out: &mut String,
    ) -> Result<()> {
        let mut names: BTreeSet<String> = reserved
            .into_iter()
            .chain(["DESCRIPTOR".into(), "_runtime".into()])
            .collect();
        for message in messages {
            names.insert(message.name.clone());
            names.insert(super::presentation::type_name(&message.name, options));
        }
        for enumeration in enums {
            names.insert(enumeration.name.clone());
            names.insert(super::presentation::type_name(&enumeration.name, options));
            names.extend(enumeration.values.iter().map(|v| v.name.clone()));
        }
        for enumeration in enums {
            for value in &enumeration.values {
                let name = alias(&enumeration.name, &value.name);
                if name == value.name {
                    continue;
                }
                ensure!(
                    super::validation::identifier(&name)
                        && !name.starts_with('_')
                        && ![
                            "SerializeToString",
                            "SerializePartialToString",
                            "FromString",
                            "ParseFromString",
                            "Clear",
                            "MergeFrom",
                            "HasField",
                            "ListFields",
                            "CopyFrom",
                            "ClearField",
                            "WhichOneof",
                            "IsInitialized",
                            "ByteSize",
                            "DiscardUnknownFields",
                            "SetInParent",
                            "FindInitializationErrors",
                            "UnknownFields",
                            "Extensions"
                        ]
                        .contains(&name.as_str())
                        && names.insert(name.clone()),
                    "Python enum prefix alias collision or invalid name: {prefix}{name}"
                );
                out.push_str(&format!("{prefix}{name} = {prefix}{}\n", value.name));
            }
        }
        for message in messages {
            if message.map_entry {
                continue;
            }
            scope(
                &message.messages,
                &message.enums,
                message
                    .fields
                    .iter()
                    .map(|f| f.name.clone())
                    .chain(message.extensions.iter().map(|e| e.name.clone()))
                    .collect(),
                &format!("{prefix}{}.", message.name),
                options,
                out,
            )?;
        }
        Ok(())
    }
    let mut output = String::new();
    scope(
        &file.messages,
        &file.enums,
        file.extensions.iter().map(|e| e.name.clone()).collect(),
        "",
        options,
        &mut output,
    )?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prefix_aliases_reject_scope_collisions_before_emission() {
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/grpc/aliases.proto");
        let contract = poolster_input_protobuf::load(&source)
            .unwrap()
            .rpc_contract();
        let mut schema = contract.wire_schema().unwrap();
        let file = schema
            .files
            .iter_mut()
            .find(|f| f.name == "aliases.proto")
            .unwrap();
        let options = RpcGenerationOptions {
            remove_enum_prefix: true,
            ..Default::default()
        };
        assert!(render(file, &options).unwrap().contains("RED = COLOR_RED"));
        file.enums[0]
            .values
            .push(poolster_core::native::rpc::RpcEnumValue {
                name: "RED".into(),
                number: 2,
            });
        assert!(render(file, &options).is_err());
    }
}
