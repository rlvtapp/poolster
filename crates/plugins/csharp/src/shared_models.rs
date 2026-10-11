//! Standalone shared data models; no HTTP or GraphQL execution contract.
use crate::CSharp;
use anyhow::{Result, ensure};
use poolster_core::blocks::{Blocks, ModelBlock};
use poolster_core::engine::{Handle, PluginContext};
use poolster_core::models::{ModelOutput, ModelPlan, ModelPlugin, ModelSymbol};
use poolster_core::{GeneratedFile, SchemaKind, SchemaValue};
use std::{collections::BTreeMap, path::PathBuf};

pub fn shared_models(provider: Option<Handle<Blocks<ModelBlock>>>) -> ModelPlugin<CSharp> {
    ModelPlugin::new(provider)
}

impl ModelOutput for CSharp {
    const MODEL_CONTRACT_NAME: &'static str = "poolster.csharp.shared-models.v1";
    fn emit_models(
        cx: &mut PluginContext<'_, Self>,
        plan: &ModelPlan,
    ) -> Result<BTreeMap<String, ModelSymbol>> {
        emit(cx, plan)
    }
}

fn check(value: &SchemaValue, root: bool) -> Result<()> {
    ensure!(
        value.const_value.is_none(),
        "shared literal constraints are not supported by this backend"
    );
    ensure!(
        !value.nullable || !root,
        "nullable named root models are not supported by this backend"
    );
    ensure!(
        root || value.enum_values.is_empty(),
        "inline enum models must be named before generation"
    );
    match &value.kind {
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            ensure!(
                !matches!(
                    additional_properties,
                    poolster_core::AdditionalProperties::Schema { .. }
                ),
                "typed additional-properties model bags are not supported by this backend"
            );
            ensure!(root, "inline object models must be named before generation");
            for field in fields {
                check(&field.value, false)?;
            }
        }
        SchemaKind::Array { items } => check(items, false)?,
        SchemaKind::OneOf { .. }
        | SchemaKind::AnyOf { .. }
        | SchemaKind::AllOf { .. }
        | SchemaKind::Not { .. } => {
            anyhow::bail!("shared model unions and intersections are unsupported by this backend")
        }
        _ => {}
    }
    Ok(())
}
fn emit(
    cx: &mut PluginContext<'_, CSharp>,
    plan: &ModelPlan,
) -> Result<BTreeMap<String, ModelSymbol>> {
    let namespace =
        crate::http::dotnet_namespace(cx.settings.package_name.as_deref().unwrap_or("Models"));
    let mut symbols = BTreeMap::new();
    for (i, schema) in plan.schemas.iter().enumerate() {
        ensure!(
            schema.name.len() <= 160,
            "shared model name exceeds portable filename budget"
        );
        check(&schema.value, true)?;
        let mut parts = if !schema.value.enum_values.is_empty() {
            vec![(
                format!("{}.cs", schema.name),
                closed_enum(&schema.name, &namespace, &schema.value.enum_values)?,
            )]
        } else {
            crate::http::render_model_parts(schema, &namespace, false, i)?
        };
        if let SchemaKind::Object { fields, .. } = &schema.value.kind {
            let name = &schema.name;
            let names = crate::native_names::field_names(
                fields,
                crate::http::pascal_case,
                &[name, "EqualityContract"],
            );
            let mut validation =
                String::from("\n    void IJsonOnDeserialized.OnDeserialized() {\n");
            for field in fields {
                let property = &names[&field.name];
                let ty = crate::http::csharp_type(&field.value, !field.required);
                for (_, source) in &mut parts {
                    if field.required {
                        *source = source.replace(
                            &format!("    [JsonPropertyName({:?})]", field.name),
                            &format!(
                                "    [JsonRequired]\n    [JsonPropertyName({:?})]",
                                field.name
                            ),
                        );
                    } else {
                        *source = source.replace(&format!("    public {ty} {property} "), &format!("    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingDefault)]\n    public Presence<{ty}> {property} "));
                    }
                }
                if matches!(field.value.kind, SchemaKind::Null) {
                    let expression = if field.required {
                        property.clone()
                    } else {
                        format!("{property}.Value")
                    };
                    validation.push_str(&format!("if ({expression}.HasValue && {expression}.Value.ValueKind != JsonValueKind.Null) throw new JsonException(\"expected null-only field\");\n"));
                }
                if !field.value.nullable
                    && (!field.required || crate::http::is_reference_type(&field.value))
                {
                    let condition = if field.required {
                        format!("{property} is null")
                    } else {
                        format!("{property}.IsPresent && {property}.Value is null")
                    };
                    validation.push_str(&format!(
                        "        if ({condition}) throw new JsonException({:?});\n",
                        format!("null nonnullable field {}", field.name)
                    ));
                }
            }
            for field in fields {
                if matches!(field.value.kind, SchemaKind::Array { .. }) {
                    let mut expression = names[&field.name].clone();
                    if !field.required {
                        expression = format!("{expression}.Value");
                    }
                    if !field.required {
                        validation.push_str(&format!("if ({}.IsPresent) {{\n", names[&field.name]));
                    }
                    validation.push_str(&array_checks(&field.value, &expression, 0));
                    if !field.required {
                        validation.push_str("}\n");
                    }
                }
            }
            validation.push_str("    }\n    void IJsonOnSerializing.OnSerializing() => ((IJsonOnDeserialized)this).OnDeserialized();\n");
            for (_, source) in &mut parts {
                *source = source.replace(
                    &format!("record {name}\n"),
                    &format!("record {name} : IJsonOnDeserialized, IJsonOnSerializing\n"),
                );
            }
            if let Some((_, source)) = parts.first_mut() {
                let pos = source.rfind('}').expect("record declaration");
                source.insert_str(pos, &validation);
            }
        }
        if !matches!(schema.value.kind, SchemaKind::Object { .. })
            && schema.value.enum_values.is_empty()
        {
            let target = crate::http::csharp_type(&schema.value, false);
            for (_, source) in &mut parts {
                *source = source.replace(&format!("=> new(JsonSerializer.Deserialize<{target}>(ref reader, options)!);"), &format!("{{\nif (reader.TokenType == JsonTokenType.Null) throw new JsonException(\"null nonnullable root model\");\nvar value = JsonSerializer.Deserialize<{target}>(ref reader, options)!;\n{}return new(value);\n}}", array_checks(&schema.value, "value", 0)));
                *source = source.replace(
                    "writer.WriteNullValue();",
                    "throw new JsonException(\"null nonnullable root model\");",
                );
                let mut checks = array_checks(&schema.value, "value.Value", 0);
                if crate::http::is_reference_type(&schema.value) {
                    checks.insert_str(0, "if (value.Value is null) throw new JsonException(\"null nonnullable root value\");\n");
                }
                *source = source.replace(
                    "        JsonSerializer.Serialize(writer, value.Value, options);",
                    &format!(
                        "{checks}        JsonSerializer.Serialize(writer, value.Value, options);"
                    ),
                );
            }
        }
        let mut first = None;
        for (filename, source) in parts {
            let path = PathBuf::from(format!("Models/{filename}"));
            if first.is_none() {
                first = Some(path.clone());
            }
            ensure!(
                source.len() <= 128 * 1024,
                "shared model declaration exceeds 128 KiB source budget"
            );
            cx.files.emit(GeneratedFile::new(&path, source)?)?;
        }
        symbols.insert(
            schema.name.clone(),
            ModelSymbol {
                name: schema.name.clone(),
                module: first.unwrap(),
            },
        );
    }
    cx.files.emit(GeneratedFile::new(
        "Presence.cs",
        include_str!("../templates/presence.cs.tmpl").replace("__NAMESPACE__", &namespace),
    )?)?;
    cx.files.emit(GeneratedFile::new("Models.csproj", "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><TargetFramework>net8.0</TargetFramework><Nullable>enable</Nullable><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>\n")?)?;
    Ok(symbols)
}

fn closed_enum(name: &str, namespace: &str, values: &[serde_json::Value]) -> Result<String> {
    let mut source = format!(
        "using System.Text.Json;\nusing System.Text.Json.Serialization;\nnamespace {namespace};\n[JsonConverter(typeof({name}JsonConverter))]\npublic sealed record {name} {{\n    public string Value {{ get; }}\n    public {name}(string value) {{\n        switch (value) {{\n"
    );
    for value in values {
        ensure!(value.is_string(), "shared C# enums require strings");
        source.push_str(&format!("            case {value}: break;\n"));
    }
    source.push_str("            default: throw new JsonException(\"Unknown enum value\");\n        }\n        Value = value;\n    }\n");
    for (i, value) in values.iter().enumerate() {
        source.push_str(&format!(
            "    public static {name} Value{i} {{ get; }} = new({value});\n"
        ));
    }
    source.push_str(&format!("}}\npublic sealed class {name}JsonConverter : JsonConverter<{name}> {{\n    public override {name} Read(ref Utf8JsonReader reader, Type type, JsonSerializerOptions options) => reader.TokenType == JsonTokenType.String ? new(reader.GetString()!) : throw new JsonException(\"Expected enum string\");\n    public override void Write(Utf8JsonWriter writer, {name} value, JsonSerializerOptions options) => writer.WriteStringValue(value.Value);\n}}\n"));
    Ok(source)
}

fn array_checks(value: &SchemaValue, expression: &str, depth: usize) -> String {
    let SchemaKind::Array { items } = &value.kind else {
        return String::new();
    };
    let mut code =
        format!("if ({expression} != null) {{\nforeach (var item{depth} in {expression}) {{\n");
    if !items.nullable && crate::http::is_reference_type(items) {
        code.push_str(&format!(
            "if (item{depth} is null) throw new JsonException(\"null nonnullable array item\");\n"
        ));
    }
    if matches!(items.kind, SchemaKind::Array { .. }) {
        code.push_str(&array_checks(items, &format!("item{depth}"), depth + 1));
    }
    code.push_str("}\n}\n");
    code
}
