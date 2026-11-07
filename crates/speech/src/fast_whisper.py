"""Offline CPU speech worker. JSON requests followed by little-endian mono float32 PCM.

The parent owns this process and terminates it on cancellation. No URLs or credentials
are accepted by this protocol; model acquisition is a separate explicit action.
"""
import argparse
import importlib.metadata
import json
import os
import sys

PINS = {"faster-whisper": "1.2.1", "ctranslate2": "4.8.3", "onnxruntime": "1.23.2",
        "av": "17.1.0", "tokenizers": "0.23.3", "numpy": "2.2.6", "huggingface-hub": "2.2.0"}


def emit(value):
    # Windows pipe encodings can otherwise produce non-UTF-8 bytes for English
    # names/punctuation. ASCII JSON escapes preserve the exact Unicode text.
    print(json.dumps(value, ensure_ascii=True, allow_nan=False), flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--model-dir")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    for name, expected in PINS.items():
        if importlib.metadata.version(name) != expected:
            raise RuntimeError("Local speech runtime version mismatch; reinstall the runtime")
    os.environ["HF_HUB_OFFLINE"] = "1"
    os.environ["TRANSFORMERS_OFFLINE"] = "1"
    os.environ["TOKENIZERS_PARALLELISM"] = "false"
    import numpy as np
    from faster_whisper import WhisperModel
    if args.check:
        # Import the CPU/VAD dependencies as part of installation verification.
        import onnxruntime  # noqa: F401
        emit({"event": "ready", "protocol": 1})
        return
    model = WhisperModel(args.model_dir, device="cpu", compute_type="int8", local_files_only=True,
                         cpu_threads=max(1, min(8, (os.cpu_count() or 4) - 1)))
    emit({"event": "ready", "protocol": 1})
    source = sys.stdin.buffer
    while line := source.readline():
        request = json.loads(line)
        count = request["samples"]
        if not isinstance(count, int) or not 0 < count <= 302 * 16000:
            raise ValueError("Invalid bounded PCM request")
        raw = bytearray()
        while len(raw) < count * 4:
            block = source.read(count * 4 - len(raw))
            if not block:
                return
            raw.extend(block)
        audio = np.frombuffer(raw, dtype="<f4")
        if not np.isfinite(audio).all():
            raise ValueError("Nonfinite PCM request")
        language = request.get("language") or "en"
        if language != "en":
            raise ValueError("The selected speech profile is English only")
        segments, info = model.transcribe(audio, language=language, word_timestamps=True,
                                          beam_size=5, vad_filter=True, condition_on_previous_text=False)
        words = []
        for segment in segments:
            for word in segment.words or []:
                if word.end > word.start and word.word.strip():
                    words.append({"text": word.word.strip(), "start": float(word.start), "end": float(word.end),
                                  "confidence": float(word.probability)})
            emit({"event": "progress", "seconds": float(segment.end)})
        emit({"event": "result", "language": info.language, "words": words})


if __name__ == "__main__":
    try:
        main()
    except Exception:
        # Provider internals and transcript material stay out of errors/logs.
        emit({"event": "error", "message": "Offline speech worker failed; verify the installed runtime and model"})
        sys.exit(1)
