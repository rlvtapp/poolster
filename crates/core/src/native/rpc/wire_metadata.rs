//! Owned source documentation and deprecation metadata for language renderers.
use super::RpcWireSchema;
use anyhow::Result;
use prost::Message;
use prost_types::{DescriptorProto, EnumDescriptorProto, FileDescriptorProto};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcDefinitionKind {
    Message,
    Field,
    Enum,
    EnumValue,
    Service,
    Method,
    Extension,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcJavaScriptType {
    Normal,
    String,
    Number,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RpcDefinitionMetadata {
    pub full_name: String,
    pub kind: RpcDefinitionKind,
    pub deprecated: bool,
    pub leading_comments: String,
    pub trailing_comments: String,
    pub detached_comments: Vec<String>,
    pub span: Vec<i32>,
    pub javascript_type: Option<RpcJavaScriptType>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RpcFileMetadata {
    pub file: String,
    pub definitions: Vec<RpcDefinitionMetadata>,
}
impl RpcFileMetadata {
    pub fn definition(
        &self,
        kind: RpcDefinitionKind,
        full_name: &str,
    ) -> Option<&RpcDefinitionMetadata> {
        self.definitions
            .iter()
            .find(|d| d.kind == kind && d.full_name == full_name)
    }
}

fn qualify(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.into()
    } else {
        format!("{parent}.{name}")
    }
}
type Locations = BTreeMap<Vec<i32>, prost_types::source_code_info::Location>;
fn add(
    out: &mut Vec<RpcDefinitionMetadata>,
    locations: &Locations,
    path: &[i32],
    full_name: String,
    kind: RpcDefinitionKind,
    deprecated: bool,
) {
    let location = locations.get(path).cloned().unwrap_or_default();
    out.push(RpcDefinitionMetadata {
        full_name,
        kind,
        deprecated,
        leading_comments: location.leading_comments.unwrap_or_default(),
        trailing_comments: location.trailing_comments.unwrap_or_default(),
        detached_comments: location.leading_detached_comments,
        span: location.span,
        javascript_type: None,
    });
}
fn child(path: &[i32], tag: i32, index: usize) -> Vec<i32> {
    let mut result = path.to_vec();
    result.extend([tag, index as i32]);
    result
}
fn enumeration(
    e: &EnumDescriptorProto,
    parent: &str,
    path: &[i32],
    locations: &Locations,
    out: &mut Vec<RpcDefinitionMetadata>,
) {
    let full = qualify(parent, e.name.as_deref().unwrap_or_default());
    add(
        out,
        locations,
        path,
        full.clone(),
        RpcDefinitionKind::Enum,
        e.options
            .as_ref()
            .and_then(|o| o.deprecated)
            .unwrap_or(false),
    );
    for (i, value) in e.value.iter().enumerate() {
        add(
            out,
            locations,
            &child(path, 2, i),
            qualify(&full, value.name.as_deref().unwrap_or_default()),
            RpcDefinitionKind::EnumValue,
            value
                .options
                .as_ref()
                .and_then(|o| o.deprecated)
                .unwrap_or(false),
        );
    }
}
fn message(
    m: &DescriptorProto,
    parent: &str,
    path: &[i32],
    locations: &Locations,
    out: &mut Vec<RpcDefinitionMetadata>,
) {
    let full = qualify(parent, m.name.as_deref().unwrap_or_default());
    add(
        out,
        locations,
        path,
        full.clone(),
        RpcDefinitionKind::Message,
        m.options
            .as_ref()
            .and_then(|o| o.deprecated)
            .unwrap_or(false),
    );
    for (tag, fields, kind) in [
        (2, &m.field, RpcDefinitionKind::Field),
        (6, &m.extension, RpcDefinitionKind::Extension),
    ] {
        for (i, field) in fields.iter().enumerate() {
            add(
                out,
                locations,
                &child(path, tag, i),
                qualify(&full, field.name.as_deref().unwrap_or_default()),
                kind,
                field
                    .options
                    .as_ref()
                    .and_then(|o| o.deprecated)
                    .unwrap_or(false),
            );
            out.last_mut().unwrap().javascript_type = field
                .options
                .as_ref()
                .and_then(|o| o.jstype)
                .and_then(|value| match value {
                    0 => Some(RpcJavaScriptType::Normal),
                    1 => Some(RpcJavaScriptType::String),
                    2 => Some(RpcJavaScriptType::Number),
                    _ => None,
                });
        }
    }
    for (i, nested) in m.nested_type.iter().enumerate() {
        message(nested, &full, &child(path, 3, i), locations, out);
    }
    for (i, e) in m.enum_type.iter().enumerate() {
        enumeration(e, &full, &child(path, 4, i), locations, out);
    }
}
impl RpcWireSchema {
    /// Source-code locations are decoded privately. Output interfaces receive
    /// only Poolster-owned strings, identities, flags and spans.
    pub fn metadata(&self) -> Result<Vec<RpcFileMetadata>> {
        self.files
            .iter()
            .map(|file| {
                let descriptor = FileDescriptorProto::decode(file.descriptor.as_slice())?;
                let locations: Locations = descriptor
                    .source_code_info
                    .unwrap_or_default()
                    .location
                    .into_iter()
                    .map(|l| (l.path.clone(), l))
                    .collect();
                let package = descriptor.package.unwrap_or_default();
                let mut definitions = Vec::new();
                for (i, m) in descriptor.message_type.iter().enumerate() {
                    message(m, &package, &[4, i as i32], &locations, &mut definitions);
                }
                for (i, e) in descriptor.enum_type.iter().enumerate() {
                    enumeration(e, &package, &[5, i as i32], &locations, &mut definitions);
                }
                for (i, field) in descriptor.extension.iter().enumerate() {
                    add(
                        &mut definitions,
                        &locations,
                        &[7, i as i32],
                        qualify(&package, field.name.as_deref().unwrap_or_default()),
                        RpcDefinitionKind::Extension,
                        field
                            .options
                            .as_ref()
                            .and_then(|o| o.deprecated)
                            .unwrap_or(false),
                    );
                    definitions.last_mut().unwrap().javascript_type =
                        field.options.as_ref().and_then(|o| o.jstype).and_then(
                            |value| match value {
                                0 => Some(RpcJavaScriptType::Normal),
                                1 => Some(RpcJavaScriptType::String),
                                2 => Some(RpcJavaScriptType::Number),
                                _ => None,
                            },
                        );
                }
                for (i, service) in descriptor.service.iter().enumerate() {
                    let full = qualify(&package, service.name.as_deref().unwrap_or_default());
                    let path = [6, i as i32];
                    add(
                        &mut definitions,
                        &locations,
                        &path,
                        full.clone(),
                        RpcDefinitionKind::Service,
                        service
                            .options
                            .as_ref()
                            .and_then(|o| o.deprecated)
                            .unwrap_or(false),
                    );
                    for (j, method) in service.method.iter().enumerate() {
                        add(
                            &mut definitions,
                            &locations,
                            &child(&path, 2, j),
                            qualify(&full, method.name.as_deref().unwrap_or_default()),
                            RpcDefinitionKind::Method,
                            method
                                .options
                                .as_ref()
                                .and_then(|o| o.deprecated)
                                .unwrap_or(false),
                        );
                    }
                }
                Ok(RpcFileMetadata {
                    file: file.name.clone(),
                    definitions,
                })
            })
            .collect()
    }
}
