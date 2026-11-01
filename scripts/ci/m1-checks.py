"""Small, headless M1 gate; full local/qualification commands remain unchanged."""
from pathlib import Path
import subprocess
import sys
import time


def main():
    root = Path(__file__).resolve().parents[2]
    commands = [
        [sys.executable, '-m', 'unittest', 'discover', '-s', 'scripts/ci', '-p', 'test_m1_checks.py'],
        ['cargo', 'fmt', '--all', '--', '--check'],
        ['cargo', 'test', '--locked', '-p', 'shotclutch-time', '-p', 'shotclutch-project',
         '-p', 'shotclutch-edit', '-p', 'shotclutch-format'],
        ['cargo', 'test', '--locked', '-p', 'shotclutch-engine', '--lib', 'm1_'],
    ]
    started = time.monotonic()
    for command in commands:
        print(' '.join(command), flush=True)
        result = subprocess.run(command, cwd=root, check=False)
        if result.returncode:
            return result.returncode
    print(f'M1 checks passed in {time.monotonic() - started:.1f}s', flush=True)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
