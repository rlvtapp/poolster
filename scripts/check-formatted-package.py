"""Compile and import a generated formatted SDK with its declared native dependencies."""
import argparse
import json
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
        return [['composer', 'install', '--no-interaction', '--no-progress']] + [['php', '-l', str(path)] for path in paths] + [
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
        result = subprocess.run(command, cwd=root, check=False)
        if result.returncode:
            return result.returncode
    return 0


if __name__ == '__main__':
    sys.exit(main())
