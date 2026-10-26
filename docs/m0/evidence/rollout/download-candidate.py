"""Download and bind a hosted qualification artifact without publishing or installing it."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import zipfile

parser = argparse.ArgumentParser()
parser.add_argument('--run', required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
out = args.output.resolve()
out.mkdir(parents=True, exist_ok=True)
if (out / 'candidate.json').exists():
    raise RuntimeError('Use a fresh output directory; completed candidate records are immutable')
gh = 'C:/Program Files/GitHub CLI/gh.exe'
prefix = 'repos/damquan1001/shotclutch/'


def api(endpoint):
    return json.loads(subprocess.check_output([gh, 'api', prefix + endpoint]))


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def extract(archive, destination):
    with zipfile.ZipFile(archive) as bundle:
        for entry in bundle.infolist():
            path = (destination / entry.filename).resolve()
            if not path.is_relative_to(destination.resolve()):
                raise RuntimeError('Archive-controlled path escaped destination')
        bundle.extractall(destination)


run = api('actions/runs/' + args.run)
artifacts = api('actions/runs/' + args.run + '/artifacts')['artifacts']
name = f'shotclutch-windows-x64-{run["head_sha"]}-{run["run_attempt"]}'
artifact, = [row for row in artifacts if row['name'] == name and not row['expired']]
archive = out / 'artifact.zip'
with archive.open('wb') as stream:
    subprocess.run([gh, 'api', prefix + f'actions/artifacts/{artifact["id"]}/zip'], stdout=stream, check=True)
expected = artifact['digest']
actual = 'sha256:' + digest(archive)
if not expected or actual != expected:
    raise RuntimeError(f'Artifact digest mismatch: expected {expected}, actual {actual}')
extract(archive, out / 'packages')
portable, = (out / 'packages').glob('*portable.zip')
extract(portable, out / 'portable')
provenance, = (out / 'portable').rglob('legal/build-info.json')
build = json.loads(provenance.read_text(encoding='utf-8-sig'))
assert build['commit'] == run['head_sha'] and build['observed_commit'] == run['head_sha'] and not build['dirty'], build
assert not list((out / 'portable').rglob('dxcompiler.dll')), 'CI-only compiler entered product payload'
summary = {'run_id': run['id'], 'attempt': run['run_attempt'], 'run_url': run['html_url'], 'revision': run['head_sha'],
           'aggregate_status_at_download': run['status'], 'aggregate_conclusion_at_download': run['conclusion'],
           'artifact': artifact, 'downloaded_archive_digest': actual, 'build_info': build, 'files': {}}
for path in list((out / 'packages').glob('*')) + list((out / 'portable').rglob('*.exe')):
    if path.is_file():
        summary['files'][str(path.relative_to(out))] = {'bytes': path.stat().st_size, 'sha256': digest(path)}
(out / 'candidate.json').write_text(json.dumps(summary, indent=2) + '\n', encoding='utf-8')
print(json.dumps(summary, indent=2))
