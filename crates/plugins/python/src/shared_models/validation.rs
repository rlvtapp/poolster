use super::*;
fn valid(value: &SchemaValue, expression: &str, plan: &ModelPlan) -> String {
    let e = expression;
    if matches!(value.kind, SchemaKind::Null) {
        return format!("{e} is None");
    }
    let base = match &value.kind {
        SchemaKind::String => format!("isinstance({e}, str)"),
        SchemaKind::Integer => format!("(isinstance({e}, int) and not isinstance({e}, bool))"),
        SchemaKind::Number => {
            format!("(isinstance({e}, (int, float)) and not isinstance({e}, bool))")
        }
        SchemaKind::Boolean => format!("isinstance({e}, bool)"),
        SchemaKind::Object { .. } => format!("isinstance({e}, dict)"),
        SchemaKind::Array { items } => format!(
            "(isinstance({e}, list) and all({} for item in {e}))",
            valid(items, "item", plan)
        ),
        SchemaKind::Reference { reference } => plan
            .schemas
            .iter()
            .find(|s| s.name == reference.rsplit('/').next().unwrap())
            .map(|s| valid(&s.value, e, plan))
            .unwrap_or("False".into()),
        SchemaKind::OneOf { variants } | SchemaKind::AnyOf { variants } => format!(
            "({})",
            variants
                .iter()
                .map(|v| valid(v, e, plan))
                .collect::<Vec<_>>()
                .join(" or ")
        ),
        SchemaKind::Null => format!("{e} is None"),
        _ => "True".into(),
    };
    let base = if !value.enum_values.is_empty() {
        let values = value
            .enum_values
            .iter()
            .map(python_literal)
            .collect::<Vec<_>>()
            .join(", ");
        format!("({base} and {e} in [{values}])")
    } else if let Some(literal) = &value.const_value {
        format!("({base} and {e} == {})", python_literal(literal))
    } else {
        base
    };
    if value.nullable {
        format!("({e} is None or {base})")
    } else {
        format!("({e} is not None and {base})")
    }
}
pub(super) fn checked_model(schema: &Schema, plan: &ModelPlan) -> String {
    let mut source = render_model(schema);
    if let SchemaKind::Object { fields, .. } = &schema.value.kind {
        let mut guards = String::new();
        for field in fields {
            let key = serde_json::to_string(&field.name).unwrap();
            let check = valid(&field.value, &format!("value[{key}]"), plan);
            let check = if field.required {
                format!("({key} in value and {check})")
            } else {
                format!("({key} not in value or {check})")
            };
            writeln!(
                guards,
                "        if not {check}:\n            raise ValueError('invalid model field')"
            )
            .unwrap();
        }
        let needle = format!(
            "    def from_dict(cls, value: dict[str, Any]) -> \"{}\":\n",
            python_type_name(&schema.name)
        );
        source = source.replace(&needle, &format!("{needle}{guards}"));
    }
    source
}
