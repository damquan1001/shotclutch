"""Prepare a private listening set for missed multi-clause retakes."""

import argparse
import csv
from difflib import SequenceMatcher
import hashlib
import html
import json
from pathlib import Path
import tempfile
import wave

import numpy as np

from compare_speech import words
from speech_benchmark import Matcher, RATE, TICKS, decode, digest, private_output, removals, union
from verify_speech import active_map, interval

MODALS = {"probably", "possibly", "likely", "perhaps", "maybe", "might", "may", "could", "seems", "guess", "think"}
SUBJECTS = {"i", "i'm", "i've", "me", "my", "we", "we've", "our", "you", "your", "they", "their"}


def overlap(span, ranges):
    a, b = span
    return sum(max(0, min(b, y) - max(a, x)) for x, y in ranges)


def length(span):
    return span[1] - span[0]


def differences(a, b):
    groups = {
        "certainty": lambda token: token in MODALS,
        "point of view": lambda token: token in SUBJECTS,
        "numbers": lambda token: token.isnumeric()
        or token in {"one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"},
        "negation": lambda token: token in {"no", "not", "never", "without", "cannot", "unless"}
        or token.endswith("n't"),
    }
    return [name for name, marker in groups.items() if {t for t in a if marker(t)} != {t for t in b if marker(t)}]


def spans(timed, cuts, reference):
    edges = [0]
    for i in range(1, len(timed)):
        gap = (timed[i]["start"] - timed[i - 1]["end"]) / TICKS
        if gap >= 0.4 or (gap >= 0.05 and timed[i - 1]["text"].endswith((".", "?", "!"))):
            edges.append(i)
    edges.append(len(timed))
    result = []
    for j, lo in enumerate(edges[:-1]):
        for hi in edges[j + 1 : j + 5]:
            if hi - lo < 9:
                continue
            span = (timed[lo]["start"] / TICKS, timed[hi - 1]["end"] / TICKS)
            if not 4 <= length(span) <= 30 or overlap(span, cuts) > 0.2 * length(span):
                continue
            text = " ".join(w["text"] for w in timed[lo:hi])
            result.append(
                {
                    "range": span,
                    "tokens": words(text),
                    "text": text,
                    "reference_fraction": overlap(span, reference) / length(span),
                }
            )
    return result


def related_pairs(attempts, cuts):
    earlier = [a for a in attempts if a["reference_fraction"] <= 0.04]
    later = [a for a in attempts if a["reference_fraction"] >= 0.62]
    pairs = []
    for b in later:
        bk = set(b["tokens"])
        for a in earlier:
            old, new = a["range"], b["range"]
            if old[1] > new[0] or new[0] - old[0] > 105 or not 0.65 <= length(old) / length(new) <= 1.45:
                continue
            if len(set(a["tokens"]) & bk) < 5:
                continue
            similarity = SequenceMatcher(None, a["tokens"], b["tokens"], autojunk=False).ratio()
            if similarity >= 0.55:
                pairs.append(
                    {
                        "A": a,
                        "B": b,
                        "lexical_overlap": similarity,
                        "earlier_retained_seconds": length(old) - overlap(old, cuts),
                        "check": differences(a["tokens"], b["tokens"]),
                    }
                )
    return sorted(pairs, key=lambda p: (p["earlier_retained_seconds"], p["lexical_overlap"]), reverse=True)


def audio_range(pcm, span):
    return np.asarray(pcm[round(span[0] * RATE) : round(span[1] * RATE)], dtype=np.float64)


def checked_anchors(anchors, original, reference):
    """Recheck existing source placements against freshly decoded waveforms."""
    result = []
    for anchor in anchors:
        if anchor["status"] != "auto_match" or anchor["score"] < 0.86:
            continue
        edited = anchor["reference_start"], anchor["reference_end"]
        start = anchor["source_start"]
        y = audio_range(reference, edited)
        if len(y) != round(length(edited) * RATE) or not len(y):
            continue
        y -= y.mean()
        candidate = round(start * RATE)
        scores = []
        for offset in range(-2, 3):
            index = candidate + offset
            if index < 0 or index + len(y) > len(original):
                continue
            x = np.array(original[index : index + len(y)], dtype=np.float64)
            x -= x.mean()
            norm = np.linalg.norm(x) * np.linalg.norm(y)
            if norm > 1e-10:
                scores.append(abs(float(x @ y)) / norm)
        if scores and max(scores) >= 0.86:
            result.append(
                {"source": [start, start + length(edited)], "reference": edited, "waveform_score": max(scores)}
            )
    return result


def inside(span, checked):
    return [a for a in checked if span[0] <= a["source"][0] and a["source"][1] <= span[1]]


def earlier_probes(span, original, matcher):
    probes = []
    edges = np.linspace(*span, 4)
    for start, stop in zip(edges[:-1], edges[1:], strict=True):
        if stop - start < 1.2:
            continue
        choices = [(float(a), float(a + 1.2)) for a in np.linspace(start, stop - 1.2, 4)]
        best = max(choices, key=lambda interval: np.mean(audio_range(original, interval) ** 2))
        matched = matcher.match(audio_range(original, best))
        if matched.get("score") is not None:
            probes.append({"source": best, "best_score": matched["score"]})
    return probes


def disjoint_pair(pair, selected):
    return all(
        overlap(pair[side]["range"], [other[opposite]["range"]]) == 0
        for other in selected
        for side in ("A", "B")
        for opposite in ("A", "B")
    )


def shortlist(pairs, checked, original, reference):
    matcher = Matcher(reference, window=1.2)
    selected = []
    # Reserve one changed-certainty case because the earlier entailment trial exposed this failure.
    certainty = [p for p in pairs if "certainty" in p["check"] and "point of view" in p["check"]]
    certainty.sort(key=lambda p: p["lexical_overlap"], reverse=True)
    for candidates, limit in ((certainty, 1), (pairs, 6)):
        for pair in candidates:
            if len(selected) >= limit:
                break
            if not disjoint_pair(pair, selected):
                continue
            evidence = inside(pair["B"]["range"], checked)
            if len(evidence) < 2:
                continue
            probes = earlier_probes(pair["A"]["range"], original, matcher)
            if len(probes) >= 2 and all(p["best_score"] < 0.8 for p in probes):
                selected.append(
                    {
                        **pair,
                        "cohort": "reference-backed B",
                        "reference_A": [],
                        "reference_B": evidence,
                        "earlier_probes": probes,
                    }
                )
    return sorted(selected, key=lambda p: p["earlier_retained_seconds"], reverse=True)


def both_in_reference(legacy, timed, cuts, checked, selected):
    controls = []
    for group in legacy["takes"]:
        if len(group["alternatives"]) != 2:
            continue
        a, b = [interval(legacy["candidates"][i]["range"]) for i in group["alternatives"]]
        if a[1] > b[0] or min(length(a), length(b)) < 2 or max(length(a), length(b)) > 30:
            continue
        evidence_a, evidence_b = inside(a, checked), inside(b, checked)
        if not evidence_a or not evidence_b or any(overlap(r, cuts) > 0.3 * length(r) for r in (a, b)):
            continue
        pair = {
            side: {
                "range": r,
                "text": " ".join(w["text"] for w in timed if r[0] <= (w["start"] + w["end"]) / (2 * TICKS) < r[1]),
            }
            for side, r in (("A", a), ("B", b))
        }
        if disjoint_pair(pair, selected):
            controls.append(
                {
                    **pair,
                    "cohort": "both represented in reference",
                    "reference_A": evidence_a,
                    "reference_B": evidence_b,
                    "check": differences(words(pair["A"]["text"]), words(pair["B"]["text"])),
                }
            )
    return max(controls, key=lambda p: len(p["reference_A"]) + len(p["reference_B"]), default=None)


def write_review(output, cases, original, reference):
    def player(name, pcm, span):
        with wave.open(str(output / f"{name}.wav"), "wb") as wav:
            wav.setnchannels(1)
            wav.setsampwidth(2)
            wav.setframerate(RATE)
            wav.writeframes((np.clip(audio_range(pcm, span), -1, 1) * 32767).astype("<i2").tobytes())
        return f'<audio controls preload="none" src="{name}.wav"></audio>'

    cards = []
    for case in cases:
        key = case["id"]
        alternatives, evidence = [], []
        for side in ("A", "B"):
            item = case[side]
            a, b = item["range"]
            alternatives.append(
                f'<article><h3>Take {side} · {a:.2f}–{b:.2f}s in source</h3>{player(key + "-" + side, original, item["range"])}'
                f'<details><summary>Saved words</summary><p>{html.escape(item["text"])}</p></details></article>'
            )
            for i, anchor in enumerate(case[f"reference_{side}"][:3], 1):
                evidence.append(
                    f'<p>Export excerpt matching take {side} · {anchor["waveform_score"]:.2f} waveform correlation</p>'
                    f'{player(f"{key}-ref-{side}-{i}", reference, anchor["reference"])}'
                )
        caution = html.escape(", ".join(case["check"]) or "listen for unique details and delivery")
        cards.append(
            f'<section><h2>{key}</h2><div class="pair">{"".join(alternatives)}</div>'
            f'<details><summary>Reference evidence and things to check</summary><p>{caution}</p>{"".join(evidence)}</details></section>'
        )
    (output / "review.html").write_text(
        '<!doctype html><meta charset="utf-8"><title>Private retake review</title>'
        "<style>body{font:16px system-ui;max-width:70rem;margin:2rem auto;padding:0 1rem}section{border-top:1px solid #999;padding:1rem 0}audio{width:100%}.pair{display:grid;grid-template-columns:repeat(auto-fit,minmax(18rem,1fr));gap:1.4rem}</style>"
        "<h1>Private retake review</h1><p>Listen to A and B first. Reply with a choice for each case, such as R01 B, R02 both, or R03 unsure. "
        "Mention a detail or boundary you want to preserve. The expandable section contains excerpts from the edited reference.</p>"
        + "".join(cards),
        encoding="utf-8",
    )
    with (output / "labels.csv").open("w", newline="", encoding="utf-8") as file:
        writer = csv.writer(file)
        writer.writerow(["id", "keep", "relation", "boundary_adjustment", "notes", "human_verified"])
        writer.writerows((p["id"], "", "", "", "", "unreviewed") for p in cases)


def run(args):
    private_output(args.output)
    benchmark = (args.benchmark / "benchmark.json").read_bytes()
    manifest = json.loads(benchmark)
    identities = [manifest["inputs"][kind] for kind in ("source", "reference")]
    paths = [Path(x["path"]) for x in identities]
    for path, info in zip(paths, identities, strict=True):
        if not 0 < info["seconds"] <= 7200 or digest(path) != info["sha256"]:
            raise ValueError("Use the unchanged source and export within the two-hour qualification budget")
    sha256 = identities[0]["sha256"]
    current_bytes, legacy_bytes = args.project.read_bytes(), args.legacy_project.read_bytes()
    current = json.loads(current_bytes)
    _, analysis = active_map(current, sha256)
    legacy, _ = active_map(json.loads(legacy_bytes), sha256)
    timed = sorted(
        (w for c in analysis["chunks"] for w in (c.get("transcript") or {}).get("words", [])), key=lambda w: w["start"]
    )
    if not timed:
        raise ValueError("The baseline needs timed source words")
    cuts = removals(current, sha256)
    args.output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(dir=args.output) as scratch:
        scratch_path = Path(scratch).resolve()
        if scratch_path.parent != args.output.resolve():
            raise ValueError("Temporary audio path is outside the private output folder")
        original, reference = (decode(path, scratch_path / f"{i}.f32") for i, path in enumerate(paths))
        try:
            for path, info in zip(paths, identities, strict=True):
                if digest(path) != info["sha256"]:
                    raise ValueError("Source or export changed during decoding")
            checked = checked_anchors(manifest["anchors"], original, reference)
            attempts = spans(timed, cuts, union([a["source"] for a in checked]))
            pairs = related_pairs(attempts, cuts)
            cases = shortlist(pairs, checked, original, reference)
            control = both_in_reference(legacy, timed, cuts, checked, cases)
            if control:
                cases.append(control)
            for i, case in enumerate(cases, 1):
                case["id"] = f"R{i:02}"
            write_review(args.output, cases, original, reference)
        finally:
            del original, reference
    summary = {
        "rechecked_reference_anchors": len(checked),
        "related_pairs": len(pairs),
        "review_cases": len(cases),
        "earlier_retained_seconds": sum(p.get("earlier_retained_seconds", 0) for p in cases),
        "human_verified": 0,
    }
    report = {
        "summary": summary,
        "cases": cases,
        "benchmark_sha256": hashlib.sha256(benchmark).hexdigest(),
        "baseline_sha256": hashlib.sha256(current_bytes).hexdigest(),
        "legacy_sha256": hashlib.sha256(legacy_bytes).hexdigest(),
    }
    (args.output / "cases.json").write_text(json.dumps(report, indent=2, allow_nan=False), encoding="utf-8")
    print(json.dumps(summary), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("benchmark", "project", "legacy-project", "output"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    run(parser.parse_args())
