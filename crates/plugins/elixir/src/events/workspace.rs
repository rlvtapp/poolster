use super::*;
use poolster_core::engine::FinalizeContext;
#[derive(Default)]
pub struct Workspace {
    pub(crate) models: bool,
    pub(crate) events: bool,
    pub(crate) native_api: bool,
}
impl Workspace {
    pub(crate) fn finalize(cx: &mut FinalizeContext<'_, Elixir>) -> Result<()> {
        if !cx.workspace.models {
            return Ok(());
        }
        let package = cx
            .settings
            .package_name
            .as_deref()
            .unwrap_or("poolster_models");
        let module = crate::pascal_case(package);
        let library = crate::snake_case(package);
        let version = version_literal(cx.common.package_version.as_deref().unwrap_or("0.1.0"));
        let deps = if cx.workspace.events {
            "[{:brod, \"== 4.6.3\"}, {:jesse, \"== 1.8.2\"}, {:jason, \"== 1.4.4\"}]"
        } else {
            "[]"
        };
        let app = if cx.workspace.events {
            "[extra_applications: [:logger, :brod]]"
        } else {
            "[]"
        };
        let mix = format!(
            "defmodule {module}.MixProject do\n  use Mix.Project\n  def project, do: [app: :{library}, version: {version}, elixir: \">= 1.14.0\", deps: {deps}]\n  def application, do: {app}\nend\n"
        );
        cx.files.emit(GeneratedFile::new("mix.exs", mix)?)?;
        Ok(())
    }
}

fn version_literal(version: &str) -> String {
    format!(
        "\"{}\"",
        crate::escape_elixir_string(version)
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
            .replace('\0', "\\0")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn package_versions_cannot_interpolate_elixir_expressions() {
        let version = "0.1.0#{raise(\"metadata executed\")}\n\\quote\"";
        let literal = version_literal(version);
        assert_eq!(literal.matches("\\#{").count(), 1);
        assert!(!literal.contains("\\\\#{"));
        assert!(literal.contains("\\n"));
        assert!(literal.contains("\\\""));
        assert_eq!(version_literal("0.1.0"), "\"0.1.0\"");
    }
}
