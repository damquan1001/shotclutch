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
format fixtures and property tests), and the headless engine/MCP `m1_` tests in one Cargo invocation. Downloads are cached;
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
The in-memory `commit_edits` remains available for hosts without persistence. The bounded
versioned structural service now adds scoped host grants, retained idempotent receipts,
expiry/cancellation, strong source revalidation and CLI/MCP adapters. Rendered verification and
general scoped mutations remain outstanding. Schema v17 adds document identity, revision,
retained history, allocator high-water mark, recovery source metadata and integrity verification
to v16 extension support; edit-state v2 retains receipts. See [DEC-002](DEC-002_PROJECT_PERSISTENCE.md)
and the [implemented service contract](SERVICE_CONTRACT.md). Native [export delivery](EXPORT_CONTRACT.md)
and [collected-project portability](COLLECTION_CONTRACT.md) are implemented with explicit qualification limits.

## Acceptance register

| Requirements | Current evidence / next action | Status |
|---|---|---|
| CORE-002 | Schema v17 identity and retained history; ID allocation survives undo/history deletion/save/reopen; optional extensions round-trip and unknown required extensions fail closed | In Progress |
| CORE-003 | Exact boundary strings and fractional-rate export sample endpoints; full mixed-rate/VFR/nesting/retiming qualification remains | In Progress |
| CORE-004 | Atomic project/history/revision snapshots, checked recovery, migration backups and injected write/publication failures; broader fault/platform/storage-budget qualification remains | In Progress |
| AGENT-001–002 | Bounded structural service: host grants, exact wire values, source SHA-256, durable retries and CLI/MCP parity; rendered evidence/general permissions remain | In Progress |
| TEST-001 | Focused gate and local checks; extend with persistence faults and new schema invariants as implemented | In Progress |
| UI-001 | Inherited editing foundation; execute complete keyboard/manual workflow against M1 candidate | Planned |
| MEDIA-001–004 | Close DEC-006 with measured fallback/proxy/render/audio corpus on Windows x64 | Planned |
| EXPORT-001 | Native original binding, verified output staging and publication implemented; full media/export qualification remains | In Progress |
| EXPORT-006 | Verified relative-path collection, full history or explicit derived copy, and local relocation; updated physical second-PC check remains | In Progress |
| SEC-001–002 | Scoped-service denial, private grants, source confinement and cancellation tests; complete offline journey/worker isolation remain | In Progress |

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


## Scoped service continuation — 2026-11-02

The [implemented contract](SERVICE_CONTRACT.md) now exposes a versioned structural subset
through the native command registry, CLI and scoped headless MCP. Host-only authorizations
bind actor/project/sequence, expire, and support atomic revocation. Read-only is the scoped
launch default. Agent JSON cannot issue grants, impersonate an actor, run raw engine commands,
choose output destinations, import/export, render sources or control the UI.

Preview stages exact-string trim/split requests privately. Original/proxy sources, including
nested references, receive full SHA-256 checks through bounded directory capabilities during
preview and commit. Changed content, traversal/junction escapes, denied scope, cancellation,
stale revisions and expired grants prevent publication. Permission and cancellation checks run
between staged commands and source chunks. The existing owner interface remains trusted.

Actor/project-bound request digests and operation receipts share the project's atomic recovery
snapshot. Retry after commit, undo, native save/reopen or recovered crash returns the prior
receipt without applying the edit again. The 128-receipt/seven-day retention policy fails closed
at capacity; expired entries prune on the next commit. Edit-state v2 adds receipts while reading
v1; unknown versions and malformed recovered identities/revisions fail before project replacement.

The CLI `transaction request.json --commit` runs preview/commit within one process. Scoped
`--save` is explicit host finalization to the startup `--project`, not wire-level file authority.
MCP line-protocol tests exercise tool routing, resources, denial and repeated commits.

Final local validation of this continuation:

- `python scripts/ci/m1-checks.py`: 255 Rust tests passed, including 19 scoped-service tests,
  18 transaction/recovery tests, 2 MCP protocol tests and 6 document-state tests. Formatting and
  the helper failure-propagation test passed. Final local duration: 158.9 seconds, including an
  engine/MCP rebuild. This is not a hosted runtime estimate.
- `cargo test --locked --offline -p shotclutch-engine -p shotclutch-automation --lib`: 759 engine
  tests and 25 MCP/automation tests passed; 3 existing engine performance tests ignored. Zero
  failures. Runtime after compilation: 48.14 seconds engine, 1.34 seconds automation.
- `cargo test --locked --offline -p shotclutch-cli --test it cli::`: all 17 CLI integration tests
  passed, including scoped save/reopen/retry, malformed scope option denial and historical
  project migration. The other 2 workflow-group tests were filtered by this local command.
- Clippy passed with `--all-targets -- -D warnings` for engine, format, automation and CLI.
  All 9 CI helper tests and normal `git diff --check` passed.
- Local untracked evidence: `target/m1/checks-service-final.log`, `clippy-service-final.log`,
  `service-regression-final.log` and `cli-service.log`.

An initial adapter compilation exhausted the E: drive. Clearing only the verified workspace's
`target/debug/incremental` cache recovered approximately 10 GB. Subsequent validation disabled
incremental caching for those processes; no repository build profile or global setting changed.
The final suites passed without suppressing tests. An explicit `core.autocrlf=false` diagnostic
made CRLF lines appear as whitespace changes; normal repository diff validation passed.

No GitHub Actions run or settings change is part of this continuation. The single ready-PR CI
job adds only the headless M1 MCP tests alongside the engine tests in one Cargo invocation;
CLI/platform/full checks stay local. Dependency changes are pinned `cap-std` and its required
lockfile additions, plus the workspace's existing SHA-256 dependency; existing locked versions
were not upgraded.

Next: qualify exact mixed-rate/VFR/nested/retimed mappings and the bounded Windows SDR corpus,
then bind export jobs to originals for their entire lifetime and verify atomic output promotion.
Run the complete offline journey and prepare the updated project-collection portability fixture.
Physical A/V and second-machine results require actual hardware evidence. General scoped
mutations, a grant/review UI and rendered verification remain open; structural previews do not
claim those capabilities. M1 is still In Progress.


## Native export continuation — 2026-11-04

Native owner exports now retain job-specific source readers and path context, verify original
bytes throughout decoding, revalidate complete hashes before delivery, and stage output privately.
A single file is published atomically after it is complete and synced. Queued relative paths and
document identity survive a live project switch. A preview's cached file or enabled proxy cannot
supply a different original to this job. Missing/intentional-offline reports remain explicit.

The binding report includes project/revision/range, settings hash, original hashes and numbered
frame identities, and completed output hashes. Image membership and holes stay frozen. Ordinary
multi-file promotion failures roll back; failed rollback retains prior files for manual recovery.
MCP no longer infers ownership from timestamps or deletes final files during cancellation.

See [EXPORT_CONTRACT](EXPORT_CONTRACT.md) for the implemented boundary, budgets, host distinctions,
file-set crash/restart limits and remaining qualification. This does not mark an M1 requirement
Done. Full native media, physical A/V, offline and second-machine qualification remain required.


Final local evidence for the native export continuation:

- `cargo test --locked --offline -p shotclutch-engine -p shotclutch-export -p shotclutch-automation --lib`:
  925 passed (775 engine, 127 export, 23 automation), zero failures; 3 existing engine performance
  tests ignored. After compilation: engine 60.59 s, export 7.42 s, automation 1.46 s.
- `cargo test --locked --offline -p shotclutch-cli --test it`: all 19 passed, including the phone VFR
  extract/gain/title-to-H.264 workflow and camera long-GOP/two-audio-track trim/dissolve-to-ProRes
  workflow. No CLI workflow test was filtered out. Runtime after compilation: 7.65 s.
- `python scripts/ci/m1-checks.py`: 273 Rust cases passed, including all 16 new native export
  tests and 2 new MCP completion tests. Formatting and the gate helper test passed. Local time
  78.5 s, including an engine/MCP rebuild; this is not a hosted duration estimate.
- Clippy with `--all-targets -- -D warnings` passed for engine, export, automation and CLI.
  `cargo check --locked --offline -p shotclutch -p shotclutch-ui-egui` passed. This is build
  compatibility evidence, not a new interactive desktop acceptance run. `git diff --check` passed.
- Untracked local logs: `target/m1/export-regression-verified.log`, `export-cli.log`,
  `export-gate.log`, `export-clippy-final.log`, and `export-desktop-check.log`.

The first local tests exposed Windows sync through a read-only handle; finished files now sync
through writable handles before destination permissions are applied. Broad regression caught
byte-imported originals being dropped with preview file caches; the job now retains their
immutable encoded buffers and opens fresh decoders, preserving the existing audio render/export
parity test. The existing queue retry's I/O error contract is retained. All final checks passed
without suppressing failures or adding ignored tests. Four obsolete timestamp-deletion helper
tests were replaced by two confirmed-result tests and a stronger real MCP cancellation test that
preserves an existing approved delivery.

No workflow triggers, GitHub settings, hosted runs, pushes or releases were used. The single
ready-PR gate automatically discovers the new headless `m1_` cases. Full local qualification
commands remain available. No existing locked dependency version was upgraded; native engine
staging uses the already-locked `tempfile` dependency.

Next: complete the bounded Windows SDR/fallback and mixed-rate/VFR/nesting/retiming qualification,
run the offline mixed-media journey, and implement/qualify collected-project portability with
explicit essential assets. Native single-file delivery is implemented; durable whole-file-set
restart recovery, physical interruption, storage/performance budgets, physical A/V sync and the
updated second-PC fixture remain open. M1 remains In Progress.


## Collected-project portability continuation — 2026-11-04

Native collect now produces a staged, verified package with relative references and a versioned
SHA-256 manifest. Full-document copies preserve identity/revision, retained undo/redo and operation
receipts, including historical-only source dependencies. Selected/pruned copies explicitly receive
new identities and omit history. Immutable byte originals and complete numbered images are
collected; generated narration and originals are essential, attached proxies are optional caches.
Embedded LUT/authored data remain. External scratch/ingest/LUT import paths are removed from copies.

`file.verifyCollection` checks files, native integrity/history and dependency mappings through a
retained directory capability. Existing destinations, traversal, links, unknown extension dependencies,
changed originals and corruption fail explicitly. Operation UUIDs support verified destination-scoped
retries. CLI waits for collection by default; MCP/bridge use existing progress/cancellation polling.
The desktop dialog explains history and new-folder behavior and disables preview-cache collection.

See [COLLECTION_CONTRACT](COLLECTION_CONTRACT.md) for exact modes, failure behavior, budgets,
font/decoder requirements, platform limits and the updated physical second-PC acceptance procedure.
Consolidation remains a separate legacy transcode workflow. M1 remains In Progress.


Final local evidence for the collection continuation:

- `cargo test --locked --offline -p shotclutch-engine -p shotclutch-automation --lib`:
  **822 passed** (798 engine, 24 automation), zero failures; 3 existing engine performance tests
  ignored. Engine runtime after compilation: 50.58 s. All 23 new native collection cases passed.
- `cargo test --locked --offline -p shotclutch-cli --test it`: **20 passed**, including the new
  process-boundary collect/wait/verify/relocate/retry test and both existing VFR/H.264 and
  long-GOP/two-audio-track/ProRes workflows. Runtime after compilation: 4.67 s.
- `python scripts/ci/m1-checks.py`: **297 Rust cases passed**, formatting and helper checks passed,
  in 168.2 s locally including compilation. This is not a hosted runtime estimate. The gate
  discovers the new headless cases automatically; the Windows junction case stays local.
- Clippy passed with `--all-targets -- -D warnings` for engine, automation, CLI and egui UI
  (39.34 s). `cargo check --locked --offline -p shotclutch -p shotclutch-ui-egui` passed (20.20 s).
  This verifies desktop build compatibility, not a new interactive UI acceptance run.
  `git diff --check` passed.
- Local untracked logs: `target/m1/collection-regression-final.log`, `collection-cli-final.log`,
  `collection-gate.log`, `collection-clippy-current.log` and `collection-desktop-current.log`.

The first collision test demonstrated that the standard Windows rename could replace an empty
existing directory. Publication now uses an explicitly exclusive Windows API, with both empty
and nonempty concurrent targets covered. Fixture placement and Windows junction argument issues
were corrected before the final pass. No tests were suppressed or newly ignored. Final review
also added stored-path policy for history-only references before host I/O and retention of encoded
byte originals when their editorial state is intentionally offline. Every numbered frame must map
to the correct essential source, not an optional proxy/cache.

The only new locked package is pinned Windows-only `atomicwrites` 0.4.4; Unix publication uses
already-locked `rustix` 1.1.5. No existing dependency version was upgraded. No hosted workflow,
trigger, settings, push, Actions dispatch or release was used. The minimal CI policy is retained.

Next: bounded Windows SDR/fallback and mixed-rate/VFR/nesting/retiming qualification, the full
mixed-media offline journey, storage/performance and durable file-set restart recovery, physical
A/V checks, and the updated physical second-PC package check. Collection is implemented and
locally qualified; EXPORT-006 and M1 remain In Progress until the remaining acceptance evidence
exists. The second-PC procedure is in COLLECTION_CONTRACT, not recorded as a completed run.
