"""Account for a saved cut's duration and missing decisions from a reviewed copy."""

import argparse
import json
from pathlib import Path

from prepare_gap_review import ranked_gaps, subtract
from prepare_retake_review import length
from speech_benchmark import digest, private_output, removals, union
from verify_speech import active_map, interval


def intersect(left, right):
    return union((max(a, c), min(b, d)) for a, b in left for c, d in right if a < d and c < b)


def account(retained, anchors):
    matched = intersect(retained, union(a["source"] for a in anchors))
    gaps = ranked_gaps(anchors, retained, [])
    skipped = union(span for gap in gaps for span in gap["retained"])
    unresolved = subtract(subtract(retained, matched), skipped)
    return {
        "matched_source_seconds": sum(map(length, matched)),
        "between_adjacent_reference_anchors_seconds": sum(map(length, skipped)),
        "unresolved_source_seconds": sum(map(length, unresolved)),
        "largest_reference_gaps": gaps[:10],
    }


def run(args):
    private_output(args.output)
    benchmark = json.loads(args.benchmark.read_bytes())
    project = json.loads(args.project.read_bytes())
    source_sha = benchmark["inputs"]["source"]["sha256"]
    current, analysis = active_map(project, source_sha)
    source = interval(analysis["source"]["range"])
    retained = subtract([source], removals(project, source_sha))
    anchors = [
        {"source": (a["source_start"], a["source_start"] + a["reference_end"] - a["reference_start"]),
         "reference": (a["reference_start"], a["reference_end"])}
        for a in benchmark["anchors"] if a["status"] == "auto_match" and a["score"] >= .95
    ]
    anchors.sort(key=lambda a: a["reference"])
    duration = sum(map(length, retained))
    result = {
        "project_sha256": digest(args.project), "benchmark_sha256": digest(args.benchmark),
        "map": str(max(project["project"]["preedit"]["maps"], key=int)),
        "retained_seconds": duration, "reference_seconds": benchmark["inputs"]["reference"]["seconds"],
        "duration_gap_seconds": duration - benchmark["inputs"]["reference"]["seconds"],
        "manual_decisions": len(current["overrides"]), **account(retained, anchors),
        "scope": "Automatic audio matches identify placements and gaps. Unresolved audio requires further analysis.",
    }
    if args.reviewed:
        reviewed = json.loads(args.reviewed.read_bytes())
        prior, prior_analysis = active_map(reviewed, source_sha)
        if prior_analysis["source"] != analysis["source"]:
            raise ValueError("Reviewed decisions must use the same source, range, channel and stream")
        # Evaluate the final ordered decisions, including Keep reversals.
        review_only = {"project": {"preedit": {"maps": {"0": {**prior, "automatic": []}}}}}
        approved_removals = removals(review_only)
        missing = intersect(retained, approved_removals)
        result.update({"reviewed_sha256": digest(args.reviewed),
                       "reviewed_removals_still_retained_seconds": sum(map(length, missing)),
                       "reviewed_removals_still_retained": missing})
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({k: v for k, v in result.items() if k.endswith("seconds")}, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--project", required=True, type=Path)
    parser.add_argument("--benchmark", required=True, type=Path)
    parser.add_argument("--reviewed", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    run(parser.parse_args())
