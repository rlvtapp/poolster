//! Poolster-owned TypeScript gRPC source generation over retained descriptors.
use anyhow::{ensure, Result};
use poolster_core::{
    engine::{Handle, Meta, Plugin, PluginContext, Provision, Requirement},
    native::rpc::{RpcContract, RpcWireSchema},
    GeneratedFile,
};
mod batch;
mod client;
mod documentation;
mod extensions;
mod facade;
mod frameworks;
mod helpers;
mod http;
mod layout;
mod naming;
mod render;
mod services;
mod shapes;
mod symbols;
mod value_shapes;
pub use symbols::{ProtobufMessageSymbols, ProtobufSymbols};
pub struct Grpc {
    meta: Meta,
    input: Option<Handle<RpcContract>>,
    options: poolster_core::native::rpc::RpcGenerationOptions,
}
pub fn grpc() -> Grpc {
    Grpc {
        meta: Meta::new(),
        input: None,
        options: Default::default(),
    }
}
pub use poolster_core::native::rpc::RpcGenerationOptions as GrpcOptions;
impl Grpc {
    pub fn handle(&self) -> Handle<ProtobufSymbols> {
        self.meta.handle()
    }
    pub fn options(mut self, options: GrpcOptions) -> Self {
        self.options = options;
        self
    }
    pub fn input(mut self, input: Handle<RpcContract>) -> Self {
        self.input = Some(input);
        self
    }
}
impl Plugin<crate::TypeScript> for Grpc {
    fn kind(&self) -> &'static str {
        "typescript-grpc"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<ProtobufSymbols>()]
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.input)]
    }
    fn generate(&self, cx: &mut PluginContext<'_, crate::TypeScript>) -> Result<()> {
        self.options.validate()?;
        ensure!(self.options.representations.is_native() || self.options.import_mappings.is_empty(), "TypeScript alternate representations cannot use importMappings: mapped modules must expose native Protobuf types; generate owned value models without external mappings");
        ensure!(
            !self.options.reactive_streams || self.options.emit_clients,
            "TypeScript reactiveStreams requires emitted clients"
        );
        ensure!(
            !self.options.immutable_views
                || self.options.emit_binary
                || self.options.emit_json
                || self.options.emit_clients
                || self.options.emit_servers
                || self.options.emit_reflection,
            "TypeScript immutableViews requires a runtime surface"
        );
        let contract = cx.inputs.get::<RpcContract>()?;
        ensure!(
            !contract.root_files.is_empty(),
            "Protobuf input requires root files"
        );
        let mut wire: RpcWireSchema = contract.wire_schema()?;
        if !self.options.emit_imported_files {
            for file in &wire.files {
                if !contract.root_files.contains(&file.name) {
                    ensure!(
                        self.options.import_mappings.contains_key(&file.name),
                        "TypeScript omitted imported Protobuf file requires importMappings: {}",
                        file.name
                    );
                }
            }
        }
        for file in &mut wire.files {
            if self.options.import_mappings.contains_key(&file.name) {
                file.services.clear();
            }
        }

        self.options.validate_for(&wire)?;
        for file in render::generate(&wire, &contract.descriptor_set, &self.options)? {
            cx.files.emit(file)?;
        }
        for file in facade::generate(&wire, &self.options)? {
            cx.files.emit(file)?;
        }
        for file in frameworks::generate(&wire, &self.options)? {
            cx.files.emit(file)?;
        }
        for file in batch::generate(&wire, &self.options)? {
            cx.files.emit(file)?;
        }
        for file in http::generate(&wire, &self.options)? {
            cx.files.emit(file)?;
        }
        if !self.options.representations.is_native()
            && (self.options.emit_binary
                || self.options.emit_json
                || self.options.emit_clients
                || self.options.emit_servers
                || self.options.emit_reflection)
        {
            cx.files.emit(GeneratedFile::new(
                "src/poolster-rpc-values.ts",
                include_str!("grpc/value-runtime.ts.tmpl").replace(
                    "SETTINGS",
                    &serde_json::to_string(&{
                        let mut settings = serde_json::to_value(&self.options.representations)?;
                        settings["unknownEnumName"] = self.options.unknown_enum_name.clone().into();
                        settings["removeEnumPrefix"] = self.options.remove_enum_prefix.into();
                        settings
                    })?,
                ),
            )?)?;
        }
        for (enabled, name, body) in [
            (
                self.options.immutable_views,
                "immutable.ts",
                include_str!("grpc/immutable.ts.tmpl"),
            ),
            (
                self.options.value_adapters,
                "values.ts",
                include_str!("grpc/values.ts.tmpl"),
            ),
            (
                self.options.reactive_streams,
                "reactive.ts",
                include_str!("grpc/reactive.ts.tmpl"),
            ),
        ] {
            if enabled {
                cx.files
                    .emit(GeneratedFile::new(format!("src/{name}"), body)?)?;
            }
        }
        if self.options.emit_index {
            let mut index = String::new();
            let mut browser = String::new();
            let mut namespaces = std::collections::BTreeSet::new();
            for file in &wire.files {
                if self.options.import_mappings.contains_key(&file.name) {
                    continue;
                }
                let stem = layout::stem(&file.name, &self.options);
                ensure!(
                    stem != "index",
                    "Protobuf source collides with package index"
                );
                let namespace = format!(
                    "Proto_{}",
                    file.name
                        .trim_end_matches(".proto")
                        .chars()
                        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                        .collect::<String>()
                );
                ensure!(
                    namespaces.insert(namespace.clone()),
                    "Protobuf index namespaces collide: {}",
                    file.name
                );
                index.push_str(&format!(
                    "export * as {namespace} from {};\n",
                    serde_json::to_string(&format!("./{stem}.js"))?
                ));
            }
            cx.files.emit(GeneratedFile::new("src/index.ts", index)?)?;
            for file in &wire.files {
                if self.options.import_mappings.contains_key(&file.name) {
                    continue;
                }
                let stem = layout::stem(&file.name, &self.options);
                ensure!(
                    stem != "browser",
                    "Protobuf source collides with browser entry point"
                );
                let namespace = format!(
                    "Proto_{}",
                    file.name
                        .trim_end_matches(".proto")
                        .chars()
                        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                        .collect::<String>()
                );
                let suffix = if (self.options.emit_clients || self.options.emit_servers)
                    && !file.services.is_empty()
                {
                    ".messages"
                } else {
                    ""
                };
                browser.push_str(&format!(
                    "export * as {namespace} from {};\n",
                    serde_json::to_string(&format!("./{stem}{suffix}.js"))?
                ));
            }
            if self.options.emit_clients {
                for (transport, module) in [
                    (poolster_core::native::rpc::RpcTransport::GrpcWeb, "grpcWeb"),
                    (poolster_core::native::rpc::RpcTransport::Twirp, "twirp"),
                ] {
                    if self.options.transports.contains(&transport) {
                        browser.push_str(&format!("export * from './{module}.js';\n"));
                    }
                }
            }
            cx.files
                .emit(GeneratedFile::new("src/browser.ts", browser)?)?;
        }
        let runtime = self.options.emit_binary
            || self.options.emit_json
            || self.options.emit_clients
            || self.options.emit_servers
            || self.options.emit_reflection;
        let rpc = self.options.emit_clients || self.options.emit_servers;
        let mut deps = serde_json::Map::new();
        if runtime {
            deps.insert("@bufbuild/protobuf".into(), "2.16.0".into());
        }
        if rpc {
            deps.insert("@grpc/grpc-js".into(), "1.14.6".into());
        }
        if self
            .options
            .frameworks
            .contains(&poolster_core::native::rpc::RpcFramework::NiceGrpc)
        {
            deps.insert("nice-grpc".into(), "2.1.17".into());
        }
        if self
            .options
            .frameworks
            .contains(&poolster_core::native::rpc::RpcFramework::NestJs)
        {
            for name in ["@nestjs/microservices", "@nestjs/common", "@nestjs/core"] {
                deps.insert(name.into(), "12.0.1".into());
            }
            deps.insert("reflect-metadata".into(), "0.2.2".into());
            deps.insert("@grpc/proto-loader".into(), "0.8.0".into());
            deps.insert("rxjs".into(), "7.8.2".into());
        }
        if self.options.reactive_streams {
            deps.insert("rxjs".into(), "7.8.2".into());
        }
        let manifest = serde_json::json!({"name":cx.settings.package_name.as_deref().unwrap_or("poolster-grpc"),"version":cx.common.package_version.as_deref().unwrap_or("0.0.0"),"type":"module","private":true,"files":["dist","proto"],"exports":{"./*":{"types":"./dist/*.d.ts","import":"./dist/*.js"}},"scripts":{"build":"tsc -p tsconfig.json"},"engines":{"node":">=20"},"dependencies":deps,"devDependencies":{"typescript":"5.9.3","@types/node":"22.19.15"}});
        let mut manifest = manifest;
        if self.options.emit_index {
            manifest["exports"]["./browser"] =
                serde_json::json!({"types":"./dist/browser.d.ts","import":"./dist/browser.js"});
            manifest["exports"]["."] =
                serde_json::json!({"types":"./dist/index.d.ts","import":"./dist/index.js"});
        }
        cx.files.emit(GeneratedFile::new(
            "package.json",
            format!("{}\n", serde_json::to_string_pretty(&manifest)?),
        )?)?;
        cx.files.emit(GeneratedFile::new(
            "tsconfig.json",
            include_str!("grpc/tsconfig.json"),
        )?)?;
        cx.files.emit(GeneratedFile::new(
            "README.md",
            include_str!("grpc/README.md"),
        )?)?;
        for file in &contract.files {
            if let Some(source) = &file.source {
                cx.files
                    .emit(GeneratedFile::new(format!("proto/{}", file.name), source)?)?;
            }
        }
        let source = cx.inputs.reference::<RpcContract>()?.cloned();
        cx.publish(symbols::build(&wire, &self.options, source))
    }
}
