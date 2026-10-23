"""Compile and import a generated formatted SDK with its declared native dependencies."""
import argparse
import json
import re
from pathlib import Path
import subprocess
import sys


def commands(language, root):
    if language == 'typescript':
        (root / 'consumer.ts').write_text("import * as sdk from './index.js';\nvoid sdk;\n")
        return [['npm', 'install', '--ignore-scripts', '--no-audit', '--no-fund'], ['npm', 'run', 'build']]
    if language == 'rust':
        return [['cargo', 'check', '--quiet']]
    if language == 'go':
        return [['go', 'test', './...'], ['go', 'vet', './...']]
    if language == 'python':
        return [[sys.executable, '-m', 'pip', 'install', '--disable-pip-version-check', '--no-deps', '--target', str(root / 'installed'), '.'],
                [sys.executable, '-c', 'import sys; sys.path.insert(0,"installed"); import quality_probe']]
    if language in ('php', 'symfony'):
        paths = sorted(root.rglob('*.php'))
        portable = root.parent / 'portable'
        if language == 'symfony' and (portable / 'composer.json').is_file():
            consumer = root.parent / 'consumer'
            consumer.mkdir()
            manifest = json.loads((root / 'composer.json').read_text())
            sdk = json.loads((portable / 'composer.json').read_text())
            (consumer / 'composer.json').write_text(json.dumps({
                'name': 'poolster/audit-consumer', 'require': {manifest['name']: '1.0.0'},
                'repositories': [{'type':'path', 'url':str(path), 'options':{'versions':{name:'1.0.0'}}}
                                 for path, name in [(root, manifest['name']), (portable, sdk['name'])]]}))
            probe = consumer / 'probe.php'
            probe.write_text(r'''<?php
require __DIR__.'/vendor/autoload.php';
$container = new Symfony\Component\DependencyInjection\ContainerBuilder();
$container->register('http_client', Symfony\Component\HttpClient\HttpClient::class)->setFactory([Symfony\Component\HttpClient\HttpClient::class, 'create']);
(new Poolster\Quality\Probe\DependencyInjection\QualityProbeExtension())->load([['base_url'=>'http://unit.invalid']], $container);
$container->getDefinition(Poolster\Quality\Probe\Sdk\Client::class)->setPublic(true);
$container->compile();
if (!$container->get(Poolster\Quality\Probe\Sdk\Client::class) instanceof Poolster\Quality\Probe\Sdk\Client) throw new RuntimeException('autowired SDK missing');
''')
            return [['composer', 'install', '--working-dir', str(consumer), '--no-interaction', '--no-progress']] + [
                ['php', '-d', 'error_reporting=32767', '-l', str(path)] for path in paths] + [['php', str(probe)]]
        return [['composer', 'install', '--no-interaction', '--no-progress', '--optimize-autoloader']] + [['php', '-d', 'error_reporting=32767', '-d', 'display_errors=stderr', '-l', str(path)] for path in paths] + [
            ['php', '-r', 'require "vendor/autoload.php"; $loader = require "vendor/autoload.php"; foreach ($loader->getClassMap() as $class => $file) { class_exists($class); }']]
    if language == 'java':
        return [['mvn', '--batch-mode', '--no-transfer-progress', '-q', '-DskipTests', 'compile']]
    if language == 'csharp':
        return [['dotnet', 'build', '--configuration', 'Release', '--disable-build-servers', '-p:UseSharedCompilation=false']]
    if language == 'ruby':
        return [['ruby', '-c', str(path)] for path in sorted((root / 'lib').rglob('*.rb'))] + [
            ['ruby', '-Ilib', '-e', 'require "quality_probe"']]
    if language == 'swift':
        return [['swift', 'build', '--disable-sandbox']]
    if language == 'elixir':
        return [['mix', 'deps.get'], ['mix', 'compile', '--warnings-as-errors']]
    raise ValueError('Unsupported language ' + language)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('language')
    parser.add_argument('package', type=Path)
    args = parser.parse_args()
    root = args.package.resolve(strict=True)
    for command in commands(args.language, root):
        result = subprocess.run(command, cwd=root, check=False, capture_output=True, text=True)
        print(result.stdout, end='')
        print(result.stderr, end='', file=sys.stderr)
        if result.returncode:
            return result.returncode
        if args.language in ('php', 'symfony') and re.search(r'(?m)^(?:PHP )?Deprecated:', result.stdout + result.stderr):
            return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
