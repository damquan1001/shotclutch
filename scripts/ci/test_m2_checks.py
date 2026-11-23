"""A failure in either newly gated suite must stop M2 qualification."""
import contextlib
import importlib.util
import io
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch


def runner():
    spec = importlib.util.spec_from_file_location('m2_checks', Path(__file__).with_name('m2-checks.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class M2Checks(unittest.TestCase):
    def test_new_suite_failures_are_required_and_stop_the_gate(self):
        for suite in ('preedit::', 'mixer_tests::'):
            with self.subTest(suite=suite):
                module = runner()

                def run(command, **kwargs):
                    return SimpleNamespace(returncode=27 if command[-1] == suite else 0)

                with patch.object(module.subprocess, 'run', side_effect=run) as child, contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(module.main(), 27)
                self.assertEqual(child.call_args_list[-1].args[0][-1], suite)


if __name__ == '__main__':
    unittest.main()
