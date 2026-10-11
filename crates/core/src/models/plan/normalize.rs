use crate::{
    AdditionalProperties, Schema, SchemaKind, SchemaValue,
    blocks::BlockId,
    symbols::{SymbolRequest, SymbolRequests, SymbolRules},
};
use anyhow::{Result, ensure};
use std::collections::{BTreeMap, BTreeSet};

type Plan = (
    Vec<Schema>,
    BTreeMap<String, String>,
    BTreeMap<String, BlockId>,
);

/// Reserve every declaration first; inline identities use their parent ID and path.
pub(super) fn normalize(entries: Vec<(BlockId, Schema)>) -> Result<Plan> {
    let mut declarations = BTreeMap::new();
    let mut originals = BTreeMap::new();
    for (id, schema) in entries {
        ensure!(
            !schema.name.trim().is_empty(),
            "shared model name cannot be empty"
        );
        ensure!(
            originals.insert(schema.name.clone(), id.clone()).is_none(),
            "duplicate shared model name {:?}",
            schema.name
        );
        let mut value = schema.value;
        collect_children(&mut value, &id, "", &schema.name, &mut declarations)?;
        ensure!(
            declarations.insert(id, (schema.name, value)).is_none(),
            "duplicate shared model entity ID"
        );
    }
    let mut requests = SymbolRequests::new();
    requests.target_rules(
        "shared-models",
        SymbolRules {
            case_sensitive: false,
        },
    )?;
    requests.suffix_separator("shared-models", "Model")?;
    for name in [
        "String",
        "Object",
        "Array",
        "Boolean",
        "Int",
        "Float",
        "Optional",
        "Presence",
        "JsonValue",
        "List",
        "Map",
        "Dictionary",
        "Set",
        "Error",
        "Result",
        "Runtime",
        "ModelRuntime",
        "Option",
        "Vec",
        "Box",
        "NullOnly",
        "JsonElement",
        "JsonNode",
        "System",
        "Task",
        "Void",
        "NullPointerException",
        "JsonException",
        "PoolsterModels",
        "OptionalValue",
        "Serialize",
        "Deserialize",
        "Long",
        "Double",
        "Decoder",
        "JsonProperty",
        "JsonInclude",
        "JsonIgnoreProperties",
        "JsonCreator",
        "JsonValue",
        "Collections",
        "LinkedHashMap",
        "JsonSerializer",
        "JsonConverter",
        "JsonStringEnumConverter",
        "JsonValueKind",
        "Utf8JsonReader",
        "Utf8JsonWriter",
        "IJsonOnDeserialized",
        "IJsonOnSerializing",
        "PresenceJsonConverterFactory",
        "JsonRequired",
        "JsonPropertyName",
        "JsonIgnore",
        "Data",
        "URL",
        "Date",
        "Encoder",
        "CodingKey",
        "Codable",
        "Sendable",
        "PoolsterCodingKey",
        "DecodingError",
        "EncodingError",
        "Bool",
    ] {
        requests.reserve_name("shared-models", "models", name)?;
    }
    let keys = declarations
        .iter()
        .map(|(id, (name, _))| {
            let key = requests.reserve(SymbolRequest {
                entity: id.clone(),
                target: "shared-models".into(),
                module: "models".into(),
                preferred: identifier(name),
            })?;
            Ok((id.clone(), key))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    requests.resolve()?;
    let allocated = keys
        .iter()
        .map(|(id, key)| Ok((id.clone(), requests.symbol(key)?.name.clone())))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let names = originals
        .iter()
        .map(|(name, id)| (name.clone(), allocated[id].clone()))
        .collect::<BTreeMap<_, _>>();
    let mut entities = BTreeMap::new();
    let mut schemas = Vec::new();
    for (id, (_, mut value)) in declarations {
        references(&mut value, &names, &allocated)?;
        let name = allocated[&id].clone();
        entities.insert(name.clone(), id);
        schemas.push(Schema::new(name, value));
    }
    Ok((schemas, names, entities))
}

fn identifier(name: &str) -> String {
    let mut result = String::new();
    let mut upper = true;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(if upper { ch.to_ascii_uppercase() } else { ch });
            upper = false;
        } else {
            upper = true;
        }
    }
    if result.is_empty() {
        result.push_str("Model");
    }
    if result.as_bytes()[0].is_ascii_digit() {
        result.insert_str(0, "Model");
    }
    result
}

fn children(
    value: &mut SchemaValue,
    mut visit: impl FnMut(&mut SchemaValue, String) -> Result<()>,
) -> Result<()> {
    // JSON Schema's null-only shape accepts null regardless of the wrapper flag.
    if matches!(value.kind, SchemaKind::Null) {
        value.nullable = true;
    }
    match &mut value.kind {
        SchemaKind::Object {
            fields,
            additional_properties,
        } => {
            let mut names = BTreeSet::new();
            for field in fields {
                ensure!(
                    names.insert(&field.name),
                    "duplicate shared model field {:?}",
                    field.name
                );
                visit(
                    &mut field.value,
                    format!(
                        "/properties/{}",
                        field.name.replace('~', "~0").replace('/', "~1")
                    ),
                )?;
            }
            if let AdditionalProperties::Schema { value: inner } = additional_properties {
                visit(inner, "/additionalProperties".into())?;
            }
        }
        SchemaKind::Array { items } => visit(items, "/items".into())?,
        SchemaKind::OneOf { variants }
        | SchemaKind::AnyOf { variants }
        | SchemaKind::AllOf { variants } => {
            ensure!(
                !variants.is_empty(),
                "shared model composition cannot be empty"
            );
            for (index, variant) in variants.iter_mut().enumerate() {
                visit(variant, format!("/variants/{index}"))?;
            }
        }
        SchemaKind::Not { schema } => visit(schema, "/not".into())?,
        _ => {}
    }
    Ok(())
}

fn collect_children(
    value: &mut SchemaValue,
    parent: &BlockId,
    path: &str,
    preferred: &str,
    declarations: &mut BTreeMap<BlockId, (String, SchemaValue)>,
) -> Result<()> {
    children(value, |child, segment| {
        let path = format!("{path}{segment}");
        let preferred = format!(
            "{preferred}{}",
            identifier(segment.rsplit('/').next().unwrap())
        );
        collect_children(child, parent, &path, &preferred, declarations)?;
        let needs_declaration = matches!(
            child.kind,
            SchemaKind::Object { .. }
                | SchemaKind::OneOf { .. }
                | SchemaKind::AnyOf { .. }
                | SchemaKind::AllOf { .. }
        ) || !child.enum_values.is_empty()
            || child.const_value.is_some();
        if needs_declaration {
            let id = BlockId {
                source: parent.source.clone(),
                local: format!("{}::inline{path}", parent.local),
            };
            let mut declaration = child.clone();
            declaration.nullable = false;
            declaration.optional = false;
            declaration.nullish = false;
            let marker = serde_json::to_string(&id)?;
            ensure!(
                declarations.insert(id, (preferred, declaration)).is_none(),
                "inline model entity identity collision"
            );
            child.kind = SchemaKind::Reference {
                reference: format!("poolster-inline:{marker}"),
            };
            child.enum_values.clear();
            child.const_value = None;
        }
        Ok(())
    })
}

fn references(
    value: &mut SchemaValue,
    names: &BTreeMap<String, String>,
    allocated: &BTreeMap<BlockId, String>,
) -> Result<()> {
    if let SchemaKind::Reference { reference } = &mut value.kind {
        let name = if let Some(marker) = reference.strip_prefix("poolster-inline:") {
            let id: BlockId = serde_json::from_str(marker)?;
            allocated.get(&id).cloned()
        } else {
            let original = if reference.starts_with('#') {
                reference
                    .rsplit('/')
                    .next()
                    .unwrap_or(reference)
                    .replace("~1", "/")
                    .replace("~0", "~")
            } else {
                reference.clone()
            };
            names.get(&original).cloned()
        };
        let name = name.ok_or_else(|| anyhow::anyhow!("unresolved shared model reference {reference:?}; model collections must include dependency closure"))?;
        *reference = format!("#/components/schemas/{name}");
    }
    children(value, |child, _| references(child, names, allocated))
}
