//! Poolster-owned Python message bindings and gRPC client/server source.
mod assembly;
mod async_typing;
mod batching;
mod codecs;
mod enum_aliases;
mod http;
mod immutable;
mod presentation;
mod render;
mod styles;
mod symbols;
pub use symbols::{ProtobufMessageSymbol, ProtobufSymbols, ProtobufValueSymbol};
#[cfg(test)]
mod tests;
mod typing;
mod validation;
mod values;
mod views;
use anyhow::Result;
use poolster_core::{
    GeneratedFile,
    engine::{Handle, Meta, Plugin, PluginContext, Provision, Requirement},
    native::rpc::RpcContract,
};
pub const GRPC_VERSION: &str = "1.78.0";
pub const PROTOBUF_VERSION: &str = "6.33.5";
pub const SETUPTOOLS_VERSION: &str = "80.9.0";
/// Poolster-generated message modules, client stubs and server interfaces.
pub struct Grpc {
    meta: Meta,
    input: Option<Handle<RpcContract>>,
    package_name: String,
    style: Option<poolster_core::SdkClientStyle>,
    options: poolster_core::native::rpc::RpcGenerationOptions,
}
pub fn grpc(package_name: impl Into<String>) -> Grpc {
    Grpc {
        meta: Meta::new(),
        input: None,
        package_name: package_name.into(),
        style: None,
        options: Default::default(),
    }
}
impl Grpc {
    pub fn handle(&self) -> Handle<ProtobufSymbols> {
        self.meta.handle()
    }
    pub fn style(mut self, style: poolster_core::SdkClientStyle) -> Self {
        self.style = Some(style);
        self
    }

    pub fn options(mut self, options: poolster_core::native::rpc::RpcGenerationOptions) -> Self {
        self.options = options;
        self
    }
    pub fn input(mut self, input: Handle<RpcContract>) -> Self {
        self.input = Some(input);
        self
    }
}
impl Plugin<crate::Python> for Grpc {
    fn kind(&self) -> &'static str {
        "python-grpc"
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
        vec![Provision::of::<ProtobufSymbols>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, crate::Python>) -> Result<()> {
        self.options.validate()?;
        anyhow::ensure!(
            self.options.enum_style == poolster_core::native::rpc::RpcEnumStyle::Native
                && !self.options.nestjs_rest_parameter,
            "Python Protobuf does not support JavaScript enum declaration or NestJS parameter options"
        );
        anyhow::ensure!(
            self.options
                .frameworks
                .iter()
                .all(|framework| *framework == poolster_core::native::rpc::RpcFramework::Native),
            "Python Protobuf supports only the native grpcio framework"
        );
        anyhow::ensure!(
            self.options.max_depth <= 100,
            "Python native Protobuf decoding currently supports maxDepth at most 100"
        );
        let contract = cx.inputs.get::<RpcContract>()?;
        let schema = contract.wire_schema()?;
        self.options.validate_for(&schema)?;
        validation::validate(contract, &schema, &self.package_name)?;
        let files = presentation::validate(contract, &schema, &self.options)?;
        let version = cx.common.package_version.as_deref().unwrap_or("0.1.0");
        for (path, contents) in render::package(
            contract,
            &schema,
            &files,
            &self.package_name,
            version,
            &self.options,
            self.style
                .map(|style| match style {
                    poolster_core::SdkClientStyle::Flat => {
                        poolster_core::native::rpc::RpcClientStyle::Flat
                    }
                    poolster_core::SdkClientStyle::Namespaced => {
                        poolster_core::native::rpc::RpcClientStyle::Idiomatic
                    }
                })
                .unwrap_or(self.options.client_style),
        )? {
            cx.files.emit(GeneratedFile::new(path, contents)?)?;
        }
        let source = cx.inputs.reference::<RpcContract>()?.cloned();
        cx.publish(symbols::build(&schema, source, &self.options, &files)?)
    }
}
