//! SDK styles delegate to the native service client, never a second transport.
use super::*;
use super::{
    messages::file_stem,
    plan::*,
    services::{client_signature, client_stream},
};
use poolster_core::native::rpc::*;
use std::fmt::Write;

pub(super) fn emit(
    cx: &mut PluginContext<'_, crate::Go>,
    schema: &RpcWireSchema,
    plan: &Plan,
    style: RpcClientStyle,
) -> Result<()> {
    let mut packages: BTreeMap<String, Vec<(&RpcWireFile, &RpcService)>> = BTreeMap::new();
    for file in &schema.files {
        if plan.options.import_mappings.contains_key(&file.name) {
            continue;
        }
        if !plan.packages[&file.name].0.starts_with(&plan.module) {
            continue;
        }
        for service in &file.services {
            packages
                .entry(plan.packages[&file.name].0.clone())
                .or_default()
                .push((file, service));
        }
    }
    for entries in packages.into_values() {
        let c = plan.context(entries[0].0)?;
        let mut names = std::collections::BTreeSet::new();
        let mut counts = BTreeMap::new();
        for (_, service) in &entries {
            let n = name(service.full_name.rsplit('.').next().unwrap());
            ensure!(names.insert(n), "Go grouped service name collision");
            for method in &service.methods {
                *counts.entry(name(&method.name)).or_insert(0usize) += 1;
            }
        }
        for reserved in [
            "FlatClient",
            "GroupedClient",
            "RawClient",
            "Client",
            "NewFlatClient",
            "NewGroupedClient",
            "NewRawClient",
            "NewClient",
        ] {
            ensure!(
                !plan
                    .types
                    .values()
                    .any(|t| t.import == c.import && t.name == reserved),
                "Go SDK facade symbol collision: {reserved}"
            );
        }
        let mut source = String::from("type GroupedClient struct {\n");
        for (_, s) in &entries {
            let n = name(s.full_name.rsplit('.').next().unwrap());
            writeln!(source, "{n} {n}Client")?;
        }
        source.push_str("}\ntype FlatClient struct {\n");
        for (_, s) in &entries {
            let n = name(s.full_name.rsplit('.').next().unwrap());
            writeln!(source, "service{n} {n}Client")?;
        }
        source.push_str("}\n");
        writeln!(
            source,
            "func NewGroupedClient(cc grpc.ClientConnInterface)*GroupedClient{{return &GroupedClient{{"
        )?;
        for (_, s) in &entries {
            let n = name(s.full_name.rsplit('.').next().unwrap());
            writeln!(source, "{n}:New{n}Client(cc),")?;
        }
        source.push_str("}}\n");
        writeln!(
            source,
            "func NewFlatClient(cc grpc.ClientConnInterface)*FlatClient{{return &FlatClient{{"
        )?;
        for (_, s) in &entries {
            let n = name(s.full_name.rsplit('.').next().unwrap());
            writeln!(source, "service{n}:New{n}Client(cc),")?;
        }
        source.push_str("}}\n");
        source.push_str("type RawClient = grpc.ClientConnInterface\nfunc NewRawClient(cc grpc.ClientConnInterface) RawClient{return cc}\n");
        match style {
            RpcClientStyle::Raw=>source.push_str("type Client = RawClient\nfunc NewClient(cc grpc.ClientConnInterface) Client {return NewRawClient(cc)}\n"),
            _=>{let selected=if style==RpcClientStyle::Flat{"FlatClient"}else{"GroupedClient"};writeln!(source,"type Client = {selected}\nfunc NewClient(cc grpc.ClientConnInterface)*Client{{return New{selected}(cc)}}")?;}
        }
        c.emit(
            cx,
            "poolster_client.pb.go",
            source,
            &["grpc \"google.golang.org/grpc\""],
        )?;
        let mut flat_names = std::collections::BTreeSet::new();
        for (file, service) in entries {
            let context = plan.context(file)?;
            let service_name = name(service.full_name.rsplit('.').next().unwrap());
            for method in &service.methods {
                let m = name(&method.name);
                let flat = if counts[&m] > 1 {
                    format!("{service_name}{m}")
                } else {
                    m.clone()
                };
                ensure!(
                    flat_names.insert(flat.clone()),
                    "Go flat operation collision: {flat}"
                );
                let signature = client_signature(method, &context)?;
                let args = if matches!(method.streaming, RpcStreaming::Unary | RpcStreaming::Server)
                {
                    "ctx,in,opts..."
                } else {
                    "ctx,opts..."
                };
                let mut source = format!(
                    "func (client *FlatClient) {flat}{signature} {{return client.service{service_name}.{m}({args})}}\n"
                );
                let signature = signature.replacen(
                    "ctx context.Context,",
                    "ctx context.Context,cc grpc.ClientConnInterface,",
                    1,
                );
                writeln!(
                    source,
                    "func Raw{service_name}{m}{signature} {{return New{service_name}Client(cc).{m}({args})}}"
                )?;
                // Resolve the native generic stream type through the same service path.
                if method.streaming != RpcStreaming::Unary {
                    let _ = client_stream(method, &context)?;
                }
                context.emit(
                    cx,
                    &format!(
                        "{}_{}_{}_facade.pb.go",
                        file_stem(&file.name),
                        service_name,
                        m
                    ),
                    source,
                    &["context \"context\"", "grpc \"google.golang.org/grpc\""],
                )?;
            }
        }
    }
    Ok(())
}
