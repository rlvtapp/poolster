use super::*;
fn valid(value: &SchemaValue, e: &str, plan: &ModelPlan) -> String {
    if matches!(value.kind, SchemaKind::Null) {
        return format!("{e} === null");
    }
    let base = match &value.kind {
        SchemaKind::String => format!("is_string({e})"),
        SchemaKind::Integer => format!("is_int({e})"),
        SchemaKind::Number => format!("(is_int({e}) || is_float({e}))"),
        SchemaKind::Boolean => format!("is_bool({e})"),
        SchemaKind::Object { .. } => format!("is_array({e})"),
        SchemaKind::Array { items } => format!(
            "(is_array({e}) && array_is_list({e}) && !array_filter({e}, fn($item) => !{}))",
            valid(items, "$item", plan)
        ),
        SchemaKind::Reference { reference } => plan
            .schemas
            .iter()
            .find(|s| s.name == reference.rsplit('/').next().unwrap())
            .map(|s| valid(&s.value, e, plan))
            .unwrap_or("false".into()),
        SchemaKind::Null => format!("{e} === null"),
        _ => "true".into(),
    };
    let base = if !value.enum_values.is_empty() {
        format!(
            "({base} && in_array({e}, [{}], true))",
            value
                .enum_values
                .iter()
                .map(|v| v.as_str().map(php_string).unwrap_or_else(|| v.to_string()))
                .collect::<Vec<_>>()
                .join(", ")
        )
    } else {
        base
    };
    if value.nullable {
        format!("({e} === null || {base})")
    } else {
        format!("({e} !== null && {base})")
    }
}
pub(super) fn checked_model(
    schema: &Schema,
    namespace: &str,
    named: &NamedTypes,
    plan: &ModelPlan,
) -> String {
    let mut source = render_model(schema, namespace, named);
    if let SchemaKind::Object { fields, .. } = &schema.value.kind {
        let mut guards = String::new();
        for field in fields {
            let key = php_string(&field.name);
            let condition = valid(&field.value, &format!("$data[{key}]"), plan);
            let condition = if field.required {
                format!("(array_key_exists({key}, $data) && {condition})")
            } else {
                format!("(!array_key_exists({key}, $data) || {condition})")
            };
            writeln!(guards,"        if (!{condition}) {{\n            throw new \\InvalidArgumentException('invalid model field');\n        }}").unwrap();
        }
        let needle = "public static function fromArray(array $data): self\n    {\n";
        source = source.replace(needle, &format!("{needle}{guards}"));
    }
    source
}
