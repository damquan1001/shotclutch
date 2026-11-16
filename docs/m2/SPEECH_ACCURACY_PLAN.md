# Smart Cut speech accuracy plan

Assessment updated 2026-11-16. The local benchmark and Context-aware V5 matcher are implemented. Verbatim filler recovery, audio-based take ranking and semantic adjudication remain proposed. Branch `codex/m2-smart-rough-cut` retains the automatic-cut-then-review workflow. See the [benchmark procedure](../../scripts/m2/SPEECH_BENCHMARK.md) and [redacted results](EVIDENCE.md).

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

**Full-recording follow-up, 2026-11-16**

The V5 project contains 40.101 seconds of still-retained bad-take audio across the labeled excerpts. A local experiment indexed uncommon two- and three-word phrases across bounded spans of up to four adjacent clauses and compared matches up to 150 seconds apart. Ordered alignment flagged 21.517 of those seconds using the saved Tiny English transcript. The union with candidates from the saved Whisper Base transcript reached 29.737 seconds when measured against the same V5 kept ranges. Tiny retrieval also flagged 113.058 seconds of explicit keeps. An earlier-take deletion policy would have removed 16.467 seconds of labeled bad takes and 76.200 seconds of explicit keeps. Requiring high mutual overlap and agreement on detected negation, numbers, point of view and certainty reduced both counts to 1.720 and 27.024 seconds.

These are interval-coverage measurements from one recording and existing editorial labels. Span counts include overlapping windows. The test models candidate discovery and hypothetical deletion policies. The 19:49 reviewed project still contains the V5 cut. Use the second transcript for its incremental candidate coverage when qualifying selective local retranscription. A complete second pass measured the possible gain, and the smaller production workload still needs to be measured.

1. **Find repeated passages in both directions.** Index short distinctive phrases from the saved transcript, with source-time limits and bounded candidates per phrase. Expand each hit across nearby clause boundaries and align the words in order. Form groups of overlapping related attempts so one keeper can cover several false starts. Verify each proposed deletion directly against the selected survivor.
2. **Distinguish an unfinished attempt from a distinct line.** Use truncated clauses, repeated self-corrections, retry cues and continuity into surrounding lines. Compare the claim, subject, quantities, negation, certainty and any unique clause before removing speech. When multiple complete takes survive these checks, use evidence of interrupted speech, clipping or distracting noise to rank them. If the evidence is inconclusive, preserve the speech. ASR confidence can guide transcription checks.
3. **Recheck difficult regions with local audio.** Trigger short second-pass transcription around several likely attempts, word gaps with continuing speech and boundaries where the alignments disagree. Test candidate recovery and runtime against the full saved Base comparison. Align accepted cuts to the audio and valid presentation frames while preserving the lead-in to retained words.
4. **Qualify automatic cuts on fixed examples.** Score added bad-take removals, loss in confirmed keeps, coverage of matched export audio and cut boundaries. Use the provided labels and source-time blocks that were excluded from tuning. The first release gate is more correctly removed speech than V5 with zero additional loss across the confirmed keep ranges. Evaluate another recording before claiming general accuracy.

V1 required near-identical utterances. V2 added ordered matching across adjacent clauses and guarded against changed speakers, names, negation, numbers and unmatched content. V3 and V4 found additional interrupted openings and retry cues. V5 follows short aborted attempts to a complete take when the surrounding words corroborate the match. Setup → Automatic → Context-aware takes (preview) → Update cut applies the latest version to saved timed words. [Version history and qualification](EVIDENCE.md)

### 4. Add semantic adjudication only where it improves the benchmark

The first local entailment trial found two candidates on the saved and Base transcripts. One had a change in certainty and point of view that the model accepted as equivalent. Use the paired listening clips to qualify editorial choices and validate explicit fact and certainty guards before integrating automatic removal. [Trial evidence](EVIDENCE.md#selective-speech-verification--2026-11-09)

The multi-clause review set contains seven confirmed keep choices. The editor selected two earlier takes even though only their later alternatives matched export excerpts. Use these choices to check the take ranking independently of the export alignment. Preserve the selected audio and verify the boundaries and joins before counting the alternate spans as successful removals. [Review set evidence](EVIDENCE.md#multi-clause-retake-review-set--2026-11-10)

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
