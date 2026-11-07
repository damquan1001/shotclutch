"""Protocol checks need only Python's standard library, not speech packages or weights."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("fast_whisper_worker", Path(__file__).resolve().parents[1] / "src" / "fast_whisper.py")
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)


class ProtocolTests(unittest.TestCase):
    def test_unicode_words_survive_a_legacy_windows_pipe_encoding(self):
        value = {"event": "result", "words": [{"text": "caf\u00e9 \u2014 \U0001f3a5"}]}
        raw = io.BytesIO()
        pipe = io.TextIOWrapper(raw, encoding="cp1252")
        with contextlib.redirect_stdout(pipe):
            worker.emit(value)
        wire = raw.getvalue()
        self.assertTrue(wire.isascii())
        self.assertEqual(json.loads(wire.decode("utf-8")), value)

    def test_nonfinite_values_never_emit_nonstandard_json(self):
        for invalid in (float("nan"), float("inf"), float("-inf")):
            output = io.StringIO()
            with contextlib.redirect_stdout(output), self.assertRaises(ValueError):
                worker.emit({"confidence": invalid})
            self.assertEqual(output.getvalue(), "")


if __name__ == "__main__":
    unittest.main()
