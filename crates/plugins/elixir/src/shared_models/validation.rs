use super::*;
fn valid(value: &SchemaValue, e: &str, plan: &ModelPlan) -> String {
    if matches!(value.kind, SchemaKind::Null) {
        return format!("is_nil({e})");
    }
    let base = match &value.kind {
        SchemaKind::String => format!("is_binary({e})"),
        SchemaKind::Integer => format!("is_integer({e})"),
        SchemaKind::Number => format!("is_number({e})"),
        SchemaKind::Boolean => format!("is_boolean({e})"),
        SchemaKind::Object { .. } => format!("is_map({e})"),
        SchemaKind::Array { items } => format!(
            "(is_list({e}) and Enum.all?({e}, fn item -> {} end))",
            valid(items, "item", plan)
        ),
        SchemaKind::Reference { reference } => plan
            .schemas
            .iter()
            .find(|s| s.name == reference.rsplit('/').next().unwrap())
            .map(|s| valid(&s.value, e, plan))
            .unwrap_or("false".into()),
        SchemaKind::Null => format!("is_nil({e})"),
        _ => "true".into(),
    };
    if value.nullable {
        format!("(is_nil({e}) or {base})")
    } else {
        format!("(!is_nil({e}) and {base})")
    }
}
pub(super) fn checked_model(module: &str, schema: &Schema, plan: &ModelPlan) -> String {
    let mut source = render_model(module, schema);
    if let SchemaKind::Object { fields, .. } = &schema.value.kind {
        let mut guards = String::new();
        for field in fields {
            let key = serde_json::to_string(&field.name)
                .unwrap()
                .replace("#{", "\\#{");
            let condition = valid(&field.value, &format!("Map.get(map, {key})"), plan);
            let condition = if field.required {
                format!("(Map.has_key?(map, {key}) and {condition})")
            } else {
                format!("(!Map.has_key?(map, {key}) or {condition})")
            };
            writeln!(
                guards,
                "    if !{condition}, do: raise(ArgumentError, \"invalid model field\")"
            )
            .unwrap();
        }
        let needle = "  def from_map(map) when is_map(map) do\n";
        source = source.replace(needle, &format!("{needle}{guards}"));
    }
    source
}
