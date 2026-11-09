"""Rank remaining reference gaps and prepare a private take review."""

import argparse
import csv
from difflib import SequenceMatcher
import html
import json
from pathlib import Path
import tempfile
import wave

import numpy as np

from compare_speech import words
from prepare_retake_review import audio_range, checked_anchors, differences, length, overlap
from qualify_retake_joins import read, remove_span
from speech_benchmark import RATE, TICKS, decode, digest, private_output, removals, union
from verify_speech import active_map


def subtract(spans, excluded):
    for lo, hi in excluded:
        spans = remove_span(spans, lo, hi)
    return spans


def ranked_gaps(anchors, retained, protected):
    matched = union([a["source"] for a in anchors])
    skips = []
    for a, b in zip(anchors, anchors[1:]):
        if abs(a["reference"][1] - b["reference"][0]) > .001:
            continue
        lo, hi = a["source"][1], b["source"][0]
        if hi - lo > .3:
            skips.append((lo, hi))
    # An excerpt can occur elsewhere in the export. Exclude all matched source audio.
    gaps = []
    for lo, hi in subtract(union(skips), matched):
        pieces = [(max(lo, s), min(hi, e)) for s, e in retained if s < hi and lo < e]
        if pieces:
            eligible = subtract(pieces, protected)
            gaps.append({"range": (lo, hi), "retained": pieces, "eligible": eligible,
                         "retained_seconds": sum(map(length, pieces)), "eligible_seconds": sum(map(length, eligible))})
    return sorted(gaps, key=lambda g: g["eligible_seconds"], reverse=True)


def portions(ranges, timed):
    """Bound each clip without cutting through a saved word to split a long excerpt."""
    portions = []
    for lo, hi in ranges:
        start = lo
        for left, right in zip(timed, timed[1:]):
            if not start + 8 <= left["end"] / TICKS < hi - 2:
                continue
            end = left["end"] / TICKS
            gap = right["start"] / TICKS - end
            if gap >= .30 or (end >= start + 14 and gap >= .08):
                edge = end + min(gap / 2, .15)
                if edge < hi:
                    portions.append((start, edge))
                    start = edge
        portions.append((start, hi))
    return portions


def excerpt(span, timed):
    text = " ".join(w["text"] for w in timed if span[0] <= (w["start"] + w["end"]) / (2 * TICKS) < span[1])
    return {"range": span, "text": text, "tokens": words(text)}


def best_counterpart(item, candidates):
    a = item["tokens"]
    if len(a) < 5:
        return None
    best = None
    for other in candidates:
        b = other["tokens"]
        if overlap(item["range"], [other["range"]]) or len(set(a) & set(b)) < 4:
            continue
        blocks = SequenceMatcher(None, a, b, autojunk=False).get_matching_blocks()[:-1]
        shared = sum(block.size for block in blocks)
        if shared < 5 or max((block.size for block in blocks), default=0) < 3:
            continue
        covered = shared / len(a)
        density = shared / (blocks[-1].b + blocks[-1].size - blocks[0].b)
        if covered < .42 or density < .48:
            continue
        # Coverage measures how much of the suspected take has a counterpart.
        # Density avoids accumulating common words across unrelated sentences.
        rank = (covered * density, shared, other["basis"] == "reference")
        if best is None or rank > best[0]:
            comparison = {key: other[key] for key in ("range", "text", "basis", "reference_range") if key in other}
            best = (rank, {**comparison, "ordered_coverage": covered, "ordered_density": density,
                           "check": differences(a, b)})
    return best[1] if best else None


def render(output, original, reference, gaps, cases, summary):
    def player(name, pcm, span):
        with wave.open(str(output / f"{name}.wav"), "wb") as wav:
            wav.setparams((1, 2, RATE, 0, "NONE", "not compressed"))
            audio = audio_range(pcm, span)
            wav.writeframes((np.clip(audio, -1, 1) * 32767).astype("<i2").tobytes())
        return f'<audio controls preload="none" src="{name}.wav"></audio>'

    sections = []
    for gap in gaps:
        cards = []
        for case in cases:
            if case["gap"] != gap["id"]:
                continue
            key, span = case["id"], case["range"]
            suspect = player(key, original, span)
            aside = ""
            if (other := case["counterpart"]) is not None:
                cue = "Matched export audio" if other["basis"] == "reference" else "Another retained source performance"
                comparison = player(key + "-compare", reference if other["basis"] == "reference" else original,
                                    other.get("reference_range", other["range"]))
                flags = ", ".join(other["check"])
                aside = (f'<p>{cue} · {other["ordered_coverage"]:.0%} ordered word coverage</p>{comparison}'
                         f'<details><summary>Comparison transcript</summary><p>{html.escape(other["text"])}</p>'
                         f'<p>{html.escape(flags)}</p></details>')
            cards.append(
                f'<article id="{key}"><h3>{key} · {span[0]:.2f}–{span[1]:.2f}s source</h3>'
                f'{suspect}<details><summary>Saved transcript</summary><p>{html.escape(case["text"])}</p></details>{aside}</article>'
            )
        sections.append(f'<section><h2>{gap["id"]} · {gap["eligible_seconds"]:.1f}s to assess</h2>{"".join(cards)}</section>')
    (output / "review.html").write_text(
        '<!doctype html><meta charset="utf-8"><title>Smart Cut remaining takes</title>'
        '<style>body{font:16px system-ui;max-width:54rem;margin:2rem auto;padding:0 1rem}'
        'section{border-top:1px solid #bbb}article{margin-bottom:2rem}audio{width:100%}details{margin:.5rem 0}</style>'
        '<h1>Smart Cut remaining takes</h1>'
        '<p>These clips are still in the reviewed cut. Mark each repeat, restart, off-script chatter, keep, or unsure. '
        'If only part should go, name the words or time range. The optional comparison is a listening aid.</p>'
        f'<p>{summary["review_seconds"]:.1f}s across {summary["review_clips"]} clips. Confirmed takes are excluded from the clips under review.</p>'
        + "\n".join(sections), encoding="utf-8",
    )
    with (output / "labels.csv").open("w", newline="", encoding="utf-8") as file:
        writer = csv.writer(file)
        writer.writerow(["id", "verdict", "boundary_adjustment", "notes", "human_verified"])
        writer.writerows((c["id"], "unreviewed", "", "", "no") for c in cases)


def run(args):
    private_output(args.output)
    benchmark_path = args.benchmark / "benchmark.json"
    benchmark = read(benchmark_path)
    inputs = [benchmark["inputs"][key] for key in ("source", "reference")]
    paths = [Path(item["path"]) for item in inputs]
    if any(not 0 < item["seconds"] <= 7200 or digest(path) != item["sha256"]
           for path, item in zip(paths, inputs, strict=True)):
        raise ValueError("Use the verified source and reference within the two-hour budget")
    project_hash = digest(args.project)
    project = read(args.project)
    _, analysis = active_map(project, inputs[0]["sha256"])
    source = analysis["source"]["range"]
    retained = subtract([(source["start"] / TICKS, (source["start"] + source["duration"]) / TICKS)], removals(project))
    choices_hash = digest(args.choices)
    choices = read(args.choices)
    reviewed = args.choices.with_name("reviewed.scproj")
    choice_map = choices["map"]
    history = project["project"]["preedit"]["maps"]
    if (digest(reviewed) != choices["project_sha256"]
            or history.get(choice_map) != read(reviewed)["project"]["preedit"]["maps"][choice_map]):
        raise ValueError("The confirmed choices must belong to an earlier revision of this cut")
    protected = union([tuple(tick / TICKS for tick in change["chosen"]) for change in choices["changes"]])
    timed = sorted((w for chunk in analysis["chunks"] for w in (chunk.get("transcript") or {}).get("words", [])),
                   key=lambda w: w["start"])
    args.output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(dir=args.output) as temp:
        original, reference = (decode(path, Path(temp) / f"{i}.f32") for i, path in enumerate(paths))
        try:
            if any(digest(path) != item["sha256"] for path, item in zip(paths, inputs, strict=True)):
                raise ValueError("Media changed while decoding")
            anchors = checked_anchors(benchmark["anchors"], original, reference)
            gaps = ranked_gaps(anchors, retained, protected)
            selected = gaps[:args.regions]
            cases = []
            for i, gap in enumerate(selected, 1):
                gap["id"] = f"G{i:02}"
                for span in portions(gap["eligible"], timed):
                    item = excerpt(span, timed)
                    if length(span) >= .8 and item["tokens"]:
                        cases.append({**item, "gap": gap["id"], "id": f"J{len(cases) + 1:02}", "basis": "source"})
            # Merge only continuous waveform placements at the same source offset.
            runs = []
            for anchor in anchors:
                start, end = anchor["source"]
                ref_lo, ref_hi = anchor["reference"]
                if runs and abs(runs[-1]["reference_range"][1] - ref_lo) < .001 and abs(runs[-1]["range"][1] - start) < .06:
                    runs[-1]["range"] = (runs[-1]["range"][0], end)
                    runs[-1]["reference_range"] = (runs[-1]["reference_range"][0], ref_hi)
                else:
                    runs.append({"range": (start, end), "reference_range": (ref_lo, ref_hi)})
            comparisons = [{**excerpt(span, timed), "basis": "reference",
                            "reference_range": (run["reference_range"][0] + span[0] - run["range"][0],
                                                run["reference_range"][1] - (run["range"][1] - span[1]))}
                           for run in runs for span in portions([run["range"]], timed)]
            for case in cases:
                case["counterpart"] = best_counterpart(case, [*comparisons, *cases])
            summary = {
                "waveform_checked_anchors": len(anchors),
                "skipped_regions_with_retained_audio": len(gaps),
                "skipped_retained_seconds": sum(g["retained_seconds"] for g in gaps),
                "protected_seconds": sum(g["retained_seconds"] - g["eligible_seconds"] for g in gaps),
                "review_regions": len(selected), "review_clips": len(cases),
                "review_seconds": sum(length(c["range"]) for c in cases),
                "clips_with_comparison": sum(c["counterpart"] is not None for c in cases),
                "comparison_clip_seconds": sum(length(c["range"]) for c in cases if c["counterpart"] is not None),
                "human_verified": 0,
            }
            if digest(args.project) != project_hash:
                raise ValueError("Project changed during review preparation")
            render(args.output, original, reference, selected, cases, summary)
        finally:
            del original, reference
    report = {"summary": summary, "project_sha256": project_hash, "benchmark_sha256": digest(benchmark_path),
              "choices_sha256": choices_hash, "gaps": gaps, "cases": cases}
    (args.output / "report.json").write_text(json.dumps(report, indent=2, allow_nan=False), encoding="utf-8")
    print(json.dumps(summary), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("benchmark", "project", "choices", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--regions", type=int, choices=range(1, 13), default=6)
    run(parser.parse_args())
