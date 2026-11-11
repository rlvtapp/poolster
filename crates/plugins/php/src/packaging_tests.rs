use super::*;
use poolster_core::engine::{Common, Packages};
fn generated(license: Option<&str>) -> GeneratedTree {
    let common = Common {
        package_license: license.map(str::to_owned),
        ..Default::default()
    };
    Packages::new()
        .package(
            package("php")
                .name("relevate/sdk")
                .common(common)
                .with(sdk()),
        )
        .generate(
            &Api {
                name: "Relevate".into(),
                version: "0.1.0".into(),
                ..Default::default()
            },
            None,
        )
        .unwrap()
}
#[test]
fn composer_uses_only_explicit_sdk_license() {
    for license in [
        None,
        Some("proprietary"),
        Some("MIT"),
        Some("(MIT OR Apache-2.0)"),
    ] {
        let tree = generated(license);
        let value: serde_json::Value =
            serde_json::from_str(tree.get("php/composer.json").unwrap()).unwrap();
        assert_eq!(
            value.get("license").and_then(|value| value.as_str()),
            license
        );
    }
    assert!(crate::package::licensed_manifest("{}", Some(" ")).is_err());
}
#[test]
#[ignore = "requires Composer 2; set POOLSTER_TEST_COMPOSER for a custom executable"]
fn composer_validates_configured_package_without_license_warning() {
    let root = tempfile::tempdir().unwrap();
    generated(Some("proprietary"))
        .write_to(root.path())
        .unwrap();
    let output = std::process::Command::new(
        std::env::var_os("POOLSTER_TEST_COMPOSER").unwrap_or_else(|| "composer".into()),
    )
    .args([
        "validate",
        "--no-check-publish",
        "--no-interaction",
        "--strict",
    ])
    .current_dir(root.path().join("php"))
    .output()
    .unwrap();
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{log}");
    assert!(!log.contains("No license specified"), "{log}");
}
