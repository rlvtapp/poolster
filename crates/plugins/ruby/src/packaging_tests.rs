use super::*;
use poolster_core::engine::{Common, Packages};

#[test]
fn generated_gems_build_with_default_and_configured_authors() {
    for authors in [
        None,
        Some(vec![
            "Relevate Team".to_owned(),
            "A \"quoted\" #{author}".to_owned(),
        ]),
    ] {
        let common = Common {
            package_authors: authors.clone(),
            ..Default::default()
        };
        let api = Api {
            name: "Relevate".into(),
            version: "0.1.0".into(),
            ..Default::default()
        };
        let tree = Packages::new()
            .package(
                package("ruby")
                    .name("relevate_sdk")
                    .common(common)
                    .with(sdk()),
            )
            .generate(&api, None)
            .unwrap();
        let root = tempfile::tempdir().unwrap();
        tree.write_to(root.path()).unwrap();
        let expected =
            serde_json::to_string(&authors.unwrap_or_else(|| vec!["SDK authors".into()])).unwrap();
        let output = std::process::Command::new(std::env::var_os("POOLSTER_TEST_RUBY").unwrap_or_else(|| "ruby".into()))
            .args(["-rjson", "-rrubygems/package", "-e", "spec = Gem::Specification.load('relevate_sdk.gemspec'); raise 'authors differ' unless spec.authors == JSON.parse(ARGV[0]); Gem::Package.build(spec)", &expected])
            .current_dir(root.path().join("ruby")).output().unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(root.path().join("ruby/relevate_sdk-0.1.0.gem").exists());
    }
}

#[test]
fn explicit_empty_authors_are_rejected() {
    for authors in [vec![], vec!["  ".to_owned()]] {
        let result = Packages::new()
            .package(
                package("ruby")
                    .common(Common::default().package_authors(authors))
                    .with(sdk()),
            )
            .generate(
                &Api {
                    name: "Test".into(),
                    ..Default::default()
                },
                None,
            );
        assert!(format!("{:#}", result.unwrap_err()).contains("authors"));
    }
}
