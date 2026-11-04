"""Prepare synthetic flash/beep media for the existing native runtime-soak.py runner.

This creates test inputs only. It never plays sound, changes devices, or claims physical sync.
Installed FFmpeg/ffprobe are local fixture/oracle tools, not runtime dependencies.
"""
import argparse
import array
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys


def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def run(args):
    result = subprocess.run([str(x) for x in args], capture_output=True, check=False)
    if result.returncode:
        raise RuntimeError(f'{args[0]} failed: {result.stderr.decode("utf-8", errors="replace")[-4000:]}')
    return result.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True, help='new fixture directory')
    parser.add_argument('--seconds', type=int, default=1800, help='observation length, 30–1800 seconds in multiples of 10')
    args = parser.parse_args()
    if not 30 <= args.seconds <= 1800 or args.seconds % 10:
        parser.error('--seconds must be 30–1800 and divisible by 10')
    ffmpeg, ffprobe = shutil.which('ffmpeg'), shutil.which('ffprobe')
    if not ffmpeg or not ffprobe:
        parser.error('installed ffmpeg and ffprobe are required; no oracle is skipped')
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    media = out / f'sync-{args.seconds}.mp4'
    manifest = dict(version=1, status='preparing', createdUtc=datetime.now(timezone.utc).isoformat(),
                    requestedObservationSeconds=args.seconds, sourceSeconds=args.seconds + 2,
                    width=1920, height=1080, frameRate='30/1', sampleRate=48000, channels=2,
                    flash='three white frames every 300 frames after a quiet first ten seconds',
                    tone='50 ms 1 kHz pulse every 480000 samples, amplitude 0.15 before stereo conversion',
                    physicalAvSync='unverified; this is only a generated timing fixture',
                    scope='A/V timing and callback underruns, not representative decoder/render throughput',
                    generatorSha256=sha(Path(__file__)))
    try:
        duration = args.seconds + 2
        # Filter quoting is for FFmpeg's parser; subprocess invokes no shell.
        video = f"color=c=black:s=1920x1080:r=30:d={duration},drawbox=c=white:t=fill:enable='gte(n,300)*lt(mod(n,300),3)'"
        audio = f"aevalsrc=exprs='if(gte(n,480000)*lt(mod(n,480000),2400),0.15*sin(2*PI*1000*n/48000),0)':s=48000:d={duration}"
        run([ffmpeg, '-v', 'error', '-nostdin', '-n', '-f', 'lavfi', '-i', video, '-f', 'lavfi', '-i', audio,
             '-map', '0:v:0', '-map', '1:a:0', '-frames:v', duration * 30,
             '-c:v', 'libx264', '-preset', 'ultrafast', '-crf', '28', '-g', '60', '-threads:v', '4',
             '-pix_fmt', 'yuv420p', '-video_track_timescale', '30000', '-color_primaries', 'bt709',
             '-color_trc', 'bt709', '-colorspace', 'bt709', '-color_range', 'tv',
             '-c:a', 'aac', '-ar', '48000', '-ac', '2', '-shortest', media])
        info = json.loads(run([ffprobe, '-v', 'error', '-show_streams', '-show_format', '-of', 'json', media]))
        v = next(s for s in info['streams'] if s['codec_type'] == 'video')
        a = next(s for s in info['streams'] if s['codec_type'] == 'audio')
        if (v['width'], v['height'], v['r_frame_rate'], int(v['nb_frames'])) != (1920, 1080, '30/1', duration * 30):
            raise RuntimeError('generated video geometry/rate/count differs from the timing plan')
        if (a['sample_rate'], a['channels']) != ('48000', 2):
            raise RuntimeError('generated audio format differs from the timing plan')
        wanted = [0, 299, 300, 302, 303, 599, 600, 602, 603]
        select = '+'.join(f'eq(n,{n})' for n in wanted)
        pixels = run([ffmpeg, '-v', 'error', '-i', media, '-an', '-vf', f"select='{select}',scale=1:1,format=gray",
                      '-frames:v', len(wanted), '-fps_mode', 'passthrough', '-f', 'rawvideo', '-'])
        expected_white = [False, False, True, True, False, False, True, True, False]
        if len(pixels) != len(wanted) or any((p < 230 if white else p > 20) for p, white in zip(pixels, expected_white)):
            raise RuntimeError('independent decoded flash boundaries do not match the frame plan')
        onsets = []
        for event in [10, 20]:
            pcm = array.array('f', run([ffmpeg, '-v', 'error', '-ss', str(event - .2), '-i', media, '-t', '0.4',
                                       '-vn', '-ac', '1', '-ar', '48000', '-f', 'f32le', '-']))
            if sys.byteorder != 'little':
                pcm.byteswap()
            first = next((i for i, sample in enumerate(pcm) if abs(sample) >= .02), None)
            if first is None or abs(first / 48000 - .2) > 1 / 30:
                raise RuntimeError('independent decoded tone onset differs by more than one video frame')
            onsets.append(event - .2 + first / 48000)
        manifest.update(status='prepared-only', media=media.name, mediaSha256=sha(media), mediaBytes=media.stat().st_size,
                        verifiedVideoFrames=wanted, decodedGray=list(pixels), referenceAudioOnsetsSeconds=onsets,
                        ffmpeg=run([ffmpeg, '-version']).decode().splitlines()[0],
                        ffprobe=run([ffprobe, '-version']).decode().splitlines()[0])
    except Exception as error:
        manifest.update(status='failed', error=str(error))
    (out / 'fixture.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(dict(status=manifest['status'], manifest=str(out / 'fixture.json'))))
    return 0 if manifest['status'] == 'prepared-only' else 1


if __name__ == '__main__':
    raise SystemExit(main())
