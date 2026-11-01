"""Exercise canonical and inherited interfaces in the compiled browser application."""
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import subprocess
import threading

root = Path(__file__).resolve().parents[2]
site = root / 'target/web/dist'
output = root / 'target/ci/evidence/web-identity'
output.mkdir(parents=True, exist_ok=True)
with ThreadingHTTPServer(('127.0.0.1', 0), partial(SimpleHTTPRequestHandler, directory=str(site))) as server:
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        with (output / 'console.log').open('w', encoding='utf-8') as log:
            subprocess.run(['node', 'apps/shotclutch-web/tests/smoke.mjs', '--identity-only',
                            '--url', f'http://127.0.0.1:{server.server_port}/?webgl',
                            '--out', str(output)], cwd=root, stdout=log, stderr=subprocess.STDOUT,
                           check=True, timeout=240)
    finally:
        server.shutdown()
        thread.join()
