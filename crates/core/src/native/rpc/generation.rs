//! Protocol-owned generation controls shared across language outputs.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcUnknownFields {
    #[default]
    Preserve,
    Discard,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcClientStyle {
    Raw,
    Flat,
    #[default]
    #[serde(alias = "namespaced", alias = "grouped")]
    Idiomatic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcTransport {
    Grpc,
    GrpcWeb,
    Twirp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcFramework {
    Native,
    NiceGrpc,
    NestJs,
}

/// Controls exported surfaces. Native runtime APIs remain available on native
/// message values; disabling a helper never changes the Protobuf wire schema.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct RpcGenerationOptions {
    pub enum_style: super::RpcEnumStyle,
    pub remove_enum_prefix: bool,
    /// Display label only; unknown wire enum numbers must never be replaced.
    pub unknown_enum_name: String,
    pub nestjs_rest_parameter: bool,
    pub representations: super::RpcRepresentations,
    pub type_prefix: String,
    pub type_suffix: String,
    pub snake_type_names: bool,
    pub lower_case_service_methods: bool,
    pub file_suffix: String,
    pub emit_imported_files: bool,
    /// Explicit externally supplied modules. Mapped files are never emitted.
    pub import_mappings: std::collections::BTreeMap<String, String>,
    /// Fully qualified message identities; an empty list selects every message.
    pub encode_include_types: Vec<String>,
    pub decode_include_types: Vec<String>,
    pub emit_index: bool,
    pub comments: bool,
    pub annotate_files_with_version: bool,
    pub batching: Vec<super::RpcBatchMapping>,
    pub frameworks: Vec<RpcFramework>,
    pub immutable_views: bool,
    pub reactive_streams: bool,
    pub value_adapters: bool,
    pub client_style: RpcClientStyle,
    pub transports: Vec<RpcTransport>,
    pub emit_types: bool,
    pub emit_binary: bool,
    pub emit_json: bool,
    pub emit_clients: bool,
    pub emit_servers: bool,
    pub emit_reflection: bool,
    pub unknown_fields: RpcUnknownFields,
    pub max_message_bytes: u32,
    pub max_depth: u32,
}
impl Default for RpcGenerationOptions {
    fn default() -> Self {
        Self {
            enum_style: super::RpcEnumStyle::Native,
            remove_enum_prefix: false,
            unknown_enum_name: "UNRECOGNIZED".into(),
            nestjs_rest_parameter: false,
            representations: Default::default(),
            type_prefix: String::new(),
            type_suffix: String::new(),
            snake_type_names: false,
            lower_case_service_methods: true,
            file_suffix: String::new(),
            emit_imported_files: true,
            import_mappings: Default::default(),
            encode_include_types: Vec::new(),
            decode_include_types: Vec::new(),
            emit_index: true,
            comments: true,
            annotate_files_with_version: true,
            batching: Vec::new(),
            frameworks: vec![RpcFramework::Native],
            immutable_views: false,
            reactive_streams: false,
            value_adapters: false,
            client_style: RpcClientStyle::Idiomatic,
            transports: vec![RpcTransport::Grpc],
            emit_types: true,
            emit_binary: true,
            emit_json: true,
            emit_clients: true,
            emit_servers: true,
            emit_reflection: true,
            unknown_fields: RpcUnknownFields::Preserve,
            max_message_bytes: 16 * 1024 * 1024,
            max_depth: 100,
        }
    }
}
impl RpcGenerationOptions {
    pub fn validate_for(&self, wire: &super::RpcWireSchema) -> Result<()> {
        self.validate()?;
        self.validate_selections(wire)?;
        let mut methods = std::collections::BTreeSet::new();
        for mapping in &self.batching {
            ensure!(
                methods.insert(&mapping.single_method),
                "Protobuf single method has more than one batch mapping: {}",
                mapping.single_method
            );
            mapping.validate(wire)?;
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<()> {
        self.validate_presentation()?;
        ensure!(
            !self.nestjs_rest_parameter || self.frameworks.contains(&RpcFramework::NestJs),
            "Protobuf nestjsRestParameter requires the NestJS framework adapter"
        );
        let frameworks: std::collections::BTreeSet<_> = self.frameworks.iter().collect();
        ensure!(
            frameworks.len() == self.frameworks.len(),
            "Protobuf frameworks must not contain duplicates"
        );
        ensure!(
            self.batching.is_empty() || self.emit_clients,
            "Protobuf batching requires client emission"
        );
        let unique: std::collections::BTreeSet<_> = self.transports.iter().collect();
        ensure!(
            unique.len() == self.transports.len(),
            "Protobuf transports must not contain duplicates"
        );
        ensure!(
            !self.transports.is_empty() || (!self.emit_clients && !self.emit_servers),
            "Protobuf RPC output requires at least one configured transport"
        );
        ensure!(
            self.max_message_bytes > 0 && self.max_message_bytes <= i32::MAX as u32,
            "Protobuf maxMessageBytes must be between 1 and 2147483647"
        );
        ensure!(
            self.max_depth > 0 && self.max_depth <= 1000,
            "Protobuf maxDepth must be between 1 and 1000"
        );
        ensure!(
            self.emit_types
                || self.emit_binary
                || self.emit_json
                || self.emit_clients
                || self.emit_servers
                || self.emit_reflection,
            "Protobuf output must enable at least one surface"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portable_controls_reject_ambiguous_or_unsafe_configuration() {
        for value in [
            serde_json::json!({"typePrefix":"../escape"}),
            serde_json::json!({"fileSuffix":"/escape"}),
            serde_json::json!({"encodeIncludeTypes":["a.Message","a.Message"]}),
            serde_json::json!({"decodeIncludeTypes":[""]}),
            serde_json::json!({"importMappings":{"../bad.proto":"module"}}),
        ] {
            assert!(
                serde_json::from_value::<RpcGenerationOptions>(value)
                    .unwrap()
                    .validate()
                    .is_err()
            );
        }
        let options: RpcGenerationOptions = serde_json::from_value(serde_json::json!({
            "typePrefix":"Api", "fileSuffix":".generated",
            "representations":{"int64":"string","timestamp":"rfc3339"}
        }))
        .unwrap();
        options.validate().unwrap();
        assert!(!options.representations.is_native());
        assert_eq!(
            options.representations.int64,
            super::super::RpcInt64Representation::String
        );
        assert!(
            serde_json::from_value::<RpcGenerationOptions>(serde_json::json!({
                "representations":{"int64":"lossy"}
            }))
            .is_err()
        );
    }

    #[test]
    fn recipe_defaults_and_bounds_are_consistent() {
        let options: RpcGenerationOptions =
            serde_json::from_str("{\"emitClients\":false,\"unknownFields\":\"discard\"}").unwrap();
        assert!(!options.emit_clients && options.emit_servers);
        assert_eq!(options.unknown_fields, RpcUnknownFields::Discard);
        assert!(options.validate().is_ok());
        assert!(
            serde_json::from_str::<RpcGenerationOptions>("{\"maxDepth\":0}")
                .unwrap()
                .validate()
                .is_err()
        );
        assert!(
            serde_json::from_str::<RpcGenerationOptions>("{\"unknownFields\":\"ignored\"}")
                .is_err()
        );
        assert!(serde_json::from_str::<RpcGenerationOptions>("{\"typo\":true}").is_err());
    }
}
