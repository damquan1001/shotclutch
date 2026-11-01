"""A failed focused check must stop the gate and preserve the child exit code."""
import importlib.util
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch


def runner():
    spec = importlib.util.spec_from_file_location('m1_checks', Path(__file__).with_name('m1-checks.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class M1Checks(unittest.TestCase):
    def test_stops_at_first_failure_and_preserves_status(self):
        module = runner()
        with patch.object(module.subprocess, 'run', side_effect=[SimpleNamespace(returncode=0), SimpleNamespace(returncode=23)]) as child:
            self.assertEqual(module.main(), 23)
            self.assertEqual(child.call_count, 2)

if __name__ == '__main__':
    unittest.main()
