# ShotClutch — Smart Rough Cut

Baseline 0.1 · 2026-10-26 · UI-002 and PRE-001 through PRE-005.

## Product contract

Smart Rough Cut is a manually invoked, nondestructive preparation workflow for speech-led footage. It proposes an editable first assembly by finding pauses, fillers, false starts, repeated attempts, and alternative takes. It preserves the user's performance and intended meaning rather than optimizing solely for the shortest duration.

Importing media, transcribing, or background indexing does not start or apply Smart Rough Cut. The user selects sources, invokes the feature, and chooses analysis settings. The agent may invoke it only as part of an explicit user-requested rough-cut task with the required scope; an indexing job cannot infer that authorization.

Results persist as PreEditAnalysis, TakeGroup, and versioned PreEditMap objects. Original media is never rewritten. Applying a reviewed map is a separate command and one undoable transaction.

## Dedicated workspace

| Surface | User purpose |
|---|---|
| Source selector and settings | Choose asset/range, language, analysis policy, source audio, and available local/provider models |
| Transcript and waveform | Inspect words, timing, pauses, and proposed removals with linked playback |
| Source monitor | Hear and see the original context surrounding a candidate |
| Result monitor | Preview the current logical rough cut with proposed joins and audio treatment |
| Take comparison | Compare repeated attempts side by side or sequentially; choose or preserve alternatives |
| Decisions panel | Filter by remove/keep/review, reason, confidence, or manual override |
| Apply controls | Choose new sequence or explicitly selected target, inspect change summary, apply or cancel |

Selection follows exact source ranges. Keyboard actions support keep, remove, restore, compare, and next unresolved candidate. Removal status uses text/icons as well as color. The workspace persists selection-independent editorial decisions across save/reopen. Long analysis shows phase progress, cancellation, and completed coverage.

## Pipeline and policies

1. Resolve source IDs, fingerprints, selected ranges, language, channel policy, and settings.
2. Run local VAD and available ASR; reuse compatible existing analysis without hiding its age or coverage.
3. Align words and speech boundaries to the source timebase; retain uncertainty.
4. Generate candidates for silence, fillers, restarts, repetitions, and take groups.
5. Apply conservative boundary padding and speech-preservation constraints.
6. Rank alternatives with reasons and confidence, leaving ambiguity visible.
7. Persist a proposal and let the user review/override it.
8. Preview the mapped result, verify boundaries/audio, and apply only when requested.

Settings include pause thresholds, minimum retained context, boundary handles, filler handling, restart/retake detection, and default treatment of low-confidence results. Exact values should be tuned on the evaluation corpus; they are not universal defaults asserted here. Provide conservative presets and explicit per-project settings.

VAD is evidence of speech activity, not a transcript or a semantic keep/remove decision. A filler can be intentional; a pause can carry meaning. ASR uncertainty, laughter, music, overlapping speakers, code-switching, whispers, and background noise should bias toward preservation or review. Never remove a negation or splice statements into a materially changed meaning simply because lexical similarity suggests a retake.

## TakeGroup semantics

A TakeGroup represents alternative attempts at an editorial unit, not necessarily contiguous identical phrases. Each candidate records exact source intervals, transcript, available audio/visual quality observations, completeness, confidence, and explanation. Ranking may consider intelligibility, completion, clipping, noise, restarts, and delivery, but no opaque score overrides a manual selection.

The user can choose one take, retain multiple attempts, split an incorrectly grouped unit, or reject the grouping. Visual quality is used only when actual visual analysis supports it. Audio-only analysis must not claim that a selected take has better eye contact or framing.

## PreEditMap algorithm and invariants

The v0.1 map is a sequence of retained forward, unit-speed source intervals. For segment `i`, source interval is `[s_i, e_i)` and duration is `d_i = e_i - s_i`. Its output interval begins at `o_i = sum(d_j for j < i)`. For output tick `t` in that interval, source time is `s_i + (t - o_i)`. All terms use the exact time contract from [04](04_PROJECT_DATA_MODEL.md).

Output ranges are contiguous and nonoverlapping for the basic assembly. Reordered or repeated source intervals are allowed only when explicitly selected and visible; inverse mapping therefore returns a list. No output tick maps into a removed gap. The map stores source fingerprints, stable segment IDs, selected takes, retained and removed decisions, reasons, confidence, settings, manual overrides, and revision history.

Video boundaries must be valid presentation/frame boundaries; audio joins retain sample precision. Boundary padding is clamped to available source handles. The basic v0.1 assembly keeps video duration equal to the sum of retained ranges. Short audio fades may be applied within retained intervals without changing output duration. Overlapping crossfades or J/L edits require explicit timeline commands and a revised output mapping; they must not be hidden in the simple concatenation formula.

## Review and lifecycle

```text
not_started -> analyzing -> proposed -> reviewing -> reviewed
                    |                         |
                    +-> failed/canceled       +-> new map revision
reviewed -> apply transaction -> applied record
```

An applied record points to a particular map revision; it is not a permanent state that prevents later revisions. A source change marks relevant analysis and maps stale. A settings change creates a new analysis revision. Reanalysis preserves user decisions by stable candidate/source references where valid and flags unresolved conflicts rather than transferring them blindly.

Actions include keep/remove/restore range, adjust boundary, select take, preserve pause, merge/split candidate grouping, and mark reviewed. Each is persisted through commands and undo. “Reset suggestions” and “reset all decisions” are distinct; the latter must clearly include manual overrides.

## Applying and revising the result

Default application creates a new editable sequence made of ordinary source-referencing clips. Group metadata retains logical association to the map without creating a second authoritative timeline. Each created clip records the map/segment revision and source mapping. The logical workspace preview can evaluate the map directly; the main editor works with committed clips.

Applying to an existing sequence requires an explicit destination/range policy, a change preview, current revision, track-lock validation, and affected audio/caption handling. A repeated request returns its existing application result through idempotency. It must not append a duplicate assembly.

After application, manual trims and rearrangements are ordinary edits. A later map revision does not silently regenerate the timeline. Offer a fresh sequence, or a revision-aware reapplication that previews conflicts with modified clips. v0.1 may limit reapplication to unchanged generated clips and require a fresh sequence otherwise. Restoring a source range is always possible in the map; inserting it into an edited timeline is an explicit transaction.

Transcript/caption alignment follows the map, splitting words only according to a declared alignment policy. Removed words cannot remain as captions on the result. Linked audio, multi-channel sources, and source offsets stay aligned. Detached audio or separately recorded audio requires a validated sync mapping; basic v0.1 must reject unsupported synchronization assumptions.

## Failure handling and acceptance

Cancellation retains valid completed analysis chunks and user decisions but marks coverage incomplete. Missing ASR falls back to explicitly available silence-only analysis or a clear unavailable state, never invented transcript text. Model failure cannot destroy an existing reviewed map. All-removed, all-kept, zero-duration, long silence, clipped speech, and single-frame boundary cases have defined validation outcomes.

Acceptance requires manual invocation, persistent review, take comparison, exact output/source reconstruction, correct captions/audio, idempotent application, one-step undo, changed-source detection, and safe reapplication after manual editing. Evaluate editorial quality with human-labeled examples and restoration rates; do not use compression ratio alone as success.
