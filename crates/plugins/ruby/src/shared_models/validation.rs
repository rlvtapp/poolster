use super::*;
fn valid(value: &SchemaValue, e: &str, plan: &ModelPlan) -> String {
    if matches!(value.kind, SchemaKind::Null) {
        return format!("{e}.nil?");
    }
    let base = match &value.kind {
        SchemaKind::String => format!("{e}.is_a?(String)"),
        SchemaKind::Integer => format!("{e}.is_a?(Integer)"),
        SchemaKind::Number => format!("{e}.is_a?(Numeric)"),
        SchemaKind::Boolean => format!("({e} == true || {e} == false)"),
        SchemaKind::Object { .. } => format!("{e}.is_a?(Hash)"),
        SchemaKind::Array { items } => format!(
            "({e}.is_a?(Array) && {e}.all? {{ |item| {} }})",
            valid(items, "item", plan)
        ),
        SchemaKind::Reference { reference } => plan
            .schemas
            .iter()
            .find(|s| s.name == reference.rsplit('/').next().unwrap())
            .map(|s| valid(&s.value, e, plan))
            .unwrap_or("false".into()),
        SchemaKind::Null => format!("{e}.nil?"),
        _ => "true".into(),
    };
    if value.nullable {
        format!("({e}.nil? || {base})")
    } else {
        format!("(!{e}.nil? && {base})")
    }
}
pub(super) fn checked_model(schema: &Schema, plan: &ModelPlan) -> String {
    let mut source = render_data_model(schema, |_| false);
    if let SchemaKind::Object { fields, .. } = &schema.value.kind {
        let mut guards = String::new();
        for field in fields {
            let key = ruby_string(&field.name);
            let condition = valid(&field.value, &format!("value[{key}]"), plan);
            let condition = if field.required {
                format!("(value.key?({key}) && {condition})")
            } else {
                format!("(!value.key?({key}) || {condition})")
            };
            writeln!(
                guards,
                "        raise ArgumentError, 'invalid model field' unless {condition}"
            )
            .unwrap();
        }
        let needle = "        return value unless value.is_a?(Hash)\n";
        source = source.replace(needle, &format!("{needle}{guards}"));
    }
    source
}
