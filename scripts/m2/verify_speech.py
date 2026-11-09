"""Test local short-window speech verification on private qualification media.

Run with the pinned fast-speech interpreter. The runner scores recognition
hypotheses and writes local review audio. It cannot label or apply a cut.
"""

import argparse
from collections import defaultdict
from difflib import SequenceMatcher
import hashlib
import html
import importlib.metadata
import json
import os
from pathlib import Path
import runpy
import subprocess
import time
import wave

import numpy as np

from compare_speech import MODELS, WORKER, model_files, verify, words
from speech_benchmark import digest, private_output, removals

RATE = 16_000
TICKS = 254016000000
FILLERS = {"um", "uhm", "umm", "uh", "uhh", "er", "erm"}
PROMPT = "Um, uh, well, I, I think that's the way it goes."


def interval(bounds):
    return bounds["start"] / TICKS, (bounds["start"] + bounds["duration"]) / TICKS


def active_map(project, sha256):
    state = project["project"]["preedit"]
    record = state["maps"][max(state["maps"], key=int)]
    analysis = state["analyses"][str(record["analysis"])]
    source = analysis["source"]
    if (
        analysis["status"] != "Complete"
        or source["sha256"] != sha256
        or source["audio_stream"] != 0
        or source["channel"] is not None
    ):
        raise ValueError("Use a completed analysis of the same original, first audio track and mono downmix")
    return record, analysis


def samples(pcm, start, end):
    return pcm[round(start * RATE) : round(end * RATE)]


def voicing(pcm):
    # Periodicity and level nominate a gap for review. They cannot identify a filler.
    frames = np.lib.stride_tricks.sliding_window_view(pcm, 640)[::320]
    centered = frames - frames.mean(axis=1, keepdims=True)
    rms = np.sqrt((centered**2).mean(axis=1))
    correlation = np.fft.irfft(abs(np.fft.rfft(centered, 1280, axis=1)) ** 2, axis=1)
    periodicity = (correlation[:, 40:230] / np.maximum(correlation[:, :1], 1e-10)).max(axis=1)
    voiced = (periodicity > 0.59) & (rms > 10 ** (-43 / 20))
    return float(voiced.mean()), float(rms.max())


def gap_samples(pcm, timed):
    candidates, controls = defaultdict(list), []
    scanned = 0
    for old, new in zip(timed, timed[1:]):
        lo, hi = old["end"] / TICKS + 0.04, new["start"] / TICKS - 0.04
        if not 0 <= lo < hi <= len(pcm) / RATE or not 0.16 <= hi - lo <= 2.3:
            continue
        scanned += 1
        periodic, level = voicing(samples(pcm, lo, hi))
        row = {
            "lo": lo,
            "hi": hi,
            "start": max(0, lo - 1.55),
            "end": min(len(pcm) / RATE, hi + 1.55),
            "voicing": periodic,
            "rms": level,
        }
        if periodic >= 0.46:
            candidates[int(lo // 120)].append({**row, "kind": "voiced"})
        elif periodic == 0 and level < 10 ** (-48 / 20):
            controls.append({**row, "kind": "quiet"})
    for block in candidates.values():
        block.sort(key=lambda g: -g["voicing"] * min(0.85, g["hi"] - g["lo"]))
    voiced = [b[rank] for rank in range(6) for b in candidates.values() if rank < len(b)][:36]
    quiet = list(np.random.default_rng(541).permutation(controls)[:12])
    return voiced + quiet, scanned, sum(map(len, candidates.values()))


def take_samples(legacy, current_cuts, timed, duration):
    def retained(a, b):
        removed = sum(max(0, min(y, b) - max(x, a)) for x, y in current_cuts)
        return 1 - removed / (b - a)

    def transcript(a, b):
        return [token for w in timed if a <= (w["start"] + w["end"]) / (2 * TICKS) < b for token in words(w["text"])]

    pairs = []
    for group in legacy["takes"]:
        if len(group["alternatives"]) != 2:
            continue
        old, new = [interval(legacy["candidates"][i]["range"]) for i in group["alternatives"]]
        a, b = old
        c, d = new
        if 0 <= a < b <= duration and 0 <= c < d <= duration and 1.5 <= b - a <= 18 and 1.5 <= d - c <= 24:
            if retained(a, b) >= 0.75 and retained(c, d) >= 0.65:
                pairs.append({"old": old, "new": new, "baseline": [transcript(*old), transcript(*new)]})
    return [pairs[i] for i in np.linspace(0, len(pairs) - 1, min(20, len(pairs)), dtype=int)]


def recognize(model, pcm, start, end, prompt=None):
    audio = np.array(samples(pcm, start, end))
    if not 0 < len(audio) <= 24 * RATE or not np.isfinite(audio).all():
        raise ValueError("Invalid bounded verification audio")
    segments, _ = model.transcribe(
        audio,
        language="en",
        word_timestamps=True,
        beam_size=5,
        condition_on_previous_text=False,
        vad_filter=False,
        initial_prompt=prompt,
    )
    result = []
    for segment in segments:
        for word in segment.words or []:
            lo, hi = start + word.start, min(end, start + word.end)
            if start <= lo < hi <= end and word.word.strip() and 0 <= word.probability <= 1:
                result.append({"text": word.word.strip(), "start": lo, "end": hi, "confidence": word.probability})
    return result


def overlap(a, b):
    return SequenceMatcher(None, a, b, autojunk=False).ratio() if a and b else 0.0


def score_run(gaps, pairs, gap_rows, pair_rows):
    scores = []
    for pair, decoded in zip(pairs, pair_rows, strict=True):
        fresh = [[token for w in side for token in words(w["text"])] for side in decoded]
        scores.append((overlap(*pair["baseline"]), overlap(*fresh)))
    return {
        "windows_with_filler_hypotheses": {
            kind: {
                mode: sum(
                    g["kind"] == kind and bool(row[mode]["fillers"]) for g, row in zip(gaps, gap_rows, strict=True)
                )
                for mode in ("plain", "verbatim")
            }
            for kind in ("voiced", "quiet")
        },
        "mean_saved_pair_overlap": sum(a for a, _ in scores) / len(scores) if scores else None,
        "mean_decoded_pair_overlap": sum(b for _, b in scores) / len(scores) if scores else None,
        "pairs_with_overlap_gain_of_0_15": sum(b - a >= 0.15 for a, b in scores),
        "human_verified_labels": 0,
    }


def write_review(output, pcm, report):
    def player(name, start, end):
        with wave.open(str(output / (name + ".wav")), "wb") as wav:
            wav.setnchannels(1)
            wav.setsampwidth(2)
            wav.setframerate(RATE)
            wav.writeframes((np.clip(samples(pcm, start, end), -1, 1) * 32767).astype("<i2").tobytes())
        return f'<audio controls preload="none" src="{name}.wav"></audio>'

    cards = []
    for i, gap in enumerate(report["gaps"]):
        hypotheses = []
        for name, run in report["runs"].items():
            for mode, variant in run["gaps"][i].items():
                transcript = " ".join(w["text"] for w in variant["words"])
                hypotheses.append(f"<p>{html.escape(name)} · {mode} · {html.escape(transcript)}</p>")
        cards.append(
            f'<section><h2>Gap {i + 1} · {gap["kind"]}</h2>{player(f"gap-{i}", gap["start"], gap["end"])}'
            f'<p>Scored interval {gap["lo"] - gap["start"]:.2f}–{gap["hi"] - gap["start"]:.2f}s within this clip</p>'
            f'<details><summary>Model transcripts</summary>{"".join(hypotheses)}</details></section>'
        )
    for i, pair in enumerate(report["pairs"]):
        a, b = (player(f"pair-{i}-{key}", *pair[key]) for key in ("old", "new"))
        lines = []
        for name, run in report["runs"].items():
            text = " / ".join(" ".join(w["text"] for w in side) for side in run["pairs"][i])
            lines.append(f"<p>{html.escape(name)} · {html.escape(text)}</p>")
        cards.append(
            f'<section><h2>Pair {i + 1}</h2>{a}{b}<details><summary>Model transcripts</summary>{"".join(lines)}</details></section>'
        )
    (output / "review.html").write_text(
        '<!doctype html><meta charset="utf-8"><title>Private speech verification</title>'
        "<style>body{font:16px system-ui;max-width:58rem;margin:2rem auto;padding:0 1rem}section{margin:1.6rem 0}audio{width:100%}</style>"
        "<h1>Private speech verification</h1><p>Review each clip in source context. All cohorts and model hypotheses are unverified.</p>"
        + "".join(cards),
        encoding="utf-8",
    )


def run(args):
    private_output(args.output)
    for name, pin in runpy.run_path(str(WORKER))["PINS"].items():
        if importlib.metadata.version(name) != pin:
            raise ValueError("Use the pinned speech interpreter")
    os.environ.update(HF_HUB_OFFLINE="1", TRANSFORMERS_OFFLINE="1", TOKENIZERS_PARALLELISM="false")
    from faster_whisper import WhisperModel

    manifest = json.loads((args.benchmark / "benchmark.json").read_text(encoding="utf-8"))
    source = manifest["inputs"]["source"]
    path, sha256 = Path(source["path"]), source["sha256"]
    if not 0 < source["seconds"] <= 7200 or digest(path) != sha256:
        raise ValueError("Use the verified original within the two-hour qualification budget")
    current_bytes = args.project.read_bytes()
    legacy_bytes = args.legacy_project.read_bytes()
    current = json.loads(current_bytes)
    _, analysis = active_map(current, sha256)
    legacy, _ = active_map(json.loads(legacy_bytes), sha256)
    catalogue = json.loads(args.catalogue.read_text(encoding="utf-8-sig"))
    artifacts = {}
    for model in MODELS:
        artifacts[model] = model_files(catalogue, model)
        verify(args.models / model, artifacts[model])

    args.output.mkdir(parents=True, exist_ok=False)
    raw = args.output / "source-16k.f32"
    subprocess.run(
        [
            "ffmpeg",
            "-nostdin",
            "-v",
            "error",
            "-i",
            str(path),
            "-map",
            "0:a:0",
            "-vn",
            "-t",
            str(source["seconds"]),
            "-ar",
            str(RATE),
            "-ac",
            "1",
            "-f",
            "f32le",
            str(raw),
        ],
        check=True,
        timeout=240,
    )
    if digest(path) != sha256:
        raise ValueError("Original source changed during decoding")
    pcm = np.memmap(raw, dtype="<f4", mode="r")
    timed = sorted(
        (w for c in analysis["chunks"] for w in (c.get("transcript") or {}).get("words", [])), key=lambda w: w["start"]
    )
    if not timed:
        raise ValueError("Verification needs saved timed words")
    gaps, scanned, voiced = gap_samples(pcm, timed)
    pairs = take_samples(legacy, removals(current, sha256), timed, len(pcm) / RATE)
    report = {
        "complete": False,
        "source_sha256": sha256,
        "pcm_sha256": digest(raw),
        "baseline_sha256": hashlib.sha256(current_bytes).hexdigest(),
        "legacy_sha256": hashlib.sha256(legacy_bytes).hexdigest(),
        "runner_sha256": digest(Path(__file__)),
        "runtime_pins": runpy.run_path(str(WORKER))["PINS"],
        "gaps_scanned": scanned,
        "voiced_candidates": voiced,
        "gaps": gaps,
        "pairs": pairs,
        "runs": {},
    }
    print(f"{scanned} gaps scanned, {len(gaps)} sampled, {len(pairs)} take pairs", flush=True)
    for name in MODELS:
        started = time.monotonic()
        model = WhisperModel(
            str(args.models / name),
            device="cpu",
            compute_type="int8",
            local_files_only=True,
            cpu_threads=max(1, min(8, (os.cpu_count() or 4) - 1)),
        )
        gap_rows = []
        for gap in gaps:
            variants = {}
            for mode, prompt in (("plain", None), ("verbatim", PROMPT)):
                hypothesis = recognize(model, pcm, gap["start"], gap["end"], prompt)
                variants[mode] = {
                    "words": hypothesis,
                    "fillers": [
                        w
                        for w in hypothesis
                        if " ".join(words(w["text"])) in FILLERS
                        and gap["lo"] <= (w["start"] + w["end"]) / 2 <= gap["hi"]
                    ],
                }
            gap_rows.append(variants)
        pair_rows = [[recognize(model, pcm, *p[key]) for key in ("old", "new")] for p in pairs]
        del model
        stats = score_run(gaps, pairs, gap_rows, pair_rows)
        report["runs"][name] = {
            "gaps": gap_rows,
            "pairs": pair_rows,
            "summary": stats,
            "seconds": time.monotonic() - started,
            "model_files": {f["name"]: f["sha256"] for f in artifacts[name]},
        }
        (args.output / "report.json").write_text(json.dumps(report, indent=2, allow_nan=False), encoding="utf-8")
        print(name, json.dumps(stats), flush=True)
    write_review(args.output, pcm, report)
    report["complete"] = True
    (args.output / "report.json").write_text(json.dumps(report, indent=2, allow_nan=False), encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("benchmark", "project", "legacy-project", "catalogue", "models", "output"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    run(parser.parse_args())
