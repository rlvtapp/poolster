//! Poolster-owned Go messages and gRPC rendering using native runtime libraries.
use anyhow::{Context, Result, ensure};
use poolster_core::{
    GeneratedFile,
    engine::{Handle, Meta, Plugin, PluginContext, Provision, Requirement},
    native::rpc::RpcContract,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub const PROTOC_VERSION: &str = "34.2";
pub const GO_PROTOBUF_VERSION: &str = "1.36.11";
pub const GO_GRPC_PLUGIN_VERSION: &str = "1.6.2";
pub const GO_GRPC_VERSION: &str = "1.83.2";

#[derive(Clone, Debug)]
pub struct GrpcToolchain {
    pub protoc: PathBuf,
    pub protoc_gen_go: PathBuf,
    pub protoc_gen_go_grpc: PathBuf,
}
impl Default for GrpcToolchain {
    fn default() -> Self {
        Self {
            protoc: "protoc".into(),
            protoc_gen_go: "protoc-gen-go".into(),
            protoc_gen_go_grpc: "protoc-gen-go-grpc".into(),
        }
    }
}
/// Native message types, gRPC client stubs and server interfaces in one module.
pub struct Grpc {
    meta: Meta,
    input: Option<Handle<RpcContract>>,
    module_path: String,
    options: poolster_core::native::rpc::RpcGenerationOptions,
    go_packages: BTreeMap<String, String>,
}
pub fn grpc(module_path: impl Into<String>) -> Grpc {
    Grpc {
        meta: Meta::new(),
        input: None,
        module_path: module_path.into(),
        options: Default::default(),
        go_packages: BTreeMap::new(),
    }
}
impl Grpc {
    pub fn handle(&self) -> Handle<GrpcApi> {
        self.meta.handle()
    }
    pub fn input(mut self, input: Handle<RpcContract>) -> Self {
        self.input = Some(input);
        self
    }
    pub fn toolchain(self, tools: GrpcToolchain) -> Self {
        let _ = tools;
        self
    }
    /// Select native emission surfaces and bounded codec helpers.
    pub fn options(mut self, options: poolster_core::native::rpc::RpcGenerationOptions) -> Self {
        self.options = options;
        self
    }
    /// Explicit equivalent of protoc's Mfile.proto=import/path;package mapping.
    pub fn go_package(mut self, file: impl Into<String>, package: impl Into<String>) -> Self {
        self.go_packages.insert(file.into(), package.into());
        self
    }
}

mod adapters;
mod batching;
mod codecs;
mod descriptors;
mod documentation;
mod enums;
mod facade;
mod messages;
mod output;
mod plan;
mod reactive;
mod representations;
mod services;
mod symbols;
mod validation;
mod values;
mod views;
pub use symbols::{GrpcApi, RpcMessageSymbol, RpcServiceSymbol, RpcValueSymbol};
#[cfg(test)]
use validation::validate;
impl Plugin<crate::Go> for Grpc {
    fn kind(&self) -> &'static str {
        "go-grpc"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.input)]
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<GrpcApi>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, crate::Go>) -> Result<()> {
        self.options.validate()?;
        ensure!(
            self.options.emit_types || !self.options.emit_servers,
            "Go server interfaces require exported message types; disable emitServers for codec-only/internal-type packages"
        );
        let provenance = cx.inputs.reference::<RpcContract>()?.cloned();
        let contract = cx.inputs.get::<RpcContract>()?.clone();
        let mut mappings = self.go_packages.clone();
        for (file, mapping) in &self.options.import_mappings {
            ensure!(
                !mappings.contains_key(file),
                "Go external importMappings conflicts with legacy goPackage override: {file}"
            );
            mappings.insert(file.clone(), mapping.clone());
        }
        let generation =
            validation::validate_options(&contract, &self.module_path, &mappings, &self.options)?;
        let schema = contract.wire_schema()?;
        self.options.validate_for(&schema)?;
        ensure!(
            self.options.emit_types || self.options.representations.is_native(),
            "Go alternate value representations require emitTypes for public value models and converters"
        );
        ensure!(
            !self.options.nestjs_rest_parameter,
            "Go does not support NestJS rest parameters; select the Native grpc-go framework"
        );
        ensure!(
            !self.options.reactive_streams || self.options.emit_clients,
            "Go reactive subscriptions require emitClients"
        );
        ensure!(
            self.options
                .frameworks
                .iter()
                .all(|framework| *framework == poolster_core::native::rpc::RpcFramework::Native),
            "Go Protobuf supports the Native grpc-go framework; NiceGrpc and NestJs are TypeScript frameworks"
        );
        let plan = plan::Plan::new(
            &contract,
            &schema,
            &self.module_path,
            &mappings,
            &self.options,
        )?;
        let mut value_packages = std::collections::BTreeSet::new();
        for file in &schema.files {
            if !generation.contains(&file.name) {
                continue;
            }
            let context = plan.context(file)?;
            messages::emit(cx, file, &context)?;
            descriptors::emit(cx, file, &context, self.options.emit_reflection)?;
            services::emit(cx, file, &context, &self.options)?;
            codecs::emit(cx, file, &context, &self.options)?;
            if self.options.immutable_views {
                views::emit(cx, file, &context)?;
            }
            if (self.options.value_adapters || !self.options.representations.is_native())
                && value_packages.insert(context.import.clone())
            {
                values::emit(cx, &context, &self.options)?;
            }
        }
        if self.options.emit_clients && self.options.emit_index {
            facade::emit(cx, &schema, &plan, self.options.client_style)?;
        }
        adapters::emit(cx, &schema, &plan, &self.options)?;
        batching::emit(cx, &schema, &plan, &self.options)?;
        if !self.options.representations.is_native() {
            representations::emit(cx, &schema, &plan, &generation)?;
        }
        if self.options.reactive_streams && self.options.emit_clients {
            reactive::emit(cx, &schema, &plan)?;
        }
        let mut manifest = include_str!("../templates/grpc_go.mod.tmpl")
            .replace("{{MODULE_PATH}}", &self.module_path)
            .replace("{{GRPC_VERSION}}", GO_GRPC_VERSION)
            .replace("{{PROTOBUF_VERSION}}", GO_PROTOBUF_VERSION);
        if self
            .options
            .transports
            .contains(&poolster_core::native::rpc::RpcTransport::GrpcWeb)
        {
            manifest.push_str("\nrequire connectrpc.com/connect v1.18.1\n");
        }
        cx.files.emit(GeneratedFile::new("go.mod", manifest)?)?;
        cx.files.emit(GeneratedFile::new(
            "go.sum",
            format!("{}{}", include_str!("../templates/grpc_go.sum.tmpl"), if self.options.transports.contains(&poolster_core::native::rpc::RpcTransport::GrpcWeb) { "connectrpc.com/connect v1.18.1 h1:PAg7CjSAGvscaf6YZKUefjoih5Z/qYkyaTrBW8xvYPw=\nconnectrpc.com/connect v1.18.1/go.mod h1:0292hj1rnx8oFrStN7cB4jjVBeqs+Yx5yDIC2prWDO8=\n" } else { "" }),
        )?)?;
        for file in &contract.files {
            if let Some(source) = &file.source {
                cx.files.emit(GeneratedFile::new(
                    Path::new("proto").join(&file.name),
                    source,
                )?)?;
            }
        }
        cx.files.emit(GeneratedFile::new("README.md","# Go Protobuf and gRPC package\n\nPoolster renders native Go source directly. Binary codecs, ProtoJSON, reflection, registries, unknown fields and extension access use google.golang.org/protobuf. Clients and servers use grpc-go, including all four stream directions, metadata, deadlines, cancellation and runtime interceptors. No upstream source generator runs during generation.\n\nUse proto.Marshal/proto.Unmarshal and protojson.MarshalOptions/UnmarshalOptions; these native options provide required-field checks, unknown retention/discard, bounded recursion and JSON policy. Embed Unimplemented<Service>Server for service implementations. Configure credentials and interceptors when creating native grpc connections/servers. Legacy toolchain paths are accepted for configuration compatibility and are not executed.\n\nRun go test ./... with Go 1.25+. Retained source documents are under proto/. Optional transports add Connect-backed gRPC-web unary/server-stream APIs and unary Twirp protobuf/ProtoJSON APIs; unsupported adapter streams fail before I/O and are listed in .poolster/rpc-capabilities.json. Optional batch mappings create explicit request-owned queues and positive caches. Immutable views return defensive copies; reactive subscriptions use bounded channels and native contexts; value adapters reject precision/range loss. Generated helper/adaptor bounds do not change direct calls to native proto/grpc runtime APIs. This profile does not implement every conformance option.\n")?)?;
        cx.publish(symbols::collect(
            &plan,
            &schema,
            &generation,
            &self.options,
            provenance,
        )?)
    }
}
#[cfg(test)]
#[path = "grpc/tests.rs"]
mod tests;
