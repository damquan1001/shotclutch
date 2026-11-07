"""Native Smart Cut qualification against a completed local analysis.

All generated projects, screenshots and review labels are private. Use an output folder
outside version control. This drives a separate app instance and never saves over the input.
"""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import secrets
import socket
import subprocess
import tempfile
import time

TICKS = 254016000000


def file_hash(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def run(args):
    executable = args.exe.resolve(strict=True)
    source = args.project.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    if any((output / name).exists() for name in ("reviewed.scproj", "editorial-labels.csv")):
        raise ValueError("Choose a fresh output folder to preserve existing review work and labels")
    saved = output / "reviewed.scproj"
    if saved == source:
        raise ValueError("Output project must differ from the input project")
    before = file_hash(source)
    token = secrets.token_hex(32)
    with socket.socket() as reserve:
        reserve.bind(("127.0.0.1", 0))
        port = reserve.getsockname()[1]
    environment = dict(os.environ, SHOTCLUTCH_CONTROL_TOKEN=token)
    startup = subprocess.STARTUPINFO()
    startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
    startup.wShowWindow = 0
    results = {"automated": {}, "editorial": "awaiting human labels"}
    review_latency = []
    with tempfile.TemporaryDirectory(dir=output) as data, (output / "native.log").open("wb") as log:
        child = subprocess.Popen(
            [str(executable), "--empty", "--no-recover", "--data-dir", data, "--control", str(port)],
            env=environment, startupinfo=startup, stdout=log, stderr=log,
        )
        stream = None
        try:
            deadline = time.monotonic() + 90
            while stream is None and time.monotonic() < deadline:
                if child.poll() is not None:
                    raise RuntimeError("Native app exited before startup")
                try:
                    stream = socket.create_connection(("127.0.0.1", port), timeout=1)
                except OSError:
                    time.sleep(.2)
            if stream is None:
                raise TimeoutError("Native control channel did not start")
            stream.settimeout(180)
            with stream, stream.makefile("rb") as reader:
                def call(method, params=None):
                    stream.sendall(json.dumps({"id": 1, "token": token, "method": method,
                                               "params": params or {}}).encode() + b"\n")
                    response = json.loads(reader.readline())
                    if not response.get("ok"):
                        raise RuntimeError(response.get("error"))
                    return response.get("result")

                def command(name, params=None):
                    started = time.perf_counter()
                    result = call("engine.execute", {"command": name, "params": params or {}})
                    if name == "preedit.review":
                        review_latency.append(round((time.perf_counter() - started) * 1000, 2))
                    if name in ("file.open", "file.saveAs", "preedit.analyze", "preedit.apply", "preedit.applyBackground"):
                        print(name + " completed", flush=True)
                    return result

                def latest():
                    state = command("preedit.inspect")
                    key = max(state["preedit"]["maps"], key=int)
                    return state, key, state["preedit"]["maps"][key]

                def widgets():
                    return {w["id"]: w for w in call("ui.elements", {"prefix": "roughCut."})}

                def click(key):
                    time.sleep(.15)
                    call("ui.click", {"id": key})
                    time.sleep(.3)

                def review(action):
                    state, key, _ = latest()
                    return command("preedit.review", {"map": key, "expectedRevision": state["revision"],
                                                       "action": action})["map"]

                def range_action(bounds, decision):
                    return {"kind": "range", "decision": decision, "range": {
                        "start": str(bounds["start"]), "end": str(bounds["start"] + bounds["duration"])}}

                command("file.open", {"path": str(source)})
                call("ui.resize", {"width": 1440, "height": 950})
                call("ui.set", {"workspace": "Smart Cut"})
                review({"kind": "cleanUp", "options": {"mode": args.cleanup_mode, "minPauseMs": 500, "handleMs": 150}})
                state, key, record = latest()
                analysis = state["preedit"]["analyses"][str(record["analysis"])]
                assert analysis["status"] == "Complete"
                assert record["automatic"]
                baseline = record
                segments = command("preedit.preview", {"map": key})["segments"]
                results["coverage"] = {"status": analysis["status"], "chunks": len(analysis["chunks"]),
                                       "seconds": analysis["source"]["range"]["duration"] / TICKS,
                                       "words": sum(len(c.get("transcript", {}).get("words", []))
                                                    for c in analysis["chunks"] if c.get("transcript"))}
                # Emit source-time labels without transcript text or a media locator.
                rows = []
                for kind in ("Pause", "Filler", "Restart", "Take"):
                    candidates = [(i, c) for i, c in enumerate(record["candidates"]) if c["kind"] == kind]
                    stride = max(1, len(candidates) // 8)
                    for index, candidate in candidates[::stride][:8]:
                        bounds = candidate["range"]
                        kept = sum(max(0, min(bounds["start"] + bounds["duration"], int(s["source"]["end"]))
                                          - max(bounds["start"], int(s["source"]["start"]))) for s in segments)
                        removed = (bounds["duration"] - kept) / TICKS
                        rows.append({"map": key, "candidate": index, "kind": kind,
                                     "source_start_seconds": bounds["start"] / TICKS,
                                     "source_end_seconds": (bounds["start"] + bounds["duration"]) / TICKS,
                                     "automatic_removed_seconds": round(removed, 6),
                                     "verdict": "unreviewed", "notes": ""})
                with (output / "editorial-labels.csv").open("w", newline="", encoding="utf-8") as labels:
                    writer = csv.DictWriter(labels, fieldnames=list(rows[0]))
                    writer.writeheader()
                    writer.writerows(rows)
                print("Private editorial worksheet prepared", flush=True)
                # This mechanically exercises pause preservation. Intentionality is a human label.
                click("roughCut.tab.cuts")
                click("roughCut.review.filter.Pauses")
                click("roughCut.review.status.1")
                click("roughCut.review.listen")
                time.sleep(.6)
                click("roughCut.review.restoreOrCut")
                _, _, record = latest()
                assert len(record["overrides"]) == len(baseline["overrides"]) + 1
                assert record["overrides"][-1]["decision"] == "Keep"
                results["automated"]["pause_restore"] = True

                click("roughCut.review.status.0")
                click("roughCut.review.filter.Takes")
                for _ in range(100):
                    if "roughCut.review.take.0" in widgets():
                        break
                    click("roughCut.review.next")
                assert "roughCut.review.take.1" in widgets()
                click("roughCut.review.take.0")
                time.sleep(.5)
                click("roughCut.review.take.1")
                time.sleep(.5)
                click("roughCut.review.keepBoth")
                _, _, record = latest()
                assert record["overrides"][-1]["decision"] == "Keep"
                results["automated"]["compare_and_retain_takes"] = True

                click("roughCut.tab.script")
                visible = list(widgets())
                words = [name for name in visible if name.startswith("roughCut.word.")]
                assert len(words) >= 3
                call("ui.drag", {"from": {"id": words[0]}, "to": {"id": words[2]}})
                time.sleep(.5)
                click("roughCut.text.cut")
                assert latest()[2]["overrides"][-1]["decision"] == "Remove"
                click("roughCut.text.uncut")
                assert latest()[2]["overrides"][-1]["decision"] == "Keep"
                results["automated"]["phrase_cut_restore"] = True
                command("edit.undo")
                assert latest()[2]["overrides"][-1]["decision"] == "Remove"
                command("edit.redo")
                expected = latest()[2]["overrides"]
                click("roughCut.preview.result")
                click("roughCut.play")
                assert widgets()["roughCut.play"]["label"] == "Pause"
                time.sleep(.5)
                click("roughCut.play")
                assert widgets()["roughCut.play"]["label"] == "Play"
                results["automated"]["result_transport"] = True

                # Refresh audio locally using the same saved words and selected stream.
                state, key, record = latest()
                analysis_id = str(record["analysis"])
                binding = analysis["source"]
                preview_before = call("perf.stats")["smartCut"]
                ui_checks = 0
                started = time.monotonic()
                command("preedit.analyze", {
                    "item": str(binding["item"]), "audioStream": binding["audio_stream"],
                    "channel": binding["channel"], "model": analysis["settings"]["model"],
                    "language": analysis["settings"]["language"], "reuseTranscript": analysis_id,
                    "previousMap": key, "autoCut": True, "cleanup": record["cleanup"],
                    "range": {"start": str(binding["range"]["start"]),
                              "end": str(binding["range"]["start"] + binding["range"]["duration"])},
                })
                while time.monotonic() - started < 900:
                    state, _, record = latest()
                    if not state["running"]:
                        break
                    elements = widgets()
                    viewer = elements["roughCut.viewer"]["rect"]
                    play = elements["roughCut.play"]["rect"]
                    editor = elements["roughCut.openInEditor"]["rect"]
                    assert play[1] + play[3] <= viewer[1] + viewer[3] + 2
                    assert editor[1] < viewer[1] + 110
                    current_preview = call("perf.stats")["smartCut"]
                    if current_preview["map"] == preview_before["map"]:
                        assert current_preview["builds"] == preview_before["builds"]
                    ui_checks += 1
                    time.sleep(.5)
                assert not state["running"]
                assert record["overrides"] == expected
                results["automated"]["refresh_preserves_decisions"] = True
                assert ui_checks > 0
                results["automated"]["analysis_keeps_preview_and_transport"] = True
                results["audio_refresh_seconds"] = round(time.monotonic() - started, 2)
                state, key, record = latest()
                revision = state["revision"]
                click("roughCut.openInEditor")
                deadline = time.monotonic() + 180
                while time.monotonic() < deadline:
                    view = call("ui.inspect")
                    if view["ui"]["workspace"] == "Editing":
                        break
                    time.sleep(.25)
                assert view["ui"]["workspace"] == "Editing"
                sequence = view["activeSequence"]
                assert sequence is not None
                state, _, _ = latest()
                application = str(max(state["preedit"]["applications"], key=int))
                receipt = command("preedit.apply", {"map": key, "expectedRevision": revision,
                                  "operationId": f"rough-cut-{key}-{revision}"})
                assert int(receipt["sequenceId"]) == sequence
                command("file.saveAs", {"path": str(saved)})
                command("file.open", {"path": str(saved)})
                assert latest()[2]["overrides"] == expected
                command("edit.undo")
                assert not command("preedit.inspect")["preedit"]["applications"]
                command("edit.redo")
                assert command("preedit.inspect")["preedit"]["applications"]
                command("sequence.open", {"item": sequence})
                command("mixer.setStrip", {"strip": "Mix", "volumeDb": -3.0})
                state, _, _ = latest()
                try:
                    command("preedit.apply", {"map": key, "expectedRevision": state["revision"],
                        "operationId": "m2-qualify-replace-edited", "replace": application})
                except RuntimeError as error:
                    assert "REVISION_CONFLICT" in str(error)
                else:
                    raise AssertionError("Edited target was replaced")
                results["automated"]["apply_reopen_undo_redo_idempotency"] = True
                results["automated"]["edited_target_rejected"] = True
                count = len(state["preedit"]["applications"])
                command("preedit.applyBackground", {"map": key, "expectedRevision": state["revision"],
                                                     "operationId": "m2-qualify-fresh-after-edit"})
                deadline = time.monotonic() + 180
                while time.monotonic() < deadline:
                    state, _, _ = latest()
                    if len(state["preedit"]["applications"]) > count:
                        break
                    time.sleep(.25)
                assert len(state["preedit"]["applications"]) == count + 1
                new_id = max(state["preedit"]["applications"], key=int)
                new_sequence = state["preedit"]["applications"][new_id]["sequence"]
                assert new_sequence != sequence
                results["automated"]["fresh_sequence_after_edit"] = True
                # The UI consumes the background receipt on a later frame.
                call("ui.set", {"workspace": "Smart Cut"})
                deadline = time.monotonic() + 30
                while time.monotonic() < deadline:
                    view = call("ui.inspect")
                    if view["activeSequence"] == new_sequence and view["ui"]["workspace"] == "Editing":
                        break
                    time.sleep(.15)
                assert view["activeSequence"] == new_sequence
                assert view["ui"]["workspace"] == "Editing"
                # Return to the review workspace for the editorial pass.
                call("ui.set", {"workspace": "Smart Cut"})
                time.sleep(.5)
                assert call("ui.inspect")["ui"]["workspace"] == "Smart Cut"
                command("file.saveAs", {"path": str(saved)})
                call("ui.screenshot", {"path": str(output / "workspace-private.png")})
                call("app.quit")
            assert child.wait(timeout=60) == 0
        finally:
            if child.poll() is None:
                child.terminate()
                child.wait(timeout=30)
    assert file_hash(source) == before, "Input project changed"
    results["input_project_unchanged"] = True
    results["review_command_roundtrip_ms"] = review_latency
    (output / "summary.json").write_text(json.dumps(results, indent=2), encoding="utf-8")
    print(json.dumps(results), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exe", type=Path, required=True)
    parser.add_argument("--project", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cleanup-mode", choices=["glingCompatible", "natural", "transcript"], default="glingCompatible")
    run(parser.parse_args())
