use anyhow::{Result, ensure};
use poolster_core::native::rpc::{RpcContract, RpcWireMessage, RpcWireSchema};
use std::collections::BTreeSet;
pub(super) fn identifier(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with("_poolster_")
        && s.chars()
            .enumerate()
            .all(|(i, c)| c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
        && ![
            "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
            "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
            "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise",
            "return", "try", "while", "with", "yield",
        ]
        .contains(&s)
}
fn message(value: &RpcWireMessage) -> Result<()> {
    ensure!(
        identifier(&value.name),
        "Python Protobuf cannot expose message name {}",
        value.full_name
    );
    for field in &value.fields {
        ensure!(
            identifier(&field.name)
                && ![
                    "DESCRIPTOR",
                    "Extensions",
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
                    "UnknownFields"
                ]
                .contains(&field.name.as_str())
                && !field.name.starts_with("__"),
            "Python Protobuf cannot expose keyword field {}.{}",
            value.full_name,
            field.name
        );
    }
    for nested in &value.messages {
        message(nested)?;
    }
    for enumeration in &value.enums {
        ensure!(
            identifier(&enumeration.name) && enumeration.values.iter().all(|v| identifier(&v.name)),
            "Python Protobuf cannot expose enum {}",
            enumeration.full_name
        );
    }
    Ok(())
}
pub(super) fn validate(
    contract: &RpcContract,
    schema: &RpcWireSchema,
    name: &str,
) -> Result<Vec<String>> {
    ensure!(
        !name.is_empty()
            && name.len() <= 128
            && name
                .bytes()
                .enumerate()
                .all(|(i, c)| c.is_ascii_alphanumeric() || (i > 0 && b"._-".contains(&c)))
            && name
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_alphanumeric),
        "Python gRPC distribution name must be a portable package name"
    );
    ensure!(
        !contract.root_files.is_empty() && !schema.files.iter().all(|f| f.services.is_empty()),
        "Python gRPC needs roots and RPC services"
    );
    let mut names = BTreeSet::new();
    let mut folded = BTreeSet::new();
    let mut generation = Vec::new();
    for file in &schema.files {
        let path = file.name.strip_suffix(".proto").unwrap_or("");
        ensure!(
            !path.is_empty() && path.split('/').all(identifier),
            "Python gRPC requires portable Python module paths: {:?}",
            file.name
        );
        ensure!(
            names.insert(&file.name) && folded.insert(file.name.to_ascii_lowercase()),
            "duplicate or case-colliding Protobuf file {}",
            file.name
        );
        ensure!(
            matches!(file.syntax.as_str(), "proto2" | "proto3"),
            "Python gRPC unsupported Protobuf syntax {}",
            file.syntax
        );
        let source = contract
            .files
            .iter()
            .find(|f| f.name == file.name)
            .and_then(|f| f.source.as_ref());
        if file.name.starts_with("google/protobuf/") {
            ensure!(
                source.is_none(),
                "Python gRPC custom files inside runtime-owned google/protobuf are unsupported"
            );
            continue;
        }
        for definition in &file.messages {
            message(definition)?;
        }
        let mut exports = BTreeSet::new();
        for definition in &file.messages {
            ensure!(exports.insert(&definition.name), "Python export collision");
        }
        for enumeration in &file.enums {
            ensure!(
                identifier(&enumeration.name) && exports.insert(&enumeration.name),
                "Python enum export collision or unsupported name"
            );
            for value in &enumeration.values {
                ensure!(
                    identifier(&value.name) && exports.insert(&value.name),
                    "Python enum value export collision or unsupported name"
                );
            }
        }
        for extension in &file.extensions {
            ensure!(
                identifier(&extension.name) && exports.insert(&extension.name),
                "Python extension export collision or unsupported name"
            );
        }
        ensure!(
            !exports.contains(&String::from("DESCRIPTOR")),
            "Python DESCRIPTOR export collision"
        );
        for service in &file.services {
            ensure!(
                identifier(service.full_name.rsplit('.').next().unwrap()),
                "Python service name unsupported"
            );
            for method in &service.methods {
                ensure!(
                    identifier(&method.name) && !method.name.starts_with("__"),
                    "Python method name unsupported"
                );
            }
        }
        generation.push(file.name.clone());
    }
    for root in &contract.root_files {
        ensure!(names.contains(root), "missing root descriptor {root}");
    }
    generation.sort();
    ensure!(!generation.is_empty(), "Python gRPC has no local files");
    Ok(generation)
}
