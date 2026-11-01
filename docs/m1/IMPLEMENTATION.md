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

`Session::prepare_edits` stages explicit structural edits. `commit_edits_durable` now requires
an enabled recovery worker and acknowledges a complete atomic snapshot before live publication.
The in-memory `commit_edits` remains available for hosts without persistence. This is still an
internal Rust API: external grants, idempotent receipts, source revalidation, expiry and visual
verification remain outstanding. Schema v17 adds document identity, revision, retained history,
allocator high-water mark, recovery source metadata and integrity verification to the v16
extension support. See [DEC-002](DEC-002_PROJECT_PERSISTENCE.md).

## Acceptance register

| Requirements | Current evidence / next action | Status |
|---|---|---|
| CORE-002 | Schema v17 identity and retained history; ID allocation survives undo/history deletion/save/reopen; optional extensions round-trip and unknown required extensions fail closed | In Progress |
| CORE-003 | Retained timebase and exact boundary strings; mixed-rate/VFR/nesting/retiming qualification remains | Planned |
| CORE-004 | Atomic project/history/revision snapshots, checked recovery, migration backups and injected write/publication failures; broader fault/platform/storage-budget qualification remains | In Progress |
| AGENT-001–002 | Engine structural staging and tests in `m1_transaction_tests.rs`; durable commit/recovery implemented internally; finish grants, retries, expiry, source revalidation and adapter parity | In Progress |
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

## Durable persistence continuation — 2026-11-02

Native save, auto-save and recovery now carry document identity, monotonic revision, retained
undo/redo and a non-rewinding ID allocator. Recovery location, legacy protection and original
schema version travel in the same atomic checksummed snapshot. Corrupt or recursive history is
rejected before replacing the live project. Unknown optional extensions remain protected in both
current and historical states.

`commit_edits_durable` fails closed without enabled storage. With a worker, the snapshot is the
commit marker; display metadata failure cannot turn a committed edit into an apparent failed edit.
Recovery writes its replacement snapshot before switching projects or deleting the old candidate.
Grouped commands suppress intermediate snapshots/events and commit one history item/revision.

Fault tests cover failed atomic promotion, a crash after the durable marker but before live
publication, advisory metadata failure, failed recovery replacement/retry, disabled storage,
corrupt/nested history, migration-source protection after recovery, and allocator/revision bounds.
Tests also retain schema v1–v16 and legacy Save As compatibility. The SHA-256 digest hashes actual
wire floats with exact parsing; mixed f32/f64 values and generic JSON reformatting have a regression
check.

Final local checks for this continuation:

- `python scripts/ci/m1-checks.py`: 233 Rust tests passed, including 18 transaction/recovery
  tests and 5 document-state/integrity tests; formatting and the gate's failure-propagation
  test passed. Local time 199.5 seconds including compilation; this is not a hosted estimate.
- `cargo test --locked -p shotclutch-engine --lib`: 740 passed, 3 existing performance tests
  ignored, 0 failed. The suite includes save/reopen, legacy recovery, editing, proxy, media and
  rendered-export regressions. Full runtime 67.75 seconds after compilation.
- `cargo clippy --locked -p shotclutch-engine -p shotclutch-format --all-targets -- -D warnings`:
  passed. `git diff --check` passed.
- Local untracked logs: `target/m1/checks-recovery-final.log` and
  `target/m1/engine-recovery-complete.log`.

The broad regression run exposed an initial f32 checksum mismatch; hashing the actual wire
representation and exact JSON parsing fixed it, with a dedicated regression test. No failed
tests were suppressed. All fault tests here are local injected failures, not physical power-loss
qualification.

The runtime remains Windows-qualified only for executed local tests. Browser/other-OS runtime,
physical power loss, large-history file size/latency, complete service permissions/idempotency,
media qualification and the full offline journey are still outstanding. No new hosted workflow,
GitHub Actions dispatch, push or settings change is needed for this slice.
