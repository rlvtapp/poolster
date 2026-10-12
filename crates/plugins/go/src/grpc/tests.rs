use super::*;
use poolster_core::native::rpc::{RpcFile, RpcService};
fn contract() -> RpcContract {
    RpcContract {
        root_files: vec!["api.proto".into()],
        descriptor_set: vec![1],
        files: vec![RpcFile {
            name: "api.proto".into(),
            package: "demo".into(),
            syntax: "proto3".into(),
            go_package: Some("example.com/demo/api;api".into()),
            imports: vec![],
            source: Some(String::new()),
        }],
        services: vec![RpcService {
            full_name: "demo.Store".into(),
            file: "api.proto".into(),
            methods: vec![],
        }],
    }
}
#[test]
fn malformed_contract_names_and_unknown_mappings_are_rejected() {
    let mut c = contract();
    c.files[0].name = "../outside.proto".into();
    assert!(validate(&c, "example.com/demo", &BTreeMap::new()).is_err());
    let c = contract();
    assert!(
        validate(
            &c,
            "example.com/demo",
            &BTreeMap::from([("unknown.proto".into(), "example.com/demo/api".into())])
        )
        .is_err()
    );
    let mut c = c;
    c.descriptor_set.clear();
    assert!(validate(&c, "example.com/demo", &BTreeMap::new()).is_err());
    let mut c = contract();
    c.services.clear();
    assert!(validate(&c, "example.com/demo", &BTreeMap::new()).is_ok());
}

#[test]
fn mappings_and_modules_fail_before_tools_run() {
    let c = contract();
    assert_eq!(
        validate(&c, "example.com/demo", &BTreeMap::new()).unwrap(),
        vec!["api.proto"]
    );
    assert!(validate(&c, "../escape", &BTreeMap::new()).is_err());
    assert!(validate(&c, "example.com/other", &BTreeMap::new()).is_err());
    let mut c = c;
    c.files[0].go_package = None;
    assert!(validate(&c, "example.com/demo", &BTreeMap::new()).is_err());
    assert!(
        validate(
            &c,
            "example.com/demo",
            &BTreeMap::from([("api.proto".into(), "example.com/demo/api;api".into())])
        )
        .is_ok()
    );
}
