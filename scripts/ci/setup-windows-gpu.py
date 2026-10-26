"""Pin the DXC test toolchain; never add compiler DLLs to the product payload."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import urllib.request
import zipfile


def install(root):
    if os.name != 'nt':
        raise RuntimeError('This compiler profile is Windows x64 only')
    locked = json.loads((Path(__file__).parent / 'windows-gpu.lock.json').read_text())
    destination = root / 'target/ci/dxc'
    destination.mkdir(parents=True, exist_ok=True)
    archive = destination / 'dxc.zip'
    if not archive.exists() or digest(archive) != locked['sha256']:
        temporary = archive.with_suffix('.download')
        with urllib.request.urlopen(locked['url'], timeout=120) as source, temporary.open('wb') as output:
            shutil.copyfileobj(source, output)
        if digest(temporary) != locked['sha256']:
            raise RuntimeError('DXC archive checksum mismatch')
        temporary.replace(archive)
    binaries = destination / 'bin'
    binaries.mkdir(exist_ok=True)
    files = {}
    with zipfile.ZipFile(archive) as bundle:
        for name in ['dxc.exe', 'dxcompiler.dll', 'dxil.dll']:
            member = 'bin/x64/' + name
            with bundle.open(member) as source, (binaries / name).open('wb') as output:
                shutil.copyfileobj(source, output)
            files[name] = digest(binaries / name)
    version = subprocess.check_output([str(binaries / 'dxc.exe'), '--version'], text=True).strip()
    if locked['binary_version'] not in version:
        raise RuntimeError(f'Unexpected DXC version: {version}')
    # Compile a real shader before exposing this directory to test executables.
    shader = destination / 'probe.hlsl'
    shader.write_text('[numthreads(1,1,1)] void main() {}\n')
    subprocess.run([str(binaries / 'dxc.exe'), '-T', 'cs_6_0', '-E', 'main', str(shader), '-Fo', str(destination / 'probe.dxil')], check=True)
    manifest = {**locked, 'reported_version': version, 'files_sha256': files,
                'scope': 'CI test-only; wgpu Auto selects dynamic DXC from PATH; hosted adapter is not physical GPU qualification'}
    (destination / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    if os.environ.get('GITHUB_PATH'):
        with open(os.environ['GITHUB_PATH'], 'a', encoding='utf-8') as output:
            output.write(str(binaries) + '\n')
    print(json.dumps(manifest, indent=2))


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


if __name__ == '__main__':
    install(Path(__file__).resolve().parents[2])
