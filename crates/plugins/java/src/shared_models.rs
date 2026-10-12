//! Standalone shared data models; no HTTP or GraphQL execution contract.
use crate::Java;
use anyhow::{Result, ensure};
use poolster_core::blocks::{Blocks, ModelBlock};
use poolster_core::engine::{Handle, PluginContext};
use poolster_core::models::{ModelOutput, ModelPlan, ModelPlugin, ModelSymbol};
use poolster_core::{GeneratedFile, SchemaKind, SchemaValue};
use std::{collections::BTreeMap, path::PathBuf};

pub fn shared_models(provider: Option<Handle<Blocks<ModelBlock>>>) -> ModelPlugin<Java> {
    ModelPlugin::new(provider)
}

impl ModelOutput for Java {
    const MODEL_CONTRACT_NAME: &'static str = "poolster.java.shared-models.v1";
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
        SchemaKind::Object { fields, .. } => {
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
    cx: &mut PluginContext<'_, Java>,
    plan: &ModelPlan,
) -> Result<BTreeMap<String, ModelSymbol>> {
    let package = crate::naming::java_package_name(
        cx.settings
            .package_name
            .as_deref()
            .unwrap_or("io.poolster.models"),
    );
    let root = format!("src/main/java/{}/model", package.replace('.', "/"));
    let mut symbols = BTreeMap::new();
    for (i, schema) in plan.schemas.iter().enumerate() {
        ensure!(
            schema.name.len() <= 160,
            "shared model name exceeds portable filename budget"
        );
        check(&schema.value, true)?;
        let mut parts = crate::models::render_model_parts(schema, &package, false, i)?;
        if let SchemaKind::Object {
            fields,
            additional_properties,
        } = &schema.value.kind
        {
            ensure!(
                fields.len() <= 200,
                "shared Java presence-preserving records support at most 200 fields"
            );
            ensure!(
                matches!(
                    additional_properties,
                    poolster_core::AdditionalProperties::Forbidden
                ),
                "shared Java additional-properties objects are not supported yet"
            );
            let names = crate::native_names::field_names(
                fields,
                crate::naming::field_name,
                &[
                    "clone",
                    "getClass",
                    "toString",
                    "hashCode",
                    "equals",
                    "wait",
                    "notify",
                    "notifyAll",
                    "finalize",
                    "poolsterIsDeclaredProperty",
                    "poolsterWirePropertyName",
                ],
            );
            let mut validation = format!("\n) {{\n    public {} {{\n", schema.name);
            for field in fields {
                let native = &names[&field.name];
                let ty = crate::surface::java_type(&field.value);
                for (_, source) in &mut parts {
                    if field.required {
                        *source = source.replace(
                            &format!("@JsonProperty({:?})", field.name),
                            &format!("@JsonProperty(value = {:?}, required = true)", field.name),
                        );
                    } else {
                        *source = source.replace(
                            &format!("{ty} {native}"),
                            &format!("Presence<{ty}> {native}"),
                        );
                    }
                }
                if matches!(field.value.kind, SchemaKind::Null) {
                    for (_, source) in &mut parts {
                        let declaration = if field.required {
                            format!("Void {native}")
                        } else {
                            format!("Presence<Void> {native}")
                        };
                        *source = source.replace(&declaration, &format!("@com.fasterxml.jackson.databind.annotation.JsonDeserialize(using = NullOnly.class) {declaration}"));
                    }
                }
                if !field.value.nullable {
                    let condition = if field.required {
                        format!("{native} == null")
                    } else {
                        format!("{native} != null && {native}.value() == null")
                    };
                    validation.push_str(&format!(
                        "        if ({condition}) throw new IllegalArgumentException({:?});\n",
                        format!("null nonnullable field {}", field.name)
                    ));
                }
            }
            for field in fields {
                if matches!(field.value.kind, SchemaKind::Array { .. }) {
                    let mut expression = names[&field.name].clone();
                    if !field.required {
                        expression = format!("{expression}.value()");
                    }
                    if !field.required {
                        validation.push_str(&format!("if ({} != null) {{\n", names[&field.name]));
                    }
                    validation.push_str(&array_checks(&field.value, &expression, 0));
                    if !field.required {
                        validation.push_str("}\n");
                    }
                }
            }
            validation.push_str("    }\n}\n");
            for (_, source) in &mut parts {
                *source = source.replace("\n) {}\n", &validation);
                ensure!(
                    source.contains(&format!("public {} {{", schema.name)),
                    "shared Java model exceeded record layout capability"
                );
            }
        } else if schema.value.enum_values.is_empty() {
            for (_, source) in &mut parts {
                let name = &schema.name;
                let ty = crate::surface::java_type(&schema.value);
                let mut checks = String::from(
                    "if (value == null) throw new IllegalArgumentException(\"null nonnullable root model\");\n",
                );
                checks.push_str(&array_checks(&schema.value, "value", 0));
                *source = source.replace(
                    &format!("public {name} {{}}"),
                    &format!("public {name} {{\n{checks}}}"),
                );
                *source = source.replace(&format!("public record {name}("), &format!("@com.fasterxml.jackson.databind.annotation.JsonDeserialize(using = {name}.Decoder.class)\npublic record {name}("));
                let pos = source.rfind('}').unwrap();
                let decoder = format!(
                    "public static final class Decoder extends com.fasterxml.jackson.databind.JsonDeserializer<{name}> {{\npublic {name} deserialize(com.fasterxml.jackson.core.JsonParser parser, com.fasterxml.jackson.databind.DeserializationContext context) throws java.io.IOException {{\n{ty} value = context.readValue(parser, context.getTypeFactory().constructType(new com.fasterxml.jackson.core.type.TypeReference<{ty}>() {{}}));\ntry {{ return new {name}(value); }} catch (IllegalArgumentException error) {{ throw com.fasterxml.jackson.databind.JsonMappingException.from(parser, error.getMessage(), error); }}\n}}\npublic {name} getNullValue(com.fasterxml.jackson.databind.DeserializationContext context) throws com.fasterxml.jackson.databind.JsonMappingException {{ throw com.fasterxml.jackson.databind.JsonMappingException.from(context, \"null nonnullable root model\"); }}\n}}\n"
                );
                source.insert_str(pos, &decoder);
            }
        }
        let mut first = None;
        for (filename, source) in parts {
            let path = PathBuf::from(format!("{root}/{filename}"));
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
        format!("{root}/Presence.java"),
        include_str!("../templates/presence.java.tmpl").replace("__PACKAGE__", &package),
    )?)?;
    cx.files.emit(GeneratedFile::new(
        format!("{root}/NullOnly.java"),
        NULL_ONLY.replace("__PACKAGE__", &package),
    )?)?;
    cx.workspace.model_package = Some(package);
    Ok(symbols)
}

fn array_checks(value: &SchemaValue, expression: &str, depth: usize) -> String {
    let SchemaKind::Array { items } = &value.kind else {
        return String::new();
    };
    let mut code =
        format!("if ({expression} != null) {{\nfor (var item{depth} : {expression}) {{\n");
    if !items.nullable {
        code.push_str(&format!("if (item{depth} == null) throw new IllegalArgumentException(\"null nonnullable array item\");\n"));
    }
    if matches!(items.kind, SchemaKind::Array { .. }) {
        code.push_str(&array_checks(items, &format!("item{depth}"), depth + 1));
    }
    code.push_str("}\n}\n");
    code
}

const NULL_ONLY: &str = r#"package __PACKAGE__.model;
import com.fasterxml.jackson.core.JsonParser;
import com.fasterxml.jackson.databind.*;
import com.fasterxml.jackson.databind.deser.ContextualDeserializer;
public final class NullOnly extends JsonDeserializer<Object> implements ContextualDeserializer {
    private final boolean optional;
    public NullOnly() { optional = false; }
    private NullOnly(boolean optional) { this.optional = optional; }
    public JsonDeserializer<?> createContextual(DeserializationContext context, BeanProperty property) {
        return new NullOnly(property != null && property.getType().hasRawClass(Presence.class));
    }
    public Object deserialize(JsonParser parser, DeserializationContext context) throws java.io.IOException {
        throw JsonMappingException.from(parser, "expected null-only field");
    }
    public Object getNullValue(DeserializationContext context) { return optional ? Presence.of(null) : null; }
    public Object getAbsentValue(DeserializationContext context) { return null; }
}
"#;
