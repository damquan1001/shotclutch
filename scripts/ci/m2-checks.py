"""Headless Smart Cut qualification. Private footage and credentials are optional local checks."""
from pathlib import Path
import subprocess
import sys
import time


def main():
    root = Path(__file__).resolve().parents[2]
    commands = [
        [sys.executable, "-m", "unittest", "discover", "-s", "scripts/ci", "-p", "test_m2_checks.py"],
        [sys.executable, "-m", "unittest", "discover", "-s", "crates/speech/tests", "-p", "test_fast_whisper_protocol.py"],
        ["cargo", "test", "--locked", "-p", "shotclutch-engine", "--lib", "m2_"],
        ["cargo", "test", "--locked", "-p", "shotclutch-engine", "--lib", "preedit::"],
        ["cargo", "test", "--locked", "-p", "shotclutch-render", "--lib", "mixer_tests::"],
        ["cargo", "test", "--locked", "-p", "shotclutch-ui-egui", "--lib", "smart_cut_cleanup_form"],
        ["cargo", "test", "--locked", "-p", "shotclutch-codecs", "--lib",
         "edit_boundaries_handle_reordered_and_repeated_timestamps"],
        ["cargo", "test", "--locked", "-p", "shotclutch-speech", "--features", "download",
         "--lib"],
    ]
    started = time.monotonic()
    for command in commands:
        print(" ".join(command), flush=True)
        result = subprocess.run(command, cwd=root, check=False)
        if result.returncode:
            return result.returncode
    print(f"M2 checks passed in {time.monotonic() - started:.1f}s", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
