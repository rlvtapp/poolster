//! Native Python package assembly, pins, retained documents and version metadata.
use super::{GRPC_VERSION, PROTOBUF_VERSION, SETUPTOOLS_VERSION, render::literal};
use anyhow::Result;
use poolster_core::native::rpc::{RpcContract, RpcGenerationOptions, RpcWireSchema};
use std::collections::BTreeSet;
pub(super) struct Context<'a> {
    pub contract: &'a RpcContract,
    pub schema: &'a RpcWireSchema,
    pub files: &'a [String],
    pub name: &'a str,
    pub version: &'a str,
    pub options: &'a RpcGenerationOptions,
    pub runtime: &'a str,
    pub id: &'a str,
}
impl Context<'_> {
    pub(super) fn finish(self, output: &mut Vec<(String, String)>) -> Result<()> {
        let Self {
            contract,
            schema,
            files,
            name,
            version,
            options,
            runtime,
            id,
        } = self;
        let only_types = options.emit_types
            && !options.emit_binary
            && !options.emit_json
            && !options.emit_clients
            && !options.emit_servers
            && !options.emit_reflection;
        if only_types {
            output.retain(|(path, _)| !path.starts_with("src/") || !path.ends_with(".py"));
        }
        let mut modules: BTreeSet<_> = output
            .iter()
            .filter_map(|(path, _)| {
                path.strip_prefix("src/")
                    .filter(|p| !p.contains('/') && (p.ends_with(".py") || p.ends_with(".pyi")))
                    .map(|p| {
                        p.trim_end_matches(".pyi")
                            .trim_end_matches(".py")
                            .to_owned()
                    })
            })
            .collect();
        let helpers = super::codecs::render(runtime, options);
        if let Some((source, annotations)) = helpers {
            let name = format!("_poolster_codecs_{id}");
            modules.insert(name.clone());
            output.push((format!("src/{name}.py"), source));
            if options.emit_types {
                output.push((format!("src/{name}.pyi"), annotations));
            }
        }
        let modules = modules
            .iter()
            .map(|m| literal(m))
            .collect::<Vec<_>>()
            .join(", ");
        let dependencies = if (options.emit_clients || options.emit_servers)
            && schema.files.iter().any(|f| !f.services.is_empty())
        {
            format!("\"grpcio=={GRPC_VERSION}\", \"protobuf=={PROTOBUF_VERSION}\"")
        } else {
            format!("\"protobuf=={PROTOBUF_VERSION}\"")
        };
        let mut manifest = format!(
            "[build-system]\nrequires = [\"setuptools=={SETUPTOOLS_VERSION}\"]\nbuild-backend = \"setuptools.build_meta\"\n\n[project]\nname = {}\nversion = {}\nrequires-python = \">=3.10\"\ndependencies = [{dependencies}]\n\n[tool.setuptools]\npackage-dir = {{\"\" = \"src\"}}\npy-modules = [{modules}]\ninclude-package-data = true\n\n[tool.setuptools.packages.find]\nwhere = [\"src\"]\nnamespaces = true\n\n[tool.setuptools.package-data]\n\"*\" = [\"*.pyi\", \"py.typed\"]\n",
            literal(name),
            literal(version)
        );
        if options.emit_types {
            manifest.push_str("\n[project.optional-dependencies]\ntyping = [\"types-protobuf==6.32.1.20251210\", \"types-grpcio==1.0.0.20260614\"]\n");
        }
        output.push(("pyproject.toml".into(), manifest));
        output.push(("setup.py".into(), include_str!("setup.py.tmpl").into()));
        output.push((
            "MANIFEST.in".into(),
            "recursive-include src *.py *.pyi py.typed\n".into(),
        ));
        for file in files {
            if let Some((root, _)) = file.split_once('/') {
                let marker = format!("src/{root}/py.typed");
                if !output.iter().any(|(path, _)| path == &marker) {
                    output.push((marker, String::new()));
                }
            }
        }
        for file in &contract.files {
            if let Some(source) = &file.source {
                output.push((format!("proto/{}", file.name), source.clone()));
            }
        }
        output.push((
            "proto/descriptor-set.json".into(),
            serde_json::to_string(&contract.descriptor_set)?,
        ));
        let mut readme = include_str!("README.md.tmpl").to_owned();
        if only_types {
            readme.push_str("\nThis is a declaration-only package: src contains .pyi bindings and no executable message or transport modules. Install this package for static analysis alongside separately supplied compatible native Protobuf bindings at runtime. Descriptor names, module mappings and protobuf dependencies remain authoritative.\n");
        }
        output.push(("README.md".into(), readme));
        for (path, contents) in output {
            if !path.ends_with(".py") && !path.ends_with(".pyi") {
                continue;
            }
            if options.annotate_files_with_version {
                *contents = format!(
                    "# Poolster {}; protobuf={PROTOBUF_VERSION}; grpcio={GRPC_VERSION}\n{contents}",
                    env!("CARGO_PKG_VERSION")
                );
            } else {
                *contents = contents
                    .lines()
                    .filter(|line| !line.starts_with("# Generated by Poolster."))
                    .collect::<Vec<_>>()
                    .join("\n")
                    + "\n";
            }
        }
        Ok(())
    }
}
