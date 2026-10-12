"""Runner checks: discovery, ignored-test accounting and missing-prerequisite reporting."""
import importlib.util
import json
import os
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import tempfile

spec = importlib.util.spec_from_file_location('conformance', Path(__file__).with_name('kafka-conformance.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ConformanceTests(unittest.TestCase):
    def test_catalog_and_evidence_paths_are_valid(self):
        self.assertEqual(module.validate(json.loads(module.CATALOG.read_text())), [])

    def test_tested_cases_must_have_executable_suite_links(self):
        catalog = json.loads(module.CATALOG.read_text())
        catalog['languages']['go']['coverage']['utf8']['suites'] = []
        self.assertTrue(any('executable suite' in error for error in module.validate(catalog)))

    def test_missing_and_unknown_cases_are_rejected(self):
        catalog = json.loads(module.CATALOG.read_text())
        del catalog['languages']['go']['coverage']['utf8']
        catalog['languages']['go']['suites'][0]['cases'].append('imaginary')
        errors = module.validate(catalog)
        self.assertTrue(any('every case' in error for error in errors))
        self.assertTrue(any('unknown suite case' in error for error in errors))

    def test_documented_evidence_grid_matches_catalog(self):
        catalog = json.loads(module.CATALOG.read_text())
        doc = (module.ROOT / 'docs/reference/outputs/kafka-conformance.md').read_text()
        grid = doc.split('<!-- kafka-evidence:start -->', 1)[1].split('<!-- kafka-evidence:end -->', 1)[0].strip()
        self.assertEqual(grid, module.evidence_matrix(catalog))

    def test_count_pass_failure_ignore_separately(self):
        output = 'test result: ok. 2 passed; 0 failed; 1 ignored\n' \
                 'test result: FAILED. 1 passed; 1 failed; 0 ignored'
        self.assertEqual(module.counts(output), dict(passed=3, failed=1, ignored=1))

    def test_native_zero_and_ignored_are_not_success(self):
        suite = dict(id='broker', tier='native', command=['cargo'], cases=[])
        with tempfile.TemporaryDirectory() as directory:
            args = SimpleNamespace(timeout=1, logs=Path(directory))
            for output in ('test result: ok. 0 passed; 0 failed; 0 ignored',
                           'test result: ok. 1 passed; 0 failed; 1 ignored'):
                with patch.object(module.subprocess, 'run', return_value=SimpleNamespace(returncode=0, stdout=output, stderr='')):
                    self.assertEqual(module.run_suite('fake', suite, args)['status'], 'failed')

    def test_missing_prerequisite_is_skip_not_pass(self):
        suite = dict(id='broker', tier='native', command=['cargo'], cases=[], required_env=['POOLSTER_CONFORMANCE_MISSING'])
        with patch.dict(os.environ, {}, clear=True), patch.object(module.subprocess, 'run') as command:
            result = module.run_suite('fake', suite, SimpleNamespace())
            self.assertEqual(result['status'], 'skipped')
            command.assert_not_called()

    def test_failed_adapter_cannot_report_pass(self):
        suite = dict(id='generation', tier='generation', command=['cargo'], cases=[])
        with tempfile.TemporaryDirectory() as directory:
            args = SimpleNamespace(timeout=1, logs=Path(directory))
            with patch.object(module.subprocess, 'run', return_value=SimpleNamespace(returncode=1, stdout='test result: ok. 1 passed; 0 failed; 0 ignored', stderr='compiler failure')):
                self.assertEqual(module.run_suite('fake', suite, args)['status'], 'failed')


if __name__ == '__main__':
    unittest.main()
