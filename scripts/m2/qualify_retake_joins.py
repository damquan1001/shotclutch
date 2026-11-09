"""Apply confirmed take choices to a private copy and prepare local join audio."""

import argparse
from contextlib import contextmanager
import csv
import json
import os
from pathlib import Path
import re
import secrets
import socket
import subprocess
import tempfile
import time
import wave

import numpy as np

from speech_benchmark import digest, private_output
from verify_speech import RATE, TICKS, active_map


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


@contextmanager
def native_app(exe, output):
    token = secrets.token_hex(32)
    with socket.socket() as reserve:
        reserve.bind(("127.0.0.1", 0))
        port = reserve.getsockname()[1]
    startup = subprocess.STARTUPINFO()
    startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
    startup.wShowWindow = 0
    with tempfile.TemporaryDirectory(dir=output) as data, (output / "native.log").open("wb") as log:
        child = subprocess.Popen(
            [str(exe.resolve(strict=True)), "--empty", "--no-recover", "--data-dir", data, "--control", str(port)],
            env=dict(os.environ, SHOTCLUTCH_CONTROL_TOKEN=token), startupinfo=startup, stdout=log, stderr=log,
        )
        try:
            deadline = time.monotonic() + 90
            while True:
                try:
                    stream = socket.create_connection(("127.0.0.1", port), timeout=1)
                    break
                except OSError:
                    if child.poll() is not None or time.monotonic() >= deadline:
                        raise RuntimeError("Native control channel did not start") from None
                    time.sleep(.2)
            stream.settimeout(180)
            with stream, stream.makefile("rb") as reader:
                def call(method, params):
                    message = {"id": 1, "token": token, "method": method, "params": params}
                    stream.sendall(json.dumps(message).encode() + b"\n")
                    response = json.loads(reader.readline())
                    if not response.get("ok"):
                        raise RuntimeError(response.get("error"))
                    return response.get("result")

                def command(name, params=None):
                    return call("engine.execute", {"command": name, "params": params or {}})

                yield command
                call("app.quit", {})
            if child.wait(timeout=60) != 0:
                raise RuntimeError("Native app exited with an error")
        finally:
            if child.poll() is None:
                child.terminate()
                child.wait(timeout=30)


def coverage(span, retained):
    lo, hi = span
    return sum(max(0, min(hi, end) - max(lo, start)) for start, end in retained)


def remove_span(retained, lo, hi):
    return [(start, end) for left, right in retained
            for start, end in ((left, min(right, lo)), (max(left, hi), right)) if start < end]


def native_ranges(command, map_id):
    segments = command("preedit.preview", {"map": map_id})["segments"]
    source_ranges = []
    cursor = 0
    for segment in segments:
        start, end = (int(segment["source"][edge]) for edge in ("start", "end"))
        next_cursor = cursor + end - start
        if (int(segment["output"]["start"]), int(segment["output"]["end"])) != (cursor, next_cursor):
            raise ValueError("Preview assembly is not contiguous at the expected duration")
        source_ranges.append((start, end))
        cursor = next_cursor
    return source_ranges, cursor


def listen_audio(pcm, retained, boundary, path):
    # Assemble a short window on the result clock. Approximate the native 5 ms clip fades
    # with linear fades on the verified 16 kHz listening proxy.
    context = 3 * RATE
    join = round(boundary * RATE / TICKS)
    cursor = 0
    parts = []
    for start, end in retained:
        next_cursor = cursor + end - start
        first, last = (round(tick * RATE / TICKS) for tick in (cursor, next_cursor))
        lo, hi = max(first, join - context), min(last, join + context)
        if lo < hi:
            offset = round(start * RATE / TICKS) + lo - first
            audio = pcm[offset:offset + hi - lo].copy()
            if len(audio) != hi - lo:
                raise ValueError("Join window exceeds the verified PCM")
            fade = min(RATE * 5 // 1000, (last - first) // 2)
            if fade:
                pos = np.arange(lo, hi)
                audio *= np.minimum(1, np.minimum(pos - first, last - 1 - pos) / fade)
            parts.append(audio)
        cursor = next_cursor
    with wave.open(str(path), "wb") as wav:
        wav.setparams((1, 2, RATE, 0, "NONE", "not compressed"))
        wav.writeframes(np.rint(np.clip(np.concatenate(parts), -1, 1) * 32767).astype("<i2").tobytes())


def run(args):
    project, output = args.project.resolve(strict=True), args.output.resolve()
    private_output(output)
    preferences = read(args.review / "verified-preferences.json")
    report = read(args.verification / "report.json")
    pcm_path = args.verification / "source-16k.f32"
    baseline_sha = digest(project)
    if not (
        baseline_sha == preferences["baseline_sha256"] == report["baseline_sha256"]
        and digest(args.review / "cases.json") == preferences["cases_sha256"]
        and digest(args.review / "labels.csv") == preferences["labels_sha256"]
        and report["complete"] and digest(pcm_path) == report["pcm_sha256"]
    ):
        raise ValueError("Inputs differ from the verified take choices or listening PCM")
    document = read(project)
    _, analysis = active_map(document, report["source_sha256"])
    media = document["project"]["items"][str(analysis["source"]["item"])]["kind"]["Media"]["media"]
    if digest(project.parent / media["File"]["path"]) != report["source_sha256"]:
        raise ValueError("Original media differs from the verified source")
    words = [(w["start"], w["end"]) for chunk in analysis["chunks"] if chunk.get("transcript")
             for w in chunk["transcript"]["words"]]
    rows = read(args.review / "cases.json")["cases"]
    cases = {case["id"]: case for case in rows}
    with (args.review / "labels.csv").open(newline="", encoding="utf-8-sig") as file:
        labels = list(csv.DictReader(file))
    choices = {row["id"]: row["keep"] for row in labels}
    if (not labels or len(cases) != len(rows) or choices.keys() != cases.keys() or len(choices) != len(labels)
            or any(not re.fullmatch(r"R[0-9]+", case_id) for case_id in cases)
            or any(row["human_verified"] != "yes" or row["keep"] not in ("A", "B")
                   or row["boundary_adjustment"].strip() for row in labels)):
        raise ValueError("Use one confirmed A/B choice with no boundary adjustment per case")
    output.mkdir(parents=True, exist_ok=False)
    saved = output / "reviewed.scproj"
    changes = []
    print("Verified local inputs. Opening the private baseline.", flush=True)
    with native_app(args.exe, output) as command:
        command("file.open", {"path": str(project)})
        state = command("preedit.inspect")
        key = max(state["preedit"]["maps"], key=int)
        baseline, before = native_ranges(command, key)
        retained = baseline
        for case_id, keep in choices.items():
            alternate = "B" if keep == "A" else "A"
            chosen, removed = [tuple(round(value * TICKS) for value in cases[case_id][side]["range"])
                               for side in (keep, alternate)]
            if coverage(chosen, baseline) != chosen[1] - chosen[0]:
                raise ValueError(f"{case_id} selected take is incomplete in the input")
            key = command("preedit.review", {"map": key, "expectedRevision": state["revision"], "action": {
                "kind": "range", "decision": "Remove", "range": dict(zip(("start", "end"), map(str, removed))),
            }})["map"]
            state = command("preedit.inspect")
            edit = state["preedit"]["maps"][key]["overrides"][-1]["range"]
            lo, hi = edit["start"], edit["start"] + edit["duration"]
            expected = remove_span(retained, lo, hi)
            updated, after = native_ranges(command, key)
            if updated != expected or not lo <= removed[0] < removed[1] <= hi:
                raise ValueError(f"{case_id} preview differs from the requested range removal")
            for word in words:
                if not removed[0] <= word[0] < word[1] <= removed[1]:
                    if coverage(word, retained) != coverage(word, updated):
                        raise ValueError(f"{case_id} snapped cut removes speech outside the alternate")
            lost = coverage((lo, hi), retained)
            changes.append({"id": case_id, "keep": keep, "chosen": chosen, "alternate": removed,
                            "snapped": (lo, hi), "removed_ticks": lost})
            retained = updated
            print(f"{case_id} checked", flush=True)
        for change in changes:
            lo, hi = change["snapped"]
            chosen = change["chosen"]
            if coverage(chosen, retained) != chosen[1] - chosen[0] or coverage(change["alternate"], retained):
                raise ValueError(f"{change['id']} retained the wrong take")
            joins = [(left[1], right[0]) for left, right in zip(retained, retained[1:])
                     if left[1] <= lo < hi <= right[0]]
            if len(joins) != 1:
                raise ValueError(f"{change['id']} does not create an interior join")
            change["join"] = joins[0]
        command("file.saveAs", {"path": str(saved)})
        command("file.open", {"path": str(saved)})
        if native_ranges(command, key) != (retained, after):
            raise ValueError("Saved and reopened preview differs")
    if digest(project) != baseline_sha or before - after != sum(c["removed_ticks"] for c in changes):
        raise ValueError("Baseline changed or removed durations do not add up")

    pcm = np.memmap(pcm_path, mode="r", dtype="<f4")
    sections = []
    for change in changes:
        case_id = change["id"]
        left, _ = change["join"]
        boundary = sum(end - start for start, end in retained if end <= left)
        clip = f"{case_id}-join.wav"
        listen_audio(pcm, retained, boundary, output / clip)
        seconds = boundary / TICKS
        sections.append(f'<h2>{case_id} · keep {change["keep"]}</h2><p>Result {int(seconds // 60):02d}:'
                        f'{seconds % 60:05.2f}</p><audio controls preload="none" src="{clip}"></audio>')
    (output / "review.html").write_text(
        '<!doctype html><meta charset="utf-8"><title>Smart Cut join review</title>'
        '<style>body{font:16px system-ui;max-width:40rem;margin:3rem auto;padding:0 1rem}'
        'h2{margin-top:2rem}audio{width:100%}</style><h1>Smart Cut join review</h1>'
        '<p>Listen for clipped words, rushed pacing or a jarring change in delivery. Each clip has three '
        'seconds on either side of the join. These are mono listening proxies with approximate fades.</p>'
        + "\n".join(sections), encoding="utf-8",
    )
    with (output / "join-labels.csv").open("w", newline="", encoding="utf-8") as file:
        writer = csv.writer(file)
        writer.writerow(["id", "verdict", "notes"])
        writer.writerows((c["id"], "unreviewed", "") for c in changes)
    result = {"baseline_sha256": baseline_sha, "project_sha256": digest(saved), "map": key,
              "labels_sha256": preferences["labels_sha256"], "cases_sha256": preferences["cases_sha256"],
              "before_ticks": before, "after_ticks": after, "changes": changes}
    (output / "report.json").write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(f"Saved and reopened {len(changes)} joins. Result {after / TICKS:.3f}s, "
          f"removed {(before - after) / TICKS:.3f}s.", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("exe", "project", "review", "verification", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    run(parser.parse_args())
