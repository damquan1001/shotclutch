"""Reproduce bounded native lifecycle observations against a downloaded CI candidate."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import shutil
import socket
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('--bin-dir', type=Path, required=True)
parser.add_argument('--short-fixture', type=Path, required=True)
parser.add_argument('--long-fixture', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
out = args.output.resolve()
if out.exists():
    raise RuntimeError('Use a fresh output directory')
out.mkdir(parents=True)
binary = args.bin_dir.resolve() / 'shotclutch.exe'
source = out / 'source.mp4'
shutil.copyfile(args.short_fixture, source)
report = {'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'result': 'failed', 'commands': [], 'claims': []}
provenance = binary.parent / 'legal/build-info.json'
report['build_info'] = json.loads(provenance.read_text(encoding='utf-8-sig'))
process = None
log = (out / 'desktop.log').open('w', encoding='utf-8')


def launch():
    global process, port, token
    with socket.socket() as reservation:
        reservation.bind(('127.0.0.1', 0))
        port = reservation.getsockname()[1]
    token = secrets.token_hex(32)
    environment = dict(os.environ, SHOTCLUTCH_CONTROL_TOKEN=token, SHOTCLUTCH_CONTROL_PORT=str(port),
                       PATH=os.pathsep.join([str(binary.parent), os.path.join(os.environ['SystemRoot'], 'System32')]))
    for name in ['FILMCRAFT_FFMPEG', 'SHOTCLUTCH_FFMPEG', 'FILMCRAFT_FFPROBE', 'SHOTCLUTCH_FFPROBE']:
        environment.pop(name, None)
    process = subprocess.Popen([str(binary), '--empty', '--no-recover', '--data-dir', str(out / 'data')], env=environment, stdout=log, stderr=log)
    deadline = time.monotonic() + 90
    while True:
        try:
            call('ui.inspect')
            break
        except OSError:
            if time.monotonic() >= deadline or process.poll() is not None:
                raise RuntimeError('Native startup failed')
            time.sleep(.1)


def call(method, params=None):
    with socket.create_connection(('127.0.0.1', port), timeout=10) as connection:
        connection.settimeout(60)
        connection.sendall((json.dumps({'token': token, 'method': method, 'params': params or {}}) + '\n').encode())
        response = json.loads(connection.makefile('rb').readline())
    if not response['ok']:
        raise RuntimeError(f'{method}: {response}')
    return response['result']


def execute(command, params=None):
    result = call('engine.execute', {'command': command, 'params': params or {}})
    report['commands'].append({'command': command, 'params': params or {}, 'result': result})
    return result


def quit():
    call('app.quit')
    process.wait(timeout=30)


try:
    launch()
    imported = execute('file.import', {'paths': [str(source)]})
    assert not imported['errors'], imported
    item = imported['items'][0]
    execute('file.newSequenceFromClip', {'items': [item]})
    sequence = execute('sequence.inspect')
    clip = sequence['video'][0]['items'][0]['clip']
    for index in range(50):
        execute('timeline.trim', {'clip': clip, 'edge': 'out', 'mode': 'ripple', 'deltaFrames': -1})
        assert execute('sequence.inspect')['durationFrames'] == 59
        assert execute('edit.undo')['undone']
        assert execute('sequence.inspect')['durationFrames'] == 60
        assert execute('edit.redo')['redone']
        assert execute('sequence.inspect')['durationFrames'] == 59
        assert execute('edit.undo')['undone']
    project = out / 'lifecycle.fcproj'
    execute('file.saveAs', {'path': str(project)})
    quit()
    moved = out / 'moved.mp4'
    source.replace(moved)  # Only this harness-owned copied fixture is moved.
    launch()
    opened = execute('file.open', {'path': str(project)})
    assert opened['missingMedia'] == 1, opened
    missing = execute('media.findMissing')
    assert len(missing['missing']) == 1, missing
    linked = execute('media.relink', {'item': item, 'path': str(moved), 'match': {'fileName': False}, 'relinkOthers': False})
    assert not linked['remaining'], linked
    assert not execute('media.findMissing')['missing']
    execute('file.saveAs', {'path': str(out / 'relinked.fcproj')})
    assert execute('file.open', {'path': str(out / 'relinked.fcproj')})['missingMedia'] == 0
    assert execute('sequence.inspect')['durationFrames'] == 60
    report['claims'].append('50 trim/undo/redo/undo cycles preserved exact duration; saved project reopened offline then relinked and reopened online')
    imported = execute('file.import', {'paths': [str(args.long_fixture.resolve())]})
    assert not imported['errors'], imported
    execute('file.newSequenceFromClip', {'items': imported['items']})
    long_project = out / 'long-form.fcproj'
    execute('file.saveAs', {'path': str(long_project)})
    assert execute('file.open', {'path': str(long_project)})['missingMedia'] == 0
    long_sequence = execute('sequence.inspect')
    assert long_sequence['durationFrames'] == 54060, long_sequence
    rendered = out / 'cancelled.mov'
    job = execute('file.exportMedia', {'path': str(rendered), 'format': 'prores', 'range': 'entire', 'wait': False})['job']
    time.sleep(.2)
    before = execute('jobs.list')
    assert not next(row for row in before if row['id'] == job)['finished'], before
    started = time.monotonic()
    execute('jobs.cancel', {'job': job})
    while True:
        jobs = execute('jobs.list')
        row = next(row for row in jobs if row['id'] == job)
        if row['finished']:
            assert row['result'].get('error') == 'cancelled', row
            break
        if time.monotonic() - started > 30:
            raise RuntimeError('Cancellation exceeded 30 seconds')
        time.sleep(.05)
    report['cancel_seconds'] = time.monotonic() - started
    report['cancelled_output_exists'] = rendered.exists()
    assert not rendered.exists(), 'Cancelled export left its partial output'
    report['claims'].append('1802-second project saved/reopened at 54060 frames; active export cancelled and partial output removed')
    report['final_perf'] = call('perf.stats')
    quit()
    report['result'] = 'passed'
except Exception as error:
    report['error'] = str(error)
    raise
finally:
    if process and process.poll() is None:
        process.terminate()
        process.wait(timeout=10)
    log.close()
    (out / 'summary.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
print('Native lifecycle passed:', out)
