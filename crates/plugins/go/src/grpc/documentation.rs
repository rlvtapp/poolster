//! Documentation is derived from owned descriptor metadata, never parser types.
use anyhow::Result;
use poolster_core::native::rpc::*;
pub(super) fn comment(
    context: &super::plan::Context<'_>,
    kind: RpcDefinitionKind,
    full_name: &str,
) -> Result<String> {
    if !context.plan.options.comments {
        return Ok(String::new());
    }
    let file = context.file;
    let metadata = RpcWireSchema {
        files: vec![file.clone()],
    }
    .metadata()?;
    let Some(definition) = metadata[0].definition(kind, full_name) else {
        return Ok(String::new());
    };
    let mut output = String::new();
    for text in definition
        .detached_comments
        .iter()
        .chain([&definition.leading_comments, &definition.trailing_comments])
    {
        for line in text.lines() {
            output.push_str("// ");
            output.push_str(&line.replace(['\r', '\0'], " "));
            output.push('\n');
        }
    }
    if definition.deprecated {
        output.push_str("// Deprecated: marked deprecated in the Protobuf schema.\n");
    }
    Ok(output)
}
pub(super) fn bytes_default(input: &str) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut chars = input.bytes().peekable();
    while let Some(byte) = chars.next() {
        if byte != b'\\' {
            output.push(byte);
            continue;
        }
        let escaped = chars
            .next()
            .ok_or_else(|| anyhow::anyhow!("unterminated bytes default escape"))?;
        match escaped {
            b'a' => output.push(7),
            b'b' => output.push(8),
            b'f' => output.push(12),
            b'n' => output.push(10),
            b'r' => output.push(13),
            b't' => output.push(9),
            b'v' => output.push(11),
            b'\\' | b'\'' | b'"' => output.push(escaped),
            b'0'..=b'7' => {
                let mut value = u16::from(escaped - b'0');
                for _ in 0..2 {
                    if let Some(next @ b'0'..=b'7') = chars.peek().copied() {
                        chars.next();
                        value = value * 8 + u16::from(next - b'0');
                    } else {
                        break;
                    }
                }
                anyhow::ensure!(value <= 255, "bytes default octal escape out of range");
                output.push(value as u8);
            }
            b'x' => {
                let mut hex = String::new();
                for _ in 0..2 {
                    if let Some(next) = chars.peek().copied() {
                        if next.is_ascii_hexdigit() {
                            chars.next();
                            hex.push(next as char);
                        } else {
                            break;
                        }
                    }
                }
                anyhow::ensure!(!hex.is_empty(), "invalid bytes default hex escape");
                output.push(u8::from_str_radix(&hex, 16)?);
            }
            _ => anyhow::bail!("unsupported bytes default escape"),
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    #[test]
    fn descriptor_bytes_defaults_decode_wire_escapes() {
        assert_eq!(
            super::bytes_default(r"\000\377\x80\n\\").unwrap(),
            vec![0, 255, 128, 10, 92]
        );
        assert!(super::bytes_default(r"\400").is_err());
        assert!(super::bytes_default(r"\x").is_err());
    }
}
