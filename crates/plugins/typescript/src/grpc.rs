//! Poolster-owned TypeScript gRPC source generation over retained descriptors.
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile,
    engine::{Handle, Meta, Plugin, PluginContext, Requirement},
    native::rpc::{RpcContract, RpcWireSchema},
};
mod render;
pub struct Grpc {
    meta: Meta,
    input: Option<Handle<RpcContract>>,
}
pub fn grpc() -> Grpc {
    Grpc {
        meta: Meta::new(),
        input: None,
    }
}
impl Grpc {
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
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.input)]
    }
    fn generate(&self, cx: &mut PluginContext<'_, crate::TypeScript>) -> Result<()> {
        let contract = cx.inputs.get::<RpcContract>()?;
        ensure!(
            !contract.root_files.is_empty() && !contract.services.is_empty(),
            "gRPC input requires root files and services"
        );
        let wire: RpcWireSchema = contract.wire_schema()?;
        for file in render::generate(&wire, &contract.descriptor_set)? {
            cx.files.emit(file)?;
        }
        let manifest = serde_json::json!({"name":cx.settings.package_name.as_deref().unwrap_or("poolster-grpc"),"version":cx.common.package_version.as_deref().unwrap_or("0.0.0"),"type":"module","private":true,"files":["dist","proto"],"exports":{"./*":{"types":"./dist/*.d.ts","import":"./dist/*.js"}},"scripts":{"build":"tsc -p tsconfig.json"},"engines":{"node":">=20"},"dependencies":{"@grpc/grpc-js":"1.14.6","@bufbuild/protobuf":"2.16.0"},"devDependencies":{"typescript":"5.9.3","@types/node":"22.19.15"}});
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
        Ok(())
    }
}
