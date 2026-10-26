"""Guard failures that could otherwise turn a broken CI run green."""
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import types
import unittest
from unittest.mock import patch


def load(name):
    spec = importlib.util.spec_from_file_location(name.replace('-', '_'), Path(__file__).parent / (name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class FailureGuards(unittest.TestCase):
    def setUp(self):
        self.scratch = Path(__file__).resolve().parents[2] / 'target/ci/helper-tests'
        self.scratch.mkdir(parents=True, exist_ok=True)

    def test_corrupt_download_is_never_executed(self):
        setup = load('setup-oracles')
        # Prime Windows platform discovery before mocking subprocess execution.
        setup.platform.system()
        setup.platform.machine()
        with tempfile.TemporaryDirectory(dir=self.scratch) as temporary:
            self.assertTrue(Path(temporary).resolve().is_relative_to(self.scratch.resolve()))
            with patch.object(setup.urllib.request, 'urlopen', return_value=io.BytesIO(b'corrupt archive')):
                with patch.object(setup.subprocess, 'check_output') as execute:
                    with self.assertRaisesRegex(RuntimeError, 'Checksum mismatch'):
                        setup.install(Path(temporary))
                    execute.assert_not_called()
                    self.assertFalse(list(Path(temporary).rglob('ffmpeg.exe')))

    def test_child_failure_preserves_exit_code_and_diagnostics(self):
        runner = load('run-checks')
        runner.platform.platform()
        runner.platform.processor()
        with tempfile.TemporaryDirectory(dir=self.scratch) as temporary:
            root = Path(temporary)
            (root / 'Cargo.lock').write_text('test lock')
            runner.__file__ = str(root / 'scripts/ci/run-checks.py')
            process = types.SimpleNamespace(stdout=['observable child failure\n'], wait=lambda: 7)
            with patch('sys.argv', ['run-checks.py', 'checks']), \
                 patch.object(runner.subprocess, 'check_output', return_value='test-version\n'), \
                 patch.object(runner.subprocess, 'Popen', return_value=process):
                with self.assertRaises(SystemExit) as failure:
                    runner.main()
            self.assertEqual(failure.exception.code, 7)
            report = root / 'target/ci/evidence/checks'
            summary = json.loads((report / 'summary.json').read_text())
            self.assertEqual(summary['result'], 'failed')
            self.assertEqual(summary['commands'][0]['exit_code'], 7)
            self.assertEqual((report / '01.log').read_text(), 'observable child failure\n')

    @unittest.skipUnless(__import__('os').name == 'nt', 'Windows compiler setup')
    def test_corrupt_dxc_is_never_executed(self):
        setup = load('setup-windows-gpu')
        with tempfile.TemporaryDirectory(dir=self.scratch) as temporary:
            with patch.object(setup.urllib.request, 'urlopen', return_value=io.BytesIO(b'corrupt compiler')):
                with patch.object(setup.subprocess, 'check_output') as execute:
                    with self.assertRaisesRegex(RuntimeError, 'checksum mismatch'):
                        setup.install(Path(temporary))
                    execute.assert_not_called()
                    self.assertFalse(list(Path(temporary).rglob('*.dll')))


if __name__ == '__main__':
    unittest.main()
