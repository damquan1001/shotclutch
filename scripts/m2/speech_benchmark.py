"""Private, local source/export audio alignment and speech-review sampling.

Requires NumPy and FFmpeg. Automatic anchors are hypotheses, never human labels.
Writes only to a fresh output directory; no media is uploaded.
"""

import argparse
import csv
import hashlib
import html
import json
import math
from pathlib import Path
import subprocess
import time
import wave

import numpy as np

RATE = 8000
TICKS = 254016000000


def private_output(path):
    repo = Path(__file__).resolve().parents[2]
    try:
        path.resolve().relative_to(repo)
    except ValueError:
        return
    if subprocess.run(["git", "check-ignore", "-q", str(path.resolve())], cwd=repo).returncode:
        raise ValueError("Private benchmark output inside the repository must be git-ignored")


def digest(path):
    initial = path.stat()
    if not 0 < initial.st_size <= 8 * 1024**3:
        raise ValueError("Input exceeds the nonempty 8 GiB per-file verification budget")
    h = hashlib.sha256()
    size = 0
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024**2), b""):
            size += len(block)
            if size > 8 * 1024**3:
                raise ValueError("Input grew beyond verification budget")
            h.update(block)
    final = path.stat()
    if size != initial.st_size or final.st_size != size or initial.st_mtime_ns != final.st_mtime_ns:
        raise ValueError("Input changed during verification")
    return h.hexdigest()


def probe(path):
    value = json.loads(
        subprocess.check_output(
            ["ffprobe", "-v", "error", "-show_entries", "format=duration:stream=codec_type", "-of", "json", str(path)]
        )
    )
    if not any(s["codec_type"] == "audio" for s in value["streams"]):
        raise ValueError("Input has no audio")
    return float(value["format"]["duration"])


def decode(path, output):
    subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-nostdin",
            "-i",
            str(path),
            "-map",
            "0:a:0",
            "-vn",
            "-ac",
            "1",
            "-ar",
            str(RATE),
            "-c:a",
            "pcm_f32le",
            "-f",
            "f32le",
            str(output),
        ],
        check=True,
    )
    return np.memmap(output, dtype="<f4", mode="r")


def features(audio):
    hop, size = 400, 320
    count = max(0, (len(audio) - size) // hop + 1)
    result = np.empty((12, count), dtype=np.float64)
    edges = np.geomspace(80, 3600, 13)
    frequencies = np.fft.rfftfreq(size, 1 / RATE)
    for start in range(0, count, 2048):
        n = min(2048, count - start)
        frames = np.lib.stride_tricks.sliding_window_view(audio[start * hop : (start + n - 1) * hop + size], size)[
            ::hop
        ]
        power = abs(np.fft.rfft(frames * np.hanning(size), axis=1)) ** 2
        for band in range(12):
            mask = (frequencies >= edges[band]) & (frequencies < edges[band + 1])
            result[band, start : start + n] = np.log(power[:, mask].mean(axis=1) + 1e-12)
    return result


class Matcher:
    def __init__(self, audio, window=2.0):
        self.audio = audio
        self.window = window
        self.raw = features(audio)
        self.scale = np.maximum(self.raw.std(axis=1, keepdims=True), 0.25)
        self.values = self.raw / self.scale
        self.nfft = 1 << (self.values.shape[1] + int(window / 0.05) + 1).bit_length()
        self.fft = np.fft.rfft(self.values, self.nfft, axis=1)

    def candidates(self, query, count=4):
        q = features(query) / self.scale
        width = q.shape[1]
        if width < 5 or width > self.values.shape[1]:
            return []
        q = q - q.mean(axis=1, keepdims=True)
        norm = float((q * q).sum())
        if norm < 1e-8:
            return []
        cross = np.fft.irfft((self.fft * np.conj(np.fft.rfft(q, self.nfft, axis=1))).sum(axis=0), self.nfft)
        sums = np.pad(np.cumsum(self.values, axis=1), ((0, 0), (1, 0)))
        squares = np.pad(np.cumsum(self.values * self.values, axis=1), ((0, 0), (1, 0)))
        energy = ((squares[:, width:] - squares[:, :-width]) - (sums[:, width:] - sums[:, :-width]) ** 2 / width).sum(
            axis=0
        )
        scores = cross[: len(energy)] / np.sqrt(np.maximum(energy * norm, 1e-12))
        positions = []
        for _ in range(count):
            index = int(np.argmax(scores))
            positions.append(index * 0.05)
            scores[max(0, index - int(self.window / 0.05)) : index + int(self.window / 0.05) + 1] = -np.inf
        return positions

    def match(self, query, minimum=0.80, margin=0.08):
        rms = float(np.sqrt(np.mean(query.astype(np.float64) ** 2)))
        if rms < 10 ** (-45 / 20):
            return {"status": "unmatched", "reason": "low_energy", "score": None, "source_start": None}
        matches = []
        centered = query.astype(np.float64) - float(np.mean(query))
        norm = float(centered @ centered)
        for candidate in self.candidates(query):
            start = max(0, int((candidate - 0.4) * RATE))
            stop = min(len(self.audio), int((candidate + 0.4) * RATE) + len(query))
            x = np.asarray(self.audio[start:stop], dtype=np.float64)
            if len(x) < len(query):
                continue
            nfft = 1 << (len(x) + len(query) - 1).bit_length()
            cross = np.fft.irfft(np.fft.rfft(x, nfft) * np.conj(np.fft.rfft(centered, nfft)), nfft)[
                : len(x) - len(query) + 1
            ]
            sums = np.r_[0.0, np.cumsum(x)]
            squares = np.r_[0.0, np.cumsum(x * x)]
            energy = (
                squares[len(query) :]
                - squares[: -len(query)]
                - (sums[len(query) :] - sums[: -len(query)]) ** 2 / len(query)
            )
            scores = abs(cross) / np.sqrt(np.maximum(energy * norm, 1e-12))
            peak = int(np.argmax(scores))
            matches.append((float(min(1, scores[peak])), (start + peak) / RATE))
        matches.sort(reverse=True)
        distinct = []
        for score, at in matches:
            if all(abs(at - other[1]) > self.window / 2 for other in distinct):
                distinct.append((score, at))
        if not distinct:
            return {"status": "unmatched", "reason": "no_candidate", "score": None, "source_start": None}
        best, at = distinct[0]
        runner = distinct[1][0] if len(distinct) > 1 else 0.0
        status = (
            "auto_match"
            if best >= minimum and best - runner >= margin
            else "ambiguous" if best >= minimum else "unmatched"
        )
        return {
            "status": status,
            "score": best,
            "runner_up": runner,
            "source_start": at,
            "candidates": [{"score": s, "source_start": p} for s, p in distinct],
        }


def union(ranges):
    out = []
    for a, b in sorted(ranges):
        if b <= a:
            continue
        if out and a <= out[-1][1]:
            out[-1][1] = max(out[-1][1], b)
        else:
            out.append([a, b])
    return out


def removals(project, source_sha=None):
    state = project["project"]["preedit"]
    m = state["maps"][max(state["maps"], key=int)]
    if source_sha and state["analyses"][str(m["analysis"])]["source"]["sha256"] != source_sha:
        raise ValueError("Baseline map belongs to a different original source")
    if m.get("automatic") is None:
        raise ValueError("Baseline requires an automatic map")
    removed = []
    for cut in [*m["automatic"], *m["overrides"]]:
        a = cut["range"]["start"] / TICKS
        b = a + cut["range"]["duration"] / TICKS
        if cut["decision"] == "Remove":
            removed = union([*removed, [a, b]])
        else:
            removed = [[lo, hi] for x, y in removed for lo, hi in [(x, min(y, a)), (max(x, b), y)] if hi > lo]
    return removed


def sample_windows(anchors, duration, count=24, seconds=20):
    # Temporal blocks own their entire excerpt: overlapping audio cannot leak between splits.
    rng = np.random.default_rng(5917)
    pool = [(a["source_start"] + 1, "reference_anchor") for a in anchors if a["status"] == "auto_match"]
    rng.shuffle(pool)
    conflicts = [
        (a["source_start"] + 1, "provisional_reference_conflict")
        for a in anchors
        if a["status"] == "auto_match" and a.get("baseline_removed_seconds", 0) > 0.2
    ]
    rng.shuffle(conflicts)
    picks = [
        *conflicts[: count // 3],
        *pool[: count // 3],
        *((float(x), "random") for x in rng.uniform(0, duration, count * 2)),
        *((float(x) + seconds / 2, "coverage") for x in np.arange(0, duration, seconds)),
    ]
    result = []
    for center, stratum in picks:
        block = int(center // 120)
        lo, hi = block * 120, min(duration, (block + 1) * 120)
        start = max(lo, min(center - seconds / 2, hi - seconds))
        end = math.floor(min(hi, start + seconds) * 1000) / 1000
        if end - start < 2 or any(abs(start - r["start"]) < seconds for r in result):
            continue
        split = (
            "holdout"
            if int(hashlib.sha256(f"speech-v1:{block}".encode()).hexdigest()[:8], 16) % 5 == 0
            else "development"
        )
        result.append(
            {
                "id": f"s{len(result):03}",
                "start": round(start, 3),
                "end": round(end, 3),
                "block": block,
                "split": split,
                "stratum": stratum,
            }
        )
        if len(result) == count:
            break
    return result


def write_audit(directory):
    manifest = json.loads((directory / "benchmark.json").read_text(encoding="utf-8"))
    source = np.memmap(directory / "source.f32", dtype="<f4", mode="r")
    reference = np.memmap(directory / "reference.f32", dtype="<f4", mode="r")
    (directory / "audit").mkdir(exist_ok=True)
    conflicts = [a for a in manifest["anchors"] if a.get("baseline_removed_seconds", 0) > 0.2]
    others = [a for a in manifest["anchors"] if a not in conflicts]
    selected = conflicts + others[:: max(1, len(others) // 12)][:12]
    cards = []
    for anchor in selected:
        players = []
        for kind, audio, start in [
            ("reference", reference, anchor["reference_start"]),
            ("source", source, anchor["source_start"]),
        ]:
            if start is None:
                continue
            stop = start + anchor["reference_end"] - anchor["reference_start"]
            clip = audio[int(start * RATE) : int(stop * RATE)]
            name = f'{anchor["id"]}-{kind}.wav'
            with wave.open(str(directory / "audit" / name), "wb") as output:
                output.setnchannels(1)
                output.setsampwidth(2)
                output.setframerate(RATE)
                output.writeframes((np.clip(clip, -1, 1) * 32767).astype("<i2").tobytes())
            players.append(f'<div>{kind} {start:.3f}s<audio controls preload="none" src="audit/{name}"></audio></div>')
        cards.append(
            f'<section><h2>{html.escape(anchor["id"])} · {html.escape(anchor["status"])}</h2><p>Unverified match. Baseline removal overlap: {anchor.get("baseline_removed_seconds",0):.3f}s.</p>'
            + "".join(players)
            + "</section>"
        )
    (directory / "alignment-audit.html").write_text(
        '<!doctype html><meta charset="utf-8"><title>Private alignment audit</title><style>body{font:16px/1.6 system-ui;max-width:1000px;margin:40px auto;background:#181818;color:#ddd}section{border-top:1px solid #555;padding:20px}audio{display:block;width:100%}</style><h1>Source / human-export match audit</h1><p>Automatic hypotheses, not editing verdicts. Listen to both performances; mark human_verified yes/no in alignment-review.csv. Unmatched regions remain unknown.</p>'
        + "".join(cards),
        encoding="utf-8",
    )


def run(args):
    source, reference = args.source.resolve(strict=True), args.reference.resolve(strict=True)
    private_output(args.output)
    if args.output.exists():
        raise ValueError("Choose a fresh output directory; existing labels are never overwritten")
    args.output.mkdir(parents=True)
    started = time.monotonic()
    identities = {
        "source": {"path": str(source), "sha256": digest(source), "seconds": probe(source)},
        "reference": {"path": str(reference), "sha256": digest(reference), "seconds": probe(reference)},
    }
    original = decode(source, args.output / "source.f32")
    edited = decode(reference, args.output / "reference.f32")
    matcher = Matcher(original, args.window)
    removed = (
        removals(json.loads(args.project.read_text(encoding="utf-8")), identities["source"]["sha256"])
        if args.project
        else []
    )
    anchors = []
    for index, start in enumerate(np.arange(0, max(0, len(edited) / RATE - args.window), args.step)):
        query = edited[int(start * RATE) : int((start + args.window) * RATE)]
        row = matcher.match(query, args.minimum, args.margin)
        row.update(
            {
                "id": f"a{index:04}",
                "reference_start": float(start),
                "reference_end": float(start + args.window),
                "human_verified": False,
            }
        )
        if row["status"] == "auto_match":
            a, b = row["source_start"], row["source_start"] + args.window
            row["baseline_removed_seconds"] = sum(max(0, min(b, y) - max(a, x)) for x, y in removed)
        anchors.append(row)
        if index % 25 == 0:
            print(f"Aligned {index+1} reference windows", flush=True)
    samples = sample_windows(anchors, len(original) / RATE, args.samples)
    for name, path in [("source", source), ("reference", reference)]:
        if digest(path) != identities[name]["sha256"]:
            raise ValueError("Input changed during alignment; discard this uncompleted run")
    manifest = {
        "schema": 1,
        "inputs": identities,
        "settings": {
            "rate": RATE,
            "window": args.window,
            "step": args.step,
            "minimum": args.minimum,
            "margin": args.margin,
        },
        "anchors": anchors,
        "samples": samples,
        "scope": "Unverified audio matches; no deletion-category ground truth",
    }
    (args.output / "benchmark.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    with (args.output / "alignment-review.csv").open("w", newline="", encoding="utf-8") as out:
        fields = [
            "id",
            "reference_start",
            "reference_end",
            "source_start",
            "status",
            "score",
            "human_verified",
            "notes",
        ]
        writer = csv.DictWriter(out, fields)
        writer.writeheader()
        for a in anchors:
            writer.writerow({**{k: a.get(k, "") for k in fields}, "human_verified": "unreviewed"})
    with (args.output / "speech-labels.csv").open("w", newline="", encoding="utf-8") as out:
        fields = [
            "id",
            "start",
            "end",
            "split",
            "stratum",
            "verbatim_text",
            "transcript_verified",
            "events_verified",
            "notes",
        ]
        writer = csv.DictWriter(out, fields, extrasaction="ignore")
        writer.writeheader()
        for sample in samples:
            writer.writerow({**sample, "transcript_verified": "unreviewed", "events_verified": "unreviewed"})
    (args.output / "events.json").write_text("[]\n", encoding="utf-8")
    write_audit(args.output)
    counts = {
        status: sum(a["status"] == status for a in anchors) for status in ("auto_match", "ambiguous", "unmatched")
    }
    matched = union([(a["reference_start"], a["reference_end"]) for a in anchors if a["status"] == "auto_match"])
    summary = {
        "windows": len(anchors),
        **counts,
        "automatically_matched_reference_seconds": sum(b - a for a, b in matched),
        "provisional_conflict_anchors": sum(a.get("baseline_removed_seconds", 0) > 0.2 for a in anchors),
        "samples": len(samples),
        "human_verified": 0,
        "accuracy": None,
        "elapsed_seconds": time.monotonic() - started,
    }
    (args.output / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(json.dumps(summary), flush=True)


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--source", type=Path, required=True)
    p.add_argument("--reference", type=Path, required=True)
    p.add_argument("--project", type=Path)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--window", type=float, default=2)
    p.add_argument("--step", type=float, default=2)
    p.add_argument("--minimum", type=float, default=0.80)
    p.add_argument("--margin", type=float, default=0.08)
    p.add_argument("--samples", type=int, default=24)
    a = p.parse_args()
    if not (
        1 <= a.window <= 10
        and 0.25 <= a.step <= 30
        and 0 < a.minimum <= 1
        and 0 <= a.margin <= 1
        and 1 <= a.samples <= 100
    ):
        p.error("Invalid bounded alignment settings")
    run(a)
