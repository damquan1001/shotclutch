# OpenGling cleanup adaptation

## Local reference assessment

Reviewed the local OpenGling 0.12.0 source on 2026-11-06, specifically `src/speech-cleanup.js`, `src/acoustic-cleanup.js`, `src/cleanup-state.js`, `public/app.js`, and the natural-cleanup and speech-cleanup tests. These are implementation references. The requested Shotclutch workflow remains automatic cutting followed by review.

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

Natural is the default. Transcript rules is an explicit option for more direct, Gling-like timing behavior. It can remove untranscribed sounds within a word gap, so users can inspect the original from the cut review panel. Neither mode provides semantic judgment of which performance is best.

The adaptation uses integer source ticks, immutable map revisions, the existing indexed video/sample boundary checks, and ordinary linked timeline clips. It does not embed an OpenGling runtime. The 20-second checkpoints, cancellation, source fingerprint checks, manual override precedence, and scoped-client permissions remain in force.

Natural quiet detection uses 10 ms per-channel RMS, rolling four-second distributions, a chunk-level fallback, and a minimum 12 dB separation. The threshold is the smaller of noise +6 dB and speech −12 dB. This adapts the reference's more conservative speech −18 dB threshold to quieter dialogue recordings. Constant low-dynamic-range audio remains untouched unless it falls below −70 dBFS. A long noise-only chunk with no identifiable speech/noise separation can still remain uncut. Transcript mode is available for timed gaps in that situation.

Natural pauses retain at least 250 ms of ordinary pacing or 450 ms after sentence punctuation, subject to the selected padding and available gap. Audible islands remain. Word removals need quiet cutting room and protect 120 ms after the preceding word and 80 ms before the following word. Existing frame alignment and short fades remain responsible for realizable edit boundaries and audio joins. This implementation does not add the reference's sample-minimum search or paragraph pacing because the saved analysis does not retain those samples or paragraph identities.

Take matching stays within 20 seconds and the same speaker label. Cross-take suffix removal requires at least two consecutive words. Unrelated intervening takes are retained. These constraints intentionally differ from the reference's broader abandoned-take and single-word suffix heuristics. Recognition errors and meaning changes still require editorial review. Detection confidence remains explicitly unknown rather than presenting a heuristic score as a calibrated probability.

## Saved projects and commands

`cleanup.mode` accepts `natural` and `transcript`, defaulting to `natural` when omitted. Existing project files remain readable. Regeneration creates a new map and preserves compatible manual overrides. Existing saved maps are not silently reclassified when opened.

`preedit.analyze.reuseTranscript` takes a decimal analysis ID. It requires completed coverage of the same asset, range, stream, channel, language, model, and source fingerprint. It decodes bounded audio chunks and copies the saved timed words without loading Whisper. The new analysis uses analyzer version 2. The native Update cut action refreshes old acoustic measurements when Natural is selected. Refresh audio & cut explicitly requests this pass for any completed analysis.

New sessions use a 500 ms pause threshold. Saved user settings are preserved. Review cuts provides P/N navigation and R to restore when a text field does not own keyboard input. Clicking a removed transcript word opens its original context. All cut generation still happens before review, with no suggestion approval step.

Automatic removal intervals are sorted and unioned before timeline assembly. Ordered manual edits are applied afterward. Transcript/waveform hit checks use binary search, and cut review renders only visible rows. These changes reduce repeated work when a recording contains many cuts.

## Validation

See [the evidence record](EVIDENCE.md) for aggregate test and recording results. Footage, transcripts, review projects, detailed measurements, and screenshots remain in ignored local qualification storage. More detected silence or more cuts alone does not establish editorial accuracy. Human listening and labeled false-cut/restoration measurements remain required for M2 closeout.
