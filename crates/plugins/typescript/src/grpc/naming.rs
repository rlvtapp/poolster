//! Reserve all module symbols before rendering; collisions never depend on order.
use anyhow::{ensure, Result};
use poolster_core::native::rpc::{RpcGenerationOptions, RpcWireSchema};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn validate(
    wire: &RpcWireSchema,
    names: &BTreeMap<String, String>,
    owners: &BTreeMap<String, String>,
    options: &RpcGenerationOptions,
) -> Result<()> {
    for file in &wire.files {
        let mut reserved: BTreeSet<String> = [
            "registry",
            "encode",
            "decode",
            "parseJson",
            "stringifyJson",
            "create",
            "MessageInitShape",
            "JsonValue",
            "JsonReadOptions",
            "JsonWriteOptions",
            "GenExtension",
            "fromBinary",
            "toBinary",
            "Message",
            "GenMessage",
            "Client",
            "makeGenericClientConstructor",
            "ClientOptions",
            "ChannelCredentials",
            "Metadata",
            "CallOptions",
            "ClientUnaryCall",
            "ClientReadableStream",
            "ClientWritableStream",
            "ClientDuplexStream",
            "ServiceError",
            "UntypedServiceImplementation",
            "handleUnaryCall",
            "handleClientStreamingCall",
            "handleServerStreamingCall",
            "handleBidiStreamingCall",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        if !options.representations.is_native() {
            reserved.insert("valueCodec".into());
        }
        for name in names.values() {
            let mut exports = vec![
                name.clone(),
                format!("{name}Schema"),
                format!("{name}Binary"),
                format!("{name}Json"),
                format!("{name}Create"),
                format!("{name}Name"),
            ];
            if !options.representations.is_native() {
                exports.extend([format!("{name}Value"), format!("{name}ValueCodec")]);
            }
            for export in exports {
                ensure!(
                    reserved.insert(export.clone()),
                    "Protobuf generated symbols collide in {}: {export}",
                    file.name
                );
            }
        }
        for (full, name) in names {
            if owners[full] != file.name {
                continue;
            }
            let short = super::layout::short(full, &file.package, options);
            if short != *name {
                let mut aliases = vec![
                    short.clone(),
                    format!("{short}Schema"),
                    format!("{short}Binary"),
                    format!("{short}Json"),
                    format!("{short}Create"),
                    format!("{short}Name"),
                ];
                if !options.representations.is_native() {
                    aliases.extend([format!("{short}Value"), format!("{short}ValueCodec")]);
                }
                for alias in aliases {
                    ensure!(
                        reserved.insert(alias.clone()),
                        "Protobuf exports collide in {}: {alias}",
                        file.name
                    );
                }
            }
        }
        for service in &file.services {
            let name = service.full_name.rsplit('.').next().unwrap();
            for suffix in ["Client", "Server", "Service"] {
                let export = format!("{name}{suffix}");
                ensure!(
                    reserved.insert(export.clone()),
                    "Protobuf service exports collide in {}: {export}",
                    file.name
                );
            }
            let mut methods = BTreeSet::new();
            for method in &service.methods {
                let local = super::layout::method(&method.name, options);
                ensure!(
                    methods.insert(local.clone()),
                    "Protobuf methods collide after TypeScript normalization in {}: {local}",
                    service.full_name
                );
                ensure!(
                    ![
                        "close",
                        "getChannel",
                        "waitForReady",
                        "makeUnaryRequest",
                        "makeClientStreamRequest",
                        "makeServerStreamRequest",
                        "makeBidiStreamRequest"
                    ]
                    .contains(&local.as_str()),
                    "Protobuf method collides with native Client API: {}.{}",
                    service.full_name,
                    method.name
                );
            }
        }
    }
    Ok(())
}
