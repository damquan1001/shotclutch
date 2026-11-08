# OpenGling cleanup adaptation

## Local reference assessment

Reviewed the local OpenGling 0.12.0 source, specifically `src/speech-cleanup.js`, `src/acoustic-cleanup.js`, `src/cleanup-state.js`, `src/transcribe.py`, `src/transcription.js`, `public/app.js`, and their tests. These are implementation references. The requested workflow remains automatic cutting followed by review. Updated 2026-11-08 after the user's parity request.

## Parity implementation

### Pause pacing refinement — 2026-11-08

Following the user's overcutting report, new native workspaces enable **Preserve natural pauses**, keeping gaps up to 800 ms between retained words. This is independent of the 500 ms take-grouping threshold: repeated-take/filler classification is unchanged. Longer pauses retain the selected total breathing room (150 ms by default, split across both sides), with at least 250 ms for sentence gaps. Quiet handles beside filler/within-take removals are preserved only beside retained words. Gaps between discarded takes stay removed. Adjoining removals are unioned before presentation-grid snapping to avoid phantom video frames at shared boundaries. Standalone pause removals shorter than 120 ms after snapping are omitted; short gaps attached to discarded words join their word removal. The same pacing floor applies to silence-only analysis.

The optional `cleanup.preservePausesMs` stores this setting; absence preserves the earlier exact behavior. Disable the checkbox for exact Gling transcript rules. Existing saved maps/settings remain unchanged on open. Enable it and use Update cut to regenerate from saved words; no inference or upload is needed. Manual decisions remain authoritative. The legacy `minPauseMs` still separates takes; the new field controls pause pacing independently. The control is labeled Breathing room in this mode and represents total join context; the older modes retain their per-edge padding semantics.

The supplied human-edited reference was probed and measured locally: 692.011 seconds, with 357 quiet intervals of at least 100 ms at a fixed −42 dBFS threshold; median 135 ms and 95th percentile 305 ms. These measurements support retaining breathing room. They are not speech alignment, individual cut labels or a calibrated accuracy score. Private reference paths, media and detailed measurements remain uncommitted.

New Smart Cut workspaces select **Gling compatible** (`glingCompatible`), natural pause preservation and **Tiny English · Fast** (`faster-whisper-tiny.en`). Existing saved modes and model selections remain unchanged. The earlier Natural behavior is labeled Conservative; the earlier Transcript behavior is labeled Legacy transcript. Omitted command/project mode fields still default to Natural for backward compatibility. The exact-parity behavior below applies with pause preservation disabled.

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
