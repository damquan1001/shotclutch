"""Compare two M0 measurement directories without inventing performance gates."""
import argparse
from collections import defaultdict
import json
from pathlib import Path
from statistics import median


METRICS = {
    "decode": ("fps",),
    "seek": ("p50_ms", "p95_ms"),
    "playback": ("drop_pct",),
    "scrub": ("seek_p95_ms", "timeouts"),
    "timeline": ("update_p95_ms",),
    "export": ("fps",),
    "project": ("p50_ms",),
}
IDENTITY = ("fixture", "scenario", "scene", "view", "res", "mode", "format", "what", "clips")


def load(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def grouped(rows):
    result = defaultdict(list)
    for row in rows:
        key = tuple((k, str(row[k])) for k in IDENTITY if k in row)
        result[key].append(row)
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("before", type=Path)
    parser.add_argument("after", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    before, after = load(args.before / "manifest.json"), load(args.after / "manifest.json")
    if before["settings"] != after["settings"]:
        raise ValueError("Settings differ; regenerate a comparable pair")
    for field in ("cpu", "logical_cores", "os", "memory_bytes", "power_scheme"):
        if before["hardware"].get(field) != after["hardware"].get(field):
            raise ValueError(f"Hardware/profile differs: {field}")
    if before["rustc"] != after["rustc"]:
        raise ValueError("Toolchains differ")
    hashes = lambda manifest: {f["path"]: f["sha256"] for f in manifest["fixtures"]}
    if hashes(before) != hashes(after):
        raise ValueError("Fixture corpus differs; regenerate a comparable pair")
    lines = [
        "# M0 CPU benchmark comparison", "",
        f"Inherited revision: `{before['revision']}`. Milestone revision: `{after['revision']}`.", "",
        "Software decode and CPU composition, quick workloads, identical fixture hashes. "
        "Fixture preparation is excluded from measurement. OS filesystem caches are uncontrolled. "
        "These are descriptive baselines, not qualification against the release budgets in document 14.", "",
        "Repeated rows are summarized by the median of the harness-reported values; this is not a pooled percentile. "
        "Quick seek percentiles have small sample counts. FPS is higher-is-better; latency, timeouts, and drop percentage are lower-is-better. "
        "A zero-frame playback case or high drop rate remains a failed real-time workload even when the benchmark process completes.", "",
        "| Section / case | Metric | Upstream | ShotClutch | Change |",
        "|---|---|---:|---:|---:|",
    ]
    for section, metrics in METRICS.items():
        left = grouped(load(args.before / f"{section}.json")["rows"])
        right = grouped(load(args.after / f"{section}.json")["rows"])
        if left.keys() != right.keys() or not left:
            raise ValueError(f"Missing or different cases: {section}")
        for key in left:
            if len(left[key]) != len(right[key]):
                raise ValueError(f"Case sample counts differ: {section} {key}")
            label = ", ".join(f"{k}={v}" for k, v in key).replace("|", "\\|")
            for metric in metrics:
                values = lambda rows: [r[metric] for r in rows if isinstance(r.get(metric), (int, float))]
                a, b = values(left[key]), values(right[key])
                if not a or not b:
                    continue
                a, b = median(a), median(b)
                delta = f"{(b / a - 1) * 100:+.1f}%" if a else "n/a"
                lines.append(f"| {section}: {label} | {metric} | {a:.3f} | {b:.3f} | {delta} |")
    lines += ["", "Raw section results, process resource measurements, hardware/software profiles, "
              "and corpus hashes are retained beside each manifest. No regression pass/fail is inferred from a single quick comparison.", ""]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text("\n".join(lines), encoding="utf-8")
    print(f"Comparison written: {args.output}")


if __name__ == "__main__":
    main()
