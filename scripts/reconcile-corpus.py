"""Reconcile explicit chronological corpus attempts without losing failures or missing cases."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

LANGUAGES = ('rust', 'typescript', 'go', 'python', 'php', 'java', 'csharp', 'elixir', 'ruby', 'swift')


def reconcile(manifest, reports):
    contracts = {entry['name']: entry['sha256'] for entry in manifest['contracts']}
    expected = {(name, language) for name in contracts for language in LANGUAGES}
    attempts = {}
    errors = []
    sources = []
    for path, report in reports:
        provenance = report.get('provenance', {})
        sources.append({'report': str(path), 'complete': report.get('complete'),
                        'provenance': provenance})
        for case in report['cases']:
            key = (case['contract'], case['language'])
            if key not in expected:
                errors.append('unexpected case: ' + '/'.join(key))
                continue
            if case.get('source_sha256') != contracts[key[0]]:
                errors.append('source checksum mismatch: ' + '/'.join(key))
            attempts.setdefault(key, []).append({'report': str(path), 'provenance': provenance, 'case': case})
    selected = []
    binaries = {}
    for key in sorted(expected):
        history = attempts.get(key, [])
        if not history:
            errors.append('missing case: ' + '/'.join(key))
            continue
        latest = history[-1]
        case = latest['case']
        valid = (case.get('status') == 'passed' and (case.get('generation') or {}).get('exit_code') == 0
                 and (case.get('native') or {}).get('exit_code') == 0
                 and not (case.get('native') or {}).get('deprecations')
                 and (case.get('regeneration') or {}).get('status') == 'passed'
                 and (case.get('regeneration') or {}).get('generation', {}).get('exit_code') == 0)
        identity = latest['provenance'].get('binaries', {})
        if not all(identity.get(name, {}).get('sha256') for name in ('generator', 'openapi_compiler')):
            valid = False
        if not valid:
            errors.append('unverified final case: ' + '/'.join(key))
        digests = tuple(identity.get(name, {}).get('sha256') for name in ('generator', 'openapi_compiler'))
        binaries.setdefault(key[1], set()).add(digests)
        selected.append({'contract': key[0], 'language': key[1], 'verified': valid,
                         'selected_report': latest['report'], 'attempts': history})
    for language, identities in binaries.items():
        if len(identities) != 1:
            errors.append('multiple final binary pairs for ' + language)
    return {'schema_version': 1, 'expected_cases': len(expected),
            'verified_cases': sum(entry['verified'] for entry in selected),
            'missing_cases': len(expected - attempts.keys()),
            'status': 'passed' if not errors else 'failed', 'errors': errors,
            'reports': sources, 'cases': selected}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=Path, default=Path(__file__).parent / 'fixtures/guru-contracts.json')
    parser.add_argument('--reports', nargs='+', type=Path, required=True,
                        help='Explicit chronological report paths; later attempts supersede earlier cases')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = reconcile(json.loads(args.manifest.read_text()),
                       [(path, json.loads(path.read_text())) for path in args.reports])
    result['manifest_sha256'] = hashlib.sha256(args.manifest.read_bytes()).hexdigest()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({key: result[key] for key in ('status', 'expected_cases', 'verified_cases', 'missing_cases')}))
    return int(result['status'] != 'passed')


if __name__ == '__main__':
    sys.exit(main())
