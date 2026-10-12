//! Typed policy-selected value models with checked, lossless native conversion.
use anyhow::{Result, ensure};
use poolster_core::native::rpc::*;
use std::collections::{BTreeMap, BTreeSet};
struct Model<'a> {
    file: &'a RpcWireFile,
    message: &'a RpcWireMessage,
    name: String,
    module: String,
}
fn models<'a>(
    schema: &'a RpcWireSchema,
    files: &[String],
    options: &RpcGenerationOptions,
) -> Result<BTreeMap<String, Model<'a>>> {
    fn collect<'a>(
        messages: &'a [RpcWireMessage],
        file: &'a RpcWireFile,
        prefix: &str,
        options: &RpcGenerationOptions,
        out: &mut BTreeMap<String, Model<'a>>,
        names: &mut BTreeSet<String>,
    ) -> Result<()> {
        for message in messages {
            if message.map_entry {
                continue;
            }
            ensure!(
                message.fields.iter().all(|field| !matches!(
                    field.name.as_str(),
                    "_native" | "_native_type" | "to_native" | "from_native"
                )),
                "Python value-model field collides with conversion API: {}",
                message.full_name
            );
            let original = format!("{prefix}{}", message.name);
            let name = super::presentation::type_name(&original, options);
            ensure!(
                names.insert(name.clone()),
                "Python value-model naming collision: {}",
                message.full_name
            );
            let native = super::presentation::module(&file.name, options);
            let module = format!("{}_values", native.trim_end_matches("_pb2"));
            out.insert(
                message.full_name.clone(),
                Model {
                    file,
                    message,
                    name,
                    module,
                },
            );
            collect(
                &message.messages,
                file,
                &format!("{original}_"),
                options,
                out,
                names,
            )?;
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    for file in &schema.files {
        if files.contains(&file.name) {
            collect(
                &file.messages,
                file,
                "",
                options,
                &mut out,
                &mut BTreeSet::new(),
            )?;
        }
    }
    Ok(out)
}
fn scalar(
    field: &RpcWireField,
    models: &BTreeMap<String, Model<'_>>,
    current: &str,
    types: &super::typing::Types<'_>,
    options: &RpcGenerationOptions,
) -> Result<String> {
    Ok(match &field.kind {
        RpcFieldKind::Int64
        | RpcFieldKind::Uint64
        | RpcFieldKind::Sint64
        | RpcFieldKind::Fixed64
        | RpcFieldKind::Sfixed64 => match options.representations.int64 {
            RpcInt64Representation::Native => "int",
            RpcInt64Representation::String => "str",
            RpcInt64Representation::NumberExact => "float",
        }
        .into(),
        RpcFieldKind::Bytes => if options.representations.bytes == RpcBytesRepresentation::Base64 {
            "str"
        } else {
            "bytes"
        }
        .into(),
        RpcFieldKind::String => "str".into(),
        RpcFieldKind::Bool => "bool".into(),
        RpcFieldKind::Double | RpcFieldKind::Float => "float".into(),
        RpcFieldKind::Enum(_) => if options.representations.enums == RpcEnumsRepresentation::Name {
            "str | _values.UnknownEnum"
        } else {
            "int"
        }
        .into(),
        RpcFieldKind::Message(name) | RpcFieldKind::Group(name) => {
            if name == "google.protobuf.Timestamp"
                && options.representations.timestamp != RpcTimestampRepresentation::Native
            {
                match options.representations.timestamp {
                    RpcTimestampRepresentation::DateTime => "_datetime.datetime",
                    _ => "str",
                }
                .into()
            } else if name == "google.protobuf.Duration"
                && options.representations.duration != RpcDurationRepresentation::Native
            {
                match options.representations.duration {
                    RpcDurationRepresentation::TimeDelta => "_datetime.timedelta",
                    _ => "float",
                }
                .into()
            } else if let Some(model) = models.get(name) {
                if model.file.name == current {
                    model.name.clone()
                } else {
                    format!(
                        "_model_{}.{}",
                        crate::python_identifier(
                            &model.file.name.trim_end_matches(".proto").replace('/', "_")
                        ),
                        model.name
                    )
                }
            } else {
                types.reference(name)?
            }
        }
        _ => "int".into(),
    })
}
pub(super) fn render(
    schema: &RpcWireSchema,
    files: &[String],
    runtime: &str,
    id: &str,
    options: &RpcGenerationOptions,
) -> Result<Vec<(String, String)>> {
    if options.representations.is_native() {
        return Ok(Vec::new());
    }
    let models = models(schema, files, options)?;
    let helper = format!("_poolster_values_{id}");
    let mapping: serde_json::Map<String, serde_json::Value> = models
        .iter()
        .map(|(name, model)| (name.clone(), serde_json::json!([model.module, model.name])))
        .collect();
    let source = include_str!("values_runtime.py.tmpl")
        .replace("{{RUNTIME}}", runtime)
        .replace(
            "{{UNKNOWN_ENUM_NAME}}",
            &serde_json::to_string(&options.unknown_enum_name)?,
        )
        .replace(
            "{{POLICY}}",
            &format!(
                "_json.loads({})",
                serde_json::to_string(&serde_json::to_string(&options.representations)?)?
            ),
        )
        .replace(
            "{{MODELS}}",
            &format!(
                "_json.loads({})",
                serde_json::to_string(&serde_json::to_string(&mapping)?)?
            ),
        );
    let mut output = vec![(format!("src/{helper}.py"), source)];
    for file in &schema.files {
        let local: Vec<_> = models
            .values()
            .filter(|model| model.file.name == file.name)
            .collect();
        if local.is_empty() {
            continue;
        }
        let types = super::typing::Types::configured("", schema, options);
        let mut code = format!(
            "from __future__ import annotations\nimport dataclasses as _dataclasses\nimport datetime as _datetime\nimport typing as _typing\nimport {helper} as _values\n"
        );
        code.push_str(&types.imports());
        code.push_str("if _typing.TYPE_CHECKING:\n");
        let mut count = 0;
        for other in &schema.files {
            if other.name != file.name {
                if let Some(model) = models.values().find(|model| model.file.name == other.name) {
                    code.push_str(&format!(
                        "    import {} as _model_{}\n",
                        model.module,
                        crate::python_identifier(
                            &other.name.trim_end_matches(".proto").replace('/', "_")
                        )
                    ));
                    count += 1;
                }
            }
        }
        if count == 0 {
            code.push_str("    pass\n");
        }
        for model in &local {
            code.push_str(&format!("\n@_dataclasses.dataclass\nclass {}(_values.ValueMessage):\n    _native_type: _typing.ClassVar[type[{}]]\n    _native: {} | None = _dataclasses.field(default=None, init=False, repr=False, compare=False)\n",model.name,types.reference(&model.message.full_name)?,types.reference(&model.message.full_name)?));
            for field in &model.message.fields {
                let mut ty = scalar(field, &models, &file.name, &types, options)?;
                if field.label == RpcFieldLabel::Repeated {
                    if let RpcFieldKind::Message(name) = &field.kind {
                        fn find<'a>(
                            messages: &'a [RpcWireMessage],
                            name: &str,
                        ) -> Option<&'a RpcWireMessage> {
                            for message in messages {
                                if message.full_name == name {
                                    return Some(message);
                                }
                                if let Some(found) = find(&message.messages, name) {
                                    return Some(found);
                                }
                            }
                            None
                        }
                        if let Some(entry) = schema
                            .files
                            .iter()
                            .find_map(|file| find(&file.messages, name))
                            .filter(|message| message.map_entry)
                        {
                            let key =
                                scalar(&entry.fields[0], &models, &file.name, &types, options)?;
                            let value =
                                scalar(&entry.fields[1], &models, &file.name, &types, options)?;
                            ty = if options.representations.maps == RpcMapsRepresentation::Entries {
                                format!("list[tuple[{key}, {value}]]")
                            } else {
                                format!("dict[{key}, {value}]")
                            };
                        } else {
                            ty = format!("list[{ty}]");
                        }
                    } else {
                        ty = format!("list[{ty}]");
                    }
                } else if field.label == RpcFieldLabel::Required
                    || field.oneof_index.is_some()
                    || field.proto3_optional
                    || file.syntax == "proto2"
                    || matches!(
                        field.kind,
                        RpcFieldKind::Message(_) | RpcFieldKind::Group(_)
                    )
                {
                    ty.push_str(
                        if options.representations.optional == RpcOptionalRepresentation::Nullable {
                            " | None"
                        } else {
                            " | _values.Unset"
                        },
                    );
                }
                code.push_str(&format!("    {}: {ty} = _dataclasses.field(default_factory=lambda: _values.default({}, {}))\n",field.name,serde_json::to_string(&model.message.full_name)?,serde_json::to_string(&field.name)?));
            }
            code.push_str(&format!("    def to_native(self) -> {}:\n        return _values.to_native(self)\n    @classmethod\n    def from_native(cls, value: {}) -> {}:\n        return _values.from_native(cls, value)\n\n_values.register({}, {})\n",types.reference(&model.message.full_name)?,types.reference(&model.message.full_name)?,model.name,serde_json::to_string(&model.message.full_name)?,model.name));
        }
        let path = format!("src/{}.py", local[0].module.replace('.', "/"));
        output.push((path, code.clone()));
        if options.emit_types {
            output.push((
                format!("src/{}.pyi", local[0].module.replace('.', "/")),
                code.lines()
                    .filter(|line| !line.starts_with("_values.register("))
                    .map(|line| {
                        if line.trim_start().starts_with("return _values.") {
                            "        ..."
                        } else {
                            line
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            ));
        }
    }
    if options.emit_types {
        output.push((format!("src/{helper}.pyi"),"from google.protobuf.message import Message\nfrom typing import Any\nclass ValueMessage:\n    def to_native(self) -> Message: ...\nclass Unset: ...\nUNSET: Unset\nclass UnknownEnum:\n    number: int\n    name: str\n    def __init__(self, number: int, name: str = ...) -> None: ...\ndef default(full_name: str, field_name: str) -> Any: ...\ndef to_native(value: ValueMessage) -> Any: ...\ndef from_native(cls: Any, value: Message) -> Any: ...\ndef register(full_name: str, cls: Any) -> None: ...\n".into()));
    }
    Ok(output)
}

pub(super) fn references(
    schema: &RpcWireSchema,
    files: &[String],
    options: &RpcGenerationOptions,
) -> Result<BTreeMap<String, super::symbols::ProtobufValueSymbol>> {
    if options.representations.is_native() {
        return Ok(BTreeMap::new());
    }
    Ok(models(schema, files, options)?
        .into_iter()
        .map(|(name, model)| {
            (
                name,
                super::symbols::ProtobufValueSymbol {
                    module: model.module,
                    type_name: model.name,
                    to_native: "to_native".into(),
                    from_native: "from_native".into(),
                },
            )
        })
        .collect())
}
