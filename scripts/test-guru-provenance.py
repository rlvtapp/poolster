"""Provenance, incremental reports and exact corpus shard coverage."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

loader = importlib.util.spec_from_file_location('guru_corpus', Path(__file__).with_name('guru-corpus.py'))
corpus = importlib.util.module_from_spec(loader)
loader.loader.exec_module(corpus)


class ProvenanceTests(unittest.TestCase):
    def test_shards_cover_every_contract_once_and_restore_selection(self):
        selected = [{'name': 'contract-' + str(index)} for index in range(205)]
        completed = []
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, {'POOLSTER_PUBLIC_CONTRACTS': ''}):
            for shard in range(5):
                root = Path(directory) / str(shard)
                with patch.object(corpus.public, 'contracts', return_value=selected), \
                     patch.object(corpus.public, 'fetch') as fetch, \
                     patch.object(corpus, 'run_case', return_value={'status': 'passed'}) as run:
                    with patch('builtins.print'):
                        self.assertEqual(corpus.main(['--output', str(root), '--language', 'go',
                                                     '--shard-count', '5', '--shard-index', str(shard)]), 0)
                completed.extend(call.args[0]['name'] for call in run.call_args_list)
                self.assertEqual(run.call_count, 41)
                self.assertEqual(fetch.call_count, 1)
                self.assertEqual(os.environ['POOLSTER_PUBLIC_CONTRACTS'], '')
            self.assertEqual(sorted(completed), sorted(value['name'] for value in selected))

    def test_provenance_identifies_binary_bytes_and_missing_compiler(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / 'manifest.json'
            manifest.write_text('{"contracts": []}')
            binary = root / 'poolster'
            binary.write_bytes(b'frozen generator')
            result = corpus.run_provenance(manifest, {
                'POOLSTER_BINARY': str(binary),
                'POOLSTER_OPENAPI_BIN': str(root / 'missing'),
            })
            self.assertEqual(result['manifest_sha256'], hashlib.sha256(manifest.read_bytes()).hexdigest())
            self.assertEqual(result['binaries']['generator']['sha256'],
                             hashlib.sha256(b'frozen generator').hexdigest())
            self.assertEqual(result['binaries']['openapi_compiler']['status'], 'missing')
            self.assertIsNone(result['binaries']['openapi_compiler']['sha256'])

    def test_incremental_report_distinguishes_partial_from_complete_run(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            provenance = {'revision': 'frozen'}
            result = corpus.save_report(root, root / 'manifest', [], provenance, 2)
            self.assertFalse(result['complete'])
            self.assertEqual(result['expected_cases'], 2)
            self.assertEqual(result['completed_cases'], 0)
            cases = [{'status': 'passed'}, {'status': 'failed'}]
            result = corpus.save_report(root, root / 'manifest', cases, provenance, 2)
            self.assertTrue(result['complete'])
            self.assertEqual(result['failed'], 1)
            self.assertEqual(json.loads((root / 'report.json').read_text())['provenance'], provenance)

    def test_fetch_failure_retains_an_explicit_incomplete_report(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / 'run'
            with patch.object(corpus.public, 'contracts', return_value=[{'name': 'sample'}]), \
                 patch.object(corpus.public, 'fetch', side_effect=ValueError('checksum mismatch')):
                with self.assertRaisesRegex(ValueError, 'checksum mismatch'):
                    corpus.main(['--output', str(root), '--language', 'go'])
            report = json.loads((root / 'report.json').read_text())
            self.assertFalse(report['complete'])
            self.assertEqual(report['expected_cases'], 1)
            self.assertEqual(report['completed_cases'], 0)
            self.assertIn('manifest_sha256', report['provenance'])
            self.assertIn('checksum mismatch', (root / 'fetch-error.txt').read_text())


if __name__ == '__main__':
    unittest.main()
