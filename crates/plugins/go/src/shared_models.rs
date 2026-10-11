//! Standalone shared data models; no HTTP or GraphQL execution contract.
use crate::Go;
use anyhow::{Result, ensure};
use poolster_core::blocks::{Blocks, ModelBlock};
use poolster_core::engine::{Handle, PluginContext};
use poolster_core::models::{ModelOutput, ModelPlan, ModelPlugin, ModelSymbol};
use poolster_core::{GeneratedFile, SchemaKind, SchemaValue};
use std::{collections::BTreeMap, path::PathBuf};

pub fn shared_models(provider: Option<Handle<Blocks<ModelBlock>>>) -> ModelPlugin<Go> {
    ModelPlugin::new(provider)
}

impl ModelOutput for Go {
    const MODEL_CONTRACT_NAME: &'static str = "poolster.go.shared-models.v1";
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
fn emit(cx: &mut PluginContext<'_, Go>, plan: &ModelPlan) -> Result<BTreeMap<String, ModelSymbol>> {
    let mut symbols = BTreeMap::new();
    for schema in &plan.schemas {
        ensure!(
            schema.name.len() <= 160,
            "shared model name exceeds portable filename budget"
        );
        check(&schema.value, true)?;
        let mut source = String::new();
        crate::models::render_schema(&mut source, schema);
        let name = &schema.name;
        if let SchemaKind::Object { fields, .. } = &schema.value.kind {
            let names = crate::models::model_field_names(fields);
            for field in fields {
                if field.required
                    && matches!(field.value.kind, SchemaKind::Reference { .. })
                    && !field.value.nullable
                {
                    let target = crate::models::go_type(&field.value);
                    source = source.replace(
                        &format!("\t{} {target} `", names[&field.name]),
                        &format!("\t{} *{target} `", names[&field.name]),
                    );
                }
            }
            let decode = format!("func (model *{name}) UnmarshalJSON");
            let has_decode = source.contains(&decode);
            source = source.replace(&decode, &format!("func (model *{name}) poolsterDecodeJSON"));
            let encode = format!("func (model {name}) MarshalJSON");
            let has_encode = source.contains(&encode);
            source = source.replace(&encode, &format!("func (model {name}) poolsterEncodeJSON"));
            let mut checks = String::from(
                "var fields map[string]json.RawMessage\nif err := json.Unmarshal(data, &fields); err != nil { return err }\nif fields == nil { return fmt.Errorf(\"expected object\") }\n",
            );
            for field in fields {
                let key = serde_json::to_string(&field.name)?;
                let condition = if field.required {
                    format!(
                        "if _, ok := fields[{key}]; !ok {{ return fmt.Errorf(\"missing required field %s\", {key}) }}\n"
                    )
                } else {
                    String::new()
                };
                checks.push_str(&condition);
                if matches!(field.value.kind, SchemaKind::Null) {
                    checks.push_str(&format!("if raw, ok := fields[{key}]; ok && string(raw) != \"null\" {{ return fmt.Errorf(\"expected null-only field\") }}\n"));
                }
                if !field.value.nullable {
                    checks.push_str(&format!("if raw, ok := fields[{key}]; ok && string(raw) == \"null\" {{ return fmt.Errorf(\"null nonnullable field %s\", {key}) }}\n"));
                }
            }
            for field in fields {
                let key = serde_json::to_string(&field.name)?;
                if matches!(field.value.kind, SchemaKind::Array { .. }) {
                    checks.push_str(&format!(
                        "if raw, ok := fields[{key}]; ok && string(raw) != \"null\" {{\n"
                    ));
                    checks.push_str(&array_checks(&field.value, "raw", 0));
                    checks.push_str("}\n");
                }
            }
            source.push_str(&format!("func validate{name}(data []byte) error {{\n{checks}return nil\n}}\nfunc (model *{name}) UnmarshalJSON(data []byte) error {{\nif err := validate{name}(data); err != nil {{ return err }}\n"));
            if has_decode {
                source.push_str("return model.poolsterDecodeJSON(data)\n}\n");
            } else {
                source.push_str(&format!("type plain {name}\nvar decoded plain\nif err := json.Unmarshal(data, &decoded); err != nil {{ return err }}\n*model = {name}(decoded)\nreturn nil\n}}\n"));
            }
            source.push_str(&format!(
                "func (model {name}) MarshalJSON() ([]byte, error) {{\n"
            ));
            if has_encode {
                source.push_str("data, err := model.poolsterEncodeJSON()\n");
            } else {
                source.push_str(&format!(
                    "type plain {name}\ndata, err := json.Marshal(plain(model))\n"
                ));
            }
            source.push_str(&format!("if err != nil {{ return nil, err }}\nif err := validate{name}(data); err != nil {{ return nil, err }}\nreturn data, nil\n}}\n"));
        } else if !schema.value.enum_values.is_empty() {
            let values = schema
                .value
                .enum_values
                .iter()
                .map(|v| serde_json::to_string(v).unwrap())
                .collect::<Vec<_>>()
                .join(", ");
            source.push_str(&format!("func (model *{name}) UnmarshalJSON(data []byte) error {{\nvar value string\nif string(data) == \"null\" {{ return fmt.Errorf(\"null enum\") }}\nif err := json.Unmarshal(data, &value); err != nil {{ return err }}\nswitch value {{\ncase {values}: *model = {name}(value); return nil\ndefault: return fmt.Errorf(\"unknown {name} value %s\", value)\n}}\n}}\nfunc (model {name}) MarshalJSON() ([]byte,error) {{\nswitch string(model) {{\ncase {values}: return json.Marshal(string(model))\ndefault: return nil, fmt.Errorf(\"unknown {name} enum value\")\n}}\n}}\n"));
        }
        if !matches!(schema.value.kind, SchemaKind::Object { .. })
            && schema.value.enum_values.is_empty()
        {
            let target = if matches!(schema.value.kind, SchemaKind::Any) {
                "json.RawMessage".into()
            } else {
                crate::models::go_type(&schema.value)
            };
            source = format!(
                "type {name} {target}\nfunc (model *{name}) UnmarshalJSON(data []byte) error {{\nif string(data) == \"null\" {{ return fmt.Errorf(\"null nonnullable root model\") }}\n"
            );
            source.push_str(&array_checks(&schema.value, "data", 0));
            source.push_str(&format!("var value {target}\nif err := json.Unmarshal(data, &value); err != nil {{ return err }}\n*model = {name}(value)\nreturn nil\n}}\nfunc (model {name}) MarshalJSON() ([]byte,error) {{\ndata, err := json.Marshal({target}(model))\nif err != nil {{ return nil, err }}\nvar decoded {name}\nif err := decoded.UnmarshalJSON(data); err != nil {{ return nil, err }}\nreturn data,nil\n}}\n"));
        }
        let path = PathBuf::from(format!("model_{}.go", schema.name.to_ascii_lowercase()));
        ensure!(
            source.len() <= 128 * 1024,
            "shared model declaration exceeds 128 KiB source budget"
        );
        cx.files.emit(GeneratedFile::new(
            &path,
            format!("package models\n\nimport (\n\"encoding/json\"\n\"fmt\"\n)\n\n{source}"),
        )?)?;
        symbols.insert(
            name.clone(),
            ModelSymbol {
                name: name.clone(),
                module: path,
            },
        );
    }
    if plan.schemas.is_empty() {
        cx.files
            .emit(GeneratedFile::new("models.go", "package models\n")?)?;
    }
    let package = cx
        .settings
        .package_name
        .as_deref()
        .unwrap_or("example.com/poolster-models");
    cx.workspace.native_module = Some(package.to_owned());
    Ok(symbols)
}

fn array_checks(value: &SchemaValue, raw: &str, depth: usize) -> String {
    let SchemaKind::Array { items } = &value.kind else {
        return String::new();
    };
    let mut code = format!(
        "var items{depth} []json.RawMessage\nif err := json.Unmarshal({raw}, &items{depth}); err != nil {{ return err }}\nfor _, item{depth} := range items{depth} {{\n"
    );
    if !items.nullable {
        code.push_str(&format!("if string(item{depth}) == \"null\" {{ return fmt.Errorf(\"null nonnullable array item\") }}\n"));
    }
    if matches!(items.kind, SchemaKind::Array { .. }) {
        code.push_str(&format!("if string(item{depth}) != \"null\" {{\n"));
        code.push_str(&array_checks(items, &format!("item{depth}"), depth + 1));
        code.push_str("}\n");
    }
    code.push_str("}\n");
    code
}
