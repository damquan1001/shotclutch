# Local speech-accuracy benchmark

These tools prepare private evidence for improving recognition and take selection. They do not change the desktop's cut rules or claim a model is more accurate from transcript length. Python with NumPy and FFmpeg/FFprobe on PATH is required; the installed fast-speech runtime supplies the pinned recognition packages. Tests use synthetic audio, not a model or private media.

## Prepare and align

Export the canonical model pins, then choose a fresh ignored/private output folder:

```powershell
cargo run --locked -q -p shotclutch-speech --example model_manifest > target/m2-qualification/speech-catalogue.json
python scripts/m2/speech_benchmark.py --source '<original>' --reference '<human export>' --project '<same-source completed analysis.scproj>' --output target/m2-qualification/accuracy-run
```

The optional project supplies the current automatic removal map and ordered manual overrides. It must bind the same original SHA-256. Source and reference are streamed through an 8 GiB per-file verification budget, decoded from their first audio tracks to local 8 kHz mono PCM, and verified again before completion. Spectral retrieval finds possible locations; waveform correlation refines them. Distinct competing locations remain ambiguous, low energy/poor matches stay unmatched, and reordered passages are permitted. Music, time stretching, effects or edits inside a query may prevent matching. The default two-second windows are anchors, not an interpolated edit-decision list.

`benchmark.json` stores inputs, settings, automatic hypotheses and deterministic excerpts. `alignment-review.csv` holds human verification; `alignment-audit.html` provides paired local clips for provisional conflicts and a spread of other windows. **Do not assume an automatic match is correct or infer a deletion reason from a missing source interval.** Mark verified matches `yes`, rejected matches `no`, and uncertain rows `unreviewed`. Correct source times before marking a match verified.

`speech-labels.csv` contains source-time excerpts, strata, temporal blocks and development/holdout assignments. Excerpts do not cross their block boundary. Random excerpts help expose failures that candidate-driven sampling misses. This small single-recording set is an initial diagnostic; inspect the actual split counts and expand holdout coverage/recordings before model selection or release claims.

## Explicit model acquisition and comparison

The tool never downloads weights during comparison. To install missing pinned Base English weights explicitly:

```powershell
python scripts/m2/compare_speech.py --catalogue target/m2-qualification/speech-catalogue.json --models '<private data>/models' --install-model faster-whisper-base.en
python scripts/m2/compare_speech.py --catalogue target/m2-qualification/speech-catalogue.json --models '<private data>/models' --benchmark target/m2-qualification/accuracy-run --python '<private runtime>/Scripts/python.exe'
```

Installation checks pinned revision URLs, filenames, sizes and SHA-256; it does not install Python packages. Inference uses the desktop's exact offline worker, validates its runtime pins, and loads one model at a time. Each source excerpt is decoded once to bounded 16 kHz PCM. Both models receive identical bytes, recorded by SHA-256. Completed excerpts are checkpointed with model/worker/benchmark/label hashes. Interrupted reports remain explicitly incomplete. Existing comparisons/labels are not overwritten; use `--output '<fresh private comparison folder>'` for a new inference run.

The comparison produces JSON evidence and a local `comparison/review.html` with source audio and collapsible Tiny/Base hypotheses. Listen and write verbatim labels before revealing model text to reduce anchoring bias. Word and filler counts are observations, not accuracy. This excerpt recipe differs from full five-minute production inference contexts; do not equate their output or extrapolate throughput without measurement.

## Human labels and scoring without recognition

Fill `verbatim_text` in `speech-labels.csv` and mark `transcript_verified=yes` only after listening. Include hesitations, repetitions and cut-off speech rather than copying a cleaned model transcript. A verified silent excerpt can have an empty transcript. Leave uncertain text unverified.

`events.json` starts as an empty array. Add only listened-to events, with absolute original-source seconds inside the named excerpt:

```json
[{"sample_id":"s000","category":"filler","start":102.15,"end":102.48}]
```

Supported label categories are `filler`, `restart`, `bad_take`, `retake`, `intentional_repetition`, `changed_fact`, `natural_pause`, `breath`, and `other`. Mark `events_verified=yes` only when the entire excerpt's events have been checked, including negative cases. Do not silently omit uncertain boundary events and then mark the window complete.

```powershell
python scripts/m2/compare_speech.py --benchmark target/m2-qualification/accuracy-run --score-run target/m2-qualification/accuracy-run/comparison --baseline-project '<same-source analysis.scproj>'
python -m unittest discover -s scripts/m2 -p test_speech_benchmark.py -v
```

Scoring writes a new hash-addressed score artifact, leaving inference evidence untouched. Unverified scores remain null. WER records substitutions/deletions/insertions; fillers use one-to-one event matching (IoU ≥0.3 or midpoint separation ≤200 ms) with precision/recall and matched-event boundary error. Tolerances are explicit initial evaluation settings, not calibrated guarantees. The current scorer evaluates ASR/filler recovery, not a new restart/take classifier. Those categories are collected for the next implementation slice.

Verified source/export matches can also report overlap with a supplied cut map. This is a **reference-performance conflict**, not automatically a semantic error or a bad-take label: another performance could convey the same content. Unreviewed matches and unmatched reference audio never become negative ground truth. Multiple windows may describe one editorial decision; do not count them as independent bad takes.

All media, excerpts, transcripts, notes, labels and detailed alignments belong in ignored local storage. Tools reject private output into non-ignored repository folders. Commit only code, synthetic tests and redacted aggregate evidence. Optional cloud recognition remains a separate explicitly selected workflow.

## Short-window verification

Run the trial with the pinned speech interpreter and existing Tiny and Base English weights. The current project supplies the saved words and cuts. The earlier project supplies pairs that legacy cleanup would have treated as repeated takes. Both must bind the same original and main audio track.

```powershell
& '<private runtime>/Scripts/python.exe' scripts/m2/verify_speech.py --benchmark target/m2-qualification/accuracy-run --project '<current cut.scproj>' --legacy-project '<legacy cut.scproj>' --catalogue target/m2-qualification/speech-catalogue.json --models '<private data>/models' --output target/m2-qualification/verification-run
```

The runner samples up to 36 periodic gaps, 12 quiet controls and 20 retained take pairs. It compares plain and filler-seeded decoding on the same short PCM excerpts with VAD disabled. The report records input hashes, the installed model pins, hypotheses and lexical overlap. Open `review.html` to listen locally. Voicing and word overlap are screening evidence. A missing hypothesis leaves filler recall unknown until the audio is labeled.

The optional text trial reads a completed report and checks entailment in both directions on the saved words and both new transcriptions. It also tests known paraphrases, changed facts, added details and changes in certainty. Install the published `onnx/model_quint8_avx2.onnx`, `tokenizer.json` and `config.json` as separate local files from the pinned model revision. The runner verifies their hashes and uses the pinned interpreter's ONNX Runtime and tokenizer. [Pinned DeBERTa model revision](https://huggingface.co/cross-encoder/nli-deberta-v3-xsmall/tree/03d06b2f6f06023bd596151a16477e2fcec601fb)

```powershell
& '<private runtime>/Scripts/python.exe' scripts/m2/verify_take_meaning.py --speech-report target/m2-qualification/verification-run/report.json --model '<private DeBERTa folder>' --output target/m2-qualification/verification-run/meaning.json
```

Use the reported pair numbers to find the corresponding audio on the listening page. The reported 0.85 and 0.95 thresholds describe this trial. Editorial decisions also require checking the words, context and performance.
