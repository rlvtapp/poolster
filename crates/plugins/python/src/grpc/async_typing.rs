//! Async APIs use grpc.aio call objects and async stream iterators explicitly.
use anyhow::Result;
use poolster_core::native::rpc::RpcService;

pub(super) fn stub(
    short: &str,
    service: &RpcService,
    reference: impl Fn(&str) -> Result<String>,
) -> Result<String> {
    let mut out = format!(
        "\nclass Async{short}Stub:\n    def __init__(self, channel: grpc.aio.Channel) -> None: ...\n"
    );
    for method in &service.methods {
        let (channel, _, _) = super::render::stream(method.streaming);
        let call = match channel {
            "unary_unary" => "UnaryUnaryMultiCallable",
            "stream_unary" => "StreamUnaryMultiCallable",
            "unary_stream" => "UnaryStreamMultiCallable",
            _ => "StreamStreamMultiCallable",
        };
        out.push_str(&format!(
            "    {}: grpc.aio.{call}[{}, {}]\n",
            method.name,
            reference(&method.input_type)?,
            reference(&method.output_type)?
        ));
    }
    Ok(out)
}

pub(super) fn servicer(
    short: &str,
    service: &RpcService,
    reference: impl Fn(&str) -> Result<String>,
) -> Result<String> {
    let mut out = format!("\nclass Async{short}Servicer:\n");
    if service.methods.is_empty() {
        out.push_str("    pass\n");
    }
    for method in &service.methods {
        let (_, input_stream, output_stream) = super::render::stream(method.streaming);
        let input = reference(&method.input_type)?;
        let output = reference(&method.output_type)?;
        let input = if input_stream {
            format!("_poolster_typing.AsyncIterator[{input}]")
        } else {
            input
        };
        let output = if output_stream {
            format!("_poolster_typing.AsyncIterator[{output}]")
        } else {
            output
        };
        // An async generator returns an AsyncIterator directly; unary handlers
        // return a coroutine, matching Python's async def typing semantics.
        let keyword = if output_stream { "def" } else { "async def" };
        out.push_str(&format!("    {keyword} {}(self, request: {input}, context: grpc.aio.ServicerContext[{}, {}]) -> {output}: ...\n", method.name, reference(&method.input_type)?, reference(&method.output_type)?));
    }
    out.push_str(&format!("\ndef add_Async{short}Servicer_to_server(servicer: Async{short}Servicer, server: grpc.aio.Server) -> None: ...\n"));
    Ok(out)
}
