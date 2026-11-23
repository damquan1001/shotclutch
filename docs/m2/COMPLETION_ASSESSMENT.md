# M2 completion assessment

Assessed 2026-11-24 on Windows x64, branch `codex/m2-smart-rough-cut`. Closeout completed later the same day at the user's explicit instruction.

**Decision: M2 is Done in the original review-assisted scope. Both assessed closeout tasks are completed.** [M2 completion](M2_COMPLETION.md) fixes the source/build checkpoint and acceptance. The initial assessment changed no production code; closeout adds CI coverage and updates records without promoting the failed automatic-review experiment.

## Scope and acceptance

The [M2 roadmap exit](../16_MVP_ROADMAP.md) is a review-assisted journey: explicitly analyze speech, review alternatives, preserve a pause, restore material, apply an editable result, save/reopen and undo. Source mapping, captions/audio, manual-decision preservation and safe reapplication are required. It does not specify an error-free unattended final edit.

The user chose to retain Granite 2B and wrap M2 after join polish. Further join review was declined earlier in the conversation. This assessment uses the existing source labels, recorded native walkthroughs and production-renderer seam audit; it does not request another human pass or infer whole-recording editorial accuracy from numerical tests.

| Requirement | Current evidence and assessment |
|---|---|
| PRE-001: explicit initiation | Already accepted. Analysis/install/upload choices remain explicit; import does not silently start an editorial cut. |
| PRE-002: persistent analysis/maps | Already accepted. Versioned source-bound data, completed checkpoints, manual decisions, recovery, save/reopen and source mapping have regression evidence. |
| UI-002: native review workspace | Implemented. Existing native walkthroughs exercise transcript/waveform, original/result preview, take comparison, restoration and application; all three current cleanup-form checks pass. |
| PRE-003: cleanup preserving editorial meaning | Accepted within review-assisted scope. The restored result passes every confirmed recording label after saved review decisions. Automatic take selection still has known false cuts and misses; the failed evidence-review experiment remains unpromoted. |
| PRE-004: editable application | Implemented and verified: shared preview/apply assembly, linked A/V, caption mapping, exact export frame/sample counts, revision checks, idempotency and grouped Undo. |
| PRE-005: restoration/revision safety | Implemented and verified: original retention, manual-decision carryover, restoration, reapplication guards for edited results, fresh-sequence fallback and durable Undo/redo. |

## Current verification

- Standard M1 runner: **passed**, including formatting, 217 time/project/edit/format checks, 81 engine regressions and five automation regressions.
- Expanded standard M2 runner: **passed**, including its new failure-propagation guard, two Python protocol checks, 58 `m2_` engine regressions, the full **72 preedit engine checks and 25 mixer checks**, three cleanup-form checks, one indexed-boundary check and 20 speech checks. The engine filters overlap; two private-model tests were ignored in the `m2_` filter and four opt-in tests in the full preedit filter.
- The newly gated suites include reviewer validation, sample-boundary direction, preview/applied audio equality and fade endpoint/block-size cases. No private model or recording is required by this headless coverage.
- Both experimental model project copies passed save/reopen with identical segments and a fresh-session Undo/redo check.
- The current Windows candidate was built successfully. It retains Granite 2B; the 8B research choice was removed from the interface.

The first sandboxed M1 attempt could not create two Windows junction fixtures. Both tests passed in the subsequent permissioned standard run. This was an environment restriction; no source changes or skipped failing tests were used to obtain the passing result. These are local Windows runs; no new hosted CI result is claimed.

## Reviewed recording

The **877.362-second (14:37.362)** restored result was checked independently from the raw automatic benchmarks:

| Check | Result |
|---|---:|
| Confirmed keep ranges fully retained | 11 / 11 |
| Confirmed keep time missing | 0.000 seconds |
| Confirmed bad-take time still present | 0.000 / 55.517 seconds |
| Abandoned glasses restart still present | 0.000 seconds |
| Later glasses keeper missing | 0.000 seconds |
| Production-rendered interior joins audited | 80 / 80 passed |
| Maximum adjacent-sample step at rendered seams | 0.0 |

The map contains **49 stored overrides**. These results validate the reviewed artifact and preservation/restoration workflow; they are not automatic-detector accuracy measurements. The original restored project checksum remains `e8b7fee73b103443e3fcc6f58ca92a3dc5afb322c0f6d3bfa7196b6f821bb301`.

Detailed local evidence:

- `target/m2-completion-m1-native.log`
- `target/m2-completion-m2-native.log`
- `target/m2-closeout-m1.log` and `target/m2-closeout-m2.log` (passing final gates after CI expansion)
- `target/m2-qualification/m2-completion/reviewed-baseline-labels.json`
- `target/m2-qualification/smart-cut-polish/report.json`
- `target/m2-qualification/evidence-review-complete/report.json`
- Candidate: `target/m2-qualification/local-take-review/shotclutch-smart-cut-polished.exe`

## Completed closeout work

1. **Newer regressions are in routine CI.** `scripts/ci/m2-checks.py` retains `m2_` and adds complete model-independent `preedit::` and renderer `mixer_tests::` coverage. The new runner test ensures failure in either added suite stops qualification. Private recordings, model downloads and live accounts remain outside that headless gate. Both final standard gates passed.
2. **Acceptance/tracker records and source/build checkpoint are finalized.** Records now mark the bounded review-assisted M2 scope Done, retain Granite 2B and preserve the failed experiment's evidence. The commit carrying `m2-closeout-2026-11-24` freezes the implementation and closeout changes; [M2 completion](M2_COMPLETION.md) pins the existing qualified Windows candidate by checksum.

No new model search or join-review cycle was required. M2 is closed; M3 implementation has not started.

## Limits carried forward

Automatic bad-take selection is still imperfect. The evidence-based two-pass experiment failed its unassisted accuracy gates, and neither model passed the frozen editorial cases sufficiently for promotion. Acoustic filler recovery and unseen-recording accuracy remain unqualified. Optional live AssemblyAI qualification remains separate from the local M2 profile.

M3 can build on the stable project/maps, source references, transactions and restoration controls after closeout. It must retain these limitations and verify proposed agent changes with actual source/composed-output evidence; agreement between model requests is not sufficient verification. Improved automatic speech-cleanup accuracy remains an explicit future follow-up, rather than an implied M3 deliverable or a claim of Gling-level quality.
