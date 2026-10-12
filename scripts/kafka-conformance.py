#!/usr/bin/env python3
"""Run real generated-package Kafka adapters; never count ignored tests as passes."""
import argparse
import datetime
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CATALOG = ROOT / 'scripts/fixtures/kafka-conformance.json'


def validate(catalog, root=ROOT):
    errors = []
    ids = [case['id'] for case in catalog['cases']]
    if len(ids) != len(set(ids)):
        errors.append('duplicate case IDs')
    for language, adapter in catalog['languages'].items():
        suite_ids = {suite['id'] for suite in adapter['suites']}
        if set(adapter['coverage']) != set(ids):
            errors.append(f'{language}: coverage must describe every case')
        for case, evidence in adapter['coverage'].items():
            if case not in ids:
                errors.append(f'{language}: unknown case {case}')
            if evidence['status'] not in ('tested', 'gap', 'native-policy'):
                errors.append(f'{language}/{case}: invalid evidence status')
            linked = evidence.get('suites', [])
            if evidence['status'] in ('tested', 'native-policy') and not linked:
                errors.append(f'{language}/{case}: tested evidence requires an executable suite')
            for suite_id in linked:
                matching = [suite for suite in adapter['suites'] if suite['id'] == suite_id]
                if suite_id not in suite_ids or not matching or case not in matching[0]['cases']:
                    errors.append(f'{language}/{case}: invalid suite evidence {suite_id}')
            for source in evidence.get('sources', []):
                if not (root / source).is_file():
                    errors.append(f'{language}/{case}: missing source {source}')
        for suite in adapter['suites']:
            if any(case not in ids for case in suite['cases']):
                errors.append(f'{language}/{suite["id"]}: unknown suite case')
            if suite['tier'] not in ('generation', 'native'):
                errors.append(f'{language}: invalid suite tier')
            if not suite['command'] or suite['command'][0] != 'cargo':
                errors.append(f'{language}: invalid adapter command')
    return errors


def evidence_matrix(catalog):
    """Render deterministic test evidence, never a transport capability promise."""
    titles = {'typescript': 'TypeScript', 'go': 'Go', 'rust': 'Rust',
              'python': 'Python', 'java': 'Java', 'csharp': 'C#'}
    languages = list(catalog['languages'])
    lines = ['| Case | ' + ' | '.join(titles.get(name, name) for name in languages) + ' |',
             '| --- | ' + ' | '.join(':---:' for _ in languages) + ' |']
    symbols = {'tested': 'T', 'native-policy': 'P', 'gap': '—'}
    for case in catalog['cases']:
        row = [symbols[catalog['languages'][name]['coverage'][case['id']]['status']]
               for name in languages]
        lines.append('| `' + case['id'] + '` | ' + ' | '.join(row) + ' |')
    return '\n'.join(lines)


def counts(output):
    matches = re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored', output)
    return {key: sum(int(row[index]) for row in matches)
            for index, key in enumerate(('passed', 'failed', 'ignored'))}


def run_suite(language, suite, args):
    entry = {'language': language, 'suite': suite['id'], 'tier': suite['tier'],
             'command': suite['command'], 'cases': suite['cases']}
    missing = [name for name in suite.get('required_env', []) if not os.environ.get(name)]
    if missing:
        return dict(entry, status='skipped', reason='missing environment: ' + ', '.join(missing))
    try:
        result = subprocess.run(suite['command'], cwd=ROOT, capture_output=True,
                                text=True, timeout=args.timeout)
        output = result.stdout + result.stderr
    except subprocess.TimeoutExpired as error:
        return dict(entry, status='failed', reason=f'timeout after {args.timeout}s')
    args.logs.mkdir(parents=True, exist_ok=True)
    log = args.logs / f'{language}-{suite["id"]}.log'
    log.write_text(output)
    totals = counts(output)
    # Zero matched tests, or a forgotten --ignored, must never become green evidence.
    success = result.returncode == 0 and totals['passed'] > 0 and totals['failed'] == 0
    if suite['tier'] == 'native' and totals['ignored']:
        success = False
    return dict(entry, status='passed' if success else 'failed', exit_code=result.returncode,
                tests=totals, log=str(log),
                reason=None if success else 'adapter failed or no tests executed')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--languages', nargs='+', help='default: all registered languages')
    parser.add_argument('--native', action='store_true', help='also build and execute generated packages against local Kafka')
    parser.add_argument('--matrix', action='store_true', help='print deterministic Markdown test-evidence grid without running tests')
    parser.add_argument('--check', action='store_true', help='validate catalog without running tests')
    parser.add_argument('--timeout', type=int, default=900)
    parser.add_argument('--logs', type=Path, default=Path('/private/tmp/poolster-kafka-conformance'))
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error('--timeout must be positive')
    catalog = json.loads(CATALOG.read_text())
    errors = validate(catalog)
    languages = args.languages or list(catalog['languages'])
    errors += [f'unknown language: {name}' for name in languages if name not in catalog['languages']]
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        return 2
    if args.matrix:
        print(evidence_matrix(catalog))
        return 0
    if args.check:
        print(f'Valid catalog: {len(catalog["cases"])} cases, {len(catalog["languages"])} adapters')
        return 0
    results = []
    # Sequential: cargo and native dependency caches share locks; each broker probe
    # also owns its topic lifecycle. Parallel external invocations are unsupported.
    for language in languages:
        for suite in catalog['languages'][language]['suites']:
            if suite['tier'] == 'native' and not args.native:
                continue
            print(f'{language}: {suite["id"]}', flush=True)
            results.append(run_suite(language, suite, args))
    report = {'schema_version': 1, 'generated_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'scope': 'adapter-level execution, not per-assertion instrumentation',
              'results': results,
              'summary': {status: sum(item['status'] == status for item in results)
                          for status in ('passed', 'failed', 'skipped')}}
    rendered = json.dumps(report, indent=2) + '\n'
    if args.output:
        args.output.write_text(rendered)
    print(rendered)
    # Missing prerequisites are distinct from success, including in CI.
    return 1 if report['summary']['failed'] or report['summary']['skipped'] else 0


if __name__ == '__main__':
    raise SystemExit(main())
