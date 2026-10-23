"""Install released Poolster packages in clean consumers; exercise bundled binaries."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys

PROJECT = Path(__file__).resolve().parent.parent
OVERRIDES = ('POOLSTER_BINARY', 'POOLSTER_OPENAPI_BIN', 'POOLSTER_NODE_BINARY', 'PYTHONPATH')


def isolated_environment(root):
    environment = {key: value for key, value in os.environ.items() if key not in OVERRIDES}
    environment['PATH'] = str(root / 'no-toolchains')
    return environment


def run(command, root, name, environment=None, cwd=None):
    result = subprocess.run(command, cwd=cwd or root, env=environment,
                            capture_output=True, text=True, timeout=1200)
    (root / (name + '.log')).write_text(result.stdout + result.stderr)
    if result.returncode:
        raise RuntimeError(f'{name} exited {result.returncode}: {result.stdout[-2000:]}{result.stderr[-2000:]}')
    return {'command': command, 'exit_code': result.returncode}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--version', default=(PROJECT / 'version.txt').read_text().strip())
    args = parser.parse_args()
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    (root / 'no-toolchains').mkdir()
    report = {'platform': platform.platform(), 'machine': platform.machine(),
              'version': args.version, 'status': 'failed', 'checks': {}}
    try:
        node = shutil.which('node')
        npm = shutil.which('npm.cmd' if os.name == 'nt' else 'npm')
        if not node or not npm:
            raise RuntimeError('Node and npm are required for installation')
        npm_args = ['install', '--prefix', str(root), '--ignore-scripts', '--no-audit', '--no-fund',
                    f'poolster@{args.version}', f'@relevate/poolster@{args.version}',
                    f'@relevate/poolster-plugin-typescript@{args.version}']
        # npm.cmd requires cmd.exe on Windows; it is used only during installation.
        command = ([os.environ['COMSPEC'], '/d', '/c', npm] if os.name == 'nt' else [npm]) + npm_args
        report['checks']['npm_install'] = run(command, root, 'npm-install')
        environment = isolated_environment(root)
        fixture = str(PROJECT / 'crates/cli/tests/fixtures/pets.yaml')
        cli = root / 'node_modules/poolster/bin/poolster.cjs'
        report['checks']['npm_cli'] = run([node, str(cli), 'generate', fixture, '--language',
                                          'go,typescript,python', '--output', str(root / 'cli-output')],
                                         root, 'npm-cli', environment)
        probe = root / 'sdk-probe.cjs'
        probe.write_text("""const assert = require('node:assert/strict');
const {generate} = require('@relevate/poolster');
const {pluginTypeScript} = require('@relevate/poolster-plugin-typescript');
(async()=>{
 const config={input:process.argv[2],output:process.argv[3],name:'Install probe',version:'1.0.0',plugins:[pluginTypeScript()]};
 const first=await generate(config); assert.ok(first.files.some(f=>f.path.endsWith('.ts')));
 assert.deepEqual((await generate(config)).changes,{added:[],modified:[],removed:[]});
})().catch(e=>{console.error(e);process.exitCode=1});
""")
        report['checks']['npm_sdk'] = run([node, str(probe), fixture, str(root / 'sdk-output')],
                                         root, 'npm-sdk', environment)
        python_version = args.version.replace('-alpha.', 'a')
        target = root / 'python-package'
        report['checks']['pypi_install'] = run([sys.executable, '-m', 'pip', 'install',
            '--disable-pip-version-check', '--only-binary=:all:', '--no-deps', '--target', str(target),
            f'poolster=={python_version}'], root, 'pypi-install')
        environment['PYTHONPATH'] = str(target)
        report['checks']['pypi_cli'] = run([sys.executable, '-c',
            'from poolster import main; main()', 'generate', fixture, '--language', 'go,typescript,python',
            '--output', str(root / 'python-output')], root, 'pypi-cli', environment)
        # Verify the published Rust facade as a library, not the unpublished CLI crate.
        consumer = root / 'rust-consumer'
        (consumer / 'src').mkdir(parents=True)
        (consumer / 'Cargo.toml').write_text('[package]\nname="release_consumer"\nversion="0.1.0"\nedition="2024"\n'
            f'[dependencies]\npoolster={{version="={args.version}",features=["typescript"]}}\n'
            f'poolster-core="={args.version}"\n')
        (consumer / 'src/main.rs').write_text('use poolster::prelude::*;\nfn main() {\n'
            'let api = poolster_core::Api { name: "Install probe".into(), version: "1".into(), ..Default::default() };\n'
            'let tree = Packages::new().package(poolster::ts::package("sdk").with(poolster::ts::types())).generate(&api, None).unwrap();\n'
            'assert!(tree.iter().any(|(path, _)| path.extension().is_some_and(|ext| ext == "ts")));\n}\n')
        report['checks']['crates_consumer'] = run(['cargo', 'run', '--quiet', '--manifest-path',
            str(consumer / 'Cargo.toml')], root, 'crates-consumer')
        report['installed_native_files'] = {
            path.relative_to(root).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
            for folder in [root / 'node_modules/@relevate', target / 'poolster/bin']
            for path in folder.rglob('*') if path.is_file() and
            (path.name in ('poolster', 'poolster.exe', 'poolster-openapi', 'poolster-openapi.exe') or path.suffix == '.node')}
        report['status'] = 'passed'
    except Exception as error:
        report['error'] = str(error)
    (root / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({key: value for key, value in report.items() if key not in ('checks', 'installed_native_files')}))
    return int(report['status'] != 'passed')


if __name__ == '__main__':
    sys.exit(main())
