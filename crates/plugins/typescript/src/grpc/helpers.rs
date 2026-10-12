use poolster_core::native::rpc::*;
pub(super) fn runtime(options: &RpcGenerationOptions) -> String {
    let retain = options.unknown_fields == RpcUnknownFields::Preserve;
    format!(
        "import {{descriptorBytes}} from './poolster-rpc-descriptors.js';\nimport {{createFileRegistry,isFieldSet,getExtension,hasExtension,fromBinary,fromJson,fromJsonString,toBinary,toJson,toJsonString,type DescMessage,type MessageShape,type JsonValue,type JsonReadOptions,type JsonWriteOptions}} from '@bufbuild/protobuf';\nimport {{FileDescriptorSetSchema}} from '@bufbuild/protobuf/wkt';\nexport const registry=createFileRegistry(fromBinary(FileDescriptorSetSchema,descriptorBytes));\nconst maxBytes={};const maxDepth={};\n",
        options.max_message_bytes, options.max_depth
    ) + &include_str!("runtime.ts.tmpl")
        .replace("{{RETAIN}}", if retain { "true" } else { "false" })
}
pub(super) fn message(name: &str, full: &str, options: &RpcGenerationOptions) -> String {
    let mut out = String::new();
    if options.emit_binary {
        let mut methods = vec![];
        if options.encode_include_types.is_empty()
            || options.encode_include_types.iter().any(|n| n == full)
        {
            methods.push(format!(
                "encode:(message:{name})=>encode({name}Schema,message)"
            ));
        }
        if options.decode_include_types.is_empty()
            || options.decode_include_types.iter().any(|n| n == full)
        {
            methods.push(format!(
                "decode:(bytes:Uint8Array)=>decode({name}Schema,bytes)"
            ));
        }
        if !methods.is_empty() {
            out.push_str(&format!(
                "export const {name}Binary={{{}}};\n",
                methods.join(",")
            ));
        }
    }
    if options.emit_json {
        out.push_str(&format!("export const {name}Json={{parse:(input:string|JsonValue,options?:Partial<JsonReadOptions>)=>parseJson({name}Schema,input,options),stringify:(message:{name},options?:Partial<JsonWriteOptions>)=>stringifyJson({name}Schema,message,options)}};\n"));
    }
    if options.emit_binary || options.emit_json || options.emit_reflection {
        out.push_str(&format!("export const {name}Create=(value?:MessageInitShape<typeof {name}Schema>)=>create({name}Schema,value);\n"));
    }
    out
}

pub(super) fn has_binary(full: &str, options: &RpcGenerationOptions) -> bool {
    options.encode_include_types.is_empty()
        || options.decode_include_types.is_empty()
        || options
            .encode_include_types
            .iter()
            .chain(&options.decode_include_types)
            .any(|n| n == full)
}
