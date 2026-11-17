"""Measure how much saved speech can be paired by ordered matching. No cut is applied."""
from collections import Counter, defaultdict
import argparse
from difflib import SequenceMatcher
from itertools import pairwise
import json
from pathlib import Path


from compare_speech import words
from prepare_gap_review import subtract
from prepare_retake_review import differences, length, overlap
from speech_benchmark import TICKS, removals, digest, private_output


def read(path):
    return json.loads(path.read_bytes())


def speech_spans(timed):
    edges = [0]
    for i, (left, right) in enumerate(pairwise(timed), 1):
        gap = right["start"] - left["end"]
        if gap >= TICKS * .30 or left["text"].rstrip("\"”").endswith((".", "?", "!", "—")) or left.get("speaker") != right.get("speaker"):
            edges.append(i)
    edges.append(len(timed))
    spans = []
    for i, start in enumerate(edges[:-1]):
        for end in edges[i + 1:i + 5]:
            if end - start > 64 or timed[end - 1]["end"] - timed[start]["start"] > 28 * TICKS:
                break
            if end - start < 5:
                continue
            span = (timed[start]["start"] / TICKS, max(w["end"] for w in timed[start:end]) / TICKS)
            tokens = tuple(words(" ".join(w["text"] for w in timed[start:end]).replace("’", "'")))
            speakers = {w.get("speaker") for w in timed[start:end]} - {None}
            if len(tokens) >= 5 and len(speakers) <= 1 and length(span) >= .65:
                spans.append((span, tokens, next(iter(speakers), None)))
    return spans


def retrieve(timed, spans, distance=150):
    frequencies = Counter(words(" ".join(w["text"] for w in timed).replace("’", "'")))
    postings = defaultdict(list)
    for i, (_, tokens, _) in enumerate(spans):
        for size in (2, 3):
            for phrase in set(zip(*(tokens[j:] for j in range(size)))):
                if any(frequencies[word] <= 12 for word in phrase):
                    postings[phrase].append(i)
    candidates = defaultdict(set)
    for rows in postings.values():
        for at, i in enumerate(rows):
            for j in rows[at + 1:]:
                if spans[j][0][0] - spans[i][0][1] > distance:
                    break
                if spans[i][0][1] + .45 <= spans[j][0][0]:
                    candidates[i].add(j)
    for i, matches in sorted(candidates.items()):
        span_a, a, speaker_a = spans[i]
        for j in sorted(matches):
            span_b, b, speaker_b = spans[j]
            if speaker_a is not None and speaker_b is not None and speaker_a != speaker_b:
                continue
            alignment = SequenceMatcher(None, a, b, autojunk=False)
            blocks = alignment.get_matching_blocks()[:-1]
            shared = sum(block.size for block in blocks)
            rare = {a[block.a + k] for block in blocks for k in range(block.size) if frequencies[a[block.a + k]] <= 12}
            if len(rare) < 2 or shared < 5 or max(shared / len(a), shared / len(b)) < .68:
                continue
            density = min(shared / (blocks[-1].a + blocks[-1].size - blocks[0].a),
                          shared / (blocks[-1].b + blocks[-1].size - blocks[0].b))
            if density >= .65:
                yield span_a, span_b, shared / len(a), shared / len(b), differences(a, b)


def select_windows(timed, retained, limit=10):
    ranked = []
    for a, b, ca, cb, flags in retrieve(timed, speech_spans(timed)):
        for span, counterpart, coverage in ((a, b, ca), (b, a, cb)):
            seconds = overlap(span, retained)
            if seconds >= 1 and coverage >= .68:
                ranked.append((seconds * coverage, span, counterpart, flags))
    selected = []
    for score, span, counterpart, flags in sorted(ranked, key=lambda x: (-x[0], x[1])):
        window = (max(0, span[0] - .75), min(timed[-1]["end"] / TICKS, span[1] + .75))
        if any(overlap(window, [x["range"]]) > 0 for x in selected):
            continue
        selected.append({"range": window, "counterpart": counterpart, "meaning_differences": flags})
        if len(selected) == limit:
            break
    return selected


def run(args):
    private_output(args.output)
    project = read(args.project)
    state = project["project"]["preedit"]
    cut = state["maps"][max(state["maps"], key=int)]
    analysis = state["analyses"][str(cut["analysis"])]
    if analysis["status"] != "Complete":
        raise ValueError("A completed transcript is required")
    source = analysis["source"]["range"]
    retained = subtract([(source["start"] / TICKS, (source["start"] + source["duration"]) / TICKS)], removals(project))
    timed = sorted((w for c in analysis["chunks"] for w in (c.get("transcript") or {}).get("words", [])
                    if w["end"] > w["start"]), key=lambda w: w["start"])
    selected = select_windows(timed, retained)
    result = {"project_sha256": digest(args.project), "source_sha256": analysis["source"]["sha256"],
              "windows": selected, "seconds": sum(length(x["range"]) for x in selected),
              "scope": "Candidates for audio verification. No deletion decisions or project changes."}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(json.dumps({"windows": len(selected), "seconds": result["seconds"]}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--project", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    run(parser.parse_args())


