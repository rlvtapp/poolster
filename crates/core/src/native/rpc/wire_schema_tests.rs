use super::*;
use prost::Message;
use prost_types::{DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet};
fn contract(files: Vec<FileDescriptorProto>) -> RpcContract {
    RpcContract {
        root_files: vec!["root.proto".into()],
        descriptor_set: FileDescriptorSet { file: files }.encode_to_vec(),
        files: vec![],
        services: vec![],
    }
}
fn file(n: &str) -> FileDescriptorProto {
    FileDescriptorProto {
        name: Some(n.into()),
        syntax: Some("proto3".into()),
        ..Default::default()
    }
}
fn field(n: &str, num: i32, t: i32) -> FieldDescriptorProto {
    FieldDescriptorProto {
        name: Some(n.into()),
        number: Some(num),
        label: Some(1),
        r#type: Some(t),
        ..Default::default()
    }
}
fn message(n: &str) -> DescriptorProto {
    DescriptorProto {
        name: Some(n.into()),
        ..Default::default()
    }
}
#[test]
fn dependency_order_and_presence_remain_owned() {
    let mut root = file("root.proto");
    root.dependency = vec!["common.proto".into()];
    let mut m = message("Payload");
    m.oneof_decl = vec![prost_types::OneofDescriptorProto {
        name: Some("_value".into()),
        ..Default::default()
    }];
    let mut f = field("value", 1, 9);
    f.proto3_optional = Some(true);
    f.oneof_index = Some(0);
    m.field.push(f);
    root.message_type.push(m);
    let schema = contract(vec![root, file("common.proto")])
        .wire_schema()
        .unwrap();
    assert_eq!(schema.files[0].name, "common.proto");
    let value = &schema.files[1].messages[0].fields[0];
    assert!(value.proto3_optional);
    assert_eq!(value.oneof_index, Some(0));
    assert_eq!(value.kind, RpcFieldKind::String);
}
#[test]
fn malformed_unknown_and_duplicate_descriptors_fail() {
    let mut c = contract(vec![file("root.proto")]);
    c.descriptor_set = vec![10, 255];
    assert!(c.wire_schema().is_err());
    c.descriptor_set = vec![16, 0];
    assert!(
        c.wire_schema()
            .unwrap_err()
            .to_string()
            .contains("unexpected")
    );
    let c = contract(vec![file("root.proto"), file("root.proto")]);
    assert!(
        c.wire_schema()
            .unwrap_err()
            .to_string()
            .contains("duplicate descriptor file")
    );
}
#[test]
fn invalid_field_kinds_references_and_duplicates_fail() {
    for ty in [99, 11] {
        let mut root = file("root.proto");
        let mut m = message("Payload");
        let mut f = field("value", 1, ty);
        if ty == 11 {
            f.type_name = Some(".Missing".into());
        }
        m.field.push(f);
        root.message_type.push(m);
        assert!(contract(vec![root]).wire_schema().is_err());
    }
    let mut root = file("root.proto");
    let mut m = message("Payload");
    m.field = vec![field("a", 1, 9), field("b", 1, 9)];
    root.message_type.push(m);
    assert!(
        contract(vec![root])
            .wire_schema()
            .unwrap_err()
            .to_string()
            .contains("duplicate message field")
    );
}
#[test]
fn missing_and_cyclic_dependencies_fail() {
    let mut root = file("root.proto");
    root.dependency.push("missing.proto".into());
    assert!(
        contract(vec![root.clone()])
            .wire_schema()
            .unwrap_err()
            .to_string()
            .contains("missing descriptor dependency")
    );
    root.dependency = vec!["child.proto".into()];
    let mut child = file("child.proto");
    child.dependency.push("root.proto".into());
    assert!(
        contract(vec![root, child])
            .wire_schema()
            .unwrap_err()
            .to_string()
            .contains("cyclic")
    );
}
#[test]
fn raw_file_descriptor_keeps_unknown_option_bytes() {
    let mut raw = file("root.proto").encode_to_vec();
    raw.extend_from_slice(&[0xa0, 0x06, 0x07]);
    let mut c = contract(vec![]);
    c.descriptor_set = vec![10, raw.len() as u8];
    c.descriptor_set.extend_from_slice(&raw);
    let schema = c.wire_schema().unwrap();
    assert_eq!(schema.files[0].descriptor, raw);
}

#[test]
fn invalid_presence_and_defaults_fail() {
    let mut root = file("root.proto");
    let mut m = message("Payload");
    let mut f = field("value", 1, 8);
    f.default_value = Some("true".into());
    m.field.push(f);
    root.message_type.push(m);
    assert!(
        contract(vec![root.clone()])
            .wire_schema()
            .unwrap_err()
            .to_string()
            .contains("default")
    );
    root.syntax = Some("proto2".into());
    root.message_type[0].field[0].default_value = Some("not-a-bool".into());
    assert!(
        contract(vec![root])
            .wire_schema()
            .unwrap_err()
            .to_string()
            .contains("invalid default")
    );
}
