"""Run CI commands with durable diagnostics and their actual exit status."""
import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import threading
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('lane', choices=['checks', 'build', 'package', 'gpu-probe'])
    options = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    output = root / 'target/ci/evidence' / options.lane
    output.mkdir(parents=True, exist_ok=True)
    commands = {
        'gpu-probe': [['cargo', 'test', '--locked', '-p', 'filmcraft-gpu', '--lib', 'qualification_adapter', '--', '--nocapture']],
        'checks': [
            ['cargo', 'fmt', '--all', '--', '--check'], ['cargo', 'xtask', 'layers'], ['cargo', 'xtask', 'assets'],
            ['cargo', 'clippy', '--locked', '--workspace', '--all-targets', '--', '-D', 'warnings'],
            ['cargo', 'test', '--locked', '--workspace', '--lib'],
            ['cargo', 'test', '--locked', '-p', 'filmcraft', '--bin', 'shotclutch'],
            ['cargo', 'test', '--locked', '-p', 'filmcraft-cli', '--bin', 'shotclutch-cli'],
            ['cargo', 'test', '--locked', '-p', 'filmcraft-format', '--test', 'fixtures'],
            ['cargo', 'test', '--locked', '-p', 'filmcraft-cli', '--test', 'it'],
            ['cargo', 'test', '--locked', '-p', 'filmcraft-ui-egui', '--example', 'bench'],
            ['cargo', 'test', '--locked', '--workspace', '--doc'],
        ],
        'build': [['cargo', 'build', '--locked', '--release', '-p', 'filmcraft', '-p', 'filmcraft-cli']],
        'package': [['pwsh', '-NoProfile', '-File', 'packaging/windows/package.ps1', '-Arch', 'x64'],
                    ['pwsh', '-NoProfile', '-File', 'scripts/m0/validate-package.ps1', '-Dist', 'dist/release']],
    }[options.lane]
    if options.lane == 'build' and platform.system() == 'Linux':
        commands += [['rustup', 'target', 'add', 'wasm32-unknown-unknown'], ['cargo', 'xtask', 'wasm']]
    summary = {
        'revision': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
        'os': platform.platform(), 'architecture': platform.machine(), 'cpu': platform.processor(),
        'logical_cores': os.cpu_count(), 'physical_memory_bytes': None,
        'image_version': os.environ.get('ImageVersion'), 'image_os': os.environ.get('ImageOS'),
        'run_id': os.environ.get('GITHUB_RUN_ID'), 'attempt': os.environ.get('GITHUB_RUN_ATTEMPT'),
        'dependency_cache_hit': os.environ.get('CARGO_CACHE_HIT'), 'compiled_target_cache': 'disabled',
        'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip(),
        'cargo_lock_sha256': __import__('hashlib').sha256((root / 'Cargo.lock').read_bytes()).hexdigest(),
        'min_free_disk_bytes': shutil.disk_usage(root).free,
        'profile_overrides': {k: v for k, v in os.environ.items() if k.startswith('CARGO_PROFILE_') or k in ['CARGO_INCREMENTAL', 'CARGO_BUILD_JOBS']},
        'gpu_required': os.environ.get('SHOTCLUTCH_REQUIRE_GPU') == '1',
        'commands': [], 'result': 'failed',
    }
    stop = threading.Event()

    def sample():
        while not stop.wait(5):
            summary['min_free_disk_bytes'] = min(summary['min_free_disk_bytes'], shutil.disk_usage(root).free)

    monitor = threading.Thread(target=sample, daemon=True)
    monitor.start()
    try:
        for index, command in enumerate(commands, 1):
            record = {'command': command, 'log': f'{index:02}.log', 'exit_code': None}
            summary['commands'].append(record)
            started = time.monotonic()
            print('==> ' + ' '.join(command), flush=True)
            with (output / record['log']).open('w', encoding='utf-8') as log:
                process = subprocess.Popen(command, cwd=root, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, encoding='utf-8', errors='replace')
                for line in process.stdout:
                    print(line, end='', flush=True)
                    log.write(line)
                    log.flush()
                record['exit_code'] = process.wait()
            record['wall_seconds'] = time.monotonic() - started
            if record['exit_code']:
                raise SystemExit(record['exit_code'])
        summary['result'] = 'passed'
    finally:
        stop.set()
        monitor.join()
        (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
