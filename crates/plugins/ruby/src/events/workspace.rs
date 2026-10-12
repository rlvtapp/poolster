use super::*;
use poolster_core::engine::FinalizeContext;
#[derive(Default)]
pub struct Workspace {
    pub(crate) models: bool,
    pub(crate) events: bool,
    pub(crate) native_api: bool,
    pub(crate) model_index: String,
}
impl Workspace {
    pub(crate) fn finalize(cx: &mut FinalizeContext<'_, Ruby>) -> Result<()> {
        if !cx.workspace.models {
            return Ok(());
        }
        let package = cx
            .settings
            .package_name
            .as_deref()
            .unwrap_or("poolster_models");
        let library = crate::snake_case(package);
        let version = cx.common.package_version.as_deref().unwrap_or("0.1.0");
        let package_literal = crate::ruby_string(package);
        let version_literal = crate::ruby_string(version);
        let deps = if cx.workspace.events {
            "  spec.add_dependency 'rdkafka', '= 0.28.0'\n  spec.add_dependency 'json_schemer', '= 2.5.0'\n"
        } else {
            ""
        };
        let ruby = if cx.workspace.events {
            ">= 3.2"
        } else {
            ">= 3.1"
        };
        let manifest = format!(
            "Gem::Specification.new do |spec|\n  spec.name = {package_literal}\n  spec.version = {version_literal}\n  spec.summary = 'Generated Poolster models'\n  spec.authors = ['Poolster']\n  spec.files = Dir['lib/**/*.rb']\n  spec.require_paths = ['lib']\n  spec.required_ruby_version = '{ruby}'\n{deps}end\n"
        );
        cx.files
            .emit(GeneratedFile::new(format!("{library}.gemspec"), manifest)?)?;
        let mut index = cx.workspace.model_index.clone();
        if cx.workspace.events {
            index.push_str(&format!("require_relative '{library}/events'\n"));
        }
        cx.files
            .emit(GeneratedFile::new(format!("lib/{library}.rb"), index)?)?;
        Ok(())
    }
}
