//! Poolster-owned Python message bindings and gRPC client/server source.
mod render;
#[cfg(test)]
mod tests;
mod typing;
mod validation;
use anyhow::Result;
use poolster_core::{
    GeneratedFile,
    engine::{Handle, Meta, Plugin, PluginContext, Requirement},
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
}
pub fn grpc(package_name: impl Into<String>) -> Grpc {
    Grpc {
        meta: Meta::new(),
        input: None,
        package_name: package_name.into(),
    }
}
impl Grpc {
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
    fn generate(&self, cx: &mut PluginContext<'_, crate::Python>) -> Result<()> {
        let contract = cx.inputs.get::<RpcContract>()?;
        let schema = contract.wire_schema()?;
        let files = validation::validate(contract, &schema, &self.package_name)?;
        let version = cx.common.package_version.as_deref().unwrap_or("0.1.0");
        for (path, contents) in
            render::package(contract, &schema, &files, &self.package_name, version)?
        {
            cx.files.emit(GeneratedFile::new(path, contents)?)?;
        }
        Ok(())
    }
}
