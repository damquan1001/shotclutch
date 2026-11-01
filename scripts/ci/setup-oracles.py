"""Install checksum-locked test oracles in the workspace, not in the application."""
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import urllib.request
import zipfile


def sha256(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def install(root):
    machine = platform.machine().lower()
    machine = {'amd64': 'x86_64', 'aarch64': 'arm64'}.get(machine, machine)
    key = f'{platform.system().lower()}-{machine}'
    locked = json.loads((Path(__file__).parent / 'oracles.lock.json').read_text())[key]
    destination = root / 'target/ci/oracles'
    destination.mkdir(parents=True, exist_ok=True)
    binaries = destination / 'bin'
    binaries.mkdir(exist_ok=True)
    evidence = {'platform': key, 'locked_version': locked['version'], 'publisher': locked['publisher'], 'tools': {}}
    environment = {'SHOTCLUTCH_REQUIRE_ORACLES': '1'}
    for tool, asset in locked['tools'].items():
        archive = destination / (asset['sha256'] + '.zip')
        if not archive.exists() or sha256(archive) != asset['sha256']:
            temporary = archive.with_suffix('.download')
            request = urllib.request.Request(asset['url'], headers={'User-Agent': 'ShotClutch-CI'})
            with urllib.request.urlopen(request, timeout=120) as response, temporary.open('wb') as output:
                shutil.copyfileobj(response, output)
            if sha256(temporary) != asset['sha256']:
                raise RuntimeError(f'Checksum mismatch: {asset["url"]}')
            temporary.replace(archive)
        name = tool + ('.exe' if os.name == 'nt' else '')
        executable = binaries / name
        # Extract only the selected executable, avoiding archive-controlled paths.
        with zipfile.ZipFile(archive) as bundle:
            candidates = [entry for entry in bundle.namelist() if entry.split('/')[-1] == name]
            if len(candidates) != 1:
                raise RuntimeError(f'Expected one {name}, found {candidates}')
            with bundle.open(candidates[0]) as source, executable.open('wb') as output:
                shutil.copyfileobj(source, output)
        executable.chmod(0o755)
        version = subprocess.check_output([str(executable), '-version'], text=True).splitlines()[0]
        if not version.startswith(f'{tool} version {locked["version"]}'):
            raise RuntimeError(f'Unexpected tool version: {version}')
        evidence['tools'][tool] = {**asset, 'version': version, 'executable_sha256': sha256(executable)}
        environment['SHOTCLUTCH_' + tool.upper()] = str(executable)
        environment['SHOTCLUTCH_' + tool.upper()] = str(executable)
    encoders = subprocess.check_output([str(binaries / ('ffmpeg.exe' if os.name == 'nt' else 'ffmpeg')), '-hide_banner', '-encoders'], text=True)
    names = {parts[1] for line in encoders.splitlines() if len(parts := line.split()) >= 2}
    required = {'libx264', 'libx265', 'libvpx-vp9', 'libaom-av1', 'aac', 'prores_ks', 'mpeg2video', 'mp2'}
    if missing := required - names:
        raise RuntimeError(f'Oracle bundle lacks required fixture encoders: {sorted(missing)}')
    evidence['verified_encoders'] = sorted(required)
    (destination / 'manifest.json').write_text(json.dumps(evidence, indent=2) + '\n')
    if os.environ.get('GITHUB_ENV'):
        with open(os.environ['GITHUB_ENV'], 'a', encoding='utf-8') as output:
            for key, value in environment.items():
                output.write(f'{key}={value}\n')
        with open(os.environ['GITHUB_PATH'], 'a', encoding='utf-8') as output:
            output.write(str(binaries) + '\n')
    print(json.dumps(evidence, indent=2))


if __name__ == '__main__':
    install(Path(__file__).resolve().parents[2])
