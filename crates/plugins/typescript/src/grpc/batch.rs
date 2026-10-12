use anyhow::Result;
use poolster_core::{native::rpc::*, GeneratedFile};
fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}
fn local(s: &str) -> String {
    format!("{}{}", s[..1].to_ascii_lowercase(), &s[1..])
}
pub(super) fn generate(
    wire: &RpcWireSchema,
    options: &RpcGenerationOptions,
) -> Result<Vec<GeneratedFile>> {
    if options.batching.is_empty() {
        return Ok(vec![]);
    }
    anyhow::ensure!(
        options.emit_clients,
        "Protobuf batching requires emitted clients"
    );
    let mut s = String::from(
        "import {create} from '@bufbuild/protobuf';\nimport {registry} from './poolster-rpc-runtime.js';\nimport {unary,type RpcContext} from './poolster-rpc-facade.js';\nimport type {RawClient} from './client.js';\nimport {batchLoader} from './poolster-rpc-batch.js';\nexport function createBatchScope(raw:RawClient,context:RpcContext={}) {\n",
    );
    let mut exported = vec![];
    for (index, mapping) in options.batching.iter().enumerate() {
        let methods = wire
            .files
            .iter()
            .flat_map(|f| &f.services)
            .flat_map(|s| s.methods.iter().map(move |m| (s, m)))
            .collect::<Vec<_>>();
        let (single_service, single) = methods
            .iter()
            .find(|(_, m)| m.full_name == mapping.single_method)
            .unwrap();
        let (service, batch) = methods
            .iter()
            .find(|(_, m)| m.full_name == mapping.batch_method)
            .unwrap();
        let group = local(service.full_name.rsplit('.').next().unwrap());
        let method = super::layout::method(&batch.name, options);
        let input = format!(
            "Parameters<RawClient[{}][{}]>[0]",
            q(&local(single_service.full_name.rsplit('.').next().unwrap())),
            q(&super::layout::method(&single.name, options))
        );
        s.push_str(&format!("const loader{index}=batchLoader((input:{input})=>input[{}],keys=>{{const request=create(registry.getMessage({})!,{{[{}]:keys}});const call=unary(raw[{}][{}].bind(raw[{}]),request as Parameters<RawClient[{}][{}]>[0],context);return Object.assign(call.then(result=>result[{}]),{{call:call.call,headers:call.headers,status:call.status,cancel:()=>call.cancel()}});}},{},{},{});\n",q(&super::render::property(&mapping.input_field)),q(&batch.input_type),q(&super::render::property(&mapping.batch_input_field)),q(&group),q(&method),q(&group),q(&group),q(&method),q(&super::render::property(&mapping.batch_output_field)),mapping.max_batch_size,mapping.cache,mapping.output_key.as_ref().map(|k|format!("output=>output[{}]",q(&super::render::property(k)))).unwrap_or("undefined".into())));
        exported.push(format!(
            "{}:(input:{input},signal?:AbortSignal)=>loader{index}.load(input,signal)",
            q(&mapping.single_method)
        ));
    }
    s.push_str(&format!(
        "return{{{},close(){{{}}}}};}}\n",
        exported.join(","),
        (0..options.batching.len())
            .map(|i| format!("loader{i}.close();"))
            .collect::<String>()
    ));
    Ok(vec![
        GeneratedFile::new("src/batch-client.ts", s)?,
        GeneratedFile::new("src/poolster-rpc-batch.ts", include_str!("batch.ts.tmpl"))?,
    ])
}
