use super::*;
use super::{messages::file_stem, plan::*};
use poolster_core::native::rpc::*;
use std::fmt::Write;
pub(super) fn emit(
    cx: &mut PluginContext<'_, crate::Go>,
    file: &RpcWireFile,
    c: &plan::Context<'_>,
    options: &RpcGenerationOptions,
) -> Result<()> {
    if !options.emit_binary && !options.emit_json {
        return Ok(());
    }
    let mut helper = false;
    for message in messages(file).into_iter().filter(|m| !m.map_entry) {
        let t = c.ty(&message.full_name)?;
        let mut source = String::new();
        let discard = options.unknown_fields == RpcUnknownFields::Discard;
        let encode = options.encode_include_types.is_empty()
            || options.encode_include_types.contains(&message.full_name);
        let decode = options.decode_include_types.is_empty()
            || options.decode_include_types.contains(&message.full_name);
        if options.emit_binary && encode {
            writeln!(
                source,
                "func(x *{t})MarshalBinary()([]byte,error){{\nif err:=poolster{}CheckDepth(x,{},map[proto.Message]bool{{}});err!=nil{{return nil,err}}\nif proto.Size(x)>{}{{return nil,fmt.Errorf(\"Protobuf message exceeds byte limit\")}}\nreturn proto.MarshalOptions{{Deterministic:true}}.Marshal(x)\n}}",
                c.prefix, options.max_depth, options.max_message_bytes
            )?;
        }
        if options.emit_binary && decode {
            writeln!(
                source,
                "func(x *{t})UnmarshalBinary(data []byte)error{{\nif len(data)>{}{{return fmt.Errorf(\"Protobuf message exceeds byte limit\")}}\nreturn(proto.UnmarshalOptions{{DiscardUnknown:{discard},RecursionLimit:{}}}).Unmarshal(data,x)\n}}",
                options.max_message_bytes, options.max_depth
            )?;
        }
        if options.emit_json && encode {
            writeln!(
                source,
                "func(x *{t})MarshalJSON()([]byte,error){{return x.MarshalJSONWithOptions(protojson.MarshalOptions{{}})}}\nfunc(x *{t})MarshalJSONWithOptions(options protojson.MarshalOptions)([]byte,error){{\nif err:=poolster{}CheckDepth(x,{},map[proto.Message]bool{{}});err!=nil{{return nil,err}}\ndata,err:=options.Marshal(x)\nif len(data)>{}{{return nil,fmt.Errorf(\"ProtoJSON message exceeds byte limit\")}}\nreturn data,err\n}}",
                c.prefix, options.max_depth, options.max_message_bytes
            )?;
        }
        if options.emit_json && decode {
            writeln!(
                source,
                "func(x *{t})UnmarshalJSON(data []byte)error{{return x.UnmarshalJSONWithOptions(data,protojson.UnmarshalOptions{{}})}}\nfunc(x *{t})UnmarshalJSONWithOptions(data []byte,options protojson.UnmarshalOptions)error{{\nif len(data)>{}{{return fmt.Errorf(\"ProtoJSON message exceeds byte limit\")}}\noptions.RecursionLimit={};return options.Unmarshal(data,x)\n}}",
                options.max_message_bytes, options.max_depth
            )?;
        }
        if source.is_empty() {
            continue;
        }
        helper |= encode;
        c.emit(
            cx,
            &format!("{}_{t}_codecs.pb.go", file_stem(&file.name)),
            source,
            &[
                "fmt \"fmt\"",
                "proto \"google.golang.org/protobuf/proto\"",
                "protojson \"google.golang.org/protobuf/encoding/protojson\"",
            ],
        )?;
    }
    if helper {
        c.emit(
            cx,
            &format!("{}_codecs.pb.go", file_stem(&file.name)),
            include_str!("depth.go.tmpl").replace("{{PREFIX}}", &format!("poolster{}", c.prefix)),
            &[
                "fmt \"fmt\"",
                "proto \"google.golang.org/protobuf/proto\"",
                "protoreflect \"google.golang.org/protobuf/reflect/protoreflect\"",
            ],
        )?;
    }
    Ok(())
}
