"""Run identical private excerpts through the pinned offline Tiny/Base speech worker.

Model acquisition is only performed by --install-model. No audio leaves this host.
Accuracy remains null until the corresponding human labels are marked verified.
"""

import argparse
import csv
import hashlib
import html
import json
import os
from pathlib import Path
import queue
import re
import subprocess
import threading
import time
import urllib.request
import wave

import numpy as np

from speech_benchmark import digest, private_output, removals, union

MODELS = ("faster-whisper-tiny.en", "faster-whisper-base.en")
FILLERS = set("er erm ermm err errr hm hmm hmmm um umm ummm ah uh uhh uhhh mhm mmhm mm-hm mm-hmm".split())
WORKER = Path(__file__).resolve().parents[2] / "crates/speech/src/fast_whisper.py"


def model_files(catalogue, name):
    if name not in MODELS:
        raise ValueError("Only pinned Tiny/Base English profiles are supported")
    model = next(m for m in catalogue if m["id"] == name)
    expected = {"config.json", "model.bin", "tokenizer.json", "vocabulary.txt"}
    if len(model["files"]) != 4 or {f["name"] for f in model["files"]} != expected:
        raise ValueError("Incomplete pinned profile")
    for f in model["files"]:
        url_pattern = (
            re.escape(f"https://huggingface.co/Systran/{name}/resolve/") + r"[0-9a-f]{40}/" + re.escape(f["name"])
        )
        if (
            not re.fullmatch(url_pattern, f["url"])
            or not isinstance(f["size"], int)
            or not 0 < f["size"] <= 8 * 1024**3
            or not re.fullmatch("[0-9a-f]{64}", f["sha256"])
        ):
            raise ValueError("Invalid pinned catalogue artifact")
    return model["files"]


def verify(directory, files):
    for f in files:
        p = directory / f["name"]
        if not p.is_file() or p.stat().st_size != f["size"] or digest(p) != f["sha256"]:
            raise ValueError("Pinned model missing/invalid; run explicit --install-model first")


def install(root, files, name):
    private_output(root)
    directory = root / name
    directory.mkdir(parents=True, exist_ok=True)
    for f in files:
        destination = directory / f["name"]
        if destination.is_file() and destination.stat().st_size == f["size"] and digest(destination) == f["sha256"]:
            continue
        part = directory / (f["name"] + ".part")
        print(f"Downloading pinned {name}: {f['name']} ({f['size']} bytes)", flush=True)
        h, size = hashlib.sha256(), 0
        with urllib.request.urlopen(f["url"], timeout=120) as response, part.open("wb") as out:
            while block := response.read(1024**2):
                size += len(block)
                if size > f["size"]:
                    raise ValueError("Model artifact exceeds pinned length")
                h.update(block)
                out.write(block)
        if size != f["size"] or h.hexdigest() != f["sha256"]:
            part.unlink(missing_ok=True)
            raise ValueError("Pinned artifact checksum mismatch")
        part.replace(destination)
    verify(directory, files)


def words(text):
    return re.findall(r"[^\W_]+(?:['’][^\W_]+)?", text.lower(), re.UNICODE)


def edit_counts(reference, hypothesis):
    # Counts track substitutions/deletions/insertions, preserving filler tokens in WER.
    row = [(i, 0, 0, i) for i in range(len(hypothesis) + 1)]
    for i, a in enumerate(reference, 1):
        next_row = [(i, 0, i, 0)]
        for j, b in enumerate(hypothesis, 1):
            if a == b:
                next_row.append(row[j - 1])
            else:
                choices = [
                    (row[j - 1][0] + 1, row[j - 1][1] + 1, row[j - 1][2], row[j - 1][3]),
                    (row[j][0] + 1, row[j][1], row[j][2] + 1, row[j][3]),
                    (next_row[j - 1][0] + 1, next_row[j - 1][1], next_row[j - 1][2], next_row[j - 1][3] + 1),
                ]
                next_row.append(min(choices))
        row = next_row
    errors, substitutions, deletions, insertions = row[-1]
    return {
        "reference_words": len(reference),
        "errors": errors,
        "substitutions": substitutions,
        "deletions": deletions,
        "insertions": insertions,
        "wer": errors / len(reference) if reference else None,
    }


def filler_events(timed):
    return [w for w in timed if w["text"].strip().lower().rstrip(".,!?;:") in FILLERS]


def event_counts(gold, predicted, tolerance=0.2):
    # One-to-one maximum-cardinality matching prevents repeated predictions sharing one label.
    edges = []
    for a in gold:
        choices = []
        for j, b in enumerate(predicted):
            intersection = max(0, min(a["end"], b["end"]) - max(a["start"], b["start"]))
            union = max(a["end"], b["end"]) - min(a["start"], b["start"])
            if (intersection / union if union else 0) >= 0.3 or abs(
                (a["start"] + a["end"] - b["start"] - b["end"]) / 2
            ) <= tolerance:
                choices.append(j)
        edges.append(choices)
    matched = {}

    def assign(i, seen):
        for j in edges[i]:
            if j not in seen:
                seen.add(j)
                if j not in matched or assign(matched[j], seen):
                    matched[j] = i
                    return True
        return False

    tp = sum(assign(i, set()) for i in range(len(gold)))
    boundary_error = sum(
        abs(gold[i][edge] - predicted[j][edge]) for j, i in matched.items() for edge in ("start", "end")
    )
    return {
        "tp": tp,
        "fp": len(predicted) - tp,
        "fn": len(gold) - tp,
        "precision": tp / len(predicted) if predicted else None,
        "recall": tp / len(gold) if gold else None,
        "boundary_mae_seconds": boundary_error / (2 * tp) if tp else None,
    }


def score(label, timed, events):
    result = {"word_errors": None, "fillers": None}
    if label.get("transcript_verified", "").lower() == "yes":
        result["word_errors"] = edit_counts(words(label["verbatim_text"]), words(" ".join(w["text"] for w in timed)))
    if label.get("events_verified", "").lower() == "yes":
        gold = [e for e in events if e["sample_id"] == label["id"] and e["category"] == "filler"]
        result["fillers"] = event_counts(gold, filler_events(timed))
    return result


def validate_events(events, samples):
    categories = {
        "filler",
        "restart",
        "bad_take",
        "retake",
        "intentional_repetition",
        "changed_fact",
        "natural_pause",
        "breath",
        "other",
    }
    by_id = {s["id"]: s for s in samples}
    for event in events:
        sample = by_id.get(event.get("sample_id"))
        if (
            sample is None
            or event.get("category") not in categories
            or not isinstance(event.get("start"), (int, float))
            or not isinstance(event.get("end"), (int, float))
            or not sample["start"] <= event["start"] < event["end"] <= sample["end"]
        ):
            raise ValueError("Invalid event label: use absolute source times inside its sample")


def alignment_score(rows, removed, source_duration=float("inf"), reference_duration=float("inf")):
    covered = []
    conflicts = []
    rejected = 0
    for row in rows:
        verified = row.get("human_verified", "").lower()
        if verified == "no":
            rejected += 1
        if verified != "yes":
            continue
        a, b, source = float(row["reference_start"]), float(row["reference_end"]), float(row["source_start"])
        if (
            not all(np.isfinite(v) for v in (a, b, source))
            or not 0 <= a < b <= reference_duration
            or not 0 <= source <= source_duration - (b - a)
        ):
            raise ValueError("Invalid verified alignment times")
        covered.append((a, b))
        if removed is not None:
            conflicts.extend(
                (a + max(source, x) - source, a + min(source + b - a, y) - source)
                for x, y in removed
                if min(source + b - a, y) > max(source, x)
            )
    seconds = sum(b - a for a, b in union(covered))
    removed_seconds = sum(b - a for a, b in union(conflicts))
    return {
        "verified_reference_seconds": seconds,
        "rejected_matches": rejected,
        "reference_performance_conflict_seconds": removed_seconds if removed is not None and seconds else None,
        "scope": "Verified performance overlap; not a semantic error or deletion-category label",
    }


def rescore(benchmark, run, baseline=None):
    private_output(run)
    label_paths = [benchmark / name for name in ("speech-labels.csv", "events.json", "alignment-review.csv")]
    hashes = {p.name: digest(p) for p in label_paths}
    manifest = json.loads((benchmark / "benchmark.json").read_text(encoding="utf-8"))
    with (benchmark / "speech-labels.csv").open(newline="", encoding="utf-8-sig") as f:
        labels = {r["id"]: r for r in csv.DictReader(f)}
    events = json.loads((benchmark / "events.json").read_text(encoding="utf-8"))
    validate_events(events, manifest["samples"])
    results = {}
    for model in MODELS:
        report = json.loads((run / (model + ".json")).read_text(encoding="utf-8"))
        if not report["complete"] or report["benchmark_sha256"] != digest(benchmark / "benchmark.json"):
            raise ValueError("Inference result does not belong to this completed benchmark")
        results[model] = {s["id"]: score(labels[s["id"]], s["words"], events) for s in report["samples"]}
    with (benchmark / "alignment-review.csv").open(newline="", encoding="utf-8-sig") as f:
        audit = list(csv.DictReader(f))
    baseline_hash = digest(baseline) if baseline else None
    removed = (
        removals(json.loads(baseline.read_text(encoding="utf-8")), manifest["inputs"]["source"]["sha256"])
        if baseline
        else None
    )
    payload = {
        "labels_sha256": digest(benchmark / "speech-labels.csv"),
        "events_sha256": digest(benchmark / "events.json"),
        "scorer_sha256": digest(Path(__file__)),
        "alignment_labels_sha256": digest(benchmark / "alignment-review.csv"),
        "baseline_sha256": baseline_hash,
        "reference_alignment": alignment_score(
            audit, removed, manifest["inputs"]["source"]["seconds"], manifest["inputs"]["reference"]["seconds"]
        ),
        "results": results,
    }
    if any(digest(p) != hashes[p.name] for p in label_paths):
        raise ValueError("Labels changed while scoring; retry")
    fingerprint = hashlib.sha256(json.dumps(payload, sort_keys=True, allow_nan=False).encode()).hexdigest()
    path = run / ("scores-" + fingerprint[:16] + ".json")
    if not path.exists():
        with path.open("x", encoding="utf-8") as out:
            json.dump(payload, out, indent=2, allow_nan=False)
    print(
        json.dumps(
            {
                "scored_transcripts": sum(v["word_errors"] is not None for m in results.values() for v in m.values()),
                "scored_event_windows": sum(v["fillers"] is not None for m in results.values() for v in m.values()),
                "score_file": path.name,
            }
        ),
        flush=True,
    )


class Worker:
    def __init__(self, python, directory):
        startup = None
        if os.name == "nt":
            startup = subprocess.STARTUPINFO()
            startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
            startup.wShowWindow = 0
        self.child = subprocess.Popen(
            [str(python), "-B", "-u", str(WORKER), "--model-dir", str(directory)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            startupinfo=startup,
            env=dict(os.environ, PYTHONIOENCODING="utf-8", HF_HUB_OFFLINE="1", TRANSFORMERS_OFFLINE="1"),
        )
        self.events = queue.Queue(maxsize=8)

        def read():
            try:
                while line := self.child.stdout.readline(8 * 1024**2 + 1):
                    if len(line) > 8 * 1024**2:
                        raise ValueError("Oversize worker event")
                    self.events.put(json.loads(line))
            finally:
                self.events.put({"event": "error"})

        threading.Thread(target=read, daemon=True).start()
        try:
            event = self.next()
            if event.get("event") != "ready":
                raise ValueError("Pinned worker did not become ready")
        except BaseException:
            self.close()
            raise

    def next(self):
        event = self.events.get(timeout=900)
        if event.get("event") == "error":
            raise ValueError("Offline worker failed; verify pinned runtime/model")
        return event

    def transcribe(self, audio):
        if not 0 < len(audio) <= 302 * 16000 * 4 or len(audio) % 4:
            raise ValueError("Invalid bounded PCM")
        self.child.stdin.write((json.dumps({"samples": len(audio) // 4, "language": "en"}) + "\n").encode())
        self.child.stdin.write(audio)
        self.child.stdin.flush()
        while True:
            event = self.next()
            if event["event"] == "result":
                return event["words"]

    def close(self):
        if self.child.poll() is None:
            self.child.kill()
        self.child.wait(timeout=10)


def run(args):
    if args.score_run:
        if not args.benchmark:
            raise ValueError("Scoring needs --benchmark")
        rescore(args.benchmark, args.score_run, args.baseline_project)
        return
    if not args.catalogue or not args.models:
        raise ValueError("Installation/inference needs --catalogue and --models")
    catalogue = json.loads(args.catalogue.read_text(encoding="utf-8-sig"))
    if args.install_model:
        install(args.models, model_files(catalogue, args.install_model), args.install_model)
        return
    if not args.benchmark or not args.python:
        raise ValueError("Comparison needs --benchmark and --python (the pinned speech interpreter)")
    manifest = json.loads((args.benchmark / "benchmark.json").read_text(encoding="utf-8"))
    source = Path(manifest["inputs"]["source"]["path"])
    if digest(source) != manifest["inputs"]["source"]["sha256"]:
        raise ValueError("Source changed since benchmark creation")
    with (args.benchmark / "speech-labels.csv").open(newline="", encoding="utf-8-sig") as f:
        labels = {r["id"]: r for r in csv.DictReader(f)}
    events = json.loads((args.benchmark / "events.json").read_text(encoding="utf-8"))
    validate_events(events, manifest["samples"])
    for model in MODELS:
        verify(args.models / model, model_files(catalogue, model))
    output = args.output or args.benchmark / "comparison"
    private_output(output)
    output.mkdir(parents=True, exist_ok=False)
    (output / "audio").mkdir()
    pcm = {}
    for sample in manifest["samples"]:
        if (
            not re.fullmatch(r"[a-zA-Z0-9_-]{1,40}", sample["id"])
            or not 0 <= sample["start"] < sample["end"] <= manifest["inputs"]["source"]["seconds"]
            or not 2 <= sample["end"] - sample["start"] <= 30
            or sample["id"] in pcm
        ):
            raise ValueError("Invalid bounded excerpt or sample ID")
        audio = subprocess.check_output(
            [
                "ffmpeg",
                "-v",
                "error",
                "-nostdin",
                "-ss",
                str(sample["start"]),
                "-i",
                str(source),
                "-t",
                str(sample["end"] - sample["start"]),
                "-map",
                "0:a:0",
                "-vn",
                "-ar",
                "16000",
                "-ac",
                "1",
                "-f",
                "f32le",
                "pipe:1",
            ],
            timeout=120,
        )
        path = output / "audio" / (sample["id"] + ".f32")
        path.write_bytes(audio)
        pcm[sample["id"]] = path
        with wave.open(str(path.with_suffix(".wav")), "wb") as wav:
            wav.setnchannels(1)
            wav.setsampwidth(2)
            wav.setframerate(16000)
            wav.writeframes((np.clip(np.frombuffer(audio, dtype="<f4"), -1, 1) * 32767).astype("<i2").tobytes())
    if digest(source) != manifest["inputs"]["source"]["sha256"]:
        raise ValueError("Source changed while extracting excerpts")
    lock = output / "running.lock"
    with lock.open("x"):
        pass
    try:
        reports = {}
        for model in MODELS:
            started = time.monotonic()
            worker = Worker(args.python, args.models / model)
            loaded = time.monotonic() - started
            rows = []
            try:
                for sample in manifest["samples"]:
                    if (
                        not 0 <= sample["start"] < sample["end"] <= manifest["inputs"]["source"]["seconds"]
                        or sample["end"] - sample["start"] > 30
                    ):
                        raise ValueError("Invalid bounded excerpt")
                    audio = pcm[sample["id"]].read_bytes()
                    at = time.monotonic()
                    timed = worker.transcribe(audio)
                    elapsed = time.monotonic() - at
                    if len(timed) > 50_000:
                        raise ValueError("Too many words in bounded result")
                    for word in timed:
                        if not (
                            isinstance(word["text"], str)
                            and len(word["text"]) <= 4096
                            and np.isfinite(word["start"])
                            and np.isfinite(word["end"])
                            and 0 <= word["start"] < word["end"]
                            and 0 <= word["confidence"] <= 1
                        ):
                            raise ValueError("Invalid worker timing or probability")
                        word["end"] = min(word["end"], len(audio) / 64000)
                    timed = [w for w in timed if w["end"] > w["start"]]
                    for word in timed:
                        word["start"] += sample["start"]
                        word["end"] += sample["start"]
                    rows.append(
                        {
                            **sample,
                            "pcm_sha256": hashlib.sha256(audio).hexdigest(),
                            "words": timed,
                            "inference_seconds": elapsed,
                            **score(labels[sample["id"]], timed, events),
                        }
                    )
                    report = {
                        "model": model,
                        "worker_sha256": hashlib.sha256(WORKER.read_bytes()).hexdigest(),
                        "model_files": [
                            {"name": f["name"], "sha256": f["sha256"]} for f in model_files(catalogue, model)
                        ],
                        "benchmark_sha256": digest(args.benchmark / "benchmark.json"),
                        "labels_sha256": digest(args.benchmark / "speech-labels.csv"),
                        "events_sha256": digest(args.benchmark / "events.json"),
                        "load_seconds": loaded,
                        "complete": False,
                        "samples": rows,
                    }
                    temporary = output / (model + ".tmp")
                    temporary.write_text(json.dumps(report, indent=2, allow_nan=False), encoding="utf-8")
                    temporary.replace(output / (model + ".json"))
                    print(f"{model}: {len(rows)}/{len(manifest['samples'])} excerpts", flush=True)
                report["complete"] = True
                report["wall_seconds"] = time.monotonic() - started
                (output / (model + ".json")).write_text(json.dumps(report, indent=2, allow_nan=False), encoding="utf-8")
                reports[model] = {
                    "excerpts": len(rows),
                    "words": sum(len(r["words"]) for r in rows),
                    "lexical_filler_candidates": sum(len(filler_events(r["words"])) for r in rows),
                    "load_seconds": loaded,
                    "inference_seconds": sum(r["inference_seconds"] for r in rows),
                    "human_verified_transcripts": sum(r["word_errors"] is not None for r in rows),
                    "human_verified_event_windows": sum(r["fillers"] is not None for r in rows),
                    "scope": "Counts are not accuracy; scoring requires verified labels",
                }
            finally:
                worker.close()
        (output / "summary.json").write_text(json.dumps(reports, indent=2), encoding="utf-8")
        render_review(output)
        print(json.dumps(reports), flush=True)
    finally:
        lock.unlink(missing_ok=True)


def render_review(output):
    runs = [json.loads((output / (m + ".json")).read_text(encoding="utf-8")) for m in MODELS]
    cards = []
    for a, b in zip(runs[0]["samples"], runs[1]["samples"]):
        if a["id"] != b["id"] or a["pcm_sha256"] != b["pcm_sha256"]:
            raise ValueError("Models were not evaluated on identical PCM")
        ta = " ".join(w["text"] for w in a["words"])
        tb = " ".join(w["text"] for w in b["words"])
        cards.append(
            f'<section><h2>{html.escape(a["id"])} · {a["start"]:.2f}–{a["end"]:.2f}s · {html.escape(a["split"])}</h2>'
            f'<p>{html.escape(a["stratum"])}</p><audio controls preload="none" src="audio/{a["id"]}.wav"></audio>'
            f'<details><summary>Show model hypotheses after labeling</summary><div class="pair"><article><h3>Tiny English</h3><p>{html.escape(ta)}</p></article>'
            f"<article><h3>Base English</h3><p>{html.escape(tb)}</p></article></div></details></section>"
        )
    page = '<!doctype html><meta charset="utf-8"><title>Private speech benchmark review</title><style>body{font:16px/1.6 system-ui;background:#181818;color:#ddd;max-width:1150px;margin:40px auto;padding:20px}section{border-top:1px solid #555;padding:24px 0}.pair{display:grid;grid-template-columns:1fr 1fr;gap:32px}h2{font-size:18px}audio{width:100%}p{white-space:pre-wrap}</style><h1>Private speech benchmark</h1><p>Unverified model transcripts. Listen locally and label speech-labels.csv and events.json before drawing accuracy conclusions. Event times use absolute original-source seconds.</p>'
    (output / "review.html").write_text(page + "".join(cards), encoding="utf-8")


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--catalogue", type=Path)
    p.add_argument("--models", type=Path)
    p.add_argument("--install-model", choices=MODELS)
    p.add_argument("--benchmark", type=Path)
    p.add_argument("--python", type=Path)
    p.add_argument("--output", type=Path)
    p.add_argument("--score-run", type=Path, help="Score saved recognition using verified labels, without inference")
    p.add_argument(
        "--baseline-project", type=Path, help="Optional same-source cut map for verified reference-performance scoring"
    )
    run(p.parse_args())
