use super::*;

fn model() -> String {
    let mut models = models::Models {
        files: BTreeMap::new(),
        names: BTreeSet::new(),
        inputs: BTreeMap::new(),
    };
    let ty = ModelType {
        nullable: false,
        kind: ModelKind::Object(vec![poolster_core::native::ModelField {
            name: "role".into(),
            optional: false,
            default_value: None,
            ty: ModelType {
                nullable: false,
                kind: ModelKind::Enum(vec!["MEMBER".into(), "ADMIN".into()]),
            },
        }]),
    };
    models.model("Selection", &ty, false).unwrap();
    format!(
        "module Probe\nclass Model\nend\n{}end\n",
        models.files["Selection"].1
    )
}

#[test]
fn enum_return_type_is_not_an_rbs_method_overload() {
    assert!(model().contains("def role: () -> (\"MEMBER\" | \"ADMIN\")"));
    let mut models = models::Models {
        files: BTreeMap::new(),
        names: BTreeSet::new(),
        inputs: BTreeMap::new(),
    };
    let ty = ModelType {
        nullable: true,
        kind: ModelKind::Union(vec![
            ModelType {
                nullable: false,
                kind: ModelKind::Scalar("String".into()),
            },
            ModelType {
                nullable: false,
                kind: ModelKind::Scalar("Int".into()),
            },
        ]),
    };
    assert_eq!(
        models.ty("Choice", &ty, false).unwrap(),
        "((String | Integer))?"
    );
}

#[test]
#[ignore = "requires Ruby and RBS; parse actual nonnull enum getter signature"]
fn native_rbs_accepts_generated_enum_return_unions() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("selection.rbs");
    std::fs::write(&path, model()).unwrap();
    let result =
        std::process::Command::new(std::env::var("POOLSTER_RUBY_BINARY").unwrap_or("ruby".into()))
            .args([
                "-rrbs",
                "-e",
                "RBS::Parser.parse_signature(File.read(ARGV[0]))",
            ])
            .arg(path)
            .output()
            .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
