"""Summarize private M2 human labels without printing source times or notes.

Verdicts: accept, restore, adjust, unreviewed. Rates concern sampled candidate rows,
not all cuts or duration; overlapping candidates must not be counted as total time.
"""
import argparse
import csv
import json
from pathlib import Path


def summarize(path):
    with path.open(newline="", encoding="utf-8-sig") as source:
        rows = list(csv.DictReader(source))
    counts = {name: 0 for name in ("accept", "restore", "adjust", "unreviewed")}
    removed = dict(counts)
    for row in rows:
        verdict = row["verdict"].strip().lower()
        if verdict not in counts:
            raise ValueError("Unknown verdict; use accept, restore, adjust or unreviewed")
        counts[verdict] += 1
        if float(row["automatic_removed_seconds"]) > 0:
            removed[verdict] += 1
    reviewed = sum(counts.values()) - counts["unreviewed"]
    reviewed_cuts = sum(removed.values()) - removed["unreviewed"]
    return {"sample_rows": len(rows), "reviewed_rows": reviewed, "verdicts": counts,
            "review_coverage": reviewed / len(rows) if rows else None,
            "reviewed_automatic_removal_rows": reviewed_cuts,
            "restoration_rate": removed["restore"] / reviewed_cuts if reviewed_cuts else None,
            "adjustment_rate": removed["adjust"] / reviewed_cuts if reviewed_cuts else None,
            "accepted_removal_rate": removed["accept"] / reviewed_cuts if reviewed_cuts else None,
            "scope": "Human-labeled sample rows only; not an estimate of whole-recording accuracy",
            "editorial_acceptance": "Requires recorded human judgment of meaning, takes and joins"}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("labels", type=Path)
    args = parser.parse_args()
    print(json.dumps(summarize(args.labels), indent=2))
