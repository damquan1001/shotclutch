# M1 implementation and acceptance

Started 2026-11-02 on `codex/m1-editing-foundation`. Owner: Codex.

M0 and the subsequent identity work are the starting baseline. M1 is In Progress. No M1
requirement is Done solely because an inherited feature exists or a new unit test passes.

## Current scope adjustments — 2026-11-05

The user deferred physical display/speaker A/V sync verification beyond M1 on 2026-11-05.
Physical sync remains unverified. The ten-minute device-clock audio criterion passed for the
declared fixture. This decision supersedes physical A/V exit obligations in the historical
records below. Disconnected operation and manual keyboard acceptance remain open.

The user excluded the physical second-PC check from M1. Local collection, integrity, relinking
and relocation remain required. Physical cross-machine qualification is deferred beyond M1 and
is not a current exit blocker. This decision supersedes second-PC obligations in the historical
continuation records below; it does not assert that a second-machine run passed. M0 evidence
remains unchanged. All other M1 acceptance criteria and the minimal hosted CI policy remain.

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
5. Run the offline journey, fault boundaries, long-form audio checks and local project
   relocation/integrity (physical second-PC qualification is deferred beyond M1). Record actual evidence and limitations against the requirements below.

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
| CORE-003 | Exact boundary strings, source PTS/half-open VFR seeks, fractional-rate export endpoints and mixed-rate/nested/retimed local journey qualified; wider runtime qualification remains | In Progress |
| CORE-004 | Atomic project/history/revision snapshots, checked recovery, migration backups and injected write/publication failures; broader fault/platform/storage-budget qualification remains | In Progress |
| AGENT-001–002 | Bounded structural service: host grants, exact wire values, source SHA-256, durable retries and CLI/MCP parity; final M1 acceptance review remains; general agent verification is M3 | In Progress |
| TEST-001 | Focused gate and local checks; extend with persistence faults and new schema invariants as implemented | In Progress |
| UI-001 | Candidate desktop smoke passed for real VFR seeking, menu split, keyboard save/undo/redo and independently reopened linked cuts; complete manual/accessibility workflow remains | In Progress |
| MEDIA-001–004 | [DEC-006](DEC-006_NATIVE_MEDIA.md) native/software/fault-fallback SDR corpus and mixed-rate proxy/render/audio journey qualified on this Windows PC; physical/runtime checks remain | In Progress |
| EXPORT-001 | Native original binding, verified output staging and publication implemented; full media/export qualification remains | In Progress |
| EXPORT-006 | Verified relative-path collection, full history or explicit derived copy, and local relocation; physical second-PC check excluded from M1 by user decision | In Progress |
| SEC-001–002 | Scoped-service denial, private grants, source confinement and cancellation tests; visible native local-only status; disconnected journey and final permission coverage review remain | In Progress |

Tests cover private staging, command equivalence, linked trim, explicit split, abort, grouped
undo/redo, failed later commands, locked targets, stale revision/project replacement, exact wide
time strings and rejection of unsupported operations. These are structural checks; they do not
establish decoder, physical audio-clock, crash recovery or second-machine qualification.

Local runtime gates remain those in documents 14–16: independent frame/sample verification,
preview/export composition agreement, no underruns in the 10-minute fixture, and A/V sync within
one frame without cumulative drift over 30 minutes. Unavailable hardware evidence keeps its applicable criterion outstanding. The physical second-PC
check is excluded from M1 by the user scope decision above. No repeated full hosted M0 qualification is required for M1.

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

## Native media and local mixed-media journey — 2026-11-04

The user's current scope decision excludes physical second-PC checks. Local collection/relink,
integrity and relocation remain required and are exercised below. Historical next-action lists
above are superseded by this section and the scope adjustment at the top of this document.

[DEC-006](DEC-006_NATIVE_MEDIA.md) selects the inherited OS Media Foundation/DXVA path with the
in-process native codec registry as fallback. A strict local-only qualifier generates nine bounded
SDR/video/image/PCM/error cases and compares the native stack against independent ffprobe/FFmpeg
oracles in software, real hardware Auto and simulated-failure modes. All 27 case/mode outcomes
passed. This adds no FFmpeg runtime dependency, downloads or routine hosted qualification jobs.

The corpus exposed zero presentation timestamps on decoded movie frames. Container clocks now
reach the shared GOP publication boundary, including MP4 edit offsets and MXF origins. Arbitrary
MP4/MKV seeks respect actual half-open presentation intervals; exact nominal-grid requests retain
the existing coarse-clock muxer policy. A small in-process real-container regression covers VFR,
fractional timing, edit offsets, frame identity, boundaries and cold/warm seeks without an oracle
tool in CI. The legacy decode benchmark now records partial failures and returns a failure status
instead of claiming nominal complete-frame throughput.

The real CLI/MCP mixed-media journey passed using generated phone VFR H.264/AAC and long-GOP
25 fps camera media with two audio tracks. It covers 24 fps editing nested into 30000/1001 fps,
linked 2x retiming and trim, exact fractional export endpoints, save/undo/redo/reopen, generated
proxy attachment, full-history collect/verify/retry, relocation with original/proxy directories
unavailable, and Save As outside the package. All four 56-frame ProRes outputs have identical
independently decoded RGBA frames and PCM samples. The retimed beep and exact sample count pass.
The sources remain byte-identical and the delivered package still verifies.

That journey exposed a Save As defect: relative collected references were interpreted beside the
new document. Native save now prepares equivalent references in current and retained history,
then adopts paths/history/cache keys only after publication succeeds. Save a Copy leaves the live
document unchanged; failed writes preserve it too. Same-folder paths remain relative where their
meaning is unchanged. Three focused regressions additionally cover history-only originals,
proxies, owned encoded buffers, failed saves and original package immutability.
The broader stored-path regression also preserves first-save behavior: an unbound authored
relative reference takes the destination folder as its base, while known user-picked bindings
retain their previous meaning. The failure regression reaches a real publication error after
snapshot preparation, as well as rejecting an occupied destination.

The complete local CLI integration suite passed: **21 tests**, no failures, ignores or filters,
with required installed oracle tools enabled. Runtime after compilation: 8.49 seconds. Evidence:
`target/m1/media-cli-final.log`; initial Save As failure: `target/m1/media-journey.log`.

Current remaining M1 acceptance: complete interactive editing/accessibility qualification beyond
the bounded desktop smoke below; a genuinely disconnected mixed-media run; long-form device-clock
underrun and physical A/V drift checks; and final review of the M1 fault/service permission coverage.
The declared native SDR subset and connected same-PC workflow do not establish those results.
Agent composed-output verification is AI-004/M3; hostile scene isolation and full release performance
are SEC-003/PERF-002/M4. Wider format/platform and file-set restart guarantees remain explicitly
unqualified; they are not inferred from the bounded native profile. Physical second-PC testing is
outside this milestone. M1 remains In Progress.

Final current-candidate checks continued on 2026-11-05:

- Focused M1 gate: **301 Rust tests passed**, formatting and the helper test passed; 259.3 seconds
  locally including compilation. No hosted run was dispatched. Log: `target/m1/media-gate-final.log`.
- Full engine/automation libraries: **826 passed** (802 engine, 24 automation), zero failures;
  3 existing engine performance tests ignored. Engine runtime: 72.35 seconds.
  Log: `target/m1/media-engine-final.log`.
- Codecs: **133 passed**; platform: **50 passed**. Their inherited conditional oracle helpers
  can return early when a backend/tool is unavailable (the legacy codec helper uses Unix paths),
  so these counts are regression-function results, not additional format qualification. Strict
  Windows corpus evidence is provided separately by DEC-006's fail-closed native qualifier.
  Logs: `target/m1/media-codecs-final.log`, `target/m1/media-platform-final.log`.
- Full CLI suite repeated on the final source: **21 passed**, no failures, ignores or filters,
  required oracle tools enabled; 20.53 seconds after compilation. This supersedes the earlier
  timing above. Log: `target/m1/media-cli-final.log`.
- Clippy with `--all-targets -- -D warnings` passed for engine, codecs, platform, automation,
  CLI and egui UI (46.85 seconds). `git diff --cached --check` passed for the complete staged
  patch, including new files. Log: `target/m1/media-clippy-final.log`.
- Desktop candidate built successfully with locked/offline Cargo (6m 21s). Strict native
  qualification regenerated all inputs and passed all **27 case/mode outcomes** on this final
  source. All verified video hashes match across software, real Auto and injected-failure modes.
  [Compact reports and scope](evidence/README.md); local inputs/logs: `target/m1/media-final/`,
  `media-desktop-build.log`, `media-native-final.log`.
- The revised benchmark was built and given a private malformed MP4 through its normal decode
  section. It returned exit **1**, `status:failed`, `decoded_frames:0` and an explicit open error,
  without a nominal FPS. Logs: `target/m1/media-bench-build.log`, `media-bench-failure.log`;
  result: `target/m1/benchmark-failure/report.json`. Shared fixtures were untouched.
- The isolated desktop candidate opened a private copy of the real generated VFR phone source.
  Shift+Right displayed source frame 5 at sequence timecode `00:00:00:05`; Sequence > Add Edit
  to All Tracks split linked video/audio into `[0,5)` and `[5,120)`. Ctrl+S saved it. Independent
  CLI opens verified both cut boundaries, source offsets `0` / `42336000000`, and matching link
  groups. Ctrl+Z / save restored one 120-frame clip per track; Ctrl+Shift+Z / save restored exactly
  the split tracks. The editor then exited normally. [Summary and clean screenshot](evidence/README.md).
  Ctrl+Shift+K was intercepted by another application's global shortcut on this PC, so its
  editor effect is not claimed; the visible menu command was verified instead. This is bounded
  UI smoke evidence, not the full manual/accessibility, physical A/V or disconnected acceptance.

No hosted workflow, trigger, settings, push or release was used. Routine CI remains the existing
single headless gate; only four small in-process regressions join it automatically. Synthetic
oracle generation, hardware modes, full workflows, benchmark checks and UI smoke remain local.

## Native processing status and same-PC acceptance preparation — 2026-11-05

General Settings now visibly reports **Local-only processing** for the native built-in profile.
It is a read-only status: there is no built-in cloud inference transport to enable. Optional model
downloads remain explicit owner actions. `privacy.status` exposes the same information without
changing preferences, project, history or revision. A host-injected recognizer (or browser host)
is opaque, so its status is `hostControlled` with `localOnly:null`; the UI does not certify it as
local. This status is neither a provider authorization nor an OS network-isolation control.
Provider-specific payload grants must precede any future cloud processing implementation.

The new [same-PC qualification procedure](OFFLINE_AND_AV_QUALIFICATION.md) reuses the existing
CLI mixed-media journey and unchanged M0 desktop runtime runner. The offline wrapper requires a
clean committed checkout, installed oracles, and no active non-loopback network adapter, including
virtual adapters, before and after the journey. It never changes adapters/firewall rules. Its
connected diagnostic passed the exact required journey with no skips; the ordinary invocation
correctly returned exit 2 while network adapters were active. **Neither is an offline pass.**
Adapter snapshots also do not claim packet capture or continuous OS network isolation.

A 30-minute 1080p30 flash/beep fixture is prepared locally (1,802 seconds of source with an
initial quiet ten seconds). Independent FFmpeg decoding checked the first two flash boundaries
and audio onsets, which occur two 48 kHz samples after the intended video-frame onset. This is a
synthetic timing fixture, not a representative decode/effects workload. A three-second native
desktop smoke completed using the real audio-device clock: 312 callbacks, zero missing samples,
zero underruns and zero dropped pictures. It validates the runner and current binary, not the
ten-minute underrun or thirty-minute physical drift criteria. The separate ten-minute result
is recorded below. Physical screen/speaker measurement still needs an external recording.

Current-source local checks:

- Focused M1 gate: **302 tests passed**, formatting/helper check passed, 188.2 seconds including
  compilation. The one added small privacy-status engine regression joins the existing gate;
  there is no new hosted job. Log: `target/m1/privacy-gate.log`.
- Settings tests: **31 engine** and **16 UI** passed. Read-only status cannot change preferences,
  and an injected host backend cannot receive a false local-only assertion.
- Complete egui integration suite: **498 passed, zero failed, five existing ignored**, 112.18
  seconds. The ignores are four screenshot helpers and one manual transcript benchmark, not new
  suppressions. Log: `target/m1/privacy-ui-full-final.log`.
- The first full UI run found two stale tests. The inherited-project fixture now represents an
  actual pre-integrity v16 file instead of corrupting a protected v17 header; production checksum
  validation is unchanged. Proxy/collection UI checks wait for confirmed job outcomes, count
  originals plus attached proxies, verify the resulting package, and close owned handles before
  cleanup. Both targeted cases and the complete suite then passed.
- Engine/UI all-target Clippy with warnings denied passed (final incremental check 12.42 seconds);
  the desktop built with locked/offline Cargo (2m 01s). Logs: `target/m1/privacy-clippy-final.log`
  and `target/m1/privacy-desktop-build.log`. Earlier native decoder, full engine and CLI evidence
  remains recorded above; those results are not relabeled as a repeat after this settings change.

No hosted Actions, network configuration, provider grant, release or remote repository state
was changed. SEC-001 remains In Progress pending actual disconnected acceptance.

### M1 boundary review

The structural service suites exercise readonly/mutation denial, actor spoofing, stale revision,
cross-project/sequence access, expiry/revocation, cancellation, source-root traversal/junctions,
bounded reads, durable receipts and commit-marker recovery. Native save tests cover failure before
publication, recovery after the durable marker, failed metadata, retry and protected migrations.
The CLI rejects incomplete scope options rather than falling back to owner access; actual scoped
CLI and MCP tests verify replay and denial of raw commands, file/UI access and frame rendering.
The 302-case gate includes the engine/MCP cases; the separately recorded 21-case CLI suite includes
the real CLI boundaries. These qualify the bounded native profile, not arbitrary future commands.

Unsupported scoped operations remain denied. General scoped mutation families, interactive grant
review and agent composed-output verification are not implemented by the structural subset; the
M3 protocol/policy/verification program must retain those limits until it broadens the grants.
Hostile scene execution isolation and the full platform/performance release matrix belong to M4.
The native multi-file export contract also retains its explicit physical-interruption/restart
limitation; ordinary rollback is tested and single-file publication is atomic. None of these
limitations is converted into a passing result by the new status UI or qualification scripts.

The next M1 acceptance actions are the actual disconnected journey and the manual
editing/keyboard-accessibility reference workflow. M1 remains In Progress.

### Completed ten-minute device-clock observation

The current desktop completed **600.094 seconds**, with 119 samples and the real device audio
clock active throughout. All **56,145 callbacks** were monotonic, with **zero underruns and zero
missing audio frames**; minimum observed retained lead was 72.54 ms. The native Media Foundation
path decoded the simple 1080p30 flash/beep fixture at Full preview resolution. The owned editor
stopped and exited normally. This passes the ten-minute audio-counter criterion for this profile.

Video recorded 17,943 shown and **24 dropped pictures (0.134%)**, exceeding the proposed <0.1%
drop budget. Do not call this a performance-target pass. Ordinary desktop/agent activity included
the six-second CLI journey diagnostic during playback, so this is not an isolated regression
benchmark or representative effects-load qualification. Clock progress minus wall time ended at
**-1.327 seconds**; that counter does not measure physical speaker/display offset and cannot
establish thirty-minute A/V sync. No external recording was made.

[Retained reports, raw samples, source/runner digests and evaluation](evidence/runtime-20261105/README.md)
include the connected diagnostic rerun from clean commit `610a6cdd` (one journey passed, zero
ignored, 6.83 seconds, source unchanged). The connected diagnostic and ten-minute runtime result
do not close the disconnected, full manual keyboard or physical drift gates. No hosted run was
used and physical second-PC qualification remains excluded.

### External capture availability — 2026-11-05

The user confirmed that no external phone or camera is available for the physical screen/speaker
measurement and subsequently deferred that measurement beyond M1. The ten-minute audio-counter
pass remains valid for its declared profile. Disconnected and manual keyboard acceptance remain
open. The earlier decision also defers physical second-PC testing beyond M1.

### Native keyboard trim verification — 2026-11-05

Opened a private copy of the VFR project in the Windows desktop editor. At frame 13, the
unshifted Q shortcut removed source frames [5, 13) from both linked tracks. The saved project
kept the edit at sequence frame 5, set the second segment's source start to frame 13 and ended
at sequence frame 112. Both segments retained matching video/audio link groups. Ctrl+S saved
the edit, Ctrl+Z restored the prior cut and Ctrl+Shift+Z reproduced the same saved track data.
The editor then closed normally. Uppercase Q sent by the automation tool invoked Shift+Q's
Extend Previous Edit and was undone before the ripple check. This covers native trim, save and
history. The full manual keyboard worksheet remains open. Active Ethernet and Wi-Fi interfaces
also prevented the disconnected run during this session.

### Remaining native keyboard steps — 2026-11-05

Completed the representative keyboard steps in the UI-001 worksheet on the isolated Windows
desktop project. Shift+2 and Shift+3 switched panels, Shift+Right sought from frame 5 to 10,
and D selected the linked clip under the playhead. Ctrl+Right moved both linked clips from
frame 5 to 6 with their source ranges intact. Ctrl+K split both tracks at frame 10 and Ctrl+S
persisted the matching ranges [0, 5), [6, 10) and [10, 113) with source starts 0, 13 and 17.
The earlier native ripple trim, undo and redo checks cover the other edit shortcuts.

Ctrl+, opened Preferences and Escape closed it. Ctrl+O opened the Windows file picker and
Escape canceled it. On the acceptance run, Alt+N selected File name, Tab visibly focused
the file-type control, and Shift+Tab returned to File name. Enter closed the picker and loaded
the distinct project, whose `sync-30.mp4` timeline appeared in the native accessibility tree.
The first Enter attempt used a copy with the same document identity, so a distinct project
was used to confirm the observed switch. The isolated editor closed normally.

The keyboard portion of the worksheet is complete. The broader manual editing walkthrough
remains open. The user deferred the network-disconnected mixed-media run beyond M1 on
2026-11-05. Actual disconnected operation remains unverified, and the connected journey
remains valid for its tested conditions. This decision supersedes earlier disconnected exit
obligations in this document. No hosted CI run was needed for these desktop checks.

### Completed local desktop editing walkthrough — 2026-11-05

The remaining local walkthrough passed on this Windows PC. Used the real desktop editor with
separate app data and private copies of the phone VFR and 25 fps camera fixtures. The connected
desktop command interface drove the repeatable editing and delivery steps. Native window input
verified reopening, the monitor and the Audio Gain dialog, complementing the earlier native
keyboard split, move, trim, history and dialog results.

- Built a 24 fps multitrack sequence, retimed the linked phone clip to 200%, applied -6 dB gain
  and a three-frame Constant Power fade, and nested it in a 30000/1001 sequence. A one-frame
  linked trim survived undo and redo. The program monitor and playhead agreed at frame 12.
  A short preview used the device audio clock. This is preview evidence for the fixture.
- Created and enabled a proxy without changing either timeline. Reopened with the camera
  original moved to a new folder under its original name and relinked it. The first renamed
  source was rejected by the filename check. Full-history collection required the old path
  referenced by undo, so the identical source was supplied at that path before collection.
- Verified the collection, then relocated it locally with original and proxy paths unavailable.
  The desktop reopened the package with zero missing media and saved a working copy outside it.
  The working copy reopened with zero missing media, the original package still verified and the
  source digests matched. The harness stored app data outside the package. An earlier attempt
  collided with the collection directory on Windows and the verifier rejected the extra files.
- Independent FFprobe/FFmpeg checks found 56 frames and 89,690 stereo sample frames in each
  explicit-range ProRes export. Decoded RGB and PCM digests matched across the original,
  proxy-enabled, relinked and relocated exports. Raw output and the result report are in
  `target/m1/desktop-mixed-jm24_gse/`.
- The visible working copy showed both sequence tabs and a decoded retimed phone frame. The
  Audio Gain dialog showed a -26.7 dB peak for the selection. Entering +3 dB in Adjust Gain
  and confirming saved the phone audio at -3 dB. Undo and save restored the prior project
  contents. The private editor exited normally.

The local editing and keyboard worksheet is complete for this profile. The approved scope
deferrals and the limits of the native media and export contracts remain recorded above.
