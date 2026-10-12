use super::*;
use super::{
    messages::{field_name, file_stem, tag},
    plan::*,
};
use anyhow::Context;
use poolster_core::native::rpc::*;
use std::fmt::Write;

pub(super) fn emit(
    cx: &mut PluginContext<'_, crate::Go>,
    file: &RpcWireFile,
    c: &plan::Context<'_>,
    public_reflection: bool,
) -> Result<()> {
    let es = enums(file);
    let ms = messages(file);
    let xs = extensions(file);
    let p = &c.prefix;
    let mut s = format!(
        "var File_{} protoreflect.FileDescriptor\n",
        p.trim_start_matches("file_")
    );
    if file.descriptor.len() <= 8192 {
        write!(s, "var {p}_rawDesc=[]byte{{")?;
        for byte in &file.descriptor {
            write!(s, "{byte},")?;
        }
        s.push_str("}\n");
    } else {
        writeln!(
            s,
            "var {p}_rawDesc={p}_descriptorBytes()\nfunc {p}_descriptorBytes() []byte {{ data:=make([]byte,0,{})",
            file.descriptor.len()
        )?;
        for (index, chunk) in file.descriptor.chunks(8192).enumerate() {
            let mut part = format!("var {p}_descriptorPart{index}=[]byte{{");
            for byte in chunk {
                write!(part, "{byte},")?;
            }
            part.push_str("}\n");
            c.emit(
                cx,
                &format!("{}_descriptor_{index}.pb.go", file_stem(&file.name)),
                part,
                &[],
            )?;
            writeln!(s, "data=append(data,{p}_descriptorPart{index}...)")?;
        }
        s.push_str("return data\n}\n");
    }
    writeln!(
        s,
        "var {p}_enumTypes=make([]protoimpl.EnumInfo,{})\nvar {p}_msgTypes=make([]protoimpl.MessageInfo,{})",
        es.len(),
        ms.len()
    )?;
    let mut types = BTreeMap::new();
    let mut ordered = Vec::new();
    for e in &es {
        types.insert(e.full_name.clone(), ordered.len());
        ordered.push(format!("({})(0)", c.ty(&e.full_name)?));
    }
    for m in &ms {
        types.insert(m.full_name.clone(), ordered.len());
        ordered.push(if m.map_entry {
            "nil".into()
        } else {
            format!("(*{})(nil)", c.ty(&m.full_name)?)
        });
    }
    let mut indexes = Vec::new();
    let mut starts = Vec::new();
    let mut dependency = |full: &str| -> Result<usize> {
        let full = full.trim_start_matches('.');
        if let Some(i) = types.get(full) {
            return Ok(*i);
        }
        let t = c.plan.types.get(full).context("dependency type missing")?;
        let i = ordered.len();
        ordered.push(if t.message {
            format!("(*{})(nil)", c.ty(full)?)
        } else {
            format!("({})(0)", c.ty(full)?)
        });
        types.insert(full.into(), i);
        Ok(i)
    };
    starts.push(indexes.len());
    for m in &ms {
        for f in &m.fields {
            if let RpcFieldKind::Message(n) | RpcFieldKind::Group(n) | RpcFieldKind::Enum(n) =
                &f.kind
            {
                indexes.push(dependency(n)?);
            }
        }
    }
    starts.push(indexes.len());
    for (x, _) in &xs {
        indexes.push(dependency(
            x.extendee.as_deref().context("extension lacks target")?,
        )?);
    }
    starts.push(indexes.len());
    for (x, _) in &xs {
        if let RpcFieldKind::Message(n) | RpcFieldKind::Group(n) | RpcFieldKind::Enum(n) = &x.kind {
            indexes.push(dependency(n)?);
        }
    }
    starts.push(indexes.len());
    for service in &file.services {
        for method in &service.methods {
            indexes.push(dependency(&method.input_type)?);
        }
    }
    starts.push(indexes.len());
    for service in &file.services {
        for method in &service.methods {
            indexes.push(dependency(&method.output_type)?);
        }
    }
    indexes.extend(starts.into_iter().rev());
    writeln!(
        s,
        "var {p}_goTypes=[]any{{ {} }}\nvar {p}_depIdxs=[]int32{{ {} }}",
        ordered.join(","),
        indexes
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )?;
    writeln!(s, "var {p}_extTypes=[]protoimpl.ExtensionInfo{{")?;
    for (x, scope) in &xs {
        let scalar = c.scalar(&x.kind)?;
        let ty = if x.label == RpcFieldLabel::Repeated {
            format!("[]{scalar}")
        } else if scalar.starts_with('*') || scalar == "[]byte" {
            scalar
        } else {
            format!("*{scalar}")
        };
        writeln!(
            s,
            "{{ ExtendedType:(*{})(nil),ExtensionType:({ty})(nil),Field:{},Name:{:?},Tag:{:?},Filename:{:?} }},",
            c.ty(x.extendee.as_deref().unwrap())?,
            x.number,
            format!("{scope}.{}", x.name).trim_start_matches('.'),
            tag(x, c, false),
            file.name
        )?;
    }
    s.push_str("}\n");
    for (i, (x, scope)) in xs.iter().enumerate() {
        let scope = scope
            .strip_prefix(&format!("{}.", file.package))
            .unwrap_or(if scope == &file.package { "" } else { scope });
        let n = if scope.is_empty() {
            name(&x.name)
        } else {
            format!(
                "{}_{}",
                scope.split('.').map(name).collect::<Vec<_>>().join("_"),
                name(&x.name)
            )
        };
        writeln!(s, "var E_{n}=&{p}_extTypes[{i}]")?;
    }
    writeln!(
        s,
        "func init() {{ {p}_init() }}\nfunc {p}_init() {{\nif File_{}!=nil{{return}}",
        p.trim_start_matches("file_")
    )?;
    for dep in &file.dependencies {
        if c.plan.packages[dep].0 == c.import {
            let prefix = format!(
                "file_{}",
                dep.chars()
                    .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
                    .collect::<String>()
            );
            writeln!(s, "{prefix}_init()")?;
        }
    }
    for (index, m) in ms.iter().enumerate() {
        let wrappers = m
            .fields
            .iter()
            .filter(|f| f.oneof_index.is_some() && !f.proto3_optional)
            .map(|f| Ok(format!("(*{}_{})(nil)", c.ty(&m.full_name)?, field_name(f))))
            .collect::<Result<Vec<_>>>()?;
        if !wrappers.is_empty() {
            writeln!(
                s,
                "{p}_msgTypes[{index}].OneofWrappers=[]any{{ {} }}",
                wrappers.join(",")
            )?;
        }
    }
    writeln!(
        s,
        "type marker struct{{}}\nout:=protoimpl.TypeBuilder{{\nFile:protoimpl.DescBuilder{{ GoPackagePath:reflect.TypeOf(marker{{}}).PkgPath(),RawDescriptor:{p}_rawDesc,NumEnums:{},NumMessages:{},NumExtensions:{},NumServices:{} }},\nGoTypes:{p}_goTypes,DependencyIndexes:{p}_depIdxs,EnumInfos:{p}_enumTypes,MessageInfos:{p}_msgTypes,ExtensionInfos:{p}_extTypes,\n}}.Build()\nFile_{}=out.File\n}}",
        es.len(),
        ms.len(),
        xs.len(),
        file.services.len(),
        p.trim_start_matches("file_")
    )?;
    if !public_reflection {
        s = s.replace("File_", "poolsterFile_");
    }
    c.emit(
        cx,
        &format!("{}.pb.go", file_stem(&file.name)),
        s,
        &[
            "reflect \"reflect\"",
            "protoreflect \"google.golang.org/protobuf/reflect/protoreflect\"",
            "protoimpl \"google.golang.org/protobuf/runtime/protoimpl\"",
        ],
    )
}
