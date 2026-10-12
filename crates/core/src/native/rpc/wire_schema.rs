//! Authoritative retained-descriptor projection with deterministic dependency order.
use super::wire_projection::*;
use super::*;
use anyhow::{Context, Result, ensure};
use prost::Message;
use prost_types::FileDescriptorSet;
use std::collections::{BTreeMap, BTreeSet};
impl RpcContract {
    /// Decode retained descriptor bytes into Poolster-owned wire metadata. Files
    /// are ordered dependencies first; individual descriptors retain unknown options.
    pub fn wire_schema(&self) -> Result<RpcWireSchema> {
        let raw = descriptor_files(&self.descriptor_set)?;
        let set = FileDescriptorSet::decode(self.descriptor_set.as_slice())
            .context("malformed Protobuf descriptor set")?;
        ensure!(!set.file.is_empty(), "empty Protobuf descriptor set");
        let mut files = BTreeMap::new();
        let mut seen = BTreeMap::new();
        for (f, descriptor) in set.file.into_iter().zip(raw) {
            let n = f.name.context("descriptor file name missing")?;
            ensure!(
                !n.is_empty()
                    && !n.starts_with('/')
                    && !n.contains(':')
                    && !n.contains('\\')
                    && !n.split('/').any(|s| s == ".." || s.is_empty()),
                "invalid descriptor filename {n}"
            );
            let package = f.package.unwrap_or_default();
            ensure!(
                package.is_empty() || !package.split('.').any(str::is_empty),
                "invalid Protobuf package {package}"
            );
            for part in package.split('.').filter(|v| !v.is_empty()) {
                name(Some(part))?;
            }
            let syntax = f.syntax.unwrap_or_else(|| "proto2".into());
            ensure!(
                ["proto2", "proto3"].contains(&syntax.as_str()),
                "unsupported Protobuf syntax {syntax}"
            );
            let messages = f
                .message_type
                .iter()
                .map(|m| message(m, &package))
                .collect::<Result<Vec<_>>>()?;
            for m in &messages {
                symbols(m, &mut seen)?;
            }
            let enums = f
                .enum_type
                .iter()
                .map(|e| enumeration(e, &package))
                .collect::<Result<Vec<_>>>()?;
            for e in &enums {
                ensure!(
                    seen.insert(e.full_name.clone(), false).is_none(),
                    "duplicate descriptor symbol {}",
                    e.full_name
                );
            }
            let mut service_names = BTreeSet::new();
            let services = f
                .service
                .iter()
                .map(|s| {
                    let sn = name(s.name.as_deref())?;
                    ensure!(service_names.insert(sn.clone()), "duplicate service {sn}");
                    let qualified = full(&package, &sn);
                    let mut method_names = BTreeSet::new();
                    let methods = s
                        .method
                        .iter()
                        .map(|m| {
                            let mn = name(m.name.as_deref())?;
                            ensure!(method_names.insert(mn.clone()), "duplicate RPC method {mn}");
                            let input =
                                m.input_type.as_deref().context("RPC input type missing")?;
                            let output = m
                                .output_type
                                .as_deref()
                                .context("RPC output type missing")?;
                            ensure!(
                                input.starts_with('.') && output.starts_with('.'),
                                "RPC types must be fully qualified"
                            );
                            let streaming = match (
                                m.client_streaming.unwrap_or(false),
                                m.server_streaming.unwrap_or(false),
                            ) {
                                (false, false) => RpcStreaming::Unary,
                                (true, false) => RpcStreaming::Client,
                                (false, true) => RpcStreaming::Server,
                                (true, true) => RpcStreaming::Bidirectional,
                            };
                            Ok(RpcMethod {
                                name: mn.clone(),
                                full_name: full(&qualified, &mn),
                                input_type: input[1..].into(),
                                output_type: output[1..].into(),
                                streaming,
                            })
                        })
                        .collect::<Result<_>>()?;
                    Ok(RpcService {
                        full_name: qualified,
                        file: n.clone(),
                        methods,
                    })
                })
                .collect::<Result<_>>()?;
            let public_dependencies = f
                .public_dependency
                .iter()
                .map(|i| {
                    ensure!(
                        *i >= 0 && (*i as usize) < f.dependency.len(),
                        "invalid public dependency index"
                    );
                    Ok(f.dependency[*i as usize].clone())
                })
                .collect::<Result<Vec<_>>>()?;
            let value = RpcWireFile {
                name: n.clone(),
                package,
                syntax,
                dependencies: f.dependency,
                public_dependencies,
                descriptor,
                messages,
                enums,
                services,
                extensions: fields(&f.extension, 0)?,
            };
            ensure!(
                files.insert(n.clone(), value).is_none(),
                "duplicate descriptor file {n}"
            );
        }
        for f in files.values() {
            for m in &f.messages {
                validate_message(m, &seen)?;
            }
            validate_fields(&f.extensions, &seen)?;
            for m in &f.messages {
                validate_syntax(m, &f.syntax)?;
            }

            for s in &f.services {
                ensure!(
                    !seen.contains_key(&s.full_name),
                    "service symbol collides with message or enum"
                );
                for m in &s.methods {
                    ensure!(
                        seen.get(&m.input_type) == Some(&true)
                            && seen.get(&m.output_type) == Some(&true),
                        "unknown RPC message type for {}",
                        m.full_name
                    );
                }
            }
            for dep in &f.dependencies {
                ensure!(
                    files.contains_key(dep),
                    "missing descriptor dependency {dep}"
                );
            }
        }
        for root in &self.root_files {
            ensure!(files.contains_key(root), "missing root descriptor {root}");
        }
        let mut ordered = Vec::new();
        while !files.is_empty() {
            let emitted = ordered
                .iter()
                .map(|f: &RpcWireFile| f.name.as_str())
                .collect::<BTreeSet<_>>();
            let next = files
                .iter()
                .find(|(_, f)| f.dependencies.iter().all(|d| emitted.contains(d.as_str())))
                .map(|(n, _)| n.clone())
                .context("cyclic descriptor dependencies")?;
            ordered.push(files.remove(&next).unwrap());
        }
        Ok(RpcWireSchema { files: ordered })
    }
}
