"""Benchmark the approved Uhm ONNX artifact on private qualification audio."""

import argparse
import csv
import hashlib
import html
import importlib.metadata
import json
from pathlib import Path
import runpy
import subprocess
import time
import wave

import numpy as np

from compare_speech import WORKER
from speech_benchmark import digest, private_output, removals, union
from verify_speech import RATE, TICKS, active_map, samples

MODEL_SHA256 = "fbd19765def76893841e0c70e498bf632d6c68175ceae5a2da8fe3bf7d9d0314"
MODEL_REVISION = "a0ab78540d724afa6d8caf09090936ffbfbbf3d1"
HOP = 320
WINDOW = 480_000
CORE_FRAMES = 1250
LEFT_CONTEXT = 100
MIN_FRAMES = 6


def probabilities(pcm, infer, progress=None):
    """Run fixed windows and take exactly one prediction per source frame."""
    count = (len(pcm) + HOP - 1) // HOP
    result = np.empty((count, 6), dtype=np.float32)
    for start in range(0, count, CORE_FRAMES):
        end = min(count, start + CORE_FRAMES)
        origin = max(0, start - LEFT_CONTEXT)
        audio = pcm[origin * HOP : origin * HOP + WINDOW]
        window = np.zeros((1, WINDOW), dtype=np.float32)
        window[0, : len(audio)] = audio
        if not np.isfinite(window).all():
            raise ValueError("Nonfinite audio in verification window")
        scores = infer(window)
        if (
            scores.shape != (1, 1499, 6)
            or not np.isfinite(scores).all()
            or not np.allclose(scores.sum(axis=-1), 1, atol=1e-5)
        ):
            raise ValueError("Unexpected Uhm output shape or probabilities")
        if np.any(scores < 0) or np.any(scores > 1):
            raise ValueError("Uhm scores outside the probability range")
        result[start:end] = scores[0, start - origin : end - origin]
        if progress is not None:
            progress(end, count)
    return result


def intervals(scores, floor, duration):
    changes = np.flatnonzero(np.diff(np.r_[False, scores >= floor, False]))
    events = []
    for lo, hi in zip(changes[::2], changes[1::2], strict=True):
        start, end = lo * HOP / RATE, min(duration, hi * HOP / RATE)
        if hi - lo >= MIN_FRAMES and end - start >= MIN_FRAMES * HOP / RATE - 1e-9:
            events.append({"start": float(start), "end": float(end), "mean_score": float(scores[lo:hi].mean())})
    return events


def overlap_seconds(event, ranges):
    start, end = event["start"], event["end"]
    return sum(max(0, min(end, b) - max(start, a)) for a, b in ranges)


def screen(events, word_ranges, reference_ranges, current_cuts):
    for event in events:
        event["word_overlap"] = overlap_seconds(event, word_ranges)
        event["provisional_reference_overlap"] = overlap_seconds(event, reference_ranges)
        event["current_cut_overlap"] = overlap_seconds(event, current_cuts)
        event["passes_overlap_screen"] = all(
            event[key] <= 1e-8 for key in ("word_overlap", "provisional_reference_overlap", "current_cut_overlap")
        )
    return events


def cohorts(report, timed):
    controls = []
    for word in timed:
        key = word["text"].strip().lower().strip(".,?!")
        lo, hi = word["start"] / TICKS, word["end"] / TICKS
        if (
            key in ("on", "in", "and", "a", "the", "am", "i'm", "an")
            and word.get("confidence", 1) >= 0.9
            and 0.1 <= hi - lo <= 0.75
        ):
            controls.append({"lo": lo, "hi": hi, "kind": "timed-word"})
    return report["gaps"] + [controls[i] for i in np.linspace(0, len(controls) - 1, min(24, len(controls)), dtype=int)]


def summarize_cohorts(events, windows):
    hits = [0.5 * (e["start"] + e["end"]) for e in events]
    return {
        kind: {
            "windows": sum(w["kind"] == kind for w in windows),
            "with_hypothesis": sum(w["kind"] == kind and any(w["lo"] <= t <= w["hi"] for t in hits) for w in windows),
        }
        for kind in ("voiced", "quiet", "timed-word")
    }


def synthetic_controls(infer):
    phrases = (
        ("filler", "Um, I think we should go."),
        ("filler", "I, uh, forgot my camera."),
        ("filler", "Well, um, the picture looks clear."),
        ("word", "I have an umbrella in the car."),
        ("word", "The motor is humming now."),
        ("word", "I was on an early flight."),
    )
    totals = {kind: {"clips": 0, "any_filler": 0, "uh_or_um": 0} for kind in ("filler", "word")}
    for voice in ("slt", "rms"):
        for kind, phrase in phrases:
            raw = subprocess.check_output(
                [
                    "ffmpeg",
                    "-nostdin",
                    "-v",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    f"flite=text='{phrase}':voice={voice}",
                    "-ar",
                    str(RATE),
                    "-ac",
                    "1",
                    "-f",
                    "f32le",
                    "pipe:1",
                ],
                timeout=15,
            )
            pcm = np.frombuffer(raw, dtype="<f4")
            scores = probabilities(pcm, infer)
            totals[kind]["clips"] += 1
            totals[kind]["any_filler"] += bool(intervals(1 - scores[:, 0], 0.75, len(pcm) / RATE))
            totals[kind]["uh_or_um"] += bool(intervals(scores[:, 1] + scores[:, 2], 0.75, len(pcm) / RATE))
    return totals


def write_review(output, pcm, events):
    cards = []
    with (output / "review.csv").open("w", newline="", encoding="utf-8") as file:
        fields = (
            "id",
            "start",
            "end",
            "mean_score",
            "word_overlap",
            "provisional_reference_overlap",
            "current_cut_overlap",
            "passes_overlap_screen",
            "human_label",
        )
        writer = csv.DictWriter(file, fieldnames=fields)
        writer.writeheader()
        for i, event in enumerate(events, 1):
            writer.writerow({**event, "id": i, "human_label": "unreviewed"})
            start = max(0, event["start"] - 1.2)
            end = min(len(pcm) / RATE, event["end"] + 1.2)
            name = f"candidate-{i}.wav"
            with wave.open(str(output / name), "wb") as wav:
                wav.setnchannels(1)
                wav.setsampwidth(2)
                wav.setframerate(RATE)
                wav.writeframes((np.clip(samples(pcm, start, end), -1, 1) * 32767).astype("<i2").tobytes())
            flags = ", ".join(
                name
                for name, key in (
                    ("saved speech", "word_overlap"),
                    ("reference match", "provisional_reference_overlap"),
                    ("current cut", "current_cut_overlap"),
                )
                if event[key] > 1e-8
            )
            cards.append(
                f'<section><h2>Candidate {i} · {event["start"]:.2f}s in source</h2><audio controls preload="none" src="{name}"></audio>'
                f'<p>Listen at {event["start"] - start:.2f}–{event["end"] - start:.2f}s in this clip. '
                f'Overlap: {html.escape(flags or "none detected")}.</p></section>'
            )
    (output / "review.html").write_text(
        '<!doctype html><meta charset="utf-8"><title>Private filler review</title>'
        "<style>body{font:16px system-ui;max-width:58rem;margin:2rem auto;padding:0 1rem}section{margin:1.6rem 0}audio{width:100%}</style>"
        "<h1>Private filler review</h1><p>Uhm hypotheses at the strict uh/um score. Record listening labels in review.csv.</p>"
        '<p>Acoustic model by <a href="https://desertant.com/">Desert Ant Labs</a>.</p>' + "".join(cards),
        encoding="utf-8",
    )


def run(args):
    from onnxruntime import InferenceSession, SessionOptions

    private_output(args.output)
    pins = runpy.run_path(str(WORKER))["PINS"]
    if any(importlib.metadata.version(name) != pins[name] for name in ("numpy", "onnxruntime")):
        raise ValueError("Use the pinned speech interpreter")
    if digest(args.model) != MODEL_SHA256:
        raise ValueError("Uhm artifact checksum mismatch")
    source_report = json.loads((args.verification / "report.json").read_text(encoding="utf-8"))
    manifest = json.loads((args.benchmark / "benchmark.json").read_text(encoding="utf-8"))
    sha256 = manifest["inputs"]["source"]["sha256"]
    project_bytes = args.project.read_bytes()
    if (
        not source_report["complete"]
        or source_report["source_sha256"] != sha256
        or hashlib.sha256(project_bytes).hexdigest() != source_report["baseline_sha256"]
    ):
        raise ValueError("Use the completed verification report and its unchanged baseline project")
    raw = args.verification / "source-16k.f32"
    if not 0 < raw.stat().st_size <= 7200 * RATE * 4 or digest(raw) != source_report["pcm_sha256"]:
        raise ValueError("PCM cache does not match the completed verification report")
    pcm = np.memmap(raw, dtype="<f4", mode="r")
    project = json.loads(project_bytes)
    _, analysis = active_map(project, sha256)
    timed = sorted(
        (w for c in analysis["chunks"] for w in (c.get("transcript") or {}).get("words", [])), key=lambda w: w["start"]
    )
    word_ranges = union([(w["start"] / TICKS - 0.04, w["end"] / TICKS + 0.04) for w in timed])
    reference_ranges = union(
        [
            (a["source_start"], a["source_start"] + a["reference_end"] - a["reference_start"])
            for a in manifest["anchors"]
            if a["status"] == "auto_match"
        ]
    )
    cuts = removals(project, sha256)
    args.output.mkdir(parents=True, exist_ok=False)
    opts = SessionOptions()
    opts.intra_op_num_threads = 4
    opts.inter_op_num_threads = 1
    session = InferenceSession(str(args.model), sess_options=opts, providers=["CPUExecutionProvider"])

    def progress(done, total):
        if done == total or done % (20 * CORE_FRAMES) == 0:
            print(f"Scored {done * HOP / RATE:.1f} seconds of {total * HOP / RATE:.1f}", flush=True)

    def infer(window):
        return session.run(["probs"], {"audio": window})[0]

    started = time.monotonic()
    scores = probabilities(pcm, infer, progress)
    inference_seconds = time.monotonic() - started
    duration = len(pcm) / RATE
    windows = cohorts(source_report, timed)
    summary = {}
    variants = {}
    for name, series, floor in (
        ("any_filler_065", 1 - scores[:, 0], 0.65),
        ("any_filler_075", 1 - scores[:, 0], 0.75),
        ("uh_or_um_075", scores[:, 1] + scores[:, 2], 0.75),
    ):
        events = screen(intervals(series, floor, duration), word_ranges, reference_ranges, cuts)
        variants[name] = events
        eligible = [e for e in events if e["passes_overlap_screen"]]
        summary[name] = {
            "events": len(events),
            "seconds": sum(e["end"] - e["start"] for e in events),
            "touch_saved_words": sum(e["word_overlap"] > 1e-8 for e in events),
            "touch_provisional_reference": sum(e["provisional_reference_overlap"] > 1e-8 for e in events),
            "touch_current_cuts": sum(e["current_cut_overlap"] > 1e-8 for e in events),
            "screened_events": len(eligible),
            "screened_seconds": sum(e["end"] - e["start"] for e in eligible),
            "cohorts": summarize_cohorts(events, windows),
        }
    control_results = synthetic_controls(infer)
    write_review(args.output, pcm, variants["uh_or_um_075"])
    report = {
        "model_revision": MODEL_REVISION,
        "model_sha256": MODEL_SHA256,
        "pcm_sha256": source_report["pcm_sha256"],
        "baseline_sha256": source_report["baseline_sha256"],
        "source_seconds": duration,
        "inference_seconds": inference_seconds,
        "summary": summary,
        "synthetic_controls": control_results,
        "events": variants,
        "human_verified_labels": 0,
    }
    (args.output / "report.json").write_text(json.dumps(report, indent=2, allow_nan=False), encoding="utf-8")
    print(
        json.dumps({"inference_seconds": inference_seconds, "summary": summary, "synthetic_controls": control_results}),
        flush=True,
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("model", "benchmark", "verification", "project", "output"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    run(parser.parse_args())
