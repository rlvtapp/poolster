use super::*;
use poolster_core::native::rpc::*;
pub(super) fn emit(
    cx: &mut PluginContext<'_, crate::Go>,
    c: &plan::Context<'_>,
    options: &RpcGenerationOptions,
) -> Result<()> {
    for symbol in [
        "ValueAdapter",
        "Int64String",
        "ParseInt64String",
        "Uint64String",
        "ParseUint64String",
        "Int64Float64",
        "Float64Int64",
        "Uint64Float64",
        "Float64Uint64",
        "BytesBase64",
        "ParseBytesBase64",
        "TimestampTime",
        "TimeTimestamp",
        "DurationTime",
        "TimeDuration",
    ] {
        ensure!(
            !c.plan
                .types
                .values()
                .any(|t| t.import == c.import && t.name == symbol),
            "Go value adapter symbol collision: {symbol}"
        );
    }
    let mut source = include_str!("values.go.tmpl")
        .replace("MAX_BYTES", &options.max_message_bytes.to_string())
        .replace("MAX_DEPTH", &options.max_depth.to_string())
        .replace(
            "DISCARD",
            if options.unknown_fields == RpcUnknownFields::Discard {
                "true"
            } else {
                "false"
            },
        );
    if !options.value_adapters {
        if let Some(position) = source.find("// ValueAdapter") {
            source.truncate(position);
        }
    }
    source.push_str(&include_str!("depth.go.tmpl").replace("{{PREFIX}}", "poolsterValue"));
    c.emit(
        cx,
        "poolster_values.pb.go",
        source,
        &[
            "fmt \"fmt\"",
            "strconv \"strconv\"",
            "base64 \"encoding/base64\"",
            "time \"time\"",
            "proto \"google.golang.org/protobuf/proto\"",
            "protoreflect \"google.golang.org/protobuf/reflect/protoreflect\"",
            "timestamppb \"google.golang.org/protobuf/types/known/timestamppb\"",
            "durationpb \"google.golang.org/protobuf/types/known/durationpb\"",
        ],
    )
}
