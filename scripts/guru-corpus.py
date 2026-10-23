"""Compile a pinned APIs.guru corpus; never call the described production APIs."""
import argparse
from contextlib import contextmanager
import hashlib
import re
import importlib.util
import json
import os
import platform
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time

PROJECT = Path(__file__).resolve().parent.parent
LANGUAGES = ('rust', 'typescript', 'go', 'python', 'php', 'java', 'csharp', 'elixir', 'ruby', 'swift')
loader = importlib.util.spec_from_file_location('public_contracts', PROJECT / 'scripts/public-contracts.py')
public = importlib.util.module_from_spec(loader)
loader.loader.exec_module(public)


def memory_command(command):
    """Use the host time utility; never infer memory from parent-process RSS."""
    executable = Path('/usr/bin/time')
    if not executable.is_file():
        return command, None
    if sys.platform == 'darwin':
        flags, method = ['-l'], 'bsd-time'
    elif sys.platform.startswith('linux'):
        flags, method = ['-v'], 'gnu-time'
    else:
        return command, None
    # Sandboxed macOS can execute time but deny its sysctl measurement call.
    # Probe before wrapping real work, so measurement never changes task success.
    try:
        probe = subprocess.run([str(executable), *flags, '/usr/bin/true'],
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                               timeout=5)
        if probe.returncode != 0:
            return command, None
    except (OSError, subprocess.TimeoutExpired):
        return command, None
    return [str(executable), *flags, *command], method


def peak_memory(log, method, interrupted=False):
    result = {'status': 'unavailable', 'peak_bytes': None, 'method': method,
              'scope': 'host_command_and_children_maximum_rss',
              'reason': 'measurement_unavailable_on_host' if method is None else 'measurement_missing'}
    if interrupted:
        result['reason'] = 'interrupted_phase'
        return result
    if method:
        pattern = (r'^\s*(\d+)\s+maximum resident set size\s*$' if method == 'bsd-time'
                   else r'^\s*Maximum resident set size \(kbytes\):\s*(\d+)\s*$')
        matches = re.findall(pattern, log.read_text(errors='replace'), re.MULTILINE)
        if matches:
            result.update(status='measured', peak_bytes=int(matches[-1]) *
                          (1024 if method == 'gnu-time' else 1), reason=None)
    return result


def run_logged(command, environment, log, timeout):
    started = time.monotonic()
    log.parent.mkdir(parents=True, exist_ok=True)
    command, memory_method = memory_command(command)
    with log.open('w') as writer:
        try:
            process = subprocess.Popen(command, env={**environment, "LC_ALL": "C"}, stdout=writer,
                                       stderr=subprocess.STDOUT, start_new_session=True)
            code = process.wait(timeout=timeout)
            return {'command': command, 'exit_code': code, 'seconds': round(time.monotonic() - started, 3),
                    'log': str(log), 'timed_out': False,
                    'peak_memory': peak_memory(log, memory_method)}
        except subprocess.TimeoutExpired:
            # A shell phase owns compiler children too. Stop the whole owned
            # group before removing its generated workspace.
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait()
            writer.write('\nCorpus phase exceeded its time limit.\n')
            return {'command': command, 'exit_code': None, 'seconds': round(time.monotonic() - started, 3),
                    'log': str(log), 'timed_out': True,
                    'peak_memory': peak_memory(log, memory_method, interrupted=True)}
        except OSError as error:
            writer.write(str(error) + '\n')
            return {'command': command, 'exit_code': None, 'seconds': round(time.monotonic() - started, 3),
                    'log': str(log), 'timed_out': False,
                    'peak_memory': {'status': 'unavailable', 'peak_bytes': None,
                                    'method': memory_method, 'reason': 'launch_error'}}



SOURCE_SUFFIXES = frozenset(('.rs', '.ts', '.tsx', '.go', '.py', '.php', '.java', '.cs', '.ex', '.exs', '.rb', '.swift'))


def output_statistics(package, warning_bytes=0):
    """Measure owned output before native checks create build artifacts.

    Metadata is reported separately; JSON collections and other assets remain
    in total bytes. Budgets are reporting heuristics, never compiler limits.
    """
    files = []
    for path in package.rglob('*'):
        if path.is_file():
            relative = path.relative_to(package)
            files.append((relative.as_posix(), path.stat().st_size,
                          '.poolster' in relative.parts, path.suffix in SOURCE_SUFFIXES))
    source = sorted(((name, size) for name, size, metadata, is_source in files
                     if is_source and not metadata), key=lambda item: (-item[1], item[0]))
    oversized = sorted(((name, size) for name, size, metadata, _ in files
                        if not metadata and warning_bytes and size > warning_bytes),
                       key=lambda item: (-item[1], item[0]))
    return {
        'generated_files': len(files),
        'generated_bytes': sum(size for _, size, _, _ in files),
        'source_files': len(source),
        'source_bytes': sum(size for _, size in source),
        'metadata_bytes': sum(size for _, size, metadata, _ in files if metadata),
        'largest_source_bytes': source[0][1] if source else 0,
        'largest_source_files': [{'path': name, 'bytes': size} for name, size in source[:10]],
        'file_warning_bytes': warning_bytes or None,
        'oversized_files': [{'path': name, 'bytes': size} for name, size in oversized],
    }


def output_fingerprint(package):
    files = {}
    for path in sorted(package.rglob('*')):
        if path.is_file() and path.name not in ('generation.lock.json', 'generation-report.json'):
            relative = path.relative_to(package).as_posix()
            files[relative] = hashlib.sha256(path.read_bytes()).hexdigest()
    digest = hashlib.sha256(json.dumps(files, sort_keys=True).encode()).hexdigest()
    return {'sha256': digest, 'files': files}


def compare_fingerprints(before, after):
    return {'status': 'passed' if before == after else 'failed',
            'sha256_before': before['sha256'], 'sha256_after': after['sha256'],
            'changed_paths': sorted(path for path in before['files'].keys() | after['files'].keys()
                                    if before['files'].get(path) != after['files'].get(path))}


def run_case(contract, language, manifest, root, environment, timeout, keep_generated,
             warning_bytes=0, verify_regeneration=False):
    name = contract['name']
    report = {'contract': name, 'language': language, 'source_url': contract['url'],
              'source_sha256': contract['sha256'], 'input_bytes': contract.get('bytes'),
              'status': 'failed', 'generation': None, 'native': None}
    with tempfile.TemporaryDirectory(prefix=name + '-', dir=root / 'work') as temporary:
        scratch = Path(temporary)
        env = {**environment, 'POOLSTER_PUBLIC_CONTRACT_MANIFEST': str(manifest),
               'POOLSTER_PUBLIC_CONTRACTS': name, 'POOLSTER_PUBLIC_LANGUAGES': language,
               'POOLSTER_PUBLIC_SPEC_DIR': str(root / 'specs'),
               'POOLSTER_PUBLIC_CONTRACT_ROOT': str(scratch)}
        logs = root / 'logs' / name / language
        report['generation'] = run_logged(['bash', str(PROJECT / 'scripts/test-public-contracts.sh'),
                                          'generate'], env, logs / 'generate.log', timeout)
        sdk_root = scratch / name
        if report['generation']['exit_code'] == 0:
            package = sdk_root / language
            report.update(output_statistics(package, warning_bytes))
            before = output_fingerprint(package)
            report['output_sha256'] = before['sha256']
            if verify_regeneration:
                repeat = run_logged(['bash', str(PROJECT / 'scripts/test-public-contracts.sh'),
                                     'generate'], env, logs / 'regenerate.log', timeout)
                comparison = compare_fingerprints(before, output_fingerprint(package))
                report['regeneration'] = {**comparison, 'generation': repeat}
                if repeat['exit_code'] != 0:
                    report['regeneration']['status'] = 'failed'
            report['native'] = run_logged(['bash', str(PROJECT / 'scripts/test-public-contracts.sh'),
                                          'check', language], env, logs / 'native.log', timeout)
            native_log = logs / 'native.log'
            deprecations = (re.findall(r'(?m)^(?:PHP )?Deprecated:.*$', native_log.read_text(errors='replace'))
                            if language == 'php' and native_log.is_file() else [])
            if deprecations:
                report['native']['deprecations'] = deprecations
            if report['native']['exit_code'] == 0 and not deprecations and report.get('regeneration', {}).get('status', 'passed') == 'passed':
                report['status'] = 'passed'
        for metadata in (sdk_root / '.poolster', sdk_root / language / '.poolster'):
            if metadata.is_dir():
                relative = metadata.relative_to(sdk_root)
                shutil.copytree(metadata, root / 'metadata' / name / language / relative,
                                dirs_exist_ok=True)
        if keep_generated and sdk_root.exists():
            destination = root / 'generated' / name / language
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.move(str(sdk_root), destination)
    return report


def run_provenance(manifest, environment):
    """Identify the source and exact executable bytes used by this run."""
    def git(*arguments):
        result = subprocess.run(['git', '-C', str(PROJECT), *arguments],
                                capture_output=True, text=True, check=False)
        return result.stdout.strip() if result.returncode == 0 else None

    binaries = {}
    for name, variable, fallback in (
        ('generator', 'POOLSTER_BINARY', PROJECT / 'target/debug/poolster'),
        ('openapi_compiler', 'POOLSTER_OPENAPI_BIN', PROJECT / 'target/debug/poolster-openapi'),
    ):
        path = Path(environment.get(variable) or fallback).resolve()
        binaries[name] = {'path': str(path), 'sha256': None, 'status': 'missing'}
        if path.is_file():
            binaries[name].update(sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                                  status='identified')
    status = git('status', '--porcelain')
    return {'revision': git('rev-parse', 'HEAD'),
            'working_tree_dirty': None if status is None else bool(status),
            'manifest_sha256': hashlib.sha256(manifest.read_bytes()).hexdigest(),
            'platform': platform.platform(), 'python': platform.python_version(),
            'binaries': binaries}


def save_report(root, manifest, cases, provenance=None, expected_cases=None):
    result = {'schema_version': 1, 'manifest': str(manifest),
              'passed': sum(case['status'] == 'passed' for case in cases),
              'failed': sum(case['status'] != 'passed' for case in cases), 'cases': cases}
    if provenance is not None:
        result['provenance'] = provenance
    if expected_cases is not None:
        result.update(expected_cases=expected_cases, completed_cases=len(cases),
                      complete=len(cases) == expected_cases)
    destination = root / 'report.json'
    staged = root / 'report.json.tmp'
    staged.write_text(json.dumps(result, indent=2) + '\n')
    staged.replace(destination)
    return result


@contextmanager
def contract_selection(names):
    previous = os.environ.get('POOLSTER_PUBLIC_CONTRACTS')
    os.environ['POOLSTER_PUBLIC_CONTRACTS'] = names
    try:
        yield
    finally:
        if previous is None:
            os.environ.pop('POOLSTER_PUBLIC_CONTRACTS', None)
        else:
            os.environ['POOLSTER_PUBLIC_CONTRACTS'] = previous


def main(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=Path, default=PROJECT / 'scripts/fixtures/guru-contracts.json')
    parser.add_argument('--output', type=Path, required=True, help='new disposable directory')
    parser.add_argument('--language', choices=LANGUAGES, required=True)
    parser.add_argument('--contracts', default='', help='comma-separated pinned names; default all')
    parser.add_argument('--spec-cache', type=Path)
    parser.add_argument('--shard-count', type=int, default=1)
    parser.add_argument('--shard-index', type=int, default=0)
    parser.add_argument('--timeout', type=int, default=600, help='seconds per generation/native phase')
    parser.add_argument('--keep-generated', action='store_true')
    parser.add_argument('--verify-regeneration', action='store_true',
                        help='regenerate into the same owned workspace and compare every emitted byte')
    parser.add_argument('--file-warning-bytes', type=int, default=0,
                        help='report generated files above this size; 0 disables warnings')
    args = parser.parse_args(arguments)
    if not 1 <= args.timeout <= 3600:
        parser.error('--timeout must be between 1 and 3600 seconds')
    if args.file_warning_bytes < 0:
        parser.error('--file-warning-bytes cannot be negative')
    if args.shard_count < 1 or not 0 <= args.shard_index < args.shard_count:
        parser.error('shard count must be positive and index must be within the shard count')
    manifest = args.manifest.resolve()
    with contract_selection(args.contracts or os.environ.get('POOLSTER_PUBLIC_CONTRACTS', '')):
        selected = public.contracts(manifest)
    selected = selected[args.shard_index::args.shard_count]
    if not selected:
        parser.error('no contracts selected')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    (root / 'work').mkdir()
    (root / 'generated').mkdir()
    provenance = run_provenance(manifest, dict(os.environ))
    provenance['selection'] = {'language': args.language, 'shard_count': args.shard_count,
                               'shard_index': args.shard_index,
                               'contracts': [contract['name'] for contract in selected]}
    save_report(root, manifest, [], provenance, len(selected))
    try:
        with contract_selection(','.join(contract['name'] for contract in selected)):
            public.fetch(manifest, root / 'specs', args.spec_cache)
    except (ValueError, OSError) as error:
        (root / 'fetch-error.txt').write_text(str(error) + '\n')
        raise
    cases = []
    for contract in selected:
        case = run_case(contract, args.language, manifest, root, dict(os.environ),
                        args.timeout, args.keep_generated, args.file_warning_bytes,
                        args.verify_regeneration)
        cases.append(case)
        result = save_report(root, manifest, cases, provenance, len(selected))
        print(f"{contract['name']} / {args.language}: {case['status']}", flush=True)
    print(f"Passed {result['passed']}; failed {result['failed']}; report: {root / 'report.json'}")
    return 1 if result['failed'] else 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, OSError) as error:
        sys.exit(str(error))
