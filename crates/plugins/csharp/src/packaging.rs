//! NuGet metadata for generated packages that ship their generated README.
use anyhow::Result;
use poolster_core::{GeneratedFile, GeneratedTree};

pub(crate) fn finalize(tree: &mut GeneratedTree) -> Result<()> {
    if tree.get("README.md").is_none() {
        return Ok(());
    }
    let projects = tree
        .iter()
        .filter(|(path, _)| path.extension().is_some_and(|ext| ext == "csproj"))
        .map(|(path, source)| (path.to_owned(), source.to_owned()))
        .collect::<Vec<_>>();
    for (path, source) in projects {
        // Auxiliary executable/test projects are not SDK packages.
        if path
            .parent()
            .is_some_and(|parent| !parent.as_os_str().is_empty())
        {
            continue;
        }
        if source.contains("<PackageReadmeFile>") {
            continue;
        }
        let metadata = "  <PropertyGroup>\n    <PackageReadmeFile>README.md</PackageReadmeFile>\n  </PropertyGroup>\n  <ItemGroup>\n    <None Update=\"README.md\" Pack=\"true\" PackagePath=\"/\" />\n  </ItemGroup>\n";
        tree.replace(GeneratedFile::new(
            path,
            source.replace("</Project>", &format!("{metadata}</Project>")),
        )?)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn generate() -> GeneratedTree {
        poolster_core::engine::Packages::new()
            .package(
                crate::package("dotnet")
                    .name("Relevate.Sdk")
                    .with(crate::sdk()),
            )
            .generate(
                &poolster_core::Api {
                    name: "Relevate".into(),
                    version: "0.1.0".into(),
                    ..Default::default()
                },
                None,
            )
            .unwrap()
    }
    use crate::PackageExt;
    #[test]
    fn nuget_metadata_includes_existing_readme_once() {
        let tree = generate();
        let project = tree.get("dotnet/RelevateSdk.csproj").unwrap();
        assert!(project.contains("<PackageReadmeFile>README.md</PackageReadmeFile>"));
        assert!(project.contains("<None Update=\"README.md\" Pack=\"true\" PackagePath=\"/\" />"));
        assert!(tree.get("dotnet/README.md").is_some());
    }
    #[test]
    #[ignore = "requires .NET 8 and Python 3; set POOLSTER_TEST_DOTNET for a custom executable"]
    fn nuget_pack_embeds_readme_and_nuspec_reference() {
        let root = tempfile::tempdir().unwrap();
        generate().write_to(root.path()).unwrap();
        let output = std::process::Command::new(
            std::env::var_os("POOLSTER_TEST_DOTNET").unwrap_or_else(|| "dotnet".into()),
        )
        .args([
            "pack",
            "-c",
            "Release",
            "--output",
            "artifacts",
            "-p:NuGetAudit=false",
        ])
        .current_dir(root.path().join("dotnet"))
        .output()
        .unwrap();
        let log = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.status.success(), "{log}");
        assert!(!log.contains("missing a readme"), "{log}");
        let check = std::process::Command::new("python3").args(["-c", "import pathlib,zipfile,xml.etree.ElementTree as E; p=next(pathlib.Path('artifacts').glob('*.nupkg')); z=zipfile.ZipFile(p); assert z.read('README.md') == pathlib.Path('README.md').read_bytes(); n=E.fromstring(z.read(next(n for n in z.namelist() if n.endswith('.nuspec')))); assert next(e.text for e in n.iter() if e.tag.endswith('}readme')) == 'README.md'"])
            .current_dir(root.path().join("dotnet")).output().unwrap();
        assert!(
            check.status.success(),
            "{}",
            String::from_utf8_lossy(&check.stderr)
        );
    }
}
