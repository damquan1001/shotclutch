"""Local-only bounded native media qualification; requires installed ffmpeg/ffprobe oracles.
No downloads, hosted jobs, driver changes, or runtime FFmpeg dependency. Never skips a case.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import struct
import subprocess
import sys
import time
import zlib
from fractions import Fraction

ROOT = Path(__file__).resolve().parents[2]
TICKS = 254_016_000_000


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n', encoding='utf-8')


def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for block in iter(lambda: f.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def command(args, log):
    result = subprocess.run([str(x) for x in args], cwd=ROOT, capture_output=True, check=False)
    log.write_bytes(result.stdout + result.stderr)
    if result.returncode:
        raise RuntimeError(f'{args[0]} failed ({result.returncode}); see {log}')
    return result.stdout


def fixtures(out, ffmpeg, ffprobe):
    cases = []
    color = ['-color_primaries', 'bt709', '-color_trc', 'bt709', '-colorspace', 'bt709', '-color_range', 'tv']
    specs = [
        ('h264_1080_ntsc', 'mp4', '1920x1080', '24000/1001', 24,
         ['-c:v', 'libx264', '-preset', 'ultrafast', '-crf', '18', '-bf', '3', '-g', '48', '-pix_fmt', 'yuv420p'], True),
        ('hevc_1080_ntsc', 'mp4', '1920x1080', '30000/1001', 30,
         ['-c:v', 'libx265', '-preset', 'ultrafast', '-crf', '18', '-pix_fmt', 'yuv420p', '-tag:v', 'hvc1', '-x265-params', 'pools=2:frame-threads=1:log-level=error'], True),
        ('prores_sdr_422', 'mov', '640x360', '25', 25,
         ['-c:v', 'prores_ks', '-profile:v', '3', '-pix_fmt', 'yuv422p10le', '-threads:v', '1'], False),
        ('h264_vfr', 'mp4', '320x180', '30', 39,
         ['-vf', "select='not(eq(mod(n,7),3))'", '-fps_mode', 'passthrough', '-c:v', 'libx264', '-preset', 'ultrafast', '-bf', '3', '-g', '90', '-pix_fmt', 'yuv420p'], True),
    ]
    for name, ext, size, rate, count, codec, fallback in specs:
        path = out / f'{name}.{ext}'
        command([ffmpeg, '-v', 'error', '-y', '-f', 'lavfi', '-i', f'testsrc2=size={size}:rate={rate}', '-frames:v', count, *color, *codec, '-an', path], out / f'{name}.generate.log')
        cases.append(video_case(out, ffmpeg, ffprobe, name, path, fallback))
    # Hand-authored RGBA PNG: independent exact alpha/color samples, no bundled image toolkit.
    pixels = bytes(v for y in range(16) for x in range(32) for v in (x * 7, y * 13, 119, (x * 11 + y * 3) % 256))
    scan = b''.join(b'\x00' + pixels[y * 128:(y + 1) * 128] for y in range(16))
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data) & 0xffffffff)
    png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 32, 16, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(scan)) + chunk(b'IEND', b'')
    path = out / 'alpha.png'; path.write_bytes(png)
    case = video_case(out, ffmpeg, ffprobe, 'png_alpha', path, False)
    case['exactPixels'] = True; cases.append(case)
    for channels, rate in [(1, 44100), (2, 48000)]:
        name = f'pcm_{rate}_{channels}ch'
        path = out / f'{name}.wav'
        expr = '0.2*sin(2*PI*997*t)' if channels == 1 else '0.2*sin(2*PI*440*t)|0.1*cos(2*PI*660*t)'
        command([ffmpeg, '-v', 'error', '-y', '-f', 'lavfi', '-i', f'aevalsrc={expr}:s={rate}:d=1', '-c:a', 'pcm_s16le', path], out / f'{name}.generate.log')
        raw = out / f'{name}.f32'
        command([ffmpeg, '-v', 'error', '-y', '-i', path, '-map', '0:a:0', '-c:a', 'pcm_f32le', '-f', 'f32le', raw], out / f'{name}.oracle.log')
        cases.append(dict(name=name, kind='audio', file=path.name, pcm=raw.name, samples=rate, channels=channels, sampleRate=rate))
    for name, data in [('malformed_mp4', b'not an MP4\x00' * 8), ('truncated_hevc', (out / 'hevc_1080_ntsc.mp4').read_bytes()[:64])]:
        path = out / f'{name}.mp4'; path.write_bytes(data)
        cases.append(dict(name=name, kind='error', file=path.name))
    write(out / 'corpus.json', cases)
    return cases


def video_case(out, ffmpeg, ffprobe, name, path, fallback):
    raw = command([ffprobe, '-v', 'error', '-select_streams', 'v:0', '-show_entries', 'stream=time_base,width,height:frame=best_effort_timestamp', '-show_frames', '-of', 'json', path], out / f'{name}.probe.log')
    probe = json.loads(raw)
    stream = probe['streams'][0]
    clock = Fraction(stream['time_base']) * TICKS
    pts = [str(int(Fraction(int(f['best_effort_timestamp'])) * clock)) for f in probe['frames']]
    rgba = out / f'{name}.rgba'
    command([ffmpeg, '-v', 'error', '-y', '-threads', '1', '-i', path, '-map', '0:v:0', '-fps_mode', 'passthrough', '-sws_flags', 'neighbor+accurate_rnd', '-pix_fmt', 'rgba', '-threads:v', '1', '-f', 'rawvideo', rgba], out / f'{name}.oracle.log')
    return dict(name=name, kind='video', file=path.name, rgba=rgba.name, pts=pts, width=stream['width'], height=stream['height'], forceFallback=fallback)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True, help='new evidence directory')
    parser.add_argument('--no-build', action='store_true', help='use an already built native qualifier')
    args = parser.parse_args()
    out = args.out.resolve(); out.mkdir(parents=True, exist_ok=False)
    summary = dict(version=1, status='running', requestedModes=['software', 'auto', 'forced-fallback'],
                   machine=dict(os=platform.platform(), cpu=os.environ.get('PROCESSOR_IDENTIFIER'), logicalCpus=os.cpu_count()),
                   secondPc='excluded from M1 by user decision 2026-11-04')
    started = time.monotonic()
    try:
        ffmpeg, ffprobe = shutil.which('ffmpeg'), shutil.which('ffprobe')
        if not ffmpeg or not ffprobe:
            raise RuntimeError('installed ffmpeg and ffprobe are required; no case will be skipped')
        env = os.environ.copy(); env['CARGO_INCREMENTAL'] = '0'; env['CARGO_NET_OFFLINE'] = 'true'
        if not args.no_build:
            with (out / 'build.log').open('wb') as log:
                subprocess.run(['cargo', 'build', '--locked', '--offline', '-p', 'shotclutch-platform', '--example', 'm1_media'], cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        binary = ROOT / 'target' / 'debug' / 'examples' / ('m1_media.exe' if os.name == 'nt' else 'm1_media')
        summary['binarySha256'] = sha(binary)
        summary['cargoLockSha256'] = sha(ROOT / 'Cargo.lock')
        sources = ['crates/codecs/src/' + f + '.rs' for f in ['gop', 'mp4', 'mkv', 'mxf', 'mpeg', 'avi', 'apv']]
        sources += ['crates/platform/examples/m1_media.rs', 'scripts/m1/qualify-media.py']
        summary['sourceSha256'] = {p: sha(ROOT / p) for p in sources}
        summary['rustc'] = command(['rustc', '--version'], out / 'rustc.log').decode().strip()
        if os.name == 'nt':
            gpu = command(['powershell.exe', '-NoProfile', '-NonInteractive', '-Command', 'Get-CimInstance Win32_VideoController | Select-Object Name,DriverVersion | ConvertTo-Json -Compress'], out / 'gpu.log')
            summary['machine']['gpu'] = json.loads(gpu.decode('utf-8-sig'))
        summary['baseRevision'] = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
        summary['dirty'] = bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT))
        cases = fixtures(out, ffmpeg, ffprobe)
        summary['casesRequested'] = len(cases)
        summary['oracles'] = dict(ffmpeg=command([ffmpeg, '-version'], out / 'ffmpeg.log').decode().splitlines()[0], ffprobe=command([ffprobe, '-version'], out / 'ffprobe.log').decode().splitlines()[0])
        summary['files'] = {p.name: dict(bytes=p.stat().st_size, sha256=sha(p)) for p in out.iterdir() if p.is_file() and p.suffix in ['.mp4', '.mov', '.wav', '.png', '.rgba', '.f32', '.json']}
        reports = []
        for mode in summary['requestedModes']:
            path = out / f'{mode}.json'
            with (out / f'{mode}.log').open('wb') as log:
                result = subprocess.run([str(binary), str(out / 'corpus.json'), mode, str(path)], cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, check=False, timeout=300)
            report = json.loads(path.read_text(encoding='utf-8')) if path.exists() else dict(status='failed', error='no native report')
            if result.returncode or report.get('status') != 'passed' or len(report.get('cases', [])) != len(cases):
                report['status'] = 'failed'
            reports.append(dict(mode=mode, status=report['status'], exitCode=result.returncode, report=path.name, sha256=sha(path) if path.exists() else None))
        summary['runs'] = reports
        summary['status'] = 'passed' if all(r['status'] == 'passed' for r in reports) else 'failed'
    except Exception as exc:
        summary.update(status='failed', error=str(exc))
    summary['wallSeconds'] = time.monotonic() - started
    write(out / 'qualification.json', summary)
    print(json.dumps(dict(status=summary['status'], report=str(out / 'qualification.json'))))
    return 0 if summary['status'] == 'passed' else 1


if __name__ == '__main__':
    sys.exit(main())
