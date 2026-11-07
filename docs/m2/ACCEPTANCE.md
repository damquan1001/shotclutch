# M2 acceptance register

Updated 2026-11-07. Owner: Codex for implementation/automated qualification; project owner for editorial acceptance. Branch: `codex/m2-smart-rough-cut`, based on M1 closeout `b15c6439`; final implementation/qualification tools `52ad1598`. **M2 is In Review. PRE-001 and PRE-002 are Done under their bounded technical acceptance.** UI-002 and PRE-003–PRE-005 remain In Review for the editorial journey. The cut-first workflow and Smart Cut product name follow the user's subsequent requests.

| Requirement | Implemented behavior and evidence | Remaining acceptance |
| --- | --- | --- |
| PRE-001 | Explicit source/stream/settings invocation; local model installation; labeled silence-only; no automatic analysis on import. Focused explicit-invocation regression and native setup checks. Scoped clients cannot invoke owner `preedit` methods. | None for the bounded technical contract. |
| PRE-002 | v18 typed analyses, immutable maps, source hashes/settings/coverage, unknown confidence, provenance, manual overrides/corrections and application records. v17 migration, checkpoints/recovery/resume, collection/relink, save/reopen and mapping regressions. | None for the bounded technical contract. |
| PRE-003 | Audio-aware Natural cleanup, timed Transcript rules, source-edge quiet, fillers/restarts/take groups; changed negation/numbers and uncertain Natural take endings retained. Real local Whisper coverage and acoustic refresh. | Human labels for meaningful pauses, speech preservation, take decisions, audible joins and restoration/adjustment rates. |
| UI-002 | Native script, waveform, original/result preview, icon transport, Skip cuts, gray removals, cut/uncut selection and trimming, text context actions, original listening and take review. Repeatable native walkthrough. | Human completes the editorial journey using the UI and assesses playback/joins. |
| PRE-004 | Shared preview/apply assembly, ordinary linked clips, sample alignment/fades/captions, durable grouped transaction, idempotent result, stale-result rejection. Independent export frame/sample checks and native application walkthrough. | Final reviewed editorial result; depends on UI-002/PRE-003. |
| PRE-005 | Restoring ranges/takes/group rejection, compatible reanalysis, conflict preservation, unchanged-target replacement guard and fresh-sequence path. Collection retains full originals. Undo/reopen/relink regressions and native edited-target check. | Final reviewed editorial result; depends on PRE-004. |

[Executed aggregate evidence](EVIDENCE.md) records results rather than inferred completion. [Qualification](QUALIFICATION.md) supplies the exact procedure and private human worksheet. The latest candidate and private project are local artifacts, not committed evidence.

No human editorial verdict or measured restoration rate has been supplied for the final build. Automated restoration is not a human label, and removal duration is not an accuracy measure. A positive response to a specific bug fix does not establish the complete M2 editorial gate. Keep UI-002 and PRE-003–PRE-005 In Review until the relevant acceptance is recorded; then update this register and the tracker with the actual outcome. A failed editorial review reopens affected implementation work.

Optional AssemblyAI parsing/error tests are separate from live provider qualification, which remains unexecuted without a local account/key. M2's pinned local English Whisper acceptance profile does not claim live cloud qualification.
