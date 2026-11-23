# M2 closeout — review-assisted Smart Cut

Closed 2026-11-24 on `codex/m2-smart-rough-cut`, at the user's explicit request to complete both closeout tasks after the [assessment](COMPLETION_ASSESSMENT.md).

**M2 is Done for the bounded review-assisted workflow. M3 is ready to start and remains unstarted.** Granite 2B is retained; the failed automatic-quality experiment is not promoted.

## Frozen checkpoint

- Source: the commit carrying local annotated tag **`m2-closeout-2026-11-24`**. The tag fixes this closeout, CI changes and the previously qualified M2 implementation before M3 edits.
- Windows x64 candidate: `target/m2-qualification/local-take-review/shotclutch-smart-cut-polished.exe`.
- Candidate SHA-256: **`6f1df5c954add3424d836837d7fe6b7b52c7828852fd3c6b0fe8e2f6f0b5ddef`**.
- Restored editing baseline: **877.362 seconds (14:37.362)**, SHA-256 **`e8b7fee73b103443e3fcc6f58ca92a3dc5afb322c0f6d3bfa7196b6f821bb301`**. Its bytes are unchanged.

The candidate was built and tested from the production source frozen here. Closeout subsequently changes qualification/CI tooling and documentation only. This is an exact checksum for the local qualified binary, not a claim of bit-identical rebuilds or a signed release installer. Keep its adjacent runtime and portable-data folders. No remote push, merge, hosted CI execution or public release is part of this checkpoint.

[Machine-readable checkpoint manifest](closeout.json) records the same source tag, binary size/hash, profile and accepted limits.

## Both closeout tasks completed

1. `scripts/ci/m2-checks.py` keeps the existing `m2_` filter and adds complete model-independent `preedit::` and renderer `mixer_tests::` coverage. A standard-library regression exercises each added suite's failing exit status and confirms the runner stops. Existing speech, UI and indexed-boundary coverage remains.
2. The acceptance register, tracker, implementation notes, qualification procedure and completion assessment now record scoped acceptance and the retained limitations. This commit and its annotated tag freeze the source; this document pins the existing qualified Windows artifact.

## Verification

Local Windows x64 gates passed on the closeout source:

| Check | Result |
|---|---|
| M1 standard runner | Passed: formatting, runner guard, 217 time/project/edit/format checks, 81 engine and five automation checks |
| M2 runner failure propagation | Passed: either newly added suite's failure stops qualification with its exit code |
| Speech worker Python protocol | 2 passed |
| Existing `m2_` engine filter | 58 passed; 2 private-model tests ignored |
| Full `preedit::` suite | 72 passed; 4 opt-in tests ignored |
| Renderer `mixer_tests::` suite | 25 passed |
| Cleanup-form UI | 3 passed |
| Indexed edit boundaries | 1 passed |
| Speech catalogue/parsing/cancellation | 20 passed |

The engine filters overlap; their counts must not be added as unique test coverage. The new headless coverage needs no private recording, downloaded model, inference service or live account. Windows junction fixtures need the filesystem permissions used by the successful native runs. No failing test was skipped to obtain those results.

Logs: `target/m2-closeout-m1.log` and `target/m2-closeout-m2.log`. Both passed after the CI change. The routine GitHub Actions workflow already invokes these runner scripts, so the additional suites are now wired into routine CI; this local closeout does not claim a new hosted run.

## Reviewed recording and smooth joins

- All **11** confirmed keep ranges survive: **71.486875 seconds**, zero loss.
- All **55.517 seconds** of confirmed bad takes are removed.
- The later glasses reading survives, with its abandoned restart removed.
- The result includes **49 stored overrides**; these measurements validate the reviewed result and restoration workflow, not raw detector accuracy.
- All **80** interior joins passed the production-mixer seam audit at 48 kHz. Maximum adjacent-sample discontinuity at the seams was **0.0**.
- Preview/applied audio equality, linked A/V export frame/sample counts, save/reopen, fresh-session Undo/redo, source preservation and edited-target replacement guards have passing evidence.

The polish fixes one-sided fade endpoints and directional audio sample snapping, retaining the short capped 5 ms fades and original linked clip geometry. The seam audit measures de-click behavior; no new whole-recording perceptual or semantic accuracy claim follows from it.

Private details remain at `target/m2-qualification/smart-cut-polish/`, `target/m2-qualification/m2-completion/`, and `target/m2-qualification/evidence-review-complete/`. [Aggregate evidence](EVIDENCE.md) and [acceptance mapping](ACCEPTANCE.md) remain tracked.

## Accepted limits and M3 handoff

Smart Cut provides a useful, reversible first edit with comparison, overrides and restoration. Automatic bad-take selection still has misses and false cuts. Granite and Qwen evidence review failed their unassisted recording/editorial gates; those failures remain recorded. Granite 2B remains the local reviewer, and experimental evidence review remains opt-in. The 8B research option is absent from the Windows interface; its research fixtures/catalogue remain for reproducibility.

Acoustic filler recovery, automatic performance-quality ranking and unseen-recording accuracy remain unqualified. Optional live AssemblyAI qualification remains separate from the accepted local profile. These limits are not silently transferred into M3's deliverables or represented as Gling-level quality.

M3 can use stable source references, maps/transcripts, permissions, transactions and grouped Undo. Agent edits must still be verified against source/composed output with explicit uncertainty. Model agreement alone is not verification. M3 implementation and further speech-cleanup accuracy research require their own evidence and scope.
