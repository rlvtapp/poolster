use super::*;
const LANGUAGES: [&str; 10] = [
    "typescript",
    "rust",
    "go",
    "python",
    "php",
    "java",
    "csharp",
    "ruby",
    "swift",
    "elixir",
];
fn config(root: &Path, format: &str, source: &str) -> PathBuf {
    let extension = if format == "graphql" {
        "graphql"
    } else {
        "yaml"
    };
    std::fs::write(root.join(format!("input.{extension}")), source).unwrap();
    let packages = LANGUAGES.map(|language| serde_json::json!({"language":language,"path":language,"plugins":[{"name":"shared-models"}]}));
    let recipe = serde_json::json!({"input":{"format":format,"path":format!("input.{extension}")}, "output":{"path":"generated"}, "packages":packages});
    let path = root.join("poolster.json");
    std::fs::write(&path, serde_json::to_vec(&recipe).unwrap()).unwrap();
    path
}

#[test]
fn schema_only_graphql_generates_models_for_ten_languages_and_regenerates() {
    let root = tempfile::tempdir().unwrap();
    let path = config(
        root.path(),
        "graphql",
        "type Query { ping: String } input Envelope { id: ID! title: String }",
    );
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    for language in LANGUAGES {
        let manifest = root.path().join(format!(
            "generated/{language}/.poolster/model-contract.json"
        ));
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(manifest).unwrap()).unwrap();
        assert_eq!(value["schemas"].as_array().unwrap().len(), 1, "{language}");
        assert_eq!(value["language"], language);
    }
    generate_from_config(&path, ColorChoice::Never, true, false).unwrap();
}

#[test]
fn asyncapi_models_generate_without_broker_connection_and_retain_rich_constraints() {
    let root = tempfile::tempdir().unwrap();
    let path = config(
        root.path(),
        "asyncapi",
        include_str!("../../../inputs/asyncapi/tests/fixtures/kafka-json.yaml"),
    );
    generate_from_config(&path, ColorChoice::Never, false, false).unwrap();
    for language in LANGUAGES {
        let manifest = root.path().join(format!(
            "generated/{language}/.poolster/model-contract.json"
        ));
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(manifest).unwrap()).unwrap();
        assert_eq!(value["schemas"].as_array().unwrap().len(), 3, "{language}");
        let key = value["schemas"]
            .as_array()
            .unwrap()
            .iter()
            .find(|schema| schema["name"].as_str().unwrap().ends_with("Key"))
            .unwrap();
        assert_eq!(key["value"]["constraints"]["minLength"], 1);
    }
    generate_from_config(&path, ColorChoice::Never, true, false).unwrap();
}

#[test]
fn unsupported_model_client_options_fail_before_writing() {
    let root = tempfile::tempdir().unwrap();
    let path = config(
        root.path(),
        "graphql",
        "type Query { ping: String } input Envelope { id: ID! }",
    );
    let mut recipe: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    recipe["packages"][0]["plugins"][0]["style"] = "flat".into();
    std::fs::write(&path, serde_json::to_vec(&recipe).unwrap()).unwrap();
    assert!(generate_from_config(&path, ColorChoice::Never, false, false).is_err());
    assert!(!root.path().join("generated").exists());
}
