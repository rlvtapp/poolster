//! Serializable Poolster-owned Protobuf wire metadata.
use super::RpcService;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RpcWireSchema {
    pub files: Vec<RpcWireFile>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RpcWireFile {
    pub name: String,
    pub package: String,
    pub syntax: String,
    pub dependencies: Vec<String>,
    pub public_dependencies: Vec<String>,
    pub descriptor: Vec<u8>,
    pub messages: Vec<RpcWireMessage>,
    pub enums: Vec<RpcWireEnum>,
    pub services: Vec<RpcService>,
    pub extensions: Vec<RpcWireField>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RpcWireMessage {
    pub name: String,
    pub full_name: String,
    pub fields: Vec<RpcWireField>,
    pub oneofs: Vec<String>,
    pub messages: Vec<RpcWireMessage>,
    pub enums: Vec<RpcWireEnum>,
    pub map_entry: bool,
    pub extensions: Vec<RpcWireField>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RpcWireEnum {
    pub name: String,
    pub full_name: String,
    pub values: Vec<RpcEnumValue>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RpcEnumValue {
    pub name: String,
    pub number: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RpcFieldLabel {
    Optional,
    Required,
    Repeated,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RpcFieldKind {
    Double,
    Float,
    Int64,
    Uint64,
    Int32,
    Fixed64,
    Fixed32,
    Bool,
    String,
    Group(String),
    Message(String),
    Bytes,
    Uint32,
    Enum(String),
    Sfixed32,
    Sfixed64,
    Sint32,
    Sint64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RpcWireField {
    pub name: String,
    pub json_name: String,
    pub number: i32,
    pub label: RpcFieldLabel,
    pub kind: RpcFieldKind,
    pub oneof_index: Option<usize>,
    pub proto3_optional: bool,
    pub default_value: Option<String>,
    pub extendee: Option<String>,
}
