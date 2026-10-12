//! Validation of portable presentation and selection controls.
use super::{RpcGenerationOptions, RpcWireMessage, RpcWireSchema};
use anyhow::{Result, ensure};
use std::collections::BTreeSet;

impl RpcGenerationOptions {
    pub(super) fn validate_presentation(&self) -> Result<()> {
        ensure!(
            !self.unknown_enum_name.trim().is_empty()
                && !self.unknown_enum_name.chars().any(char::is_control),
            "Protobuf unknownEnumName must be a nonempty display label without control characters"
        );
        for (name, value) in [
            ("typePrefix", &self.type_prefix),
            ("typeSuffix", &self.type_suffix),
        ] {
            ensure!(
                value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
                "Protobuf {name} must contain only ASCII letters, digits and underscores"
            );
        }
        ensure!(
            self.type_prefix
                .chars()
                .next()
                .is_none_or(|c| c.is_ascii_alphabetic() || c == '_'),
            "Protobuf typePrefix must begin with an ASCII letter or underscore"
        );
        ensure!(
            self.file_suffix
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')),
            "Protobuf fileSuffix must be a filename suffix without path separators"
        );
        for (file, module) in &self.import_mappings {
            ensure!(
                !file.is_empty() && !module.trim().is_empty(),
                "Protobuf importMappings entries must be nonempty"
            );
            ensure!(
                !file.contains('\\') && !file.split('/').any(|s| s == ".." || s.is_empty()),
                "Protobuf importMappings keys must be relative descriptor file names"
            );
            ensure!(
                !module.chars().any(char::is_control),
                "Protobuf importMappings modules must not contain control characters"
            );
        }
        for (name, selection) in [
            ("encodeIncludeTypes", &self.encode_include_types),
            ("decodeIncludeTypes", &self.decode_include_types),
        ] {
            let unique: BTreeSet<_> = selection.iter().collect();
            ensure!(
                unique.len() == selection.len(),
                "Protobuf {name} must not contain duplicates"
            );
            ensure!(
                selection.iter().all(|s| !s.is_empty()
                    && s.split('.').all(|part| !part.is_empty()
                        && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))),
                "Protobuf {name} must contain fully qualified message identities"
            );
        }
        Ok(())
    }

    pub(super) fn validate_selections(&self, wire: &RpcWireSchema) -> Result<()> {
        fn collect<'a>(messages: &'a [RpcWireMessage], names: &mut BTreeSet<&'a str>) {
            for message in messages {
                if !message.map_entry {
                    names.insert(&message.full_name);
                }
                collect(&message.messages, names);
            }
        }
        let mut names = BTreeSet::new();
        for file in &wire.files {
            collect(&file.messages, &mut names);
        }
        for identity in self
            .encode_include_types
            .iter()
            .chain(&self.decode_include_types)
        {
            ensure!(
                names.contains(identity.as_str()),
                "Protobuf codec selection references an unknown message: {identity}"
            );
        }
        for file in self.import_mappings.keys() {
            ensure!(
                wire.files.iter().any(|candidate| &candidate.name == file),
                "Protobuf importMappings references an unknown descriptor file: {file}"
            );
        }
        Ok(())
    }
}
