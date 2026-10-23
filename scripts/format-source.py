"""Pinned formatter adapter for the generated-source audit, never a runtime dependency."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys


def formatter(language, tools):
    php = language in ('php', 'symfony')
    if language == 'typescript':
        return ['node', str(tools / 'node_modules/prettier/bin/prettier.cjs')], ['--version'], ['--write', '--config', str(tools / 'prettier.json')]
    if language == 'python':
        return ['ruff'], ['--version'], ['format', '--config', str(tools / 'ruff.toml')]
    if php:
        return ['php', str(tools / 'php-cs-fixer.phar')], ['--version'], ['fix', '--using-cache=no', '--allow-risky=no', '--rules=@PSR12', '--quiet']
    if language == 'java':
        return ['java', '-jar', str(tools / 'google-java-format.jar')], ['--version'], ['--replace']
    if language == 'rust':
        return ['rustup', 'run', '1.90.0', 'rustfmt'], ['--version'], ['--edition', '2024', '--config', 'skip_children=true']
    if language == 'go':
        return ['gofmt'], [], ['-w']
    if language == 'ruby':
        return ['rubocop'], ['--version'], ['--autocorrect', '--only', 'Layout', '--config', str(tools / 'rubocop.yml'), '--format', 'quiet']
    if language == 'elixir':
        return ['mix'], ['--version'], ['format', '--dot-formatter', str(tools / 'formatter.exs')]
    if language == 'swift':
        return ['xcrun', 'swift-format'], ['--version'], ['format', '--in-place', '--configuration', str(tools / 'swift-format.json')]
    if language == 'csharp':
        return ['dotnet'], ['--version'], []
    raise ValueError('Unsupported formatter language: ' + language)


def invoke(command):
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f'{command[0]} exited {result.returncode}: {result.stdout}{result.stderr}')
    return (result.stdout + result.stderr).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--language', required=True)
    parser.add_argument('--tools', type=Path, required=True)
    parser.add_argument('--write-profile', type=Path)
    parser.add_argument('file', nargs='?')
    parser.add_argument('--version', action='store_true')
    args = parser.parse_args()
    tools = args.tools.resolve()
    command, version_args, flags = formatter(args.language, tools)
    if args.version or args.write_profile:
        version = invoke(['go', 'version'] if args.language == 'go' else command + version_args)
        # A tool's own version string can be "main" (Xcode swift-format). Include
        # its executable checksum, configuration bytes and pinned host-toolchain identity.
        resolved = invoke(['xcrun', '--find', 'swift-format']) if args.language == 'swift' else shutil.which(command[0])
        configs = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(tools.glob('*'))
                   if p.is_file() and p.suffix in ('.json', '.toml', '.yml', '.exs', '.editorconfig', '.phar', '.jar')}
        identity = json.dumps({'version': version, 'configuration': configs,
                               'adapter_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                               'executable_sha256': hashlib.sha256(Path(resolved).read_bytes()).hexdigest()}, sort_keys=True)
        if args.version:
            print(identity)
            return
        extensions = {'typescript':'ts', 'python':'py', 'php':'php', 'symfony':'php', 'java':'java',
                      'rust':'rs', 'go':'go', 'ruby':'rb', 'swift':'swift', 'csharp':'cs', 'elixir':'ex'}
        profile = {'mode':'formatted', 'maxFileBytes':131072, 'formatter': {
            'program':sys.executable, 'arguments':[str(Path(__file__).resolve()), '--tools', str(tools),
                '--language', args.language, '{file}'], 'versionArguments':[str(Path(__file__).resolve()),
                '--tools', str(tools), '--language', args.language, '--version'],
            'expectedVersion':identity, 'extensions':(['ex', 'exs'] if args.language == 'elixir'
                                                     else [extensions[args.language]]), 'timeoutSeconds':120}}
        args.write_profile.write_text(json.dumps(profile, indent=2) + '\n')
        return
    if not args.file:
        parser.error('file is required')
    source = Path(args.file).resolve(strict=True)
    if args.language == 'csharp':
        (source.parent / '.editorconfig').write_text((tools / 'csharp.editorconfig').read_text())
        invoke(['dotnet', 'format', 'whitespace', str(source.parent), '--folder', '--include', str(source), '--include-generated', '--verbosity', 'quiet'])
    elif args.language == 'java':
        # google-java-format can require a second pass for generated resource
        # declarations. Require convergence, rather than accepting drifting bytes.
        for _ in range(4):
            before = source.read_bytes()
            invoke(command + flags + [str(source)])
            if source.read_bytes() == before:
                break
        else:
            raise RuntimeError('google-java-format did not reach a fixed point in four passes')
    else:
        invoke(command + flags + [str(source)])


if __name__ == '__main__':
    main()
