//! Python module policy and public aliases preserve native descriptor identity.
use anyhow::{Result, ensure};
use poolster_core::native::rpc::{RpcContract, RpcGenerationOptions, RpcWireSchema};
use std::collections::BTreeSet;
pub(super) fn module(file: &str, options: &RpcGenerationOptions) -> String {
    options
        .import_mappings
        .get(file)
        .cloned()
        .unwrap_or_else(|| {
            if file.starts_with("google/protobuf/") {
                return super::render::module(file);
            }
            format!(
                "{}{}_pb2",
                file.trim_end_matches(".proto").replace('/', "."),
                options.file_suffix
            )
        })
}
pub(super) fn type_name(name: &str, options: &RpcGenerationOptions) -> String {
    let name = if options.snake_type_names {
        crate::python_identifier(name)
    } else {
        name.into()
    };
    format!("{}{name}{}", options.type_prefix, options.type_suffix)
}
pub(super) fn validate(
    contract: &RpcContract,
    schema: &RpcWireSchema,
    options: &RpcGenerationOptions,
) -> Result<Vec<String>> {
    let mut emitted = Vec::new();
    let mut modules = BTreeSet::new();
    for file in &schema.files {
        let external = options.import_mappings.contains_key(&file.name)
            || file.name.starts_with("google/protobuf/");
        if !external && (options.emit_imported_files || contract.root_files.contains(&file.name)) {
            emitted.push(file.name.clone());
        } else if !external {
            ensure!(
                file.messages.is_empty() && file.enums.is_empty() && file.services.is_empty(),
                "Python omitted Protobuf import {} needs an explicit importMappings module",
                file.name
            );
        }
        let module = module(&file.name, options);
        ensure!(
            module.split('.').all(identifier),
            "Python Protobuf module mapping must use safe dotted identifiers: {module}"
        );
        ensure!(
            modules.insert(module.to_lowercase()),
            "Python Protobuf mapped module collision: {module}"
        );
        fn names(
            messages: &[poolster_core::native::rpc::RpcWireMessage],
            enums: &[poolster_core::native::rpc::RpcWireEnum],
            options: &RpcGenerationOptions,
        ) -> Result<()> {
            let mut exports: BTreeSet<String> = messages
                .iter()
                .filter(|message| !message.map_entry)
                .map(|message| message.name.clone())
                .chain(enums.iter().map(|value| value.name.clone()))
                .chain(["DESCRIPTOR".into(), "_runtime".into(), "__name__".into()])
                .collect();
            for original in messages
                .iter()
                .filter(|message| !message.map_entry)
                .map(|message| &message.name)
                .chain(enums.iter().map(|value| &value.name))
            {
                let name = type_name(original, options);
                ensure!(identifier(&name), "Invalid Python public type name {name}");
                ensure!(
                    name == *original || exports.insert(name.clone()),
                    "Python public type naming collision: {name}"
                );
            }
            for message in messages {
                names(&message.messages, &message.enums, options)?;
            }
            Ok(())
        }
        if !external {
            names(&file.messages, &file.enums, options)?;
        }
    }
    Ok(emitted)
}
fn identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z' | b'A'..=b'Z' | b'_'))
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        && !matches!(
            value,
            "class"
                | "def"
                | "import"
                | "from"
                | "return"
                | "global"
                | "lambda"
                | "None"
                | "True"
                | "False"
                | "async"
                | "await"
                | "if"
                | "else"
                | "for"
                | "while"
                | "try"
                | "except"
                | "with"
                | "as"
                | "pass"
                | "raise"
                | "yield"
                | "del"
                | "in"
                | "is"
                | "not"
                | "and"
                | "or"
                | "assert"
                | "break"
                | "continue"
                | "elif"
                | "finally"
                | "nonlocal"
        )
}
pub(super) fn aliases(
    file: &poolster_core::native::rpc::RpcWireFile,
    options: &RpcGenerationOptions,
) -> String {
    fn visit(
        messages: &[poolster_core::native::rpc::RpcWireMessage],
        prefix: &str,
        options: &RpcGenerationOptions,
        source: &mut String,
    ) {
        for message in messages {
            if message.map_entry {
                continue;
            }
            let name = type_name(&message.name, options);
            let original = format!("{prefix}{}", message.name);
            if name != message.name {
                source.push_str(&format!("{prefix}{name} = {original}\n"));
            }
            for enumeration in &message.enums {
                let alias = type_name(&enumeration.name, options);
                if alias != enumeration.name {
                    source.push_str(&format!(
                        "{original}.{alias} = {original}.{}\n",
                        enumeration.name
                    ));
                }
            }
            visit(&message.messages, &format!("{original}."), options, source);
        }
    }
    let mut source = String::new();
    for enumeration in &file.enums {
        let alias = type_name(&enumeration.name, options);
        if alias != enumeration.name {
            source.push_str(&format!("{alias} = {}\n", enumeration.name));
        }
    }
    visit(&file.messages, "", options, &mut source);
    source
}

pub(super) fn method_name(name: &str, options: &RpcGenerationOptions) -> String {
    if options.lower_case_service_methods {
        crate::python_identifier(name)
    } else {
        name.into()
    }
}

pub(super) fn documentation(file: &poolster_core::native::rpc::RpcFileMetadata) -> String {
    let mut code = String::new();
    for definition in &file.definitions {
        if definition.leading_comments.is_empty()
            && definition.trailing_comments.is_empty()
            && definition.detached_comments.is_empty()
            && !definition.deprecated
        {
            continue;
        }
        code.push_str(&format!(
            "# {}{}\n",
            definition.full_name,
            if definition.deprecated {
                " (deprecated)"
            } else {
                ""
            }
        ));
        for text in definition
            .detached_comments
            .iter()
            .chain([&definition.leading_comments, &definition.trailing_comments])
        {
            for line in text.lines() {
                code.push_str("# ");
                code.extend(line.chars().filter(|character| !character.is_control()));
                code.push('\n');
            }
        }
    }
    code
}
