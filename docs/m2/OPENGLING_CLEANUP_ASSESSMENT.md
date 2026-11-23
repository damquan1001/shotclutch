# OpenGling cleanup adaptation

## Local second-pass review

The reference for the reviewer is OpenGling’s src/local-llm.js (reviewBadTakesWithOllama) and src/speech-cleanup.js (analyzeReviewedSpeechCleanup). Gling’s hosted model and its production quality are unknown. Smart Cut keeps its own llama.cpp backend. Each request contains up to 200 editable words with 40 context words on each side, readable takes, index-to-text mapping, baseline bad-take indices, and protected indices. The response lists *all* words to discard, including words that the rules kept. No rule candidate is required to create a cut.

The command checks the checkpoint and aligned transcript, applies only changes from the baseline, and reports reviewed, newly cut, confirmed and restored word counts. An omitted unprotected rule cut is restored; omission in an unreviewed block has no effect. Foreign, duplicate, or protected indices cause block fallback. An edit without a realizable media boundary leaves its block untouched. Removing every timed word rejects the review even if silent timeline material would remain. A conservative additional check rejects new cuts in a longer reading immediately following its exact unfinished prefix. For blocks with at least 16 editable words, selecting at least 80% as entirely new cuts also triggers fallback. On a 102-word recording excerpt, Granite confirmed all 15 supplied cut indices; with those hints withheld it selected 100 words, including 64 touching J08 (keep). The latter response is rejected. These checks are not a general semantic quality guarantee. Keep the reviewer opt-in. The model prompt stays local; the original rule map remains available via Undo.

## First-pass comparison — 2026-11-20

The raw word rules compare the first five words against up to five prior takes, find restarts within one take, and compare the preceding take’s suffix with the next opening. A later matching opening can discard intervening takes, including unrelated speech. These rules do not know whether repetition was intentional.

With **Context-aware takes → Apply transcript rules first (experimental)** (V7) the reference word marks are applied first. Supplemental English matches cannot discard a reference keeper or rely on an already discarded reading. Take grouping uses the configured pause threshold. The reference filler lists use the base language tag. Saved automatic maps remain snapshots. Existing context-aware updates stay on V6 unless V7 is explicitly selected. On the audio-rechecked transcript, V6 retake proposals overlapped 16.860 s of the 55.517 s marked bad and 0.077 s of the 71.487 s marked keep. V7 overlapped 49.100 s bad and 65.987 s keep; reference rules alone overlapped 46.485 s bad and 65.987 s keep. These are proposed source-time intersections before snapping, manual overrides, or model review.

With V7 and natural pause preservation on, an internal gap shorter than two seconds between takes needs at least 50% measured quiet and a continuous 150 ms quiet run, unless both takes were discarded. Missing audio measurements leave the transcript decision intact. This adapts OpenGling’s short-gap check to ShotClutch’s stored quiet intervals; its separate local dB estimation and edge-placement algorithm are not reproduced.

The recorded Speechmatics-shaped oracle in OpenGling’s tests/fixtures/cleanup-parity-expected.json (artifact hash beginning 86db5fe6) supplies 369 authored transcripts with three settings each. The compact local fixture stores integer microseconds, per-word expected categories, and merged intervals. The regression checks raw decisions and intervals, and that V7 retains reference bad-take classifications. This is a different snapshot from the 1,128-case Gling 1.73 study; these counts do not establish full application or model parity.

## Local reference assessment

Reviewed the local OpenGling 0.12.0 source, specifically `src/speech-cleanup.js`, `src/acoustic-cleanup.js`, `src/cleanup-state.js`, `src/transcribe.py`, `src/transcription.js`, `public/app.js`, and their tests. These are implementation references. The requested workflow remains automatic cutting followed by review. Updated 2026-11-08 after the user's parity request.

## Parity implementation

### Pause pacing refinement — 2026-11-08

Following the user's overcutting report, new native workspaces enable **Preserve natural pauses**, keeping gaps up to 800 ms between retained words. This is independent of the 500 ms take-grouping threshold: repeated-take/filler classification is unchanged. Longer pauses retain the selected total breathing room (150 ms by default, split across both sides), with at least 250 ms for sentence gaps. Quiet handles beside filler/within-take removals are preserved only beside retained words. Gaps between discarded takes stay removed. Adjoining removals are unioned before presentation-grid snapping to avoid phantom video frames at shared boundaries. Standalone pause removals shorter than 120 ms after snapping are omitted; short gaps attached to discarded words join their word removal. The same pacing floor applies to silence-only analysis.

The optional `cleanup.preservePausesMs` stores this setting; absence preserves the earlier exact behavior. Disable the checkbox for exact Gling transcript rules. Existing saved maps/settings remain unchanged on open. Enable it and use Update cut to regenerate from saved words; no inference or upload is needed. Manual decisions remain authoritative. The legacy `minPauseMs` still separates takes; the new field controls pause pacing independently. The control is labeled Breathing room in this mode and represents total join context; the older modes retain their per-edge padding semantics.

The supplied human-edited reference was probed and measured locally: 692.011 seconds, with 357 quiet intervals of at least 100 ms at a fixed −42 dBFS threshold; median 135 ms and 95th percentile 305 ms. These measurements support retaining breathing room. They are not speech alignment, individual cut labels or a calibrated accuracy score. Private reference paths, media and detailed measurements remain uncommitted.

New Smart Cut workspaces select **Automatic** (canonical ID `glingCompatible`), natural pause preservation and **Tiny English · Fast** (`faster-whisper-tiny.en`). Existing saved modes and model selections remain unchanged. The earlier Natural behavior is labeled Conservative; the earlier Transcript behavior is labeled Legacy transcript. Omitted command/project mode fields still default to Natural for backward compatibility. The exact-parity behavior below applies with pause preservation disabled. UI labels/help and rendered legacy reasons use neutral product wording; the technical reference and canonical IDs remain documented here.

The new transcript rules follow the reference's English hesitation vocabulary, single trailing-punctuation normalization, pause-based take grouping, five-take lookback, five-word opening membership/consecutive-match tests, removal of intervening abandoned attempts, repeated-token/question patterns, longest contiguous pattern matching, cross-take suffix matching, merged same-kind removals, neighboring word boundaries and overlapping-speech suppression. Leading/trailing gaps and interior gaps greater than the selected pause threshold are cut without added padding. Retained alternatives are stored separately for native comparison and are excluded from automatic removals. Unknown confidence stays unknown. Source presentation/sample boundaries round removals inward; this can retain at most the boundary quantization around a reference interval.

This mode intentionally follows the reference's broader text rules, including single-word suffix matches and take matches without the conservative numeric/negation guards. It is deterministic transcript cleanup, not semantic assessment. Every automatic cut remains restorable. The existing conservative behavior below remains a separate choice rather than silently weakening parity rules.

The Rust rule implementation matches the reference's removal intervals and types on 196 synthetic cases stored in `crates/engine/test-data/opengling-cleanup-v2.json`. The fixture contains no private transcript. Local source and packaged reference modules had the same SHA-256: `bfd5a3f49ccd65fa54ef0e960ee7a178bb973af016253559f53869162c93b0f1`. Exact differential coverage is for the raw English transcript rules on one source with timed words; it does not establish parity for paragraph/media grouping, custom filler lexicons, unsupported languages or optional acoustic refinement.

The reference uses faster-whisper/CTranslate2 CPU INT8 with the 75.5 MB Tiny English model binary, Silero VAD, word timing, beam size five and no previous-text conditioning. Shotclutch now offers that pinned inference profile through an isolated offline worker. A persistent model and five-minute bounded requests replace the legacy profile's 20-second recognition calls. The same decoded PCM feeds recognition, acoustic measurements and waveform peaks. See [DEC-005](DEC-005_SPEECH_PROVIDERS.md) for installation, dependency versions and cancellation. Whole-recording reference inference can still differ from bounded inference around chunk boundaries; identical model settings do not guarantee identical recognized words.

## Earlier conservative adaptation

| Area | OpenGling behavior | Previous Shotclutch limitation | Adaptation |
| --- | --- | --- | --- |
| Pauses | Timed-word gaps, including leading/trailing gaps | Required a quiet interval below a fixed −42 dB ceiling | Transcript-rules mode cuts timed-word gaps. Natural mode intersects them with adaptive quiet audio. |
| Natural boundaries | Noise/speech distributions, protected adjacent words, retained pacing, audible islands | Reapplied padding and minimum duration to every quiet fragment | Apply the pause threshold and pacing once per speech gap, then remove quiet islands inside it. |
| Stereo | Channel-aware energy measurement | Equal-weight downmix could cancel opposite-phase audio | Silence detection uses the loudest channel energy per 10 ms frame. Recognition still uses the selected channel/downmix. |
| Fillers | Normalized hesitation vocabulary | Small vocabulary | Expanded English vocabulary, preserving word evidence and unknown confidence. |
| Repeated takes | Five-word opening comparison across five preceding takes | Nearly identical whole phrases required | Compare opening membership and consecutive matches, retaining the later take. |
| Restarts | Repeated-token patterns and cross-take suffix matching | Only repeated opening at the beginning of an utterance | Contiguous pattern matching within a take and repeated suffix/opening across adjacent takes. |
| Review | Category/status filters, audition, restore, take context | Transcript and range edits with little explanation | Review cuts tab, category and cut/kept filters, original-context audition, take comparison, restoration, boundary marks, keyboard navigation. |
| Reprocessing | Reuse word timing for acoustic refinement | New acoustic measurements required another Whisper run | Background audio refresh reuses the completed transcript after source verification. |

## Intentional differences and limits

The following describes the retained Conservative and Legacy transcript modes. Both can be selected explicitly or loaded from saved projects. No cleanup mode provides semantic judgment of which performance is best.

All modes use integer source ticks, immutable map revisions, indexed video/sample boundary checks and ordinary linked timeline clips. Shotclutch does not embed the OpenGling application. Bounded checkpoints, cancellation, source fingerprint checks, manual override precedence and scoped-client permissions remain in force.

Natural quiet detection uses 10 ms per-channel RMS, rolling four-second distributions, a chunk-level fallback, and a minimum 12 dB separation. The threshold is the smaller of noise +6 dB and speech −12 dB. This adapts the reference's more conservative speech −18 dB threshold to quieter dialogue recordings. Constant low-dynamic-range audio remains untouched unless it falls below −70 dBFS. A long noise-only chunk with no identifiable speech/noise separation can still remain uncut. Transcript mode is available for timed gaps in that situation.

Natural pauses retain at least 250 ms of ordinary pacing or 450 ms after sentence punctuation, subject to the selected padding and available gap. Audible islands remain. Word removals need quiet cutting room and protect 120 ms after the preceding word and 80 ms before the following word. Existing frame alignment and short fades remain responsible for realizable edit boundaries and audio joins. This implementation does not add the reference's sample-minimum search or paragraph pacing because the saved analysis does not retain those samples or paragraph identities.

Take matching stays within 20 seconds and the same speaker label. Cross-take suffix removal requires at least two consecutive words. Unrelated intervening takes are retained. Automatic Natural removal additionally requires an identical earlier attempt or an earlier prefix of at least two words in the later attempt. Both modes preserve alternatives whose numeric/negation markers differ, and automatic restart removal preserves those markers. Opening similarity can still create a review group without authorizing removal. These guards intentionally differ from the reference's broader abandoned-take and single-word suffix heuristics. They do not detect every semantic change; recognition errors and performance choices still require editorial review. Detection confidence remains explicitly unknown rather than presenting a heuristic score as a calibrated probability.

Analyzer v3 independently measures near-silent source edges: contiguous 10 ms bins at or below −65 dBFS, at least 500 ms, and a same-chunk audible reference above the low-gain guard. It applies source-edge padding and protects adjacent word alignments that do not merely span the quiet edge. This addresses overlong Whisper alignment masking leading silence. Constant low-gain recordings do not qualify on the absolute threshold alone. Earlier saved measurements require an explicit refresh; opening a project does not reclassify it.

## Saved projects and commands

`cleanup.mode` accepts `glingCompatible` (also the input alias `gling-compatible`), `natural` and `transcript`. It defaults to `natural` when omitted for backward compatibility; the native new-workspace default explicitly selects `glingCompatible`. Existing project files remain readable. Regeneration creates a new map and preserves compatible manual overrides. Existing saved maps are not silently reclassified when opened.

`preedit.analyze.reuseTranscript` takes a decimal analysis ID. It requires completed coverage of the same asset, range, stream, channel, language, model, and source fingerprint. It decodes bounded audio chunks and copies the saved timed words without loading Whisper or uploading audio. The new analysis uses analyzer version 3. The native Update cut action refreshes older acoustic measurements when Natural is selected. Refresh audio & cut explicitly requests this pass for any completed analysis.

New sessions use a 500 ms pause threshold. Saved user settings are preserved. Review cuts provides P/N navigation and R to restore when a text field does not own keyboard input. Clicking a removed transcript word opens its original context. All cut generation still happens before review, with no suggestion approval step.

Automatic removal intervals are sorted and unioned before timeline assembly. Ordered manual edits are applied afterward. Transcript/waveform hit checks use binary search, and cut review renders only visible rows. These changes reduce repeated work when a recording contains many cuts.

Take review exposes Keep all, choose one, and grouping rejection/splitting. Reject grouping restores all alternatives in one undoable revision. The waveform supports selecting both removed and retained intervals, edge trimming and Cut / Uncut menus; transcript selection exposes Cut / Uncut / Correct / Export clip. Removed material is gray and remains accessible in original playback.

## Validation

See [the evidence record](EVIDENCE.md) for aggregate test and recording results. Footage, transcripts, review projects, detailed measurements, and screenshots remain in ignored local qualification storage. More detected silence or more cuts alone does not establish editorial accuracy. Human listening and labeled false-cut/restoration measurements remain required for M2 closeout.

## Surviving take pairs — 2026-11-23

“Compare remaining takes (experimental)” works on the selected saved cut. It scans fully retained timed words across ordinary review-block boundaries. It compares an earlier phrase with a nearby longer reading, preserving numbers, negation, and other differing words. The local reviewer sees both readings and surrounding retained speech, and chooses the later reading or keeps both. Only the proposed earlier phrase is eligible. Application revalidates the project revision and transcript, protects manual edits and keeper spans, and snaps the deletion inward after checking measured quiet handles. The action creates an undoable map.

On the 812.579-second recording snapshot (map 1921), one pair qualified: the abandoned glasses comparison. Its exact source interval remains in the private qualification report. The native frame-aligned result was 809.946 s, a reduction of 2.633 s with no further loss in the checked keep intervals. All previously labeled bad-take time was already gone; this is a new, model-selected example and not a measured improvement in recall on the old labels. The initial cut already retained only 4.071 of the 71.487 seconds labeled keep.

## Coverage experiment — 2026-11-23

OrderedV8 screens the broad reference marks for complete phrase coverage in the proposed later reading. Complete earlier sentences survive that screening. The conservative supplemental matcher still runs; V8 constrains retake bounds to discarded words rather than consuming neighboring gaps. Model additions in this mode require a later matching phrase within 45 seconds; unsupported decisions leave their review block at the rule checkpoint. Missing numbers and negations fail the coverage check. This is lexical evidence, not proof of semantic equivalence.

The private transcript benchmark rebuilds proposals, clears all manual overrides, and computes automatic cuts against the existing audio sample grid. It uses stored quiet measurements but does not rerun the LLM or the original video-frame grid. There are 71.486875 seconds labeled keep and 55.517 seconds labeled bad take. Run scripts/m2/score_cleanup_labels.py against the intervals exported by m2_export_first_pass_intervals to reproduce the scoring.

| Mode | Keep seconds retained | Bad-take seconds still retained | Output seconds |
|---|---:|---:|---:|
| V7 reference-first | 4.057875 | 3.072 | 840.10 |
| V6 conservative context | 71.317875 | 37.335 | 1185.27 |
| V8 coverage experiment | 71.387875 | 43.147 | 1271.69 |

V8 substantially improves preservation versus V7 but loses recall. The remaining 0.099 seconds of keep-range loss is distributed around annotated boundaries; this run does not establish that those are perceptually harmless. The acceptance condition of full keep preservation with no returned bad takes was not achieved. V8 is opt-in and does not replace the restored 877.362-second saved project. The full two-pass recording has not been requalified with V8.

## Evidence-based reviewer implementation

The opt-in evidence reviewer starts a fresh V6 rule checkpoint. V7 marks are retrieval hints only. It forms source-time utterances, retrieves the preceding eight utterances within 90 seconds using ordered alignment or a three-token shared run, includes rule proposals and within-utterance starts, and groups up to four attempts. Every supplied span has a stable ID, original aligned word interval, complete text, baseline status and protection flag.

The first model chooses keep-both, replace-earlier, remove-a-supplied-fragment, or uncertain. Its IDs and quotes are constrained to the supplied catalogue and validated again in Rust. A separate model request audits each removal. Both approvals are required. Conflicting keeper decisions, malformed evidence, unplaceable boundaries, source changes, transcript changes and cancellation cannot silently commit a removal. Completed reports persist with the automatic map; manual decisions and independently generated filler/pause cuts remain protected.

Granite 3.3 2B Q4_K_M and Qwen3 8B Q4_K_M are pinned catalogue entries. A single authenticated loopback llama.cpp server remains alive for the review job. It uses each GGUF's own template, disables reasoning output, requests constrained JSON and preflights tokenized complete prompts against the 16K context. No attempt is shortened to fit. A valid decision is cached by model, prompt, source, transcript and group digests. Each protocol failure gets one retry. Cancellation kills the owned server; CPU execution is supported.

The Windows runtime benchmark uses the SHA-256-pinned b12483 Vulkan build on the RTX 3060. Runtime loading and editorial quality are measured separately. The frozen editorial corpus has development and held-out rows; no prompt tuning uses held-out model outcomes. The recording qualification script clears manual overrides, scores candidate retrieval independently, compares V6/V7/V8 and both reviewers, saves a new copy, and verifies Undo and save/reopen. Failure to pass the keep/bad-take gates leaves the existing default in place. This is qualification on one previously inspected recording, not evidence of generalization.

Evidence review captures the source path digest, byte length, modification timestamp and imported head/tail identity before inference, and checks that stamp again before committing. This avoids hashing the entire recording on the editor thread. Existing asynchronous full-source verification for applying/exporting the cut remains in place. The standalone qualification worker carries the editor's stamp through unchanged; the model never supplies it.

Confirmation preserves the exact V6 cut geometry. Only new word removals are snapped again. A keeper with a protected manual or filler decision cannot justify a removal. Malformed evidence falls back for its own group; stale transcript/checkpoint/source results reject the whole commit. Ordinary Undo restores the V6 checkpoint; a second Undo restores the preceding edit. The optional persisted report defaults to absent in older projects.

Reproduce the controlled engine checks with `cargo test -p shotclutch-engine --features speech-download preedit:: --lib`. Build the desktop and standalone reviewer together with `cargo build -p shotclutch -p shotclutch-engine --bin shotclutch --example take_review`. `scripts/m2/qualify_evidence_review.py` takes explicit executable, worker, project, labels, model-store, server and output paths. It saves detailed per-label discrepancy intervals without a timing tolerance. Local transcripts, model weights, project copies and detailed qualification reports stay under ignored `target/m2-qualification/`.

Pair merging connects either shared endpoint and follows bridges while respecting the four-attempt cap. The recording produces 106 groups after this correction. Controlled checks include a shared later attempt and a bridging pair. The `take_review_seed` example can create a separate, current-schema qualification project with the same typed project and ID watermark but fresh history; it refuses to overwrite its destination and requires the same parent directory to preserve relative media references. This removes old Undo snapshots rather than changing transcript content. Qualification still clears all manual decisions and creates new native checkpoints whose Undo/redo and persistence are tested. Source, transcript and V6 checkpoint digests match the original qualification input.

## Evidence-review qualification outcome — 2026-11-23

The accuracy gate failed. Qwen had fewer editorial errors than Granite on this regression set, but neither reviewer is promoted. The restored 877.362-second project remains unchanged. The experimental Windows build and separate project copies are in local qualification storage; the detailed comparison is `target/m2-qualification/evidence-review-complete/report.md`.

| Workflow | Native output seconds | Confirmed keep seconds retained | Confirmed bad-take seconds remaining |
|---|---:|---:|---:|
| V6 | 1187.451 | 71.486875 | 37.418 |
| V7 | 841.162 | 4.070875 | 3.117 |
| V8 | 1273.321 | 71.486875 | 43.251 |
| V6 + Granite evidence review | 1221.731 | 50.603875 | 35.852 |
| V6 + Qwen evidence review | 1099.383 | 64.153875 | 32.118 |

Targets were all 71.486875 keep seconds retained and all 55.517 bad-take seconds removed. Granite fully preserved 8/11 keep ranges; Qwen preserved 9/11. The later glasses reading survived both, but 2.616 seconds of its abandoned restart remained with Granite and 2.166 seconds with Qwen. These are failed gates without boundary tolerance. Per-label discrepancy intervals are recorded separately.

Both reviewers received candidate coverage of all 51.280 seconds of aligned speech inside the bad-take range labels. The remaining failures occur in semantic decisions, verification, conflict handling and boundary placement. Granite scored 3/12 on independently authored cases and Qwen 5/12; both scored 2/6 on the frozen held-out subset. Agreement between two requests did not establish editorial correctness. Unseen-recording accuracy remains unqualified.

On the Ryzen 7 4800H / 32 GB RAM / RTX 3060 Laptop 6 GB machine, Granite's job took 556.491 seconds and peaked at 8,326,283,264 bytes of server working set. Qwen took 1178.991 seconds and peaked at 15,989,923,840 bytes. Each reused 61/106 groups and freshly reviewed 45, so these are partial-cache measurements, not cold-run timings. Both used 16K context and full-precision host KV (`LLAMA_ARG_KV_OFFLOAD=0`). Full native qualification took approximately 1901 seconds and the desktop process peaked at 6,780,510,208 bytes; separate process maxima are not a simultaneous total peak. The fresh-history seed avoids expanding the original document's old Undo snapshots during qualification.

Controlled engine checks passed: 70 tests, zero failures, four opt-in tests ignored. Both native experimental edits passed checkpoint Undo/redo and exact segment equality after save/reopen. Existing defaults stay unchanged because the accuracy conditions were not met.
