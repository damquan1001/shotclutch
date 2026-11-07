"""Headless Smart Cut qualification. Private footage and credentials are optional local checks."""
from pathlib import Path
import subprocess
import sys
import time


def main():
    root = Path(__file__).resolve().parents[2]
    commands = [
        ["cargo", "test", "--locked", "-p", "shotclutch-engine", "--lib", "m2_"],
        ["cargo", "test", "--locked", "-p", "shotclutch-codecs", "--lib",
         "edit_boundaries_handle_reordered_and_repeated_timestamps"],
        ["cargo", "test", "--locked", "-p", "shotclutch-speech", "--features", "download",
         "--lib", "assemblyai"],
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
