use super::*;
use anyhow::Context as _;
use poolster_core::native::rpc::*;
use std::collections::BTreeSet;

#[derive(Clone)]
pub(super) struct Type {
    pub name: String,
    pub import: String,
    pub message: bool,
    pub map_entry: bool,
}
pub(super) struct Plan {
    pub types: BTreeMap<String, Type>,
    pub packages: BTreeMap<String, (String, String)>,
    pub module: String,
    pub wire_messages: BTreeMap<String, RpcWireMessage>,
    pub options: RpcGenerationOptions,
    pub wire_enums: BTreeMap<String, RpcWireEnum>,
}
pub(super) struct Context<'a> {
    pub plan: &'a Plan,
    pub file: &'a RpcWireFile,
    pub import: String,
    pub package: String,
    pub directory: PathBuf,
    pub prefix: String,
    pub imports: BTreeMap<String, String>,
}
pub(super) fn name(value: &str) -> String {
    let mut out = String::new();
    for part in value.split('_').filter(|s| !s.is_empty()) {
        let mut c = part.chars();
        if let Some(first) = c.next() {
            out.push(first.to_ascii_uppercase());
            out.extend(c);
        }
    }
    if out.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        out.insert(0, 'X');
    }
    out
}
pub(super) fn messages(file: &RpcWireFile) -> Vec<&RpcWireMessage> {
    fn visit<'a>(m: &'a RpcWireMessage, out: &mut Vec<&'a RpcWireMessage>) {
        out.extend(m.messages.iter());
        for child in &m.messages {
            visit(child, out);
        }
    }
    let mut out = file.messages.iter().collect::<Vec<_>>();
    for m in &file.messages {
        visit(m, &mut out);
    }
    out
}
pub(super) fn enums(file: &RpcWireFile) -> Vec<&RpcWireEnum> {
    fn visit<'a>(m: &'a RpcWireMessage, out: &mut Vec<&'a RpcWireEnum>) {
        out.extend(m.enums.iter());
        for child in &m.messages {
            visit(child, out);
        }
    }
    let mut out = file.enums.iter().collect::<Vec<_>>();
    for m in &file.messages {
        visit(m, &mut out);
    }
    out
}
pub(super) fn extensions(file: &RpcWireFile) -> Vec<(&RpcWireField, String)> {
    fn visit<'a>(m: &'a RpcWireMessage, out: &mut Vec<(&'a RpcWireField, String)>) {
        out.extend(m.extensions.iter().map(|f| (f, m.full_name.clone())));
        for child in &m.messages {
            visit(child, out);
        }
    }
    let mut out = file
        .extensions
        .iter()
        .map(|f| (f, file.package.clone()))
        .collect::<Vec<_>>();
    for m in &file.messages {
        visit(m, &mut out);
    }
    out
}
impl Plan {
    pub fn new(
        contract: &RpcContract,
        schema: &RpcWireSchema,
        module: &str,
        mappings: &BTreeMap<String, String>,
        options: &RpcGenerationOptions,
    ) -> Result<Self> {
        let mut plan = Self {
            types: BTreeMap::new(),
            packages: BTreeMap::new(),
            module: module.into(),
            options: options.clone(),
            wire_messages: BTreeMap::new(),
            wire_enums: BTreeMap::new(),
        };
        let mut names = BTreeSet::new();
        for f in &schema.files {
            let source = contract
                .files
                .iter()
                .find(|v| v.name == f.name)
                .context("wire file absent from RPC contract")?;
            let mapping = mappings
                .get(&f.name)
                .or(source.go_package.as_ref())
                .cloned()
                .or_else(|| builtin(&f.name))
                .with_context(|| format!("Go mapping absent for {}", f.name))?;
            let (import, package) = mapping
                .split_once(';')
                .map(|(i, p)| (i.to_owned(), p.to_owned()))
                .unwrap_or_else(|| (mapping.clone(), mapping.rsplit('/').next().unwrap().into()));
            plan.packages
                .insert(f.name.clone(), (import.clone(), package));
            for enumeration in enums(f) {
                plan.wire_enums
                    .insert(enumeration.full_name.clone(), enumeration.clone());
            }
            for message in messages(f) {
                plan.wire_messages
                    .insert(message.full_name.clone(), message.clone());
            }
            for (full, is_message, map_entry) in
                enums(f).iter().map(|e| (&e.full_name, false, false)).chain(
                    messages(f)
                        .iter()
                        .map(|m| (&m.full_name, true, m.map_entry)),
                )
            {
                let local = full
                    .strip_prefix(&format!("{}.", f.package))
                    .unwrap_or(full);
                let local_type = local
                    .split('.')
                    .map(|part| {
                        if options.snake_type_names {
                            let mut chars = part.chars();
                            chars
                                .next()
                                .map(|first| {
                                    format!("{}{}", first.to_ascii_uppercase(), chars.as_str())
                                })
                                .unwrap_or_default()
                        } else {
                            name(part)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("_");
                let external = options.import_mappings.contains_key(&f.name)
                    || !(import == module || import.starts_with(&format!("{module}/")));
                let symbol = if external {
                    local.split('.').map(name).collect::<Vec<_>>().join("_")
                } else {
                    format!("{}{local_type}{}", options.type_prefix, options.type_suffix)
                };
                ensure!(
                    symbol
                        .as_bytes()
                        .first()
                        .is_some_and(u8::is_ascii_uppercase),
                    "Go public type names must start with an uppercase letter: {symbol}"
                );
                let symbol = if options.emit_types
                    || !(import == module || import.starts_with(&format!("{module}/")))
                {
                    symbol
                } else {
                    format!("poolster{symbol}")
                };
                ensure!(
                    names.insert((import.clone(), symbol.clone())),
                    "Go Protobuf declaration collision: {symbol}"
                );
                plan.types.insert(
                    full.clone(),
                    Type {
                        name: symbol,
                        import: import.clone(),
                        message: is_message,
                        map_entry,
                    },
                );
            }
        }
        Ok(plan)
    }
    pub fn enum_value(&self, n: &str, v: &str) -> Option<i32> {
        self.wire_enums
            .get(n)?
            .values
            .iter()
            .find(|value| value.name == v)
            .map(|v| v.number)
    }
    pub fn enum_first(&self, n: &str) -> Result<i32> {
        Ok(self
            .wire_enums
            .get(n)
            .context("enum missing")?
            .values
            .first()
            .context("enum has no values")?
            .number)
    }
    pub fn context<'a>(&'a self, file: &'a RpcWireFile) -> Result<Context<'a>> {
        let (import, package) = self.packages[&file.name].clone();
        let directory = PathBuf::from(
            import
                .strip_prefix(&self.module)
                .unwrap()
                .trim_start_matches('/'),
        );
        let mut imports = BTreeMap::new();
        for (path, _) in self.packages.values() {
            if path != &import {
                imports.entry(path.clone()).or_insert(String::new());
            }
        }
        for (index, alias) in imports.values_mut().enumerate() {
            *alias = format!("pb{index}");
        }
        let prefix = format!(
            "file_{}",
            file.name
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                .collect::<String>()
        );
        Ok(Context {
            plan: self,
            file,
            import,
            package,
            directory,
            prefix,
            imports,
        })
    }
}
impl Context<'_> {
    pub fn ty(&self, full: &str) -> Result<String> {
        let t = self
            .plan
            .types
            .get(full.trim_start_matches('.'))
            .with_context(|| format!("missing Go type {full}"))?;
        ensure!(
            t.import == self.import
                || !t
                    .name
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_lowercase),
            "Go internal/codec-only types cannot be referenced across generated packages: {full}; enable emitTypes"
        );
        Ok(if t.import == self.import {
            t.name.clone()
        } else {
            format!("{}.{}", self.imports[&t.import], t.name)
        })
    }
    pub fn scalar(&self, kind: &RpcFieldKind) -> Result<String> {
        Ok(match kind {
            RpcFieldKind::Double => "float64".into(),
            RpcFieldKind::Float => "float32".into(),
            RpcFieldKind::Int64 | RpcFieldKind::Sfixed64 | RpcFieldKind::Sint64 => "int64".into(),
            RpcFieldKind::Uint64 | RpcFieldKind::Fixed64 => "uint64".into(),
            RpcFieldKind::Int32 | RpcFieldKind::Sfixed32 | RpcFieldKind::Sint32 => "int32".into(),
            RpcFieldKind::Uint32 | RpcFieldKind::Fixed32 => "uint32".into(),
            RpcFieldKind::Bool => "bool".into(),
            RpcFieldKind::String => "string".into(),
            RpcFieldKind::Bytes => "[]byte".into(),
            RpcFieldKind::Message(n) | RpcFieldKind::Group(n) => format!("*{}", self.ty(n)?),
            RpcFieldKind::Enum(n) => self.ty(n)?,
        })
    }
    pub fn field(&self, f: &RpcWireField) -> Result<String> {
        if let RpcFieldKind::Message(n) = &f.kind {
            if self.plan.types[n].map_entry {
                let m = self
                    .plan
                    .wire_messages
                    .get(n)
                    .context("map entry missing")?;
                return Ok(format!(
                    "map[{}]{}",
                    self.scalar(&m.fields[0].kind)?,
                    self.scalar(&m.fields[1].kind)?
                ));
            }
        }
        let scalar = self.scalar(&f.kind)?;
        Ok(if f.label == RpcFieldLabel::Repeated {
            format!("[]{scalar}")
        } else if self.pointer(f) {
            format!("*{scalar}")
        } else {
            scalar
        })
    }
    pub fn pointer(&self, f: &RpcWireField) -> bool {
        f.label != RpcFieldLabel::Repeated
            && !matches!(
                f.kind,
                RpcFieldKind::Message(_) | RpcFieldKind::Group(_) | RpcFieldKind::Bytes
            )
            && (self.file.syntax == "proto2" || f.proto3_optional)
            && f.oneof_index.is_none_or(|_| f.proto3_optional)
    }
}
fn builtin(file: &str) -> Option<String> {
    let stem = file
        .strip_prefix("google/protobuf/")?
        .strip_suffix(".proto")?;
    let package = match stem {
        "descriptor" => "descriptorpb",
        "timestamp" => "timestamppb",
        "duration" => "durationpb",
        "any" => "anypb",
        "empty" => "emptypb",
        "struct" => "structpb",
        "wrappers" => "wrapperspb",
        "field_mask" => "fieldmaskpb",
        "api" => "apipb",
        "type" => "typepb",
        "source_context" => "sourcecontextpb",
        _ => return None,
    };
    Some(if stem == "descriptor" {
        format!("google.golang.org/protobuf/types/{package};{package}")
    } else {
        format!("google.golang.org/protobuf/types/known/{package};{package}")
    })
}
