use super::*;
use std::collections::BTreeSet;
pub(super) fn emit(
    cx: &mut PluginContext<'_, Java>,
    input: &EventOperations,
    symbols: &BTreeMap<String, ModelSymbol>,
    package: &str,
) -> Result<BTreeMap<String, KafkaOperationSymbols>> {
    let root = format!("src/main/java/{}/events", package.replace('.', "/"));
    let mut used = BTreeSet::new();
    let mut operations = BTreeMap::new();
    for op in &input.operations {
        let class = format!("{}Event", crate::naming::type_name(&op.name));
        ensure!(
            class.len() <= 160 && used.insert(class.to_ascii_lowercase()),
            "Java event operation names collide or exceed filename budget: {}",
            op.name
        );
        let payload = symbols[&format!("{}/payload", op.message.location)].clone();
        let headers = op
            .message
            .headers
            .as_ref()
            .map(|_| symbols[&format!("{}/headers", op.message.location)].clone());
        let key = op
            .message
            .key
            .as_ref()
            .map(|_| symbols[&format!("{}/key", op.message.location)].clone());
        let ty =
            |symbol: Option<&ModelSymbol>| symbol.map(|v| v.name.clone()).unwrap_or("Void".into());
        for symbol in [Some(&payload), headers.as_ref(), key.as_ref()]
            .into_iter()
            .flatten()
        {
            ensure!(
                symbol.module.parent().is_some_and(|v| v
                    == PathBuf::from(format!("src/main/java/{}/model", package.replace('.', "/")))),
                "Java event model module must belong to the selected package"
            );
        }
        let h = ty(headers.as_ref());
        let k = ty(key.as_ref());
        let p = &payload.name;
        let json = |value: &serde_json::Value| -> Result<String> {
            let text = serde_json::to_string(value)?;
            // A JVM CONSTANT_Utf8 entry is at most 65535 modified UTF-8 bytes.
            let bytes = text
                .encode_utf16()
                .map(|unit| match unit {
                    0 => 2,
                    1..=0x7f => 1,
                    0x80..=0x7ff => 2,
                    _ => 3,
                })
                .sum::<usize>();
            ensure!(
                bytes <= 60_000,
                "Java event schema exceeds the JVM string-constant budget; reduce schema annotations or split the message"
            );
            Ok(serde_json::to_string(&text)?)
        };
        let client = op
            .client_id
            .as_ref()
            .or(input.broker.client_id.as_ref())
            .map(serde_json::to_string)
            .transpose()?
            .unwrap_or("null".into());
        let group = op
            .group_id
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?
            .unwrap_or("null".into());
        let topic = serde_json::to_string(&op.topic)?;
        let payload_schema = json(&op.message.payload_schema)?;
        let headers_schema = op
            .message
            .headers_schema
            .as_ref()
            .map(json)
            .transpose()?
            .unwrap_or("null".into());
        let key_schema = op
            .message
            .key_schema
            .as_ref()
            .map(json)
            .transpose()?
            .unwrap_or("null".into());
        let method = match op.action {
            EventAction::Send => format!(
                r#"  public static CompletionStage<RecordMetadata> send(
      KafkaRuntime runtime, {p} payload, {h} headers, {k} key) {{
    return runtime.send(DEFINITION, payload, headers, key);
  }}"#
            ),
            EventAction::Receive => format!(
                r#"  public static KafkaSubscription<{p}, {h}, {k}> subscribe(
      KafkaRuntime runtime, String group) {{
    return runtime.subscribe(DEFINITION, group);
  }}

  public static KafkaSubscription<{p}, {h}, {k}> subscribe(KafkaRuntime runtime) {{
    return subscribe(runtime, null);
  }}"#
            ),
        };
        let source = format!(
            r#"package {package}.events;

import {package}.model.*;
import java.util.concurrent.CompletionStage;
import org.apache.kafka.clients.producer.RecordMetadata;

/** Typed operation; model declarations are shared by canonical message identity. */
public final class {class} {{
  private {class}() {{}}

  public static final KafkaCodec.Definition<{p}, {h}, {k}> DEFINITION =
      new KafkaCodec.Definition<>(
          {topic},
          {client},
          {group},
          {p}.class,
          {h}.class,
          {k}.class,
          {payload_schema},
          {headers_schema},
          {key_schema});

{method}
}}
"#
        );
        let module = PathBuf::from(format!("{root}/{class}.java"));
        ensure!(
            source.len() <= 128 * 1024,
            "Java event operation exceeds source budget"
        );
        cx.files.emit(GeneratedFile::new(&module, source)?)?;
        operations.insert(
            op.name.clone(),
            KafkaOperationSymbols {
                method: class,
                module,
                payload,
                headers,
                key,
                action: op.action,
            },
        );
    }
    let defaults = serde_json::to_string(&input.broker.brokers.join(","))?;
    let config = format!(
        r#"package {package}.events;

public final class KafkaDefaults {{
  private KafkaDefaults() {{}}

  public static KafkaRuntime connect() {{
    return new KafkaRuntime({defaults});
  }}
}}
"#
    );
    cx.files.emit(GeneratedFile::new(
        format!("{root}/KafkaDefaults.java"),
        config,
    )?)?;
    Ok(operations)
}
