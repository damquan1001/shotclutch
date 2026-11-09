"""Compare mutual entailment on a completed private speech verification report."""

import argparse
import json
from pathlib import Path
import time

import numpy as np

from compare_speech import MODELS
from speech_benchmark import digest, private_output

ARTIFACTS = {
    "model_quint8_avx2.onnx": "21b14751a95520953bfcc607ceeb617de7cbeaeb6d60f4c8966716c743985337",
    "tokenizer.json": "5124ef2ead1a10a717703bc436de7f353da76d6340e4587719b42b1693707964",
    "config.json": "8d9f07bf7ba54a6fc3b1962483056f94c39dcf188db4cf61843e1c88f94b2342",
}
LABELS = ("contradiction", "entailment", "neutral")
CONTROLS = {
    "equivalent": [
        ("I charged it for two hours before leaving.", "Before I left, I spent two hours charging it."),
        ("The charger is included in the box.", "The box comes with a charger."),
        ("The camera has no built-in microphone.", "There is no microphone built into the camera."),
        (
            "I was able to read subtitles from across the room.",
            "From across the room I could still read the subtitles.",
        ),
        ("The battery died after three hours.", "It ran out of battery in three hours."),
        ("The glasses let me use a wired headset.", "I could connect a wired headset to the glasses."),
        ("There is a case and a charging cable in the box.", "The box contains a charging cable and a case."),
        (
            "Once the screen stopped flickering, I started recording.",
            "I began recording after the screen stopped flickering.",
        ),
    ],
    "changed": [
        ("The glasses cost 399 dollars at launch.", "The glasses cost 499 dollars at launch."),
        ("The camera records at thirty frames per second.", "The camera records at sixty frames per second."),
        ("The glasses support an external microphone.", "The glasses do not support an external microphone."),
        ("The camera included a Sony lens.", "The camera included a Canon lens."),
        (
            "You should charge the camera before attaching the lens.",
            "You should charge the camera after attaching the lens.",
        ),
        ("I returned the camera after it broke.", "She returned the camera after it broke."),
        ("The device works with every headset.", "The device works with some headsets."),
        ("I prefer the black model to the silver one.", "I prefer the silver model to the black one."),
    ],
    "extra": [
        ("The glasses fit inside the case.", "The glasses fit inside the case and the case has a spare battery."),
        ("The microphone sounds clear.", "The microphone sounds clear but has a noticeable delay."),
        ("The display gets brighter indoors.", "The display gets brighter indoors and flickers outdoors."),
        ("The box comes with a charger.", "The box comes with a charger and a replacement cable."),
    ],
    "certainty": [
        (
            "You might hear a click when it starts, but it is quiet during use.",
            "I heard a click when it started, but it was quiet during use.",
        ),
        ("It probably works with an adapter if you need one.", "I tested it with an adapter and it worked."),
    ],
}


def entails_both(row, floor):
    return all(side["entailment"] >= floor for side in row)


def run(args):
    from onnxruntime import InferenceSession, SessionOptions
    from tokenizers import Tokenizer

    private_output(args.output)
    source = json.loads(args.speech_report.read_text(encoding="utf-8"))
    pairs = source["pairs"]
    if not source["complete"] or not 0 < len(pairs) <= 20:
        raise ValueError("Use a completed speech verification report with bounded take pairs")
    for filename, expected in ARTIFACTS.items():
        if digest(args.model / filename) != expected:
            raise ValueError(f"Pinned model artifact mismatch: {filename}")
    tokenizer = Tokenizer.from_file(str(args.model / "tokenizer.json"))
    options = SessionOptions()
    options.intra_op_num_threads = 4
    options.inter_op_num_threads = 1
    started = time.monotonic()
    session = InferenceSession(
        str(args.model / "model_quint8_avx2.onnx"), sess_options=options, providers=["CPUExecutionProvider"]
    )
    names = {i.name for i in session.get_inputs()}
    if not {"input_ids", "attention_mask"} <= names <= {"input_ids", "attention_mask", "token_type_ids"}:
        raise ValueError("Unexpected semantic model inputs")

    def score(a, b):
        if not a or not b:
            return dict(zip(LABELS, (0.0, 0.0, 1.0), strict=True))
        if len(a) > 4096 or len(b) > 4096:
            raise ValueError("Take text exceeds verification bounds")
        encoding = tokenizer.encode(a, b)
        if len(encoding.ids) > 256:
            raise ValueError("Take pair exceeds the token budget")
        values = {
            "input_ids": encoding.ids,
            "attention_mask": encoding.attention_mask,
            "token_type_ids": encoding.type_ids,
        }
        logits = np.asarray(
            session.run(None, {n: np.array([values[n]], dtype=np.int64) for n in names})[0], dtype=float
        ).reshape(-1)
        if logits.size != len(LABELS) or not np.isfinite(logits).all():
            raise ValueError("Unexpected semantic model output")
        probs = np.exp(logits - logits.max())
        return dict(zip(LABELS, (probs / probs.sum()).tolist(), strict=True))

    def both(a, b):
        return score(a, b), score(b, a)

    controls = {kind: [both(a, b) for a, b in rows] for kind, rows in CONTROLS.items()}
    scores = {"saved": [both(*(" ".join(side) for side in pair["baseline"])) for pair in pairs]}
    for model in MODELS:
        decoded = source["runs"][model]["pairs"]
        if len(decoded) != len(pairs):
            raise ValueError("Recognition results do not cover the same take pairs")
        scores[model] = [both(*(" ".join(w["text"] for w in side) for side in row)) for row in decoded]
    summary = {}
    for floor in (0.85, 0.95):
        summary[str(floor)] = {
            "control_acceptance": {k: sum(entails_both(row, floor) for row in rows) for k, rows in controls.items()},
            "source_acceptance": {k: sum(entails_both(row, floor) for row in rows) for k, rows in scores.items()},
            "saved_and_base_pair_numbers": [
                i
                for i, (a, b) in enumerate(zip(scores["saved"], scores[MODELS[1]], strict=True), start=1)
                if entails_both(a, floor) and entails_both(b, floor)
            ],
        }
    report = {
        "speech_report_sha256": digest(args.speech_report),
        "model_files": ARTIFACTS,
        "controls": controls,
        "scores": scores,
        "summary": summary,
        "seconds": time.monotonic() - started,
        "human_verified_labels": 0,
    }
    with args.output.open("x", encoding="utf-8") as output:
        json.dump(report, output, indent=2, allow_nan=False)
    print(json.dumps(summary), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("speech-report", "model", "output"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    run(parser.parse_args())
