//! Additive HTTP transport annotations retain native message types.
use anyhow::Result;
use poolster_core::native::rpc::RpcWireSchema;
pub(super) fn annotations(
    schema: &RpcWireSchema,
    options: &poolster_core::native::rpc::RpcGenerationOptions,
) -> Result<String> {
    let types = super::typing::Types::configured("", schema, options);
    let mut code = String::from("import typing as _typing\nimport urllib.request as _request\n");
    code.push_str(&types.imports());
    code.push_str(
        "class HttpRpcError(Exception):\n    code: str\n    metadata: dict[str, str]\n\n",
    );
    for web in [false, true] {
        let name = if web { "GrpcWebClient" } else { "TwirpClient" };
        code.push_str(&format!("class {name}:\n    def __init__(self, base_url: str, *, headers: _typing.Mapping[str, str] | None = ..., opener: _typing.Callable[..., _typing.Any] | None = ...) -> None: ...\n"));
        let methods: Vec<_> = schema
            .files
            .iter()
            .flat_map(|file| &file.services)
            .flat_map(|service| service.methods.iter().map(move |method| (service, method)))
            .filter(|(_, method)| {
                let (_, input, output) = super::render::stream(method.streaming);
                !input && (web || !output)
            })
            .collect();
        for (service, method) in &methods {
            if methods.len() > 1 {
                code.push_str("    @_typing.overload\n");
            }
            let (_, _, stream) = super::render::stream(method.streaming);
            let output = types.reference(&method.output_type)?;
            code.push_str(&format!("    def call(self, method: _typing.Literal[\"{}/{}\"], request: {}, *, timeout: float | None = ..., headers: _typing.Mapping[str, str] | None = ...) -> {}: ...\n", service.full_name, method.name, types.reference(&method.input_type)?, if stream {format!("_typing.Iterator[{output}]")} else {output}));
        }
    }
    Ok(code)
}
