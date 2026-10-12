//! Native grpc-js overloads. Each declaration calls the same runtime method.
use poolster_core::native::rpc::RpcStreaming;

pub(super) fn signatures(
    name: &str,
    input: &str,
    output: &str,
    callback: &str,
    streaming: RpcStreaming,
) -> String {
    let name = serde_json::to_string(name).unwrap();
    let (arguments, result) = match streaming {
        RpcStreaming::Unary => (
            vec![
                format!("request:{input},{callback}"),
                format!("request:{input},options:CallOptions,{callback}"),
                format!("request:{input},metadata:Metadata,{callback}"),
                format!("request:{input},metadata:Metadata,options:CallOptions,{callback}"),
            ],
            "ClientUnaryCall".to_owned(),
        ),
        RpcStreaming::Client => (
            vec![
                callback.to_owned(),
                format!("options:CallOptions,{callback}"),
                format!("metadata:Metadata,{callback}"),
                format!("metadata:Metadata,options:CallOptions,{callback}"),
            ],
            format!("ClientWritableStream<{input}>"),
        ),
        RpcStreaming::Server => (
            vec![
                format!("request:{input},options?:CallOptions"),
                format!("request:{input},metadata:Metadata,options?:CallOptions"),
            ],
            format!("ClientReadableStream<{output}>"),
        ),
        RpcStreaming::Bidirectional => (
            vec![
                "options?:CallOptions".into(),
                "metadata:Metadata,options?:CallOptions".into(),
            ],
            format!("ClientDuplexStream<{input},{output}>"),
        ),
    };
    arguments
        .into_iter()
        .map(|args| format!("{name}({args}):{result};\n"))
        .collect()
}
