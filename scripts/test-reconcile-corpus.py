"""Ensure partial results, absent regeneration and stale binaries cannot certify a corpus."""
import copy
import importlib.util
from pathlib import Path
import unittest

loader = importlib.util.spec_from_file_location('reconcile', Path(__file__).with_name('reconcile-corpus.py'))
reconcile = importlib.util.module_from_spec(loader)
loader.loader.exec_module(reconcile)


class ReconcileTests(unittest.TestCase):
    def fixture(self):
        manifest = {'contracts': [{'name': 'sample', 'sha256': 'source'}]}
        provenance = {'binaries': {name: {'sha256': name} for name in ('generator', 'openapi_compiler')}}
        cases = [{'contract': 'sample', 'language': language, 'source_sha256': 'source', 'status': 'passed',
                  'generation': {'exit_code': 0}, 'native': {'exit_code': 0},
                  'regeneration': {'status': 'passed', 'generation': {'exit_code': 0}}}
                 for language in reconcile.LANGUAGES]
        return manifest, {'provenance': provenance, 'cases': cases, 'complete': True}

    def test_exact_coverage_passes_and_missing_target_fails(self):
        manifest, report = self.fixture()
        self.assertEqual(reconcile.reconcile(manifest, [('all', report)])['status'], 'passed')
        report['cases'].pop()
        result = reconcile.reconcile(manifest, [('partial', report)])
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['missing_cases'], 1)

    def test_runtime_failure_or_missing_regeneration_is_not_a_pass(self):
        for mutation in ('native', 'regeneration', 'checksum', 'identity', 'deprecation'):
            manifest, report = self.fixture()
            case = report['cases'][0]
            if mutation == 'native': case['native']['exit_code'] = 1
            if mutation == 'regeneration': case.pop('regeneration')
            if mutation == 'checksum': case['source_sha256'] = 'changed'
            if mutation == 'identity': report['provenance'] = {}
            if mutation == 'deprecation': case['native']['deprecations'] = ['Deprecated: optional before required']
            with self.subTest(mutation=mutation):
                self.assertEqual(reconcile.reconcile(manifest, [('run', report)])['status'], 'failed')

    def test_latest_attempt_wins_and_earlier_failure_is_preserved(self):
        manifest, passing = self.fixture()
        failed = copy.deepcopy(passing)
        failed['cases'][0]['status'] = 'failed'
        fixed = {'provenance': passing['provenance'], 'cases': [passing['cases'][0]]}
        result = reconcile.reconcile(manifest, [('first', failed), ('retry', fixed)])
        self.assertEqual(result['status'], 'passed')
        history = next(case['attempts'] for case in result['cases'] if case['language'] == 'rust')
        self.assertEqual([attempt['case']['status'] for attempt in history], ['failed', 'passed'])
        self.assertEqual(reconcile.reconcile(manifest, [('first', passing), ('later', failed)])['status'], 'failed')


if __name__ == '__main__':
    unittest.main()
