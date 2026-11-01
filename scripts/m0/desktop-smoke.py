"""Exercise the real desktop host through its authenticated local command bridge.

Run from the repository root with Python 3, FFmpeg and ffprobe on PATH.
The token is private to the child environment and is never persisted.
"""
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


def main():
    root = Path(__file__).resolve().parents[2]
    parser = argparse.ArgumentParser()
    parser.add_argument("--bin-dir", type=Path, default=root / "target/release")
    parser.add_argument("--output", type=Path, default=root / "target/m0/desktop-smoke")
    options = parser.parse_args()
    out = options.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    source = out / "source.mp4"
    subprocess.run([
        "ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i",
        "testsrc2=size=1920x1080:rate=30:duration=2", "-f", "lavfi", "-i",
        "sine=frequency=440:sample_rate=48000:duration=2", "-c:v", "libx264",
        "-pix_fmt", "yuv420p", "-c:a", "aac", "-ac", "2", str(source),
    ], check=True)
    with socket.socket() as reservation:
        reservation.bind(("127.0.0.1", 0))
        port = reservation.getsockname()[1]
    token = secrets.token_hex(32)
    env = dict(os.environ, SHOTCLUTCH_CONTROL_TOKEN=token,
               FILMCRAFT_CONTROL_TOKEN="0" * 64,
               SHOTCLUTCH_CONTROL_PORT=str(port), FILMCRAFT_CONTROL_PORT="invalid")
    for prefix in ['SHOTCLUTCH_', 'FILMCRAFT_']:
        for tool in ['FFMPEG', 'FFPROBE']:
            env.pop(prefix + tool, None)
    # Only the test harness uses media oracles; the packaged editor must stand alone.
    env["PATH"] = os.pathsep.join([str(options.bin_dir.resolve()), str(Path(os.environ["SystemRoot"]) / "System32")])
    assert shutil.which("ffmpeg", path=env["PATH"]) is None
    startup = subprocess.STARTUPINFO()
    startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
    startup.wShowWindow = subprocess.SW_HIDE
    log = (out / "desktop.log").open("w", encoding="utf-8")
    process = subprocess.Popen([
        str(options.bin_dir.resolve() / "shotclutch.exe"), "--empty", "--no-recover",
        "--data-dir", str(out / "data"),
    ], env=env, startupinfo=startup, stdout=log, stderr=log)
    evidence = []

    def call(method, params=None):
        with socket.create_connection(("127.0.0.1", port), timeout=10) as connection:
            connection.settimeout(60)
            connection.sendall((json.dumps({"token": token, "method": method,
                                           "params": params or {}}) + "\n").encode())
            response = json.loads(connection.makefile("rb").readline())
        if not response.get("ok"):
            raise RuntimeError(f"{method}: {response}")
        return response.get("result")

    def execute(command, params=None):
        result = call("engine.execute", {"command": command, "params": params or {}})
        evidence.append({"command": command, "result": result})
        return result

    try:
        deadline = time.monotonic() + 90
        while True:
            if process.poll() is not None:
                raise RuntimeError("Desktop exited before bridge startup; see desktop.log")
            try:
                state = call("ui.inspect")
                break
            except (OSError, RuntimeError):
                if time.monotonic() > deadline:
                    raise RuntimeError("Desktop startup timed out; see desktop.log")
                time.sleep(0.25)
        (out / "ui-state.json").write_text(json.dumps(state, indent=2), encoding="utf-8")
        imported = execute("file.import", {"paths": [str(source)]})
        assert not imported.get("errors"), imported
        execute("file.newSequenceFromClip", {"items": [imported["items"][0]]})
        sequence = execute("sequence.inspect")
        clip = sequence["video"][0]["items"][0]["clip"]
        execute("timeline.trim", {"clip": clip, "edge": "out", "mode": "ripple", "deltaFrames": -10})
        assert execute("sequence.inspect")["durationFrames"] == 50
        execute("edit.undo")
        assert execute("sequence.inspect")["durationFrames"] == 60
        execute("edit.redo")
        project = out / "smoke.scproj"
        execute("file.saveAs", {"path": str(project)})
        saved = project.read_bytes()
        envelope = json.loads(saved)
        assert envelope["format"] == "shotclutch.project"
        assert envelope["schema_version"] == 15
        execute("file.open", {"path": str(project)})
        assert execute("sequence.inspect")["durationFrames"] == 50
        rendered = out / "render.mov"
        started = execute("file.exportMedia", {"path": str(rendered), "format": "prores", "range": "entire", "wait": False})
        deadline = time.monotonic() + 180
        while True:
            jobs = execute("jobs.list")
            job = next(j for j in jobs if j["id"] == started["job"])
            if job["finished"]:
                assert not job["result"].get("error"), job
                assert job["result"]["frames"] == 50, job
                break
            if time.monotonic() > deadline:
                raise RuntimeError("Export timed out")
            time.sleep(0.25)
        probe = json.loads(subprocess.check_output([
            "ffprobe", "-v", "error", "-count_frames", "-show_streams", "-show_format", "-of", "json", str(rendered),
        ]))
        video = next(s for s in probe["streams"] if s["codec_type"] == "video")
        assert int(video["nb_read_frames"]) == 50, video
        pcm = subprocess.check_output(["ffmpeg", "-v", "error", "-i", str(rendered),
                                       "-map", "0:a:0", "-f", "f32le", "-ac", "2", "-ar", "48000", "-"])
        assert len(pcm) // 8 == 80000, len(pcm) // 8
        evidence.append({"verification": "passed", "frames": 50, "stereo_samples": 80000,
                         "fixture_size": "1920x1080", "project_format": envelope["format"],
                         "project_sha256": hashlib.sha256(saved).hexdigest(),
                         "fixture_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
                         "render_sha256": hashlib.sha256(rendered.read_bytes()).hexdigest(),
                         "export_format_tags": probe.get("format", {}).get("tags", {}),
                         "desktop_sha256": hashlib.sha256((options.bin_dir / "shotclutch.exe").read_bytes()).hexdigest(),
                         "external_media_tools_on_child_path": False,
                         "environment_alias_precedence": "new port/token override invalid legacy values",
                         "clean_machine_install": "not performed"})
        call("app.quit")
        process.wait(timeout=30)
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=10)
        log.close()
        (out / "evidence.json").write_text(json.dumps(evidence, indent=2), encoding="utf-8")
    print(f"Desktop smoke passed: {out}")


if __name__ == "__main__":
    main()
