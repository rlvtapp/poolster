//! Request-owned batch queue typing preserves mapped native message identities.
use anyhow::Result;
use poolster_core::native::rpc::{RpcGenerationOptions, RpcWireSchema};
pub(super) fn annotations(
    schema: &RpcWireSchema,
    options: &RpcGenerationOptions,
) -> Result<String> {
    let types = super::typing::Types::configured("", schema, options);
    let mut code = String::from(
        "import asyncio as _asyncio\nimport concurrent.futures as _futures\nimport typing as _typing\nimport grpc\n",
    );
    code.push_str(&types.imports());
    for asynchronous in [false, true] {
        let name = if asynchronous {
            "AsyncBatchScope"
        } else {
            "BatchScope"
        };
        code.push_str(&format!("class {name}:\n    def __init__(self, channel: grpc.{}Channel, **call_options: _typing.Any) -> None: ...\n",if asynchronous{"aio."}else{""}));
        for mapping in &options.batching {
            let method = schema
                .files
                .iter()
                .flat_map(|file| &file.services)
                .flat_map(|service| &service.methods)
                .find(|method| method.full_name == mapping.single_method)
                .expect("validated method");
            if options.batching.len() > 1 {
                code.push_str("    @_typing.overload\n");
            }
            code.push_str(&format!("    def queue(self, method: _typing.Literal[\"{}\"], request: {}) -> {}.Future[{}]: ...\n",method.full_name,types.reference(&method.input_type)?,if asynchronous {"_asyncio"}else{"_futures"},types.reference(&method.output_type)?));
        }
        code.push_str(&format!(
            "    {}def flush(self) -> None: ...\n    def close(self) -> None: ...\n",
            if asynchronous { "async " } else { "" }
        ));
        if asynchronous {
            code.push_str(&format!("    async def __aenter__(self) -> {name}: ...\n    async def __aexit__(self, *args: _typing.Any) -> None: ...\n"));
        } else {
            code.push_str(&format!("    def __enter__(self) -> {name}: ...\n    def __exit__(self, *args: _typing.Any) -> None: ...\n"));
        }
    }
    Ok(code)
}
