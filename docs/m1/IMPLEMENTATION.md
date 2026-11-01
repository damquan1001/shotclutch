# M1 implementation and acceptance

Started 2026-11-02 on `codex/m1-editing-foundation`. Owner: Codex.

M0 and the subsequent identity work are the starting baseline. M1 is In Progress. No M1
requirement is Done solely because an inherited feature exists or a new unit test passes.

## Hosted CI policy

The user selected ready-for-review PRs only. `CI required` is one Ubuntu job, triggered for
non-draft PRs targeting main (open, reopen, synchronize, ready_for_review), or manual dispatch.
There is no duplicate main-push run. A newer run cancels the previous PR run. Documentation-only
PRs still receive the gate so future required-check configuration cannot leave them pending.

`scripts/ci/m1-checks.py` runs formatting, the time/project/edit/format tests (including historical
format fixtures and property tests), and the headless engine `m1_` tests. Downloads are cached;
compiled artifacts are not uploaded. Routine CI installs no GPU, oracle, browser or installer tools.

The old CI matrix is retained as `Full qualification (manual)` in `qualification.yml`; Windows
packaging, packaging lint, Nix, FreeBSD and Windows ARM64 remain manually dispatchable. The
FreeBSD/ARM64 nightly schedules are removed. Existing local qualification commands are preserved.
No hosted run, push, publishing or settings change is part of this local implementation session.
GitHub's branch-protection API reported `Branch not protected` for main on 2026-11-02.

## Implementation order and current slice

1. Reduce routine hosted CI and retain full qualification entry points.
2. Establish isolated structural staging through the inherited engine commands. The first Rust
   API supports explicit sequence/clip targets, exact decimal-string trim/split times, a bounded
   group, stale-state rejection and one-step undo. Preview snapshots do not reach live project,
   history, events, journal or recovery workers. Dropping a preview aborts it.
3. Extend canonical schema/time/ID contracts and durable commit/history recovery; preserve v15
   and legacy file compatibility. Complete idempotency and grant validation before exposing the
   transaction contract through UI, CLI or protocol adapters.
4. Qualify inherited manual editing, bounded Windows SDR media/fallback, source identity,
   proxy/original mapping, render/audio behavior, export promotion and project collection.
5. Run the offline journey, fault boundaries, long-form audio checks and second-machine
   portability. Record actual evidence and limitations against the requirements below.

`Session::prepare_edits` and `commit_edits` are an initial internal Rust API, not the complete
document 05 transaction envelope. Prepared values are process-local and cannot be deserialized.
No external grants, durable transaction journal, idempotent operation registry or automatic
visual verification is claimed. Project schema v16 adds versioned opaque extensions. The migration is additive; historical v15 and legacy fixtures remain supported. Unknown required extensions fail with CAPABILITY_UNAVAILABLE; optional records round-trip unchanged.

## Acceptance register

| Requirements | Current evidence / next action | Status |
|---|---|---|
| CORE-002 | Schema v16 optional extension round trips and required-extension refusal; stable project identity and ID lifetime remain required | In Progress |
| CORE-003–004 | Finish DEC-002, exact mapping qualification, revision persistence and fault-tested durable history | Planned |
| AGENT-001–002 | Engine structural staging and tests in `m1_transaction_tests.rs`; finish durable lifecycle, grants, retries and adapter parity | In Progress |
| TEST-001 | Focused gate and local checks; extend with persistence faults and new schema invariants as implemented | In Progress |
| UI-001 | Inherited editing foundation; execute complete keyboard/manual workflow against M1 candidate | Planned |
| MEDIA-001–004 | Close DEC-006 with measured fallback/proxy/render/audio corpus on Windows x64 | Planned |
| EXPORT-001, EXPORT-006 | Verify pinned originals, atomic completed output and collection/relink on another machine | Planned |
| SEC-001–002 | Offline journey and scoped-service boundary tests remain required | Planned |

Tests cover private staging, command equivalence, linked trim, explicit split, abort, grouped
undo/redo, failed later commands, locked targets, stale revision/project replacement, exact wide
time strings and rejection of unsupported operations. These are structural checks; they do not
establish decoder, physical audio-clock, crash recovery or second-machine qualification.

Local runtime gates remain those in documents 14–16: independent frame/sample verification,
preview/export composition agreement, no underruns in the 10-minute fixture, and A/V sync within
one frame without cumulative drift over 30 minutes. Unavailable hardware or second-PC evidence
keeps its criterion outstanding. No repeated full hosted M0 qualification is required for M1.

## Local validation — 2026-11-02

- `python scripts/ci/m1-checks.py`: passed in 211.9 seconds on this Windows PC. 216 Rust
  tests passed: 77 edit, 24 format, 16 historical format fixtures, 4 extension tests, 66 project,
  23 time, and 6 staged-transaction tests. Formatting and the gate's failure-propagation test
  also passed. Local diagnostic log: `target/m1/checks.log` (untracked).
- `cargo test --locked -p shotclutch-engine --lib file_tests`: 5 passed, including legacy
  read/Save As protection and versioned save/copy behavior with the new schema.
- `cargo clippy --locked -p shotclutch-engine -p shotclutch-project -p shotclutch-format --lib -- -D warnings`: passed.
- `python -m unittest discover -s scripts/ci -p 'test_*.py'`: 9 passed.
- Pinned local `actionlint` and `git diff --check`: passed.

The first sandboxed persistence run reported seven Windows Access Denied errors during file
replacement/cleanup. The same suite passed with normal filesystem access; no test suppression
or production permission workaround was introduced. Local compiled-cache hard-link warnings
fell back to copying. The local duration includes a partial engine rebuild and is not a hosted
runtime estimate. No hosted execution or Linux runtime result is claimed.
