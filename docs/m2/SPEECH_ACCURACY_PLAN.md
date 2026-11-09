# Smart Cut speech accuracy plan

Assessment: 2026-11-09. The local benchmark/Tiny–Base comparison and an opt-in ordered take matcher are implemented. Verbatim/acoustic filler recovery and semantic adjudication remain proposed; editorial accuracy remains unqualified. Branch: `codex/m2-smart-rough-cut`. The existing automatic-cut-then-review workflow remains the product contract. See the [benchmark procedure](../../scripts/m2/SPEECH_BENCHMARK.md) and [redacted results](EVIDENCE.md).

## Diagnosis from the current implementation

`crates/engine/src/preedit/gling.rs` detects English fillers through a fixed vocabulary applied to recognized words. A hesitation omitted by ASR cannot reach that detector. The qualified Tiny English recording produced 3,935 words and zero raw filler candidates. This is evidence of a detection-coverage problem to investigate, not proof of how many fillers were spoken.

Takes are grouped by timed gaps, opening matches inspect up to five words over five preceding takes, and restart rules compare token patterns/suffixes. Similar opening words can remove intervening attempts; the later attempt is generally favored. These rules neither understand whether two utterances carry the same information nor assess delivery quality. They can miss rephrased/distant retakes and mistake intentional repetition or changed facts for redundancy. Silence accuracy, source-frame correctness and matching the reference implementation do not establish editorial accuracy.

The local fast worker uses Tiny English CPU INT8, beam five, VAD and word timestamps. Its speed is useful, but transcript readability and verbatim disfluency recovery are different objectives. A larger general-purpose model is a benchmark candidate, not a guaranteed filler fix. The existing larger English profile can provide an initial comparison without changing backends.

The AssemblyAI request already sets `disfluencies: true` and `format_text: false`. Its live recognition quality remains unqualified. There is no missing flag to enable here; any account test requires the existing explicit cloud selection and upload action.

## Recommended implementation order

### 1. Establish an editorial benchmark before changing more thresholds

Align the original audio with the supplied human-edited export locally. Use audio fingerprints/correlation and piecewise sequence alignment to locate the actual performances retained, including identically worded alternate takes. Detect and leave unmatched regions unresolved; B-roll, processing, music, reordered passages or transitions can defeat a simple monotonic match. Manually verify a stratified sample of matches.

The export supplies retained-performance evidence, not reasons for every deletion. Do not label all removed material as a bad take or filler, or treat unaligned audio as a deletion. Add source-local labels for filled pauses, restarts, genuine retakes, intentional repetition, changed facts and natural pauses. Keep transcripts, locations, labels and alignments private. Commit only aggregate results and synthetic regressions.

Measure event precision/recall by category, retained-word preservation, wrong-take selection, boundary error/audible joins, human restoration rate, processing time and memory. Establish the baseline and choose explicit budgets before tuning. Use held-out spans and subsequently another recording; duration matching to the 11.5-minute export is not an accuracy target.

### 2. Recover verbatim speech and detect fillers from audio

The bounded Tiny/Base trial returned zero filler hypotheses in the selected gaps. The Vosk check produced one weak hypothesis that disappeared with competing words. The approved local Uhm trial also returned zero in the selected gaps. Its five full-recording candidates all overlapped saved words. The candidate clips and natural positive and negative labels remain the basis for a further editorial accuracy check. [Uhm trial evidence](EVIDENCE.md#uhm-acoustic-filler-trial--2026-11-10) · [Earlier trial evidence](EVIDENCE.md#selective-speech-verification--2026-11-09)

Keep Tiny as the fast first pass. Benchmark the existing Base English profile on a labeled excerpt set, and evaluate a specialist verbatim ASR or acoustic disfluency detector. Evaluate disfluency recall separately from ordinary word error rate. Check runtime, distribution terms and deployment requirements before selecting an additional model; do not silently install or upload media.

Add bounded audio verification around uncertain utterances, cut-off syllables, voiced regions unexplained by the transcript and plausible hesitation events. Candidate discovery must use acoustic evidence as well as ASR text: restricting verification to Tiny's recognized fillers would preserve its blind spots. Include random/complete labeled spans to measure missed events outside triggered windows. Widen to full verbatim recognition if selective verification cannot meet recall targets.

Merge verified events into source-time evidence with provenance. Distinguish filler sounds from breaths, laughter, acknowledgments and meaningful words. Do not globally blacklist words such as “like”, “so” or “you know”. A transcription prompt alone is not a reliable recovery mechanism.

### 3. Replace short opening heuristics with utterance alignment and take ranking

**Implemented preview `takeMatching: "orderedV2"`.** Setup → Automatic → Context-aware takes (preview) → Update cut uses the saved transcript. V1 required near-identical utterances and missed attempts split at different sentence boundaries or affected by recognition errors. V2 compares short runs of adjacent clauses and requires high ordered overlap. It checks speaker, negation, numbers, names and unmatched content, then removes only the matched attempt. It can tolerate one low-confidence unmatched token in a sufficiently long unfinished attempt. Recognition confidence measures the text evidence, while completion and continuity provide take-selection cues. V1 remains readable and reproducible from its saved strategy. Paraphrased retakes, omitted fillers and editorial performance selection still need separate evidence.

Build utterance spans from timing, syntax and acoustic cues, independently of pause-pacing settings. Retrieve possible retakes over a wider bounded context, then align ordered token sequences using edit distance/local sequence alignment. Track repeated prefixes, abandoned continuations, partial restarts and later corrections without deleting unrelated intervening material.

Separate detection (these spans may be retakes) from selection (which attempt should survive). Prefer complete coverage of the intended statement; preserve differences in negation, numbers, names and additional facts. Use clipping, masking noise and interrupted delivery as measurable signals, while acknowledging that confidence/loudness do not establish the best performance. Preserve complementary content and intentional emphasis. The final attempt must not win merely because it is last.

### 4. Add semantic adjudication only where it improves the benchmark

The first local entailment trial found two candidates on the saved and Base transcripts. One had a change in certainty and point of view that the model accepted as equivalent. Use the paired listening clips to qualify editorial choices and validate explicit fact and certainty guards before integrating automatic removal. [Trial evidence](EVIDENCE.md#selective-speech-verification--2026-11-09)

The multi-clause review set contains six reference-backed candidate pairs and one control with audio from both alternatives in the export. The waveform placements have been rechecked. Listening decisions will establish which complete spans can be removed and which lines or boundaries need protection. [Review set evidence](EVIDENCE.md#multi-clause-retake-review-set--2026-11-10)

For ambiguous/rephrased matches, evaluate a bounded language-model verifier using nearby transcript context and optional audio-derived evidence. Its output must reference existing word/utterance IDs and relations such as duplicate, restart, correction, complementary content or intentional repetition. Validate coverage and factual changes deterministically. A text-only model cannot judge delivery or hear an omitted filler.

The verifier must not invent text, timestamps, clip lengths or arbitrary deletion spans. An uncertain decision leaves the source retained in the automatic result and accessible through existing review controls; it does not introduce an approval queue. Calibrate automation thresholds from held-out labels rather than treating heuristic scores or model self-confidence as probabilities. Keep any remote processing an explicit provider choice.

### 5. Refine timing after deciding what to remove

Evaluate phoneme/word forced alignment on verified cut neighborhoods, then combine it with local acoustic evidence and existing speech guards. Alignment improves where supplied words occur; it does not recover missing fillers or decide which take is bad. Preserve the current natural-pause behavior, joined-removal-before-frame-snapping logic, source/sample alignment, fades, captions, manual overrides and grouped history.

Version new recognition/event/classification evidence and its settings. Reanalysis must preserve compatible decisions and surface conflicts. Opening an old project must not silently reclassify its cuts. Run analysis/model work off the UI thread with bounded PCM, cancellation and acknowledged completed coverage; do not regress normal cut/uncut latency.

## First implementation slice

Build the private source/export alignment and category-label benchmark, plus a comparison runner for Tiny and the already available Base English profile on the same excerpts. Record filler/restart omissions, take-selection errors and timing errors separately. Then implement ordered utterance matching and acoustic/verbatim verification against those cases. Select a specialist or semantic model only after an evidence-based comparison establishes its contribution and cost.

This slice avoids another round of global silence/phrase-threshold tuning and supplies reproducible failure cases for each subsequent change. All existing project, permission, source-change, recovery and apply gates remain required.

## Primary references

- [AssemblyAI filler-word documentation](https://www.assemblyai.com/docs/pre-recorded-audio/include-filler-words): explicit disfluency preservation; the current integration already requests it.
- [CrisperWhisper research paper](https://arxiv.org/abs/2408.16589): verbatim recognition, filler detection and word timing as explicit recognition objectives. Treat its published results as research evidence, not a Shotclutch benchmark.
- [Current CrisperWhisper project](https://github.com/nyrahealth/CrisperWhisper): candidate specialist direction; current model generations have deployment/license constraints and must not be assumed to be drop-in equivalents.
- [WhisperX paper](https://arxiv.org/abs/2303.00747) and [implementation](https://github.com/m-bain/whisperX): separate ASR/VAD/alignment stages and documented limitations.
- [Filler Word Detection and Classification](https://arxiv.org/abs/2203.15135): dedicated acoustic-event detection as an alternative to transcript-only vocabulary matching.
