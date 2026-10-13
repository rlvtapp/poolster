import base64
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock

spec = importlib.util.spec_from_file_location('publish_npm', Path(__file__).with_name('publish-npm.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class PublicationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / 'package.tgz'
        data = json.dumps({'name': '@relevate/test', 'version': '1.0.0'}).encode()
        with tarfile.open(self.path, 'w:gz') as archive:
            info = tarfile.TarInfo('package/package.json')
            info.size = len(data)
            archive.addfile(info, io.BytesIO(data))

    def test_nested_artifact_directories_are_resolved(self):
        nested = Path(self.temp.name) / 'plugins' / 'typescript'
        nested.mkdir(parents=True)
        target = nested / 'plugin.tgz'
        target.write_bytes(self.path.read_bytes())
        self.assertEqual(module.collect_artifacts([nested.parent]), [target])
        self.assertEqual(module.collect_artifacts([target, target]), [target])

    def test_empty_and_missing_artifacts_fail_preflight(self):
        empty = Path(self.temp.name) / 'empty'
        empty.mkdir()
        for path in [empty, empty / 'missing.tgz']:
            with self.assertRaisesRegex(ValueError, 'missing artifact'):
                module.collect_artifacts([self.path, path])

    def test_identical_artifact_is_skipped_without_changing_tags(self):
        integrity = 'sha512-' + base64.b64encode(hashlib.sha512(self.path.read_bytes()).digest()).decode()
        run = Mock(return_value=SimpleNamespace(returncode=0, stdout=json.dumps(integrity)))
        module.publish(self.path, 'latest', run)
        self.assertEqual(run.call_count, 1)

    def test_conflicting_existing_artifact_fails(self):
        run = Mock(return_value=SimpleNamespace(returncode=0, stdout='"sha512-other"'))
        with self.assertRaisesRegex(RuntimeError, 'different artifact'):
            module.publish(self.path, 'latest', run)
        self.assertEqual(run.call_count, 1)

    def test_missing_version_is_published(self):
        run = Mock(return_value=SimpleNamespace(returncode=1, stdout='{"error":{"code":"E404"}}'))
        module.publish(self.path, 'alpha', run)
        self.assertEqual(run.call_args.args[0], ['npm', 'publish', str(self.path), '--ignore-scripts', '--access', 'public', '--tag', 'alpha'])
        self.assertTrue(run.call_args.kwargs['check'])

    def test_network_or_auth_error_does_not_attempt_publication(self):
        for output in ('{"error":{"code":"E401"}}', 'not JSON', '{"error":{"code":"ETIMEDOUT"}}'):
            run = Mock(return_value=SimpleNamespace(returncode=1, stdout=output, stderr='registry unavailable'))
            with self.assertRaisesRegex(RuntimeError, 'Cannot check'):
                module.publish(self.path, 'latest', run)
            self.assertEqual(run.call_count, 1)


if __name__ == '__main__':
    unittest.main()
