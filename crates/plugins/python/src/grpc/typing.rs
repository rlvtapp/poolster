use super::render::{module, stream};
use anyhow::{Context, Result};
use poolster_core::native::rpc::{
    RpcFieldKind, RpcFieldLabel, RpcWireField, RpcWireFile, RpcWireMessage, RpcWireSchema,
};
use std::collections::BTreeMap;
struct Types<'a> {
    definitions: BTreeMap<String, (&'a str, String, &'a RpcWireMessage)>,
    aliases: BTreeMap<String, String>,
    current: &'a str,
}
impl<'a> Types<'a> {
    fn new(current: &'a str, schema: &'a RpcWireSchema) -> Self {
        fn visit<'a>(
            values: &'a [RpcWireMessage],
            file: &'a str,
            prefix: &str,
            map: &mut BTreeMap<String, (&'a str, String, &'a RpcWireMessage)>,
        ) {
            for value in values {
                let name = if prefix.is_empty() {
                    value.name.clone()
                } else {
                    format!("{prefix}.{}", value.name)
                };
                map.insert(value.full_name.clone(), (file, name.clone(), value));
                visit(&value.messages, file, &name, map);
            }
        }
        let mut definitions = BTreeMap::new();
        for file in &schema.files {
            visit(&file.messages, &file.name, "", &mut definitions);
        }
        let aliases = schema
            .files
            .iter()
            .filter(|f| f.name != current)
            .enumerate()
            .map(|(i, f)| (f.name.clone(), format!("_poolster_p{i}")))
            .collect();
        Self {
            definitions,
            aliases,
            current,
        }
    }
    fn reference(&self, name: &str) -> Result<String> {
        let name = name.trim_start_matches('.');
        let (file, path, _) = self
            .definitions
            .get(name)
            .with_context(|| format!("Python missing message type {name}"))?;
        Ok(if *file == self.current {
            path.clone()
        } else {
            format!("{}.{path}", self.aliases[*file])
        })
    }
    fn kind(&self, kind: &RpcFieldKind) -> Result<String> {
        Ok(match kind {
            RpcFieldKind::String => "str".into(),
            RpcFieldKind::Bool => "bool".into(),
            RpcFieldKind::Bytes => "bytes".into(),
            RpcFieldKind::Double | RpcFieldKind::Float => "float".into(),
            RpcFieldKind::Message(name) | RpcFieldKind::Group(name) => self.reference(name)?,
            _ => "int".into(),
        })
    }
    fn field(&self, field: &RpcWireField) -> Result<String> {
        if field.label == RpcFieldLabel::Repeated {
            if let RpcFieldKind::Message(name) = &field.kind {
                let (_, _, definition) = self
                    .definitions
                    .get(name.trim_start_matches('.'))
                    .context("missing repeated message")?;
                if definition.map_entry {
                    let key = self.kind(&definition.fields[0].kind)?;
                    let value = self.kind(&definition.fields[1].kind)?;
                    return Ok(format!(
                        "_poolster_containers.{}[{key}, {value}]",
                        if matches!(
                            definition.fields[1].kind,
                            RpcFieldKind::Message(_) | RpcFieldKind::Group(_)
                        ) {
                            "MessageMap"
                        } else {
                            "ScalarMap"
                        }
                    ));
                }
            }
            return Ok(format!(
                "_poolster_containers.{}[{}]",
                if matches!(
                    field.kind,
                    RpcFieldKind::Message(_) | RpcFieldKind::Group(_)
                ) {
                    "RepeatedCompositeFieldContainer"
                } else {
                    "RepeatedScalarFieldContainer"
                },
                self.kind(&field.kind)?
            ));
        }
        self.kind(&field.kind)
    }
    fn imports(&self) -> String {
        let mut result = String::new();
        for (file, alias) in &self.aliases {
            result.push_str(&format!("import {} as {alias}\n", module(file)));
        }
        result
    }
}
fn enums(values: &[poolster_core::native::rpc::RpcWireEnum], indent: &str, out: &mut String) {
    let class_type = |value: &str| {
        if indent.is_empty() {
            value.to_owned()
        } else {
            format!("_poolster_typing.ClassVar[{value}]")
        }
    };
    for value in values {
        out.push_str(&format!(
            "{indent}{}: {}\n",
            value.name,
            class_type("_poolster_enum.EnumTypeWrapper")
        ));
        for item in &value.values {
            out.push_str(&format!("{indent}{}: {}\n", item.name, class_type("int")));
        }
    }
}

fn message(
    value: &RpcWireMessage,
    types: &Types<'_>,
    indent: &str,
    out: &mut String,
) -> Result<()> {
    if value.map_entry {
        return Ok(());
    }
    out.push_str(&format!(
        "{indent}class {}(_poolster_message.Message):\n",
        value.name
    ));
    let inner = format!("{indent}    ");
    out.push_str(&format!(
        "{inner}DESCRIPTOR: _poolster_typing.ClassVar[_poolster_descriptor.Descriptor]\n"
    ));
    for nested in &value.messages {
        message(nested, types, &inner, out)?;
    }
    enums(&value.enums, &inner, out);
    for extension in &value.extensions {
        out.push_str(&format!(
            "{inner}{}: _poolster_typing.ClassVar[_poolster_descriptor.FieldDescriptor]\n",
            extension.name
        ));
    }
    for field in &value.fields {
        let ty = types.field(field)?;
        if field.label == RpcFieldLabel::Repeated
            || matches!(
                field.kind,
                RpcFieldKind::Message(_) | RpcFieldKind::Group(_)
            )
        {
            out.push_str(&format!(
                "{inner}@property\n{inner}def {}(self) -> {ty}: ...\n",
                field.name
            ));
        } else {
            out.push_str(&format!("{inner}{}: {ty}\n", field.name));
        }
    }
    out.push_str(&format!("{inner}def __init__(self"));
    if !value.fields.is_empty() {
        out.push_str(", *");
    }
    for field in &value.fields {
        let ty = if field.label == RpcFieldLabel::Repeated {
            if let RpcFieldKind::Message(name) = &field.kind {
                let (_, _, definition) = types
                    .definitions
                    .get(name.trim_start_matches('.'))
                    .context("missing repeated message")?;
                if definition.map_entry {
                    format!(
                        "_poolster_typing.Mapping[{}, {}]",
                        types.kind(&definition.fields[0].kind)?,
                        if matches!(
                            definition.fields[1].kind,
                            RpcFieldKind::Message(_) | RpcFieldKind::Group(_)
                        ) {
                            format!(
                                "{} | _poolster_typing.Mapping[str, _poolster_typing.Any]",
                                types.kind(&definition.fields[1].kind)?
                            )
                        } else {
                            types.kind(&definition.fields[1].kind)?
                        }
                    )
                } else {
                    format!(
                        "_poolster_typing.Iterable[{} | _poolster_typing.Mapping[str, _poolster_typing.Any]]",
                        types.kind(&field.kind)?
                    )
                }
            } else {
                format!("_poolster_typing.Iterable[{}]", types.kind(&field.kind)?)
            }
        } else if matches!(
            field.kind,
            RpcFieldKind::Message(_) | RpcFieldKind::Group(_)
        ) {
            format!(
                "{} | _poolster_typing.Mapping[str, _poolster_typing.Any]",
                types.kind(&field.kind)?
            )
        } else {
            types.kind(&field.kind)?
        };
        out.push_str(&format!(", {}: {ty} | None = ...", field.name));
    }
    out.push_str(") -> None: ...\n");
    Ok(())
}
pub(super) fn messages(file: &RpcWireFile, schema: &RpcWireSchema) -> Result<String> {
    let types = Types::new(&file.name, schema);
    let mut out = String::from(
        "# Generated by Poolster.\nimport typing as _poolster_typing\nimport google.protobuf.message as _poolster_message\nimport google.protobuf.descriptor as _poolster_descriptor\nfrom google.protobuf.internal import containers as _poolster_containers\nfrom google.protobuf.internal import enum_type_wrapper as _poolster_enum\n",
    );
    out.push_str(&types.imports());
    for dependency in &file.public_dependencies {
        out.push_str(&format!("from {} import *\n", module(dependency)));
    }
    out.push_str("\nDESCRIPTOR: _poolster_descriptor.FileDescriptor\n");
    enums(&file.enums, "", &mut out);
    for ext in &file.extensions {
        out.push_str(&format!(
            "{}: _poolster_descriptor.FieldDescriptor\n",
            ext.name
        ));
    }
    for value in &file.messages {
        message(value, &types, "", &mut out)?;
    }
    Ok(out)
}
pub(super) fn services(file: &RpcWireFile, schema: &RpcWireSchema) -> Result<String> {
    let types = Types::new("", schema);
    let mut out =
        String::from("# Generated by Poolster.\nimport typing as _poolster_typing\nimport grpc\n");
    out.push_str(&types.imports());
    for service in &file.services {
        let short = service.full_name.rsplit('.').next().unwrap();
        out.push_str(&format!("\nclass {short}Stub:\n    def __init__(self, channel: grpc.Channel | grpc.aio.Channel) -> None: ...\n"));
        for method in &service.methods {
            let (_, input_stream, output_stream) = stream(method.streaming);
            let call = match (input_stream, output_stream) {
                (false, false) => "UnaryUnaryMultiCallable",
                (true, false) => "StreamUnaryMultiCallable",
                (false, true) => "UnaryStreamMultiCallable",
                (true, true) => "StreamStreamMultiCallable",
            };
            out.push_str(&format!(
                "    {}: grpc.{call}[{}, {}]\n",
                method.name,
                types.reference(&method.input_type)?,
                types.reference(&method.output_type)?
            ));
        }
        out.push_str(&format!("\nclass {short}Servicer:\n"));
        if service.methods.is_empty() {
            out.push_str("    pass\n");
        }
        for method in &service.methods {
            let (_, input_stream, output_stream) = stream(method.streaming);
            let input = types.reference(&method.input_type)?;
            let output = types.reference(&method.output_type)?;
            let input = if input_stream {
                format!("_poolster_typing.Iterable[{input}]")
            } else {
                input
            };
            let output = if output_stream {
                format!("_poolster_typing.Iterable[{output}]")
            } else {
                output
            };
            out.push_str(&format!("    def {}(self, request: {input}, context: grpc.ServicerContext) -> {output}: ...\n",method.name));
        }
        out.push_str(&format!("\ndef add_{short}Servicer_to_server(servicer: {short}Servicer, server: grpc.Server | grpc.aio.Server) -> None: ...\n"));
    }
    Ok(out)
}
