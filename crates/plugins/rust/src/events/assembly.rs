//! Compose model and event entrypoints once, during language finalization.
use super::*;
use poolster_core::engine::FinalizeContext;

pub(crate) fn finalize(cx: &mut FinalizeContext<'_, Rust>) -> Result<()> {
    ensure!(
        cx.workspace.shared_model_package,
        "Rust Kafka shared models must be generated in the same package"
    );
    ensure!(
        cx.workspace.graphql_package.is_none()
            && !cx.workspace.models
            && !cx.workspace.operations
            && cx.workspace.http_api.is_none(),
        "Rust model/event package cannot also assemble an HTTP or GraphQL client"
    );
    let package = cx
        .settings
        .package_name
        .as_deref()
        .unwrap_or(if cx.workspace.event_package {
            "poolster-kafka-client"
        } else {
            "poolster-models"
        });
    let version = cx.common.package_version.as_deref().unwrap_or("0.0.0");
    let mut index = String::from("pub mod models;\npub use models::*;\n");
    let mut manifest = format!(
        "[package]\nname = {package:?}\nversion = {version:?}\nedition = \"2024\"\n\n[dependencies]\nserde = {{ version = \"1\", features = [\"derive\"] }}\nserde_json = \"1\"\n"
    );
    if cx.workspace.event_package {
        index.push_str("pub mod events;\npub use events::*;\n");
        manifest.push_str("rdkafka = { version = \"=0.38.0\", default-features = false, features = [\"cmake-build\", \"tokio\"] }\njsonschema = { version = \"=0.33.0\", default-features = false }\ntokio = { version = \"=1.52.1\", features = [\"rt-multi-thread\", \"macros\", \"time\"] }\n");
    }
    cx.files.emit(GeneratedFile::new("src/lib.rs", index)?)?;
    cx.files.emit(GeneratedFile::new("Cargo.toml", manifest)?)?;
    Ok(())
}
