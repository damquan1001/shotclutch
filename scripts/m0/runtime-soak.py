"""Observe an owned native desktop session; preserve raw counters without inferring physical A/V sync."""
import argparse
import ctypes
from ctypes import wintypes
import hashlib
import json
import os
from pathlib import Path
import secrets
import shutil
import socket
import subprocess
import time


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def memory(process):
    class Counters(ctypes.Structure):
        _fields_ = [('cb', wintypes.DWORD), ('faults', wintypes.DWORD)] + [(name, ctypes.c_size_t) for name in
            ['peakWorkingSet', 'workingSet', 'peakPaged', 'paged', 'peakNonPaged', 'nonPaged', 'pagefile', 'peakPagefile', 'private']]
    value = Counters()
    value.cb = ctypes.sizeof(value)
    query = ctypes.WinDLL('psapi').GetProcessMemoryInfo
    query.argtypes = [wintypes.HANDLE, ctypes.POINTER(Counters), wintypes.DWORD]
    query.restype = wintypes.BOOL
    if not query(wintypes.HANDLE(int(process._handle)), ctypes.byref(value), value.cb):
        return None
    return {'working_set_bytes': value.workingSet, 'peak_working_set_bytes': value.peakWorkingSet, 'private_bytes': value.private}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('mode', choices=['prepare', 'run'])
    parser.add_argument('--fixture', type=Path, required=True)
    parser.add_argument('--seconds', type=int, default=1800)
    parser.add_argument('--bin-dir', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--resolution', default='Full')
    args = parser.parse_args()
    fixture = args.fixture.resolve()
    if args.seconds < 1:
        parser.error('--seconds must be positive')
    if args.mode == 'prepare':
        fixture.parent.mkdir(parents=True, exist_ok=True)
        if fixture.exists():
            raise RuntimeError('Refusing to overwrite an existing qualification fixture')
        ffmpeg = os.environ.get('SHOTCLUTCH_FFMPEG', 'ffmpeg')
        # Brief quiet markers every minute; fixture creation is outside measured playback.
        subprocess.run([ffmpeg, '-v', 'error', '-f', 'lavfi', '-i', f'testsrc2=size=1920x1080:rate=30:duration={args.seconds + 2}',
            '-f', 'lavfi', '-i', f"aevalsrc=exprs='if(lt(mod(t,60),0.05),0.01*sin(2*PI*1000*t),0)':s=48000:d={args.seconds + 2}",
            '-c:v', 'libx264', '-preset', 'ultrafast', '-crf', '28', '-g', '60', '-threads', '4',
            '-c:a', 'aac', '-ac', '2', '-shortest', str(fixture)], check=True)
        fixture.with_suffix('.json').write_text(json.dumps({'sha256': digest(fixture), 'duration_seconds': args.seconds + 2,
            'width': 1920, 'height': 1080, 'frame_rate': '30/1', 'sample_rate': 48000,
            'audio': 'stereo; quiet 50 ms 1 kHz marker every 60 seconds', 'generator': subprocess.check_output([ffmpeg, '-version'], text=True).splitlines()[0]}, indent=2) + '\n')
        return
    if not args.bin_dir or not args.output or os.name != 'nt':
        parser.error('run requires Windows, --bin-dir and --output')
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    if (output / 'summary.json').exists():
        raise RuntimeError('Refusing to overwrite a previous runtime report')
    binary = args.bin_dir.resolve() / 'shotclutch.exe'
    with socket.socket() as reservation:
        reservation.bind(('127.0.0.1', 0))
        port = reservation.getsockname()[1]
    token = secrets.token_hex(32)
    environment = dict(os.environ, SHOTCLUTCH_CONTROL_TOKEN=token, SHOTCLUTCH_CONTROL_PORT=str(port),
        RUST_LOG='warn,shotclutch*=info,shotclutch*=info,wgpu_core=info')
    log = (output / 'desktop-stderr.log').open('w', encoding='utf-8')
    process = subprocess.Popen([str(binary), '--empty', '--no-recover', '--data-dir', str(output / 'data')], env=environment, stdout=log, stderr=log)
    summary = {'binary_sha256': digest(binary), 'fixture_sha256': digest(fixture), 'requested_seconds': args.seconds,
        'result': 'failed', 'physical_av_drift': None, 'physical_av_drift_note': 'No loopback capture; application clock/monitor counters only',
        'adapter': None, 'requested_resolution': args.resolution, 'samples': 0,
        'gpu_device_metrics_scope': 'Device-wide NVIDIA metrics include other applications; not per-process GPU allocation'}
    provenance = binary.parent / 'legal/build-info.json'
    summary['build_info'] = json.loads(provenance.read_text(encoding='utf-8-sig')) if provenance.exists() else None
    nvidia = shutil.which('nvidia-smi')

    def call(method, params=None):
        with socket.create_connection(('127.0.0.1', port), timeout=10) as connection:
            connection.settimeout(30)
            connection.sendall((json.dumps({'token': token, 'method': method, 'params': params or {}}) + '\n').encode())
            response = json.loads(connection.makefile('rb').readline())
        if not response.get('ok'):
            raise RuntimeError(f'{method}: {response}')
        return response.get('result')

    def execute(command, params=None):
        return call('engine.execute', {'command': command, 'params': params or {}})

    try:
        deadline = time.monotonic() + 90
        while True:
            if process.poll() is not None:
                raise RuntimeError('Desktop exited before control startup')
            try:
                call('ui.inspect')
                break
            except (OSError, RuntimeError):
                if time.monotonic() > deadline:
                    raise RuntimeError('Desktop control startup timed out')
                time.sleep(0.25)
        imported = execute('file.import', {'paths': [str(fixture)]})
        assert not imported['errors'], imported
        execute('file.newSequenceFromClip', {'items': imported['items']})
        execute('file.saveAs', {'path': str(output / 'soak.scproj')})
        call('ui.set', {'playbackRes': args.resolution})
        call('ui.playback', {'action': 'play'})
        start_wait = time.monotonic()
        while True:
            state = call('ui.inspect')
            if state['playback']['playing'] and not state['playback']['preroll']:
                break
            if time.monotonic() - start_wait > 30:
                raise RuntimeError('Playback did not leave preroll')
            time.sleep(0.1)
        started = time.monotonic()
        first = None
        with (output / 'samples.jsonl').open('w', encoding='utf-8') as samples:
            while True:
                elapsed = time.monotonic() - started
                state = call('ui.inspect')
                stats = call('perf.stats')
                row = {'elapsed_seconds': elapsed, 'playhead_seconds': state['playhead'] / 254016000000,
                    'audio_clock': state['playback']['audioClock'], 'playback': stats.get('playback'),
                    'monitor': stats.get('monitor'), 'gpu': stats.get('gpu'), 'decode': stats.get('decode'),
                    'ui': stats.get('ui'), 'process': stats.get('process'), 'memory': memory(process)}
                if nvidia and summary['samples'] % 6 == 0:
                    try:
                        row['gpu_device_snapshot'] = subprocess.check_output([nvidia,
                            '--query-gpu=name,driver_version,memory.total,memory.used,utilization.gpu,utilization.decoder,temperature.gpu,power.draw',
                            '--format=csv,noheader,nounits'], text=True, timeout=10).strip()
                    except (OSError, subprocess.SubprocessError) as error:
                        row['gpu_device_snapshot_unavailable'] = str(error)
                samples.write(json.dumps(row) + '\n')
                samples.flush()
                first = first or row
                summary['samples'] += 1
                summary['last'] = row
                if not state['playback']['playing'] and elapsed < args.seconds - 1:
                    raise RuntimeError('Playback stopped early')
                if elapsed >= args.seconds:
                    break
                time.sleep(min(5, args.seconds - elapsed))
        summary['first'] = first
        summary['wall_seconds'] = time.monotonic() - started
        summary['clock_progress_minus_wall_seconds'] = (row['playhead_seconds'] - first['playhead_seconds']) - (row['elapsed_seconds'] - first['elapsed_seconds'])
        summary['result'] = 'completed; evaluate counters and declared targets separately'
        call('ui.playback', {'action': 'stop'})
        call('app.quit')
        process.wait(timeout=30)
    except Exception as error:
        summary['error'] = str(error)
        raise
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=10)
        log.close()
        app_log = output / 'data/Logs/shotclutch.log'
        if app_log.exists():
            adapter_lines = [line for line in app_log.read_text(encoding='utf-8', errors='replace').splitlines() if 'graphics adapter chosen:' in line or 'AdapterInfo' in line]
            summary['adapter'] = adapter_lines or None
        (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n', encoding='utf-8')
    print(f'Runtime observation completed: {output}')


if __name__ == '__main__':
    main()
