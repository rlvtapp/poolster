//! Alternate value models retain the canonical native message ABI.
//! Conversions must reject precision/range loss and preserve unknown enum numbers.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcInt64Representation {
    #[default]
    Native,
    String,
    NumberExact,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcBytesRepresentation {
    #[default]
    Native,
    Base64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcMapsRepresentation {
    #[default]
    Native,
    Entries,
    Map,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcEnumsRepresentation {
    #[default]
    Native,
    Name,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcOptionalRepresentation {
    #[default]
    Native,
    Nullable,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcTimestampRepresentation {
    #[default]
    Native,
    DateTime,
    Rfc3339,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcDurationRepresentation {
    #[default]
    Native,
    TimeDelta,
    SecondsExact,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct RpcRepresentations {
    pub int64: RpcInt64Representation,
    pub bytes: RpcBytesRepresentation,
    pub maps: RpcMapsRepresentation,
    pub enums: RpcEnumsRepresentation,
    pub optional: RpcOptionalRepresentation,
    pub timestamp: RpcTimestampRepresentation,
    pub duration: RpcDurationRepresentation,
}

impl RpcRepresentations {
    pub fn is_native(&self) -> bool {
        self == &Self::default()
    }
}

/// JavaScript declaration alternatives; native targets keep their own ABI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcEnumStyle {
    #[default]
    Native,
    Const,
    Literal,
}
