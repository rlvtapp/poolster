//! Typed package integration for the existing complete Php generator.
use anyhow::Result;
use poolster_core::SdkClientStyle;
use poolster_core::engine::{Language, Meta, Package, Plugin, PluginContext};

pub struct Php;
#[derive(Default)]
pub struct Settings {
    pub package_name: Option<String>,
}
impl Language for Php {
    const NAME: &'static str = "php";
    type Settings = Settings;
    type Workspace = crate::events::Workspace;
    fn finalize(cx: &mut poolster_core::engine::FinalizeContext<'_, Self>) -> Result<()> {
        crate::events::Workspace::finalize(cx)
    }
    fn bundle_middleware(
        tree: &mut poolster_core::GeneratedTree,
        middleware: &[poolster_core::customization::BundledMiddleware],
    ) -> Result<()> {
        crate::bundled_middleware::bundle(tree, middleware)
    }
}
pub fn package(dir: impl Into<String>) -> Package<Php> {
    Package::new(dir)
}
pub trait PackageExt {
    fn name(self, name: impl Into<String>) -> Self;
}
impl PackageExt for Package<Php> {
    fn name(mut self, name: impl Into<String>) -> Self {
        self.settings_mut().package_name = Some(name.into());
        self
    }
}
pub struct Sdk {
    meta: Meta,
    pub(crate) http: poolster_core::engine::HttpInput,
    client_style: Option<SdkClientStyle>,
}
pub fn sdk() -> Sdk {
    Sdk {
        meta: Meta::new(),
        http: Default::default(),
        client_style: None,
    }
}
impl Sdk {
    pub fn flat(mut self) -> Self {
        self.client_style = Some(SdkClientStyle::Flat);
        self
    }
    pub fn namespaced(mut self) -> Self {
        self.client_style = Some(SdkClientStyle::Namespaced);
        self
    }
}
impl Plugin<Php> for Sdk {
    fn kind(&self) -> &'static str {
        "php-sdk"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn generate(&self, cx: &mut PluginContext<'_, Php>) -> Result<()> {
        self.http.run(cx, |cx| {
            let mut tree = crate::render_sdk(
                cx.api,
                ".",
                cx.settings.package_name.as_deref(),
                self.client_style
                    .or(cx.common.client_style)
                    .unwrap_or(SdkClientStyle::Namespaced),
            )?;
            apply_package_license(&mut tree, cx.common.package_license.as_deref())?;
            cx.files.append(tree)
        })
    }
    fn requires(&self) -> Vec<poolster_core::engine::Requirement> {
        self.http.requirements()
    }

    fn supports_native_input(&self) -> bool {
        self.http.is_explicit()
    }
}

impl Sdk {
    /// Select the authoritative HTTP contract produced by an input or transform.
    pub fn input(
        mut self,
        input: poolster_core::engine::Handle<poolster_core::AdaptedApi>,
    ) -> Self {
        self.http = self.http.input(input);
        self
    }
    /// Consume complete model blocks from the selected HTTP contract revision.
    pub fn input_models(
        mut self,
        models: poolster_core::engine::Handle<poolster_core::blocks::Blocks<poolster_core::Schema>>,
    ) -> Self {
        self.http = self.http.input_models(models);
        self
    }
    /// Consume complete endpoint blocks from the selected HTTP contract revision.
    pub fn input_endpoints(
        mut self,
        endpoints: poolster_core::engine::Handle<
            poolster_core::blocks::Blocks<poolster_core::Operation>,
        >,
    ) -> Self {
        self.http = self.http.input_endpoints(endpoints);
        self
    }
}

/// Apply only the SDK author's explicit license; API licenses need not license the SDK.
pub(crate) fn licensed_manifest(source: &str, license: Option<&str>) -> Result<String> {
    let Some(license) = license else {
        return Ok(source.to_owned());
    };
    anyhow::ensure!(
        !license.trim().is_empty() && !license.contains('\0'),
        "PHP package license must not be empty"
    );
    let mut manifest: serde_json::Value = serde_json::from_str(source)?;
    manifest["license"] = license.into();
    Ok(serde_json::to_string_pretty(&manifest)? + "\n")
}

pub(crate) fn apply_package_license(
    tree: &mut poolster_core::GeneratedTree,
    license: Option<&str>,
) -> Result<()> {
    if license.is_some() {
        let source = tree
            .get("composer.json")
            .ok_or_else(|| anyhow::anyhow!("PHP package is missing composer.json"))?;
        let manifest = licensed_manifest(source, license)?;
        tree.replace(poolster_core::GeneratedFile::new(
            "composer.json",
            manifest,
        )?)?;
    }
    Ok(())
}
