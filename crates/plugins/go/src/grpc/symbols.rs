//! Owned generated symbols for downstream companions; no parser or generator types.
use super::*;
use super::{messages::file_stem, plan::*};
use poolster_core::{blocks::ContractReference, engine::Contract, native::rpc::*};
#[derive(Clone, Debug)]
pub struct RpcMessageSymbol {
    pub name: String,
    pub import_path: String,
    pub file: PathBuf,
    pub exported: bool,
    pub value_model: Option<RpcValueSymbol>,
}
/// Alternate value model and explicit converter methods for companions.
#[derive(Clone, Debug)]
pub struct RpcValueSymbol {
    pub name: String,
    pub file: PathBuf,
    pub from_native: String,
    pub to_native: String,
}
#[derive(Clone, Debug)]
pub struct RpcServiceSymbol {
    pub client: String,
    pub server: String,
    pub import_path: String,
    pub file: PathBuf,
}
#[derive(Clone, Debug)]
pub struct GrpcApi {
    pub messages: BTreeMap<String, RpcMessageSymbol>,
    pub services: BTreeMap<String, RpcServiceSymbol>,
    pub provenance: Option<ContractReference>,
    pub options: RpcGenerationOptions,
}
impl Contract for GrpcApi {
    const NAME: &'static str = "poolster.go.protobuf-artifacts.v1";
}
pub(super) fn collect(
    plan: &Plan,
    schema: &RpcWireSchema,
    generation: &[String],
    options: &RpcGenerationOptions,
    provenance: Option<ContractReference>,
) -> Result<GrpcApi> {
    let mut result = GrpcApi {
        messages: BTreeMap::new(),
        services: BTreeMap::new(),
        provenance,
        options: options.clone(),
    };
    for file in &schema.files {
        if !generation.contains(&file.name) {
            continue;
        }
        let c = plan.context(file)?;
        for message in messages(file).into_iter().filter(|m| !m.map_entry) {
            let name = c.ty(&message.full_name)?;
            result.messages.insert(
                message.full_name.clone(),
                RpcMessageSymbol {
                    file: c.path(&format!("{}_{}.pb.go", file_stem(&file.name), name)),
                    value_model: (!options.representations.is_native()).then(|| RpcValueSymbol {
                        name: format!("{name}Values"),
                        file: c.path(&format!("{}_{}_values.pb.go", file_stem(&file.name), name)),
                        from_native: format!("{name}ValuesFromNative"),
                        to_native: format!("{name}Values.ToNative"),
                    }),
                    name,
                    import_path: c.import.clone(),
                    exported: options.emit_types,
                },
            );
        }
        if options.emit_clients || options.emit_servers {
            for service in &file.services {
                let name = name(service.full_name.rsplit('.').next().unwrap());
                result.services.insert(
                    service.full_name.clone(),
                    RpcServiceSymbol {
                        client: if options.emit_clients {
                            format!("{name}Client")
                        } else {
                            String::new()
                        },
                        server: if options.emit_servers {
                            format!("{name}Server")
                        } else {
                            String::new()
                        },
                        import_path: c.import.clone(),
                        file: c.path(&format!("{}_grpc.pb.go", file_stem(&file.name))),
                    },
                );
            }
        }
    }
    Ok(result)
}
