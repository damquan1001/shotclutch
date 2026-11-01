"""Separate fixture generation, tool-free application execution, and independent verification."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def oracle(name):
    return os.environ.get('SHOTCLUTCH_' + name.upper(), name)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('mode', choices=['prepare', 'exercise', 'verify'])
    parser.add_argument('--fixtures', type=Path, required=True)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--bin-dir', type=Path)
    args = parser.parse_args()
    fixtures = args.fixtures.resolve()
    fixtures.mkdir(parents=True, exist_ok=True)
    source = fixtures / 'source.mp4'
    if args.mode == 'prepare':
        subprocess.run([oracle('ffmpeg'), '-v', 'error', '-y', '-f', 'lavfi', '-i',
                        'testsrc2=size=320x180:rate=30:duration=2', '-f', 'lavfi', '-i',
                        'sine=frequency=440:sample_rate=48000:duration=2', '-c:v', 'libx264',
                        '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-ac', '2', str(source)], check=True)
        (fixtures / 'manifest.json').write_text(json.dumps({'source_sha256': sha(source), 'source_frames': 60,
            'expected_trimmed_frames': 50, 'expected_stereo_sample_frames': 80000, 'sample_rate': 48000,
            'fixture': 'generated testsrc2 and sine; no third-party media'}, indent=2) + '\n')
        return
    manifest = json.loads((fixtures / 'manifest.json').read_text())
    if sha(source) != manifest['source_sha256']:
        raise RuntimeError('Fixture hash differs')
    if not args.output:
        parser.error('--output is required')
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    project = output / 'smoke.scproj'
    rendered = output / 'render.mov'
    if args.mode == 'exercise':
        if not args.bin_dir:
            parser.error('--bin-dir is required')
        binary = args.bin_dir.resolve() / ('shotclutch-cli.exe' if os.name == 'nt' else 'shotclutch-cli')
        child_environment = dict(os.environ)
        for key in ['SHOTCLUTCH_FFMPEG', 'SHOTCLUTCH_FFMPEG', 'SHOTCLUTCH_FFPROBE', 'SHOTCLUTCH_FFPROBE']:
            child_environment.pop(key, None)
        # Oracles remain usable by the parent verification step, but not by the application child.
        child_environment['PATH'] = os.pathsep.join([str(binary.parent), os.path.join(os.environ['SystemRoot'], 'System32')]) if os.name == 'nt' else str(binary.parent)
        records = []

        def cli(arguments):
            command = [str(binary), '--data-dir', str(output / 'data'), *arguments]
            result = subprocess.run(command, env=child_environment, capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=180)
            records.append({'arguments': arguments, 'exit_code': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr})
            if result.returncode:
                raise RuntimeError(f'Packaged CLI failed: {arguments}: {result.stderr}')
            return json.loads(result.stdout)

        try:
            imported = cli(['--save-as', str(project), 'import', str(source)])
            assert not imported['errors'], imported
            cli(['--project', str(project), '--save-as', str(project), 'exec', 'file.newSequenceFromClip', json.dumps({'items': imported['items']})])
            sequence = cli(['--project', str(project), 'inspect', 'sequence'])
            clip = sequence['video'][0]['items'][0]['clip']
            script = output / 'edit.jsonl'
            commands = [
                {'id': 'timeline.trim', 'params': {'clip': clip, 'edge': 'out', 'mode': 'ripple', 'deltaFrames': -10}},
                {'id': 'sequence.inspect'}, {'id': 'edit.undo'}, {'id': 'sequence.inspect'},
                {'id': 'edit.redo'}, {'id': 'sequence.inspect'},
            ]
            script.write_text('\n'.join(json.dumps(command) for command in commands) + '\n')
            command = [str(binary), '--data-dir', str(output / 'data'), '--project', str(project), '--save-as', str(project), 'run', str(script)]
            run = subprocess.run(command, env=child_environment, capture_output=True, text=True, encoding='utf-8', timeout=180)
            records.append({'operation': 'trim-undo-redo', 'exit_code': run.returncode, 'stdout': run.stdout, 'stderr': run.stderr})
            assert run.returncode == 0, run.stderr
            rows = [json.loads(line) for line in run.stdout.splitlines()]
            assert all(row['ok'] for row in rows), rows
            assert [rows[i]['result']['durationFrames'] for i in [1, 3, 5]] == [50, 60, 50], rows
            reopened = cli(['--project', str(project), 'inspect', 'sequence'])
            assert reopened['durationFrames'] == 50, reopened
            cli(['--project', str(project), 'export', str(rendered), '--format', 'prores', '--range', 'entire', '--gpu-rendering', 'off'])
            (output / 'application-result.json').write_text(json.dumps({'result': 'passed', 'cli_sha256': sha(binary),
                'render_sha256': sha(rendered), 'oracle_tools_on_child_path': False, 'source_sha256': sha(source),
                'expected_frames': 50, 'expected_stereo_sample_frames': 80000}, indent=2) + '\n')
        finally:
            (output / 'application-commands.json').write_text(json.dumps(records, indent=2) + '\n')
    else:
        probe = json.loads(subprocess.check_output([oracle('ffprobe'), '-v', 'error', '-count_frames', '-show_streams', '-of', 'json', str(rendered)]))
        video = next(stream for stream in probe['streams'] if stream['codec_type'] == 'video')
        pcm = subprocess.check_output([oracle('ffmpeg'), '-v', 'error', '-i', str(rendered), '-map', '0:a:0', '-f', 'f32le', '-ac', '2', '-ar', '48000', '-'])
        assert int(video['nb_read_frames']) == manifest['expected_trimmed_frames'], video
        assert len(pcm) // 8 == manifest['expected_stereo_sample_frames'], len(pcm) // 8
        (output / 'verification.json').write_text(json.dumps({'result': 'passed', 'frames': int(video['nb_read_frames']),
            'stereo_sample_frames': len(pcm) // 8, 'sample_rate': 48000, 'render_sha256': sha(rendered), 'probe': probe}, indent=2) + '\n')
    print(f'Packaged workflow {args.mode}: passed')


if __name__ == '__main__':
    main()
