# ShotClutch — Development Tracker

Baseline 0.1 · Last updated: 2026-11-24 · Authoritative living work record.

## Current state

The documentation baseline is committed at `b739515e04b3467ed8b681e6296f7dc5867f43dd`. M0 implementation is delivered on `codex/m0-foundation`, based on inherited source `3a5f217b348bf0c9dc1678f305c34b6c88617f37`. The initial working tree was clean. Production application revision: `3aeef47218c74067fb4998b0d14e4aacee7dad10`; subsequent commits add test, measurement and evidence refinements. See [the foundation audit](m0/FOUNDATION_AUDIT.md), [executed evidence](m0/EVIDENCE.md), [qualification procedure](m0/QUALIFICATION.md), and [upstream policy](m0/UPSTREAM_POLICY.md).

Current milestone is **M3 — Semantic media and agent workflow**. AI-001 through AI-004 are Done for native observations, local evidence search, progressive inspection, and staged edit verification on `codex/m3-semantic-media`. **M2 — Smart Cut is Done under the review-assisted scope (2026-11-24).** M1 is Done under revised scope (2026-11-05) and M0 is Done (2026-11-01). M2 implementation and Windows qualification are on `codex/m2-smart-rough-cut`, frozen by `m2-closeout-2026-11-24`. The cut-first workflow replaces suggestion approval at the user's request. Granite 2B remains the local reviewer. The failed automatic-review experiment remains unpromoted. Expanded runtime/release qualification and remote repository promotion remain separate work. [M2 closeout](m2/M2_COMPLETION.md)

There are **48 implementation requirements: 44 in v0.1 (M0–M4) and 4 in M5**. CORE-001, CORE-005 and PERF-001 have their bounded M0 acceptance evidence and are Done. M1 is Done under revised scope on `codex/m1-editing-foundation`. M2 is Done under bounded review-assisted acceptance on `codex/m2-smart-rough-cut`. The M3 work item table records current status. M4–M5 remain Planned. [M1 implementation and acceptance](m1/IMPLEMENTATION.md) and [M2 implementation and qualification](m2/IMPLEMENTATION.md)

## How to maintain this document

Update after each completed work session, merged change, milestone gate, new blocker, or architecture decision. Keep the roadmap stable unless scope/order changes; this file records current reality. Status vocabulary:

| Status | Meaning |
|---|---|
| Planned | Defined but no active implementation work recorded |
| In Progress | Owner actively working; include branch/change reference and next step |
| In Review | Concrete implementation and evidence ready for review |
| Blocked | Active work cannot proceed; link a specific blocker and unblock condition |
| Done | Acceptance met, reviewed, and backed by reproducible evidence |
| Deferred | Explicit scope decision; retain ID, rationale, and intended future milestone |

Never mark a requirement Done based only on a plan, inherited feature list, tool acceptance, or a flat-render fallback for an editable export. Reopen a Done row when evidence no longer passes. Preserve the ID and history when splitting work; subtasks use suffixes such as `PRE-004.a` and roll up to the parent only when all acceptance criteria are met.

Each active item must have an owner, implementation/change reference, linked tests/results, next action, and any blocker. Planned M1–M5 rows share **Owner: unassigned; implementation/evidence: none**. M0 has per-item records below. Requirement acceptance lives in [02](02_PRODUCT_REQUIREMENTS.md); test families in [15](15_TESTING_STRATEGY.md).

## Milestone dashboard

| Milestone | Scope | Status | Exit evidence |
|---|---|---|---|
| M0 | Audit, standalone build, baseline measurements | Done | [Completion and acceptance mapping](m0/M0_COMPLETION.md): local/clean-install/core evidence, two independent same-source green CI runs, accepted second-PC verification and paired baselines. Expanded runtime/release and promotion gates remain separately tracked. |
| M1 | Durable project, exact commands, native media, local editing | Done | [Scoped closeout and evidence](m1/IMPLEMENTATION.md#m1-closeout--2026-11-05) |
| M2 | Smart Cut and workspace | Done | [Scoped closeout and frozen checkpoint](m2/M2_COMPLETION.md), [acceptance mapping](m2/ACCEPTANCE.md), [repeatable qualification](m2/QUALIFICATION.md). Granite 2B retained; automatic quality limits recorded. |
| M3 | Semantic indexing, verified agent, MCP/CLI | In Progress | AI-001 through AI-004 and AGENT-003/004 complete |
| M4 | ACP/Skills, native scenes, interchange foundation, v0.1 qualification | Not started | None |
| M5 | Web scenes and qualified destination adapters | Planned follow-on | None |

Full entry/exit criteria: [16_MVP_ROADMAP.md](16_MVP_ROADMAP.md).

## Requirement work register

Dependencies below are completion gates, not a requirement to postpone design or test preparation. TEST-002 is the aggregate release gate; “all v0.1 requirements” excludes TEST-002 itself. The common owner/evidence fields above apply until replaced by an active-work record.

| ID | Work item | Priority | Milestone | Status | Depends on |
|---|---|---|---|---|---|
| CORE-001 | Build a standalone ShotClutch fork | P0 | M0 | Done | — |
| CORE-002 | Use one canonical project with stable IDs and versioned extensions | P0 | M1 | Done | CORE-001 |
| CORE-003 | Use integer ticks and rational rates for edit math | P0 | M1 | Done | CORE-002 |
| CORE-004 | Persist changes crash-safely | P0 | M1 | Done | CORE-002 |
| CORE-005 | Establish reproducible build, automated checks, release packaging plan, and upstream-change policy | P0 | M0 | Done | CORE-001 |
| UI-001 | Provide usable multitrack editing, exact trim/split/move, linked A/V, nesting, monitor seeking, audio levels, save, relink, and undo | P0 | M1 | Done | CORE-003, AGENT-001, MEDIA-003, MEDIA-004 |
| UI-002 | Provide a dedicated Smart Cut workspace with transcript, waveform, source and result preview, take comparison, reasons, and apply/restore controls | P0 | M2 | Done | PRE-002, PRE-003 |
| MEDIA-001 | Select a usable hardware, native, or fallback decoder through one source interface | P0 | M1 | Done | CORE-001, CORE-003 |
| MEDIA-002 | Generate/use proxies while preserving source identity | P0 | M1 | Done | MEDIA-001 |
| MEDIA-003 | Evaluate tracks, effects, transitions, and nested sequences through a shared render graph | P0 | M1 | Done | MEDIA-001, CORE-003 |
| MEDIA-004 | Mix audio with sample-accurate ranges, linked edits, gain, fades, and device-clock playback | P0 | M1 | Done | MEDIA-001, CORE-003 |
| MEDIA-005 | Keep interaction responsive under load using bounded caches, prioritized work, cancellation, and adaptive preview | P1 | M4 | Planned | MEDIA-002, MEDIA-003, MEDIA-004, PERF-001 |
| PRE-001 | Start Smart Cut only by explicit invocation with selected sources and settings | P0 | M2 | Done | AGENT-001, SEC-002 |
| PRE-002 | Persist versioned PreEditAnalysis and PreEditMap data with source mapping and confidence | P0 | M2 | Done | CORE-002, CORE-003, CORE-004 |
| PRE-003 | Identify candidate pauses, fillers, restarts, and alternative takes while preserving editorial meaning | P0 | M2 | Done | PRE-001, PRE-002, MEDIA-004 |
| PRE-004 | Apply the current reviewed map through one transaction to an editable result | P0 | M2 | Done | PRE-003, UI-002, AGENT-002 |
| PRE-005 | Restore rejected source material and revise a map without mutating originals or silently replacing already-edited timelines | P0 | M2 | Done | PRE-004 |
| AI-001 | Store asset/scene/shot/segment/frame observations with provenance, coordinate domains, and confidence | P1 | M3 | Done | CORE-002, MEDIA-001, SEC-001 |
| AI-002 | Search local transcripts, tags, OCR, captions, and embeddings where available | P1 | M3 | Done | AI-001 |
| AI-003 | Support progressive visual inspection from contact sheets to bounded clips and precise frames | P1 | M3 | Done | AI-001, MEDIA-003 |
| AI-004 | Verify agent changes against composed timeline output and audio where relevant | P1 | M3 | Done | AI-003, AGENT-002 |
| AGENT-001 | Route human and automated mutations through the same typed command service | P0 | M1 | Done | CORE-002, CORE-003, SEC-002 |
| AGENT-002 | Support revision-checked atomic transactions with preview, commit, rollback, cancellation, and grouped undo | P0 | M1 | Done | AGENT-001, CORE-004 |
| AGENT-003 | Execute bounded understand/inspect/plan/edit/verify/report loops with progress and cancellation | P1 | M3 | Done | AGENT-002, AI-002, AI-004, AGENT-004 |
| AGENT-004 | Enforce Ask, Assist, and Autonomous policies independently of client transport | P0 | M3 | Done | SEC-002, AGENT-002 |
| AGENT-005 | Store transaction rationale and generated-asset provenance without secrets | P1 | M3 | Planned | CORE-002, AGENT-002, SEC-004 |
| PROTO-001 | Expose scoped, discoverable MCP tools/resources over a versioned adapter | P1 | M3 | Planned | AGENT-001, AGENT-002, SEC-002 |
| PROTO-002 | Provide scriptable CLI operations with machine-readable results, nonzero failures, revision controls, and job inspection | P1 | M3 | Planned | AGENT-001, AGENT-002, SEC-002 |
| PROTO-003 | Integrate ACP sessions, supported capability negotiation, streamed progress, cancellation, and permission responses | P1 | M4 | Planned | AGENT-003, PROTO-001 |
| PROTO-004 | Load versioned Skills as instructions/examples/templates with declared capabilities | P1 | M4 | Planned | AGENT-004, SEC-003 |
| MOTION-001 | Evaluate a programmatic scene as `render(t, context) -> SceneIR` independently of request order | P1 | M4 | Planned | CORE-003, SEC-003 |
| MOTION-002 | Render a declared native subset including groups, text, shapes, images, transforms, opacity, clipping, and keyframe/easing animation | P1 | M4 | Planned | MOTION-001, MEDIA-003 |
| MOTION-003 | Render explicit web scenes in an isolated runtime into timestamped RGBA surfaces | P2 | M5 | Planned | MOTION-002, MOTION-004, SEC-003 |
| MOTION-004 | Cache scenes by all observable inputs and preserve provenance | P1 | M4 | Planned | MOTION-002, MEDIA-005 |
| EXPORT-001 | Render a selected range from a pinned project revision with exact duration, audio, and declared output settings | P0 | M1 | Done | MEDIA-002, MEDIA-003, MEDIA-004, SEC-002 |
| EXPORT-002 | Build a transient interchange representation and per-object fidelity report | P1 | M4 | Planned | CORE-003, AGENT-005, EXPORT-001, MOTION-002 |
| EXPORT-003 | Qualify a Premiere destination adapter using an appropriate interchange route and pinned target version | P2 | M5 | Planned | EXPORT-002 |
| EXPORT-004 | Qualify a DaVinci Resolve destination adapter with the same evidence standards; destination-specific mappings remain isolated from canonical project data | P2 | M5 | Planned | EXPORT-002 |
| EXPORT-005 | Qualify a CapCut adapter only after establishing a maintainable supported route | P2 | M5 | Planned | EXPORT-002 |
| EXPORT-006 | Collect/relink a relocatable project (M1 local qualification; physical second-PC check deferred) | P0 | M1 | Done | CORE-004, MEDIA-002 |
| SEC-001 | Verify native local-only processing status and the local mixed-media workflow (disconnected acceptance deferred) | P0 | M1 | Done | CORE-001 |
| SEC-002 | Enforce least-privilege grants at service boundaries | P0 | M1 | Done | CORE-002 |
| SEC-003 | Isolate untrusted scene code and extension execution | P0 | M4 | Planned | SEC-002 |
| SEC-004 | Keep provider credentials outside projects/logs and support retention/deletion controls | P0 | M3 | In Progress | SEC-001, SEC-002 |
| PERF-001 | Establish reproducible performance fixtures and baseline captures with hardware, software, media, and cache conditions | P0 | M0 | Done | CORE-001 |
| PERF-002 | Meet or explicitly renegotiate the release budgets in document 14 on declared hardware | P1 | M4 | Planned | PERF-001, MEDIA-005, MOTION-004 |
| TEST-001 | Run invariant, command, persistence, and regression checks in CI | P0 | M1 | Done | CORE-005, CORE-004, AGENT-002 |
| TEST-002 | Pass the v0.1 release workflow, security, recovery, and qualification gates | P0 | M4 | Planned | TEST-001, PERF-002; all v0.1 requirements |

## Active work and evidence register

M0 is complete on `codex/m0-foundation`. CORE-001 combines executable evidence with user-confirmed clean/core acceptance; CORE-005 has reproducible build/check/package/policy evidence; PERF-001 covers baseline captures. [Reviewed closeout](m0/M0_COMPLETION.md) separates these criteria from additional runtime/release and repository-promotion work.

| Record | State | Owner | Evidence / next action |
|---|---|---|---|
| DOC-001 | Done | Documentation author | All 17 numbered Markdown documents authored; structural validation and archive packaging accompany this baseline |
| REVIEW-001 | Planned | Unassigned | Review baseline API/schema choices against the selected source revision; record accepted amendments |
| SETUP-001 | Done | Codex | Recorded inherited revision and clean documentation baseline; selected Windows x64 and pinned Rust 1.95.0 |
| CORE-001 | Done | Codex / user acceptance | Production `3aeef472`, installer correction `b5102d3c`; standalone builds, identity/compatibility and capability/legal inventory recorded. Native edit/export has independent frame/sample checks. User confirms clean MSI install and launch/import/edit/save/reopen/export: [acceptance](m0/evidence/clean-install-user-confirmation.md). Next: broaden runtime coverage without changing this bounded M0 conclusion. |
| CORE-005 | Done | Codex | Rust 1.95 locked builds, pinned repository-owned CI, unsigned MSI/ZIP, attribution and upstream policy have reviewed executable evidence. Two independent repaired-source CI aggregates and packaging lint passed; clean/core and second-PC evidence accepted. [M0 closeout](m0/M0_COMPLETION.md). Repository promotion/settings and expanded runtime qualification remain separate. |
| PERF-001 | Done | Codex | Harness `f35fac68`; seven matching software/CPU sections, identical generated fixture hashes, available hardware/software/cache metadata and raw samples retained. [Comparison](m0/BENCHMARKS.md), [upstream](m0/evidence/upstream/manifest.json), [ShotClutch](m0/evidence/shotclutch/manifest.json). Acceptance is baseline capture, with slow cases and unavailable telemetry disclosed; GPU/second-profile work and release budgets remain separate. |
| MSI-UX-001 | Done | Codex / user acceptance | Fix `b5102d3c` adds a wizard and Finish launch checkbox; artifact validation passes. The user confirms successful clean installation and launched core workflow. [Diagnosis](m0/evidence/msi-wizard/diagnosis.json), [manual acceptance](m0/evidence/clean-install-user-confirmation.md). Finish-checkbox and uninstall paths get separate lifecycle coverage in RT-001. |
| CI-001 | Done | Codex | GitHub CLI/admin access and read-only settings preflight recorded; strict test/oracle/logging helpers deployed at 5f697163. Existing hosted runners only; billing budget not independently verified. [Execution](m0/CI_RUNTIME_EXECUTION.md). |
| CI-002 | Done | Codex | Two independent green runs at `a7fbcf2f`: 40834967572 (package/install recovery attempt 4) and 40860747696 (recovery attempt 2). Each retained cold attempt-1 checks: Windows 2,630 / Linux 2,646 / macOS 2,600 passing, 12 ignored each; required GPU probes, release/WASM builds and package/install/export/uninstall passed. Both outputs verified at 50 frames / 80,000 stereo sample frames. Packaging lint passed. Failed attempts remain recorded; these are not two fresh green attempts without retries. [Evidence](m0/CI_RUNTIME_EXECUTION.md#two-run-ci-acceptance-retained-2026-11-01). |
| CI-003 | Planned | Repository owner / Codex | Unique aggregate names prepared. Main promotion/required checks/schedules follow reviewed green runs and explicit merge/settings authorization; main remains unchanged. |
| RT-001 | In Progress | Codex / manual tester | Both repaired-source CI MSI packages installed/exported/uninstalled with preserved user sentinel on fresh hosted runners. Run 40834967572 attempt-4 MSI/ZIP also passed local inspection; its exact desktop passed the isolated native 1080p workflow (50 frames / 80,000 stereo sample frames). User Q0 remains accepted. Offline consumer VM and extended interactive lifecycle remain outstanding; VMware is installed but no disposable VM path has been identified. Owner environment input requested. |
| RT-002 | In Progress | Codex | Successful exact CI candidate `d0b9a11d`: isolated 1080p workflow/export passed (50 frames / 80,000 samples); Full 30m 35 drops (0.0648%), Half 10m 32 drops (0.1777%). Earlier `5f697163` Full missed / Half met the proposed threshold. No audio underruns/missing frames; NVIDIA Vulkan renderer / AMD MF decoder. Fifty 1080p trim/history cycles and offline/relink/long-project reopen passed; raw cancellation retained 6.75 MB partial output. Performance repeatability, physical A/V drift and extended coverage remain open. [Evidence](m0/CI_RUNTIME_EXECUTION.md). |
| RT-003 | Done | User / Codex | User accepted the verified second-PC core workflow as done and passing. Bounded core correctness PASS on separate Windows 10 Enterprise 19045 / i5-4690 / RX 570 / 24 GiB PC. Kit hashes match; readable replacement project loads through packaged CLI, 50-frame edits confirmed, returned ProRes fully decodes to 1080p30 / 50 frames / 80,000 stereo sample frames. User manual editing/MSI PASS, network-connected. Earlier zero-filled returns superseded, preserved with unknown cause. Expanded Windows 11/performance/offline/physical A/V gates remain open. [Verified report](m0/evidence/second-pc-readable-return-20261101/README.md). |
| M2: UI-002, PRE-001–PRE-005 | Done | Codex / project owner scoped acceptance | Frozen by `m2-closeout-2026-11-24` on `codex/m2-smart-rough-cut`, following explicit authorization to complete the assessed closeout tasks. M1 and expanded M2 gates pass; reviewed result preserves all 11 confirmed keeps and removes all 55.517 seconds of confirmed bad takes with 49 stored overrides. All 80 rendered joins pass the de-click check. [Closeout and checkpoint](m2/M2_COMPLETION.md), [acceptance mapping](m2/ACCEPTANCE.md), [evidence](m2/EVIDENCE.md). Granite 2B retained; failed automatic-review gates, unseen-recording accuracy and optional live AssemblyAI remain unqualified. |
| AI-001 | Done | Codex | Native observations on `codex/m3-semantic-media`, based on `main` at `4d85eb67`. Schema v19 qualified through save, recovery, source replacement and producer changes. Windows evidence covers offline speech ingestion, indexed MOV and Matroska frames, and storage at the record limit. [Implementation and acceptance](#ai-001-native-observation-acceptance--2026-11-24) |
| AI-002 | Done | Codex | Native lexical retrieval on `codex/m3-semantic-media`. [Implementation and benchmark](#ai-002-local-evidence-search--2026-11-25) |
| AI-003 | Done | Codex | Native source and timeline inspection on `codex/m3-semantic-media`. [Implementation and acceptance](#ai-003-progressive-visual-inspection--2026-11-25) |
| AI-004 | Done | Codex | Native staged composition verification on `codex/m3-semantic-media`. [Implementation and acceptance](#ai-004-staged-composition-verification--2026-11-25) |
| AGENT-004 | Done | Codex | Native edit policy on `codex/m3-semantic-media`. [Implementation and acceptance](#agent-004-editor-authorization-and-review--2026-11-25) |
| AGENT-003 | Done | Codex | Native offline phrase split on `codex/m3-semantic-media`. [Implementation and acceptance](#agent-003-bounded-phrase-split--2026-11-25) |

For each new active requirement, add owner, change/branch or commit, test/benchmark report, expected versus actual result, review outcome, next action and any blocker ID. Do not substitute prose status updates for reproducible evidence.

### AI-001 native observation acceptance — 2026-11-24

- The [project model](../crates/project/src/observations.rs) stores stable asset, scene, shot, segment and frame observations. Evidence identifies the original asset, full-file SHA-256 and source coordinates. Ticks are decimal strings. Source relationships can overlap, and model scores carry their meaning and versioned provenance. The [native engine API](../crates/engine/src/media_observations.rs) uses project-scoped editor grants, confined roots and durable commits. Source and producer freshness are evaluated independently on inspection. The host supplies the active model profiles.
- The native API records verified asset facts and can run an installed, pinned faster-whisper English profile on one original audio stream. The speech producer accepts a source window of at most 30 seconds and stores a bounded speech segment with exact source bounds and the mean score of the retained words. The model and inference configuration have separate digests. Frame and speech decoding use verified file snapshots capped at 64 MiB, and frame decoding has a pixel budget of 4096 × 2160. [Local speech producer](../crates/engine/src/media_observations/speech.rs)
- `cargo test --locked --offline -p shotclutch-engine --lib media_observations:: -- --nocapture` passed all 8 routine tests on Windows x64. They cover all five observation kinds, overlapping ranges, indexed MOV and Matroska variable presentation timestamps, save/reopen, recovery, source and producer freshness, confined access and atomic write failure. The replacement regression changes bytes beyond the quick relink hash. At 4,096 records, the recovery snapshot with one large undo state was 9,741,031 bytes. It reopened with the human annotation intact and a further append failed without changing the project. The routine suite is wired into PR CI. [Engine test cases](../crates/engine/src/media_observations/tests.rs)
- `cargo test --locked --offline -p shotclutch-format -p shotclutch-project` passed 53 format and 66 project tests, including v18 migration and rejection of invalid evidence. `cargo test --locked --offline -p shotclutch-engine --lib -- media_observations:: m1_media_tests relink_tests m1_transaction_tests` passed 41 routine tests. Workspace formatting and CI workflow YAML validation passed locally.
- A separate installed-model run passed with the pinned Tiny English profile, an isolated runtime copy, and an English WAV synthesized with Windows System.Speech. The test ingested real model words, saved and reopened the segment, and observed it become stale when the active profile changed to Base English while the source stayed current. To reproduce, point `SHOTCLUTCH_DATA_DIR` to data with the installed model and runtime and set `SHOTCLUTCH_AI001_SPEECH_FIXTURE` to a spoken English WAV under 30 seconds. Run `cargo test --locked --offline -p shotclutch-engine --lib qualified_local_speech_observation_survives_reopen_and_model_change -- --ignored`.
- The original M2 branch and annotated tag still resolve to their starting values. The frozen Windows executable and restored editing project match the checksums in the M2 closeout. [Checkpoint manifest](m2/closeout.json)
- AI-001 acceptance covers the scoped native API and indexed originals in the qualified Windows profile. Search and progressive inspection continue under AI-002 and AI-003, CLI/MCP under PROTO-001 and PROTO-002, and user retention and deletion controls under SEC-004. The full M3 workflow and broader cross-platform decoder qualification remain open.

### AI-002 local evidence search — 2026-11-25

- Native callers use `Session::search_media` with a project read grant. Results identify the source asset, recorded fingerprint when available, exact word range or observation location, evidence ID, score meaning, and source and producer freshness. The session caches the positional text index by project snapshot and revision. It checks candidate original bytes on each query within the shared grant budget. Coverage reports indexed text roles, missing originals, omitted stale or unverified matches, and budget-limited assets. [Search implementation](../crates/engine/src/media_search.rs)
- Schema v20 adds optional tag, OCR, caption and transcript roles and validated word intervals. A reopened v19 summary retains its original precision. New offline speech segments retain individual word timing when the recognizer supplies it. Existing asset transcripts retain their speaker labels and media times and remain individually unverified because they lack a capture fingerprint. Source-search coverage excludes timeline-only captions. The current build reports embeddings unavailable. [Observation model](../crates/project/src/observations.rs)
- The versioned English corpus uses two generated 24 fps originals with authored source annotations. Judgments cover a phrase shared by both assets, overlapping caption and tag evidence, look-alike OCR numerals, a summary, and a no-match query. Reproduce ranking, localization, coverage, index payload size and timings with `cargo test --locked --offline -p shotclutch-engine --lib media_search::tests::corpus_v1_retrieval -- --nocapture`. [Corpus and labels](../crates/engine/src/media_search/corpus_v1.json)

- Expected a relevant first hit for each positive query, both shared-phrase occurrences at their exact source bounds, the correct OCR frame ahead of the distractor, and zero hits for the negative. All gates passed. Across four positive queries, macro precision at 1 and 5 was 1.00 and 0.25, and recall was 0.875 and 1.00. Precision uses the full requested cutoff as its denominator. Maximum endpoint error was zero ticks. Asset A covered transcript, tag, OCR and caption. Asset B covered transcript, summary and OCR. Eight indexed documents held 25 tokens and 25 postings, totaling 420 bytes of term strings and posting IDs on x64, excluding container overhead. The first query took 2.88 ms including indexing and hashing. Five warm queries took 0.61 to 4.53 ms on this Windows host.
- `cargo test --locked --offline -p shotclutch-engine --lib -- media_search:: media_observations:: m1_media_tests relink_tests m1_transaction_tests` passed 44 routine tests. The offline speech test passed separately using the local runtime and WAV described under AI-001. Its reopen check searched two adjacent recognized words and matched their saved observation ID and exact word bounds. `cargo test --locked --offline -p shotclutch-format -p shotclutch-project` passed 54 format and 66 project tests. Workspace formatting and diff checks passed. PR CI now includes the deterministic retrieval corpus. [Engine tests](../crates/engine/src/media_search/tests.rs)
- Acceptance covers native lexical retrieval. The authored English corpus measures ranking and timing on small originals. Multilingual and large-project relevance and performance remain unqualified. Embedding retrieval awaits a compatible local producer. AI-003 owns progressive inspection and PROTO-001/002 own transport exposure. The M2 branch, closeout tag, tracked qualification files and both recorded artifact hashes matched the frozen checkpoint. [Checkpoint manifest](m2/closeout.json)

### AI-003 progressive visual inspection — 2026-11-25

- `Session::inspect_media` accepts a source fingerprint or a sequence revision. Source inspection returns metadata, deduplicated contact sheet tiles, exact or rounded frames, displayed-pixel crops, and timestamped clips with optional selected-stream PCM. Clip frames retain their original PTS and carry a separate half-open visibility range. [Native inspection API](../crates/engine/src/media_inspection.rs)
- Timeline inspection uses the committed project snapshot, the reference CPU compositor and an isolated audio mix with its own preroll. It reports the fingerprints of the originals read during the request. Preparation checks the reachable sequence graph, including nested inputs and media used by transitions or preroll, under the caller's confined roots. A missing or failed input returns a typed error. [Timeline inspection](../crates/engine/src/media_inspection/timeline.rs)
- Inspection limits are 64 MiB of original snapshots per request, 4096 × 2160 decoded pixels per frame, 16 contact sheet tiles, and five seconds, 150 frames and 64 MiB of combined returned picture and PCM per clip. The grant can impose a smaller byte budget. Qualification focuses on indexed MOV/ProRes with PCM and Matroska/MJPEG. Large-original streaming, numbered image sets, and broader codec and platform qualification remain open. AI-004 owns verification of staged edits.
- `cargo test --locked --offline -p shotclutch-engine --lib -- media_inspection:: media_search:: media_observations:: m1_media_tests relink_tests m1_transaction_tests` passed 47 routine tests on Windows x64. The expected search-to-crop workflow succeeded, VFR frames retained their presentation spans, and the nested timeline clip matched the reference renderer and mixer. The fixture also verified grant and revision errors, identical-byte relink, in-place replacement, and read-only session behavior. The existing live speech case remains opt-in. The M3 PR suite includes the inspection tests. [Acceptance tests](../crates/engine/src/media_inspection/tests.rs)
- `cargo test --locked --offline -p shotclutch-format -p shotclutch-project` passed 54 format and 66 project tests. Formatting and diff checks passed. The frozen M2 branch, annotated tag, tracked qualification files, and both recorded artifact hashes matched their checkpoint values. [Checkpoint manifest](m2/closeout.json)
- `cargo test --locked --offline -p shotclutch-render -p shotclutch-media` passed 300 routine tests. The codec run passed 134 library tests. One integration test initially stopped during MPEG fixture generation, then passed in isolation. The full codec integration rerun passed 63 routine tests with `cargo test --locked --offline -p shotclutch-codecs --test it -- --test-threads=1`.

### AI-004 staged composition verification — 2026-11-25

- Native hosts call `Session::preview_verified_edit` with required criteria, then `Session::verify_staged_edit` on the returned draft. The verifier samples the affected base and staged compositions for split, regular trim, and ripple trim. It records both coordinate sets, including removed tails, and samples affected boundaries, transition interiors, and applicable animation keyframes. Both renders use the same confined original snapshots and one resource budget. [Verifier](../crates/engine/src/edit_verification/checks.rs)
- The common commit path checks the composed outcome, draft and criteria digests, project revision, grant, and current source bindings. It writes successful verification reports into the durable operation receipt. Reports retain affected ranges, exact frame ticks and audio sample positions, output settings, measurements, and evidence hashes. The same receipt is available after recovery and on retries. [Transaction service](../crates/engine/src/service.rs)
- Deterministic checks cover sampled display pixels, region luma, aligned mixed PCM, and peak and RMS. The verifier renders at up to 640 pixels on the longest edge. It allows up to 64 frame samples, 64 audio windows, and 64 MiB of output across both snapshots. Semantic claims without a qualified verifier and required samples without usable evidence are inconclusive. Human override rules remain under AGENT-004.

- `cargo test --locked --offline -p shotclutch-engine --lib -- edit_verification_tests:: media_inspection:: media_search:: media_observations:: m1_transaction_tests m1_service_tests` passed 57 routine tests on Windows x64. The live speech case remains opt-in. Split preservation, trim mapping, and intentional silence passed. Unintended black, a dark overlay, and an audio-only gap blocked commit. Missing sources, sampling limits, and unsupported semantic claims produced inconclusive results. The suite also covered source replacement, stale revisions, grant ownership and expiry, cancellation, durable-write failure, receipt recovery, retry, and single undo. The junction regression passed with permission to create its fixture. [Acceptance tests](../crates/engine/src/edit_verification/tests.rs)
- `cargo test --locked --offline -p shotclutch-format -p shotclutch-project` passed 54 format and 66 project tests. Formatting and diff checks passed. The M3 PR workflow includes the verification suite. The frozen M2 branch, annotated tag, tracked qualification files, and both artifact hashes matched their checkpoint values. [Checkpoint manifest](m2/closeout.json)

### AGENT-004 editor authorization and review — 2026-11-25

Native hosts issue policy-bound clients with `Session::authorize_agent_editor`. The host supplies sequence ranges, edit families, command and changed-item limits, and the existing editor grant. `replace_agent_editor_policy` revokes the previous client and drops its drafts. Verified previews use the same source checks and staged compositor as AI-004. The transaction service evaluates the full occupied spans of changed clips, captions, and transitions before and after the edit. This includes linked changes, displaced material, and removed tails. [Policy types](../crates/engine/src/agent_policy.rs) · [Service implementation](../crates/engine/src/service/agent.rs)

| Mode | In-scope split, regular trim, or ripple trim | Impact outside the task ranges |
|---|---|---|
| Ask | Staging and verification use the read grant. Publication requires mutation authority and approval of the reviewed draft. | Approval must explicitly cover the additional affected ranges. |
| Assist | A reversible edit can auto-commit after passing verification within task scope. | Approval must explicitly cover the additional affected ranges. |
| Autonomous | A reversible edit can auto-commit after passing verification within its explicitly granted scope. | Approval must explicitly cover the additional affected ranges. |

The host fetches the review request with `review_agent_edit` or `transaction.inspect` and records the human's decision through `approve_agent_edit`. Each decision binds to the grant, revision, draft, report, sources, and expiry. Explicit semantic acceptance requires passing technical checks and relevant staged evidence. The receipt retains the inconclusive verdict, the acknowledged criteria, and the reviewer's identity. The common commit gate enforces these rules for native and dispatcher calls. The approval applies to one draft, including any approved range extension, and committed receipts survive recovery. AGENT-003 owns orchestration and PROTO-001/002 own new MCP and CLI approval interfaces.

- `cargo test --locked --offline -p shotclutch-engine --lib -- agent_policy_tests:: edit_verification_tests:: media_inspection:: media_search:: media_observations:: m1_transaction_tests m1_service_tests` passed 62 tests on Windows x64. The live speech case remains opt-in. The policy cases exercised all three edit families across all three modes, adjacent half-open bounds, ripple scope extension, review binding and invalidation, denied bypasses, stale inputs, and durable retry and undo. The filesystem confinement regression used permission to create its junction fixture. The M3 PR step includes the policy cases. [Acceptance tests](../crates/engine/src/service/agent_tests.rs)
- `cargo test --locked --offline -p shotclutch-format -p shotclutch-project` passed 54 format tests and 66 project tests. Recovery returned the approved edit's receipt with its original verification outcome and a single undo entry for the edit.
- `cargo test --locked --offline -p shotclutch-automation --lib -- m1_scoped_protocol` passed both scoped MCP regressions, including denied file, UI, and render access and durable replay.
- `cargo test --locked --offline -p shotclutch-cli --test it -- m1_transaction_scoped_cli m1_incomplete_scope_options` passed both scoped CLI regressions with permission for the child CLI to open its confined directory grant. The sandboxed run passed the argument rejection case and returned Windows access denied in the scoped receipt test.
- `cargo fmt --all -- --check` and `git diff --check` passed. The M2 branch and annotated tag matched their starting refs. `git diff --exit-code main -- docs/m2 scripts/m2 scripts/ci/m2-checks.py` passed, and both qualification artifact hashes matched the manifest. [M2 checkpoint](m2/closeout.json)

### AGENT-003 bounded phrase split — 2026-11-25

- Native hosts start and advance a run through `Session::start_agent_run` and `advance_agent_run`. A cloned run handle exposes ordered progress, a pollable result, and a cooperative cancellation signal. The offline planner uses current exact transcript evidence from originals referenced by the target sequence. It inspects a short source excerpt on the matched audio stream and maps the phrase onset through the source in-point and timeline start. It rounds the cut up to the next sequence frame boundary when the onset falls between frames. Ambiguous occurrences or placements return choices for a narrower request. [Run API](../crates/engine/src/service/runs.rs) · [Phrase planner](../crates/engine/src/service/runs/phrase.rs)
- Qualification covers one split across a direct, forward, normal-speed linked A/V placement. The runner checks the staged clip identities, exact cut, and source continuity before applying AI-004 picture and audio preservation. AGENT-004 gates publication in all three modes. The run alone can dispatch verification and commit for its draft, while the host can supply human approval through the existing review API. Successful publication returns the durable operation receipt with one undo entry.
- Hosts configure elapsed time, tool calls, source bytes, frame and excerpt limits, and transaction size. An interrupted call retains its durable budget reservations, and a retry charges the new work. Recovery checks the operation receipt and any pending commit journal before restaging. An unpublished run requires a fresh grant and a new evidence and verification pass. The checkpoint retains the typed request and summaries in the host's `AgentRuns` directory. SEC-004 owns user-facing retention controls.
- Schema v21 adds optional speaker attribution to word-timed transcript observations. Existing observations migrate with no speaker label. Search can constrain verified evidence by that attribution. The built-in offline planner covers the typed phrase-split task. PROTO-001/002 own transport exposure.
- `cargo test --locked --offline -p shotclutch-engine --lib -- service::runs::tests:: media_observations:: media_search:: media_inspection:: edit_verification_tests:: m1_service_tests m1_transaction_tests autosave_tests::` passed 87 routine tests on Windows x64. One opt-in speech test was skipped. The runner cases covered all three policies, source and timeline coordinates, ambiguous and wrong-source evidence, resource limits, cancellation at each phase, stale or missing originals, planner unavailability, manual edits, revocation, storage retry, and restart on both sides of publication. The confinement regression used permission to create its Windows junction fixture. The M3 CI step includes the runner tests. [Acceptance tests](../crates/engine/src/service/runs/tests.rs)
- `cargo test --locked --offline -p shotclutch-format -p shotclutch-project` passed 54 format tests and 66 project tests. `cargo fmt --all -- --check` and `git diff --check` passed. The frozen M2 branch and tag matched their starting refs. `git diff --exit-code main -- docs/m2 scripts/m2 scripts/ci/m2-checks.py` passed, and both qualification artifact hashes matched the manifest. [M2 checkpoint](m2/closeout.json)

### SEC-004 provider credentials and local retention — 2026-11-25

- **In Progress** on `codex/m3-semantic-media`. AssemblyAI and Gemini use opaque credentials from the trusted desktop host. Smart Cut offers use-once, remembered, and explicitly selected environment keys. Remembering or replacing a key uses Windows Credential Manager, macOS Keychain, or Linux Secret Service. Legacy raw command keys are rejected before logging. Native helper commands remove provider credential variables from their child environments. [Credential API](../crates/engine/src/credentials.rs)
- **Edit → Preferences → Privacy & Data** inventories analysis, annotations, run checkpoints, reviewed Smart Cut work, recovery, retained history, render previews, and narration assets. Previewing destruction binds the confirmation to the document revision and managed-file fingerprints. Confirmed analysis cleanup sanitizes current and undo/redo snapshots, removes approved copies, retains durable operation receipts, and replays an interrupted cleanup before recovery. The diagnostics preview lists the report fields before export. [Native controls](../crates/engine/src/privacy.rs) · [Desktop panel](../crates/ui-egui/src/privacy_panel.rs)
- Destructive project and analysis cleanup requires a saved native project with host recovery storage. Deletion removes managed local references and files under OS account protections. Separate projects, external exports and provider data remain outside this scope. Reviewed maps require explicit inclusion. Captions in edited sequence copies and generated files whose ownership cannot be verified are preserved. Unverified snapshot identities are surfaced in the inventory.
- `cargo test --locked --offline -p shotclutch-engine --lib -- credentials::tests:: privacy::tests:: review_provider::tests:: preedit::tests:: service::runs::tests:: media_observations:: media_search:: media_inspection:: edit_verification_tests:: agent_policy_tests:: m1_service_tests m1_transaction_tests autosave_tests:: settings_tests::media_cache_location_policy_and_clean m1_collection_tests` passed 136 Windows x64 tests with four opt-ins skipped. The loopback and junction fixtures used permission outside the sandbox. Coverage includes rejection of scoped and legacy raw-key attempts, saved and collected project inspection, export metadata, reviewed maps, authored data protection, undo/redo, autosaves, stale writer rejection, active runs, durable receipts, and restart of an interrupted cleanup.
- `cargo test --locked --offline -p shotclutch-speech --features download --lib assemblyai::` passed four tests on Windows x64, including successful and rejected requests against a local HTTP fixture. `cargo test --locked --offline -p shotclutch-format -p shotclutch-project` passed 54 format tests and 66 project tests.
- `cargo test --locked --offline -p shotclutch-ui-egui --lib privacy_panel::tests::` passed the desktop menu route and rendered the Privacy & Data panel in the egui test harness. The native service deletion tests exercise the apply, reopen, and recovery paths.
- `cargo check --locked --offline -p shotclutch --bin shotclutch` and the matching `cargo build` passed. The built Windows app imported an isolated synthetic WAV, completed Smart Cut analysis, saved the project, and opened Privacy & Data via the authenticated local control channel. The rendered panel showed one analysis, one map, and one undo state. `cargo test --locked --offline -p shotclutch-env --lib` passed four tests, including credential environment removal for native helpers.
- The SEC-004 vault CI matrix runs the same disposable-key restart fixture on Windows, macOS, and Linux. The Windows vault round trip passed on this host. Hosted macOS Keychain and Linux Secret Service results are required before closing the item. [Qualification workflow](../.github/workflows/ci.yml)
- `cargo test --locked --offline -p shotclutch-engine --lib credentials::tests::native_vault_round_trip -- --ignored --exact` created, replaced, read from a separate process, and forgot a disposable Windows entry. Locked and unavailable status mapping passed independent tests with synthetic backend errors. `cargo fmt --all -- --check` and `git diff --check` passed. The M2 branch and tag matched their starting refs, and the tracked M2 files and both checkpoint hashes matched the baseline. [M2 checkpoint](m2/closeout.json)

## Architecture decisions pending

These are implementation details or proposed baseline choices. They do not reopen the accepted product direction in [01](01_PRODUCT_VISION.md).

| ID | Decision | Baseline / alternatives to evaluate | Needed by | Owner / state |
|---|---|---|---|---|
| DEC-001 | Initial OS and qualification hardware | Windows x64 first; Windows 11 10.0.26200, Ryzen 7 4800H, 16 logical cores, approximately 32 GiB RAM; all adapters/drivers and actual memory in benchmark manifests | M0 | Codex / Profile captured; clean-machine and GPU qualification pending |
| DEC-002 | Canonical schema, tick frequency, storage/migration layout | Retain inherited timebase/COW model; schema v17 atomic JSON document/history snapshots selected in [DEC-002](m1/DEC-002_PROJECT_PERSISTENCE.md); exact mapping and broader durability qualification remain | M1 | Codex / Implemented storage choice; qualification In Progress |
| DEC-003 | MotionScene JS runtime and TS/JSX compiler | Compare deterministic evaluation, embedding, sandboxing, startup, memory, licensing, and debugging | M4 | Unassigned / Open |
| DEC-004 | Web runtime embedding/distribution | Chromium worker strategy, headless rendering, GPU sharing, isolation, readiness, platform packaging | M5 | Unassigned / Open |
| DEC-005 | Local ASR, embeddings, and visual model profiles | Pinned Tiny English faster-whisper CPU INT8 default for new Smart Cut sessions, larger English/legacy profiles and explicit optional AssemblyAI; [speech decision](m2/DEC-005_SPEECH_PROVIDERS.md) | M2 for ASR; M3 for others | Codex / Local ASR accepted under M2's bounded profile; live cloud unqualified; embeddings/visual profiles remain M3 |
| DEC-006 | Codec fallback distribution and capability matrix | Native OS decoder plus pure-Rust fallback selected; 27 bounded Windows SDR case/mode checks pass; [decision, measured profile and limits](m1/DEC-006_NATIVE_MEDIA.md) | M1 | Codex / Implemented bounded profile; broader qualification In Progress |
| DEC-007 | ACP/MCP revisions, transports, tested pairings | Pin official specifications and SDKs; define optional capability and authentication support | M3 for MCP; M4 for ACP | Unassigned / Open |
| DEC-008 | Destination versions and interchange routes | Qualify each destination separately; establish CapCut route feasibility before editable-export commitment | M4 planning; M5 qualification | Unassigned / Open |
| DEC-009 | Specialized workspace UI implementation | Native egui Smart Cut with shared monitor/audio engine, immediate automatic result and direct review; [workspace decision](m2/DEC-009_SMART_CUT_WORKSPACE.md) | M2 | Codex / Implemented and accepted in M2's review-assisted closeout |
| DEC-010 | Native scene subset, fonts, color/alpha contract | Qualify bounded SDR/text/2D subset; font redistribution and missing-font behavior; defer unsupported features explicitly | M4 | Unassigned / Open |

Decision records should include context, options, selected choice, rationale, consequences, affected requirement IDs, migration plan, evidence, reviewer, and date. Until settled, downstream APIs are baseline proposals rather than claims of an existing SDK.

## Blockers and prerequisites

The user-confirmed clean installation/core workflow resolves the original clean-install prerequisite. Additional lifecycle, offline/backend/audio/soak and second-profile coverage is planned separately. GitHub CLI access is working and hosted CI has run. The first aggregate run exposed Linux desktop identity and Windows legacy FXC failures; repairs passed a complete aggregate, but repeat acceptance is now blocked by CI-BILLING-001 and requires the locally repaired scheduling test to pass hosted qualification. Physical loopback and a second machine remain unavailable.

| ID | Prerequisite | Unblock condition | Affected work |
|---|---|---|---|
| READY-001 | Resolved: source inspected at inherited revision `3a5f217b` | Recorded in M0 foundation audit | M0 |
| READY-002 | Resolved: Windows x64 and actual machine profile captured | Hardware, driver, toolchain and corpus hashes retained in benchmark manifests | M0 baseline |
| CLEAN-001 | Resolved: user confirms clean 0.13.0 MSI installation and launch/import/edit/save/reopen/export | [Direct manual acceptance](m0/evidence/clean-install-user-confirmation.md); exact environment/hash metadata and lifecycle extensions tracked in RT-001 | CORE-001 |
| CI-ACCESS-001 | Resolved for ordinary hosted bootstrap: GitHub CLI has admin access; Actions enabled; private main preserved | First remote run and read-only settings evidence retained; billing budget remains unverified, with no paid infrastructure or schedules enabled | Remote CI activation |
| CI-BILLING-001 | Resolved for observed execution: both independent repaired-source runs passed their recovered install/export/uninstall jobs after owner budget/usage updates | Historical startup failures retained. CI-002 is Done; no billing setting was changed by Codex | CI-003 remains separately authorized promotion/settings work |
| READY-003 | External destination versions/test environments not selected | Resolve DEC-008 and obtain reproducible import environments | M5 adapter qualification |

A future blocker entry must identify observed failure, reproduction/evidence, impact, owner, next action, and explicit unblock condition. Close it with evidence rather than deleting its history.

## Risk register

| ID | Risk | Mitigation / trigger |
|---|---|---|
| RISK-001 | Inherited features less mature than assumed | M0 capability audit and real-media corpus; replan based on failing evidence |
| RISK-002 | CPU paths, transfers, or decode limits undermine complex timelines | Instrument render graph, qualify hardware paths, proxies and bounded caches, then optimize measured bottlenecks |
| RISK-003 | Rough-cut removals alter meaning | Conservative proposals, original-context review, manual overrides, restoration and semantic-preservation evaluation |
| RISK-004 | Agent makes confident but unsupported edits | Evidence-linked retrieval, composed verification, revision checks, explicit inconclusive outcomes |
| RISK-005 | Scene/web runtime weakens security or determinism | Isolated code, pinned bundles, quotas, randomized-seek tests; web path remains M5 |
| RISK-006 | Destination format drift or no maintainable editable route | Pinned adapters and real import tests; truthful rendered fallback without closing unmet editable requirements |
| RISK-007 | Scope expands before reliable editing works | Enforce M1 durability gates and bounded v0.1 feature set; record scope changes |
| RISK-008 | Missing historical context or proposal mistaken for decision | Source-limit note in 01; explicit Accepted/Baseline/Open labels; amend if earlier discussion is supplied |

## Recently completed

- 2026-10-26 — Authored the full 01–17 documentation baseline, including shared requirements, proposed contracts, milestone gates, and this tracker. No code implementation or measured performance is claimed.
- 2026-10-26 — Delivered M0 implementation and local evidence on `codex/m0-foundation`: 1,383 passing targeted tests, 3 explicitly ignored performance tests, successful native desktop workflow and unsigned Windows packages, and paired quick CPU baselines. CLEAN-001 remains open; M1–M5 remain Planned.
- 2026-10-26 — Corrected the MSI's missing visible setup/completion flow after the user report. Original installation was successful on the development PC; updated wizard package and launch gating pass artifact checks. Full click-through remains manual because the UI tool blocks `msiexec.exe`.
- 2026-10-26 — User confirmed clean MSI 0.13.0 installation, launch, import/edit/save/reopen and video export. Recorded CORE-001 and baseline-only PERF-001 acceptance; prepared remote CI/runtime rollout with concrete workflow gaps and deployment gates. No push, CI dispatch or settings change performed.

## Change log

| Date | Change | Affected documents | Basis |
|---|---|---|---|
| 2026-10-26 | Established ShotClutch as the only product identity and preserved standalone/local-first direction | 01–17 | Current user request and accessible referenced discussion |
| 2026-10-26 | Distinguished accepted direction from proposed technical contracts and unresolved choices | 01, 03–13, 17 | Avoid treating prior assistant suggestions as approved implementation |
| 2026-10-26 | Defined requirements, acceptance journeys, tests, and M0–M5 boundaries | 02, 14–17 | Implementable and traceable baseline |
| 2026-10-26 | Initialized all implementation requirements as Planned with no evidence | 17 | No repository/build/test evidence available |
| 2026-10-26 | Recorded M0 implementation, passing checks, raw paired baseline captures and outstanding clean-install gate | 17, m0/* | Executed source/build/test/package/workflow evidence; no remote CI or clean-install claim |

## Session handoff procedure

At session start, read this file, the relevant requirement, and its owning specification. Verify current checkout/revision before trusting previous results. At session end, record actual changes, checks and outcomes, remaining work, and the next concrete action. Update architecture documents only when their contracts change. Do not estimate progress from lines of code or checklist size; if percentages are later used, define the denominator and evidence weighting explicitly.

- 2026-10-26 — Started M0 on `codex/m0-foundation`; preserved the committed user baseline and recorded source/build/packaging/benchmark decisions. Qualification gates remain explicit.

- 2026-10-26 — Executed authorized CI bootstrap at 5f697163; retained first-run failures and passing package/macOS lanes, exact CI candidate 30-minute Full playback, GPU matrix and interleaved CPU rechecks. Follow-up fixes and repeat green runs are in progress; no release, PR, merge or settings change.

## Post-M0 identity cleanup

| Record | State | Owner | Evidence / next action |
|---|---|---|---|
| IDENTITY-001 | Done | Codex | Branch `codex/shotclutch-identity` from completed M0 `9114bb6e`; qualified implementation `a6229303`. All 14 local checks passed; final Windows/Linux/macOS checks passed 2,644 / 2,660 / 2,614 Rust cases, zero failed and 12 existing ignored each. Eight helper/identity guards, packaging lint, release builds, browser aliases, unsigned MSI/ZIP, hosted install/uninstall and exact packaged Windows desktop passed. Installed and desktop exports verified at 50 frames / 80,000 stereo sample frames. [Final CI](https://github.com/damquan1001/shotclutch/actions/runs/41308404270) and required aggregate passed. [Validation evidence](identity/EVIDENCE.md); [behavior/compatibility](SHOTCLUTCH_IDENTITY.md). Main promotion, signing and publishing untouched. |

The original M0 reports and fixtures remain unchanged. This work changes product identity and compatibility boundaries, not M1 project/transaction architecture. Retained copyright/asset provenance is not rewritten as ShotClutch authorship.

## M1 active work — 2026-11-02

Owner: Codex. Branch: `codex/m1-editing-foundation`. [Implementation, test scope, remaining gates and next actions](m1/IMPLEMENTATION.md). Ready-PR-only single-job hosted CI replaces routine matrix/package runs; full workflows remain manual. CORE-002, AGENT-001/002 and TEST-001 are In Progress for the initial structural staging and focused correctness gate. No M1 requirement is marked Done.

- 2026-11-02 — Continued M1 on `codex/m1-editing-foundation`: schema v17 document identity/history/revision and integrity, durable commit acknowledgement, recovery retry safety, migration protection, and non-rewinding allocation. CORE-004 joins the active items. [Decision and limits](m1/DEC-002_PROJECT_PERSISTENCE.md); [validation and remaining gates](m1/IMPLEMENTATION.md). No requirement is marked Done by this slice.


- 2026-11-02 — Continued the M1 structural service: host-only expiring sequence grants,
  exact versioned requests, original/proxy source revalidation through directory capabilities,
  durable actor-bound idempotency receipts, cancellation, and scoped CLI/MCP adapters. SEC-002
  joins the In Progress items. [Contract and limits](m1/SERVICE_CONTRACT.md);
  [local validation and remaining gates](m1/IMPLEMENTATION.md). General scoped commands,
  rendered verification, media/export/offline and portability qualification remain open.


- 2026-11-04 — Continued M1 native exports: job-local original readers with full/block SHA-256,
  retained queue identity/path context, frozen image membership, staged and synced outputs,
  cancellation-aware publication and ordinary file-set rollback/recovery backups. Exact fractional
  audio export boundaries are tested. [Contract/limits](m1/EXPORT_CONTRACT.md) and
  [local evidence](m1/IMPLEMENTATION.md). CORE-003 and EXPORT-001 are In Progress; the complete
  offline/media/physical A/V/portability gates remain open. No hosted workflow or settings change.

- 2026-11-04 — User scope adjustment: skip the physical second-PC check in M1. Retain local
  collection, integrity, relinking and relocation checks for EXPORT-006; physical cross-machine
  qualification is deferred beyond M1 and is not a current blocker. Existing M0 second-PC
  acceptance remains unchanged. No new second-machine pass is claimed.

- 2026-11-04 — Qualified the bounded native Windows SDR corpus in software, real hardware Auto
  and simulated-fallback modes (27 case/mode results), fixed source presentation clocks and
  half-open VFR seeks, and passed the real CLI/MCP mixed-rate/nested/retimed proxy and locally
  relocated collection journey. Save As/Copy now preserve relative media semantics through
  current and retained history; failed writes preserve live state. MEDIA-002–004 join the active
  items. [DEC-006 and limits](m1/DEC-006_NATIVE_MEDIA.md);
  [actual checks and remaining acceptance](m1/IMPLEMENTATION.md). No hosted Actions were used.

- 2026-11-05 — Final local candidate: 301 focused-gate tests, 826 engine/automation tests,
  133 codec and 50 platform regression functions, 21 CLI integration tests, strict 27-result
  native corpus, all-target Clippy and desktop build passed. Bounded desktop seeking/menu split/
  keyboard save/undo/redo was independently checked through native saves and CLI reopens.
  [Retained compact evidence](m1/evidence/README.md). UI-001 joins In Progress; complete manual,
  disconnected/physical runtime and remaining fault/service qualification are still open.

- 2026-11-05 — Added visible read-only native local-only processing status and `privacy.status`;
  opaque host speech backends remain explicitly unverified. SEC-001 joins In Progress. The focused
  gate now passes 302 cases; all 498 active egui integration tests pass (five existing ignored),
  settings checks, all-target engine/UI Clippy and the desktop build pass. Corrected two stale UI
  fixtures/waits without weakening production integrity checks. Prepared and tested the fail-closed
  same-PC offline wrapper and thirty-minute timing fixture; the connected diagnostic is not an
  offline pass. [Procedure and evidence limits](m1/OFFLINE_AND_AV_QUALIFICATION.md);
  [current results and boundary review](m1/IMPLEMENTATION.md). No hosted Actions or second-PC run.

- 2026-11-05 — Current desktop completed the 600-second real-device-clock audio observation:
  56,145 callbacks, zero underruns/missing frames, 119 samples. Video recorded 24 dropped pictures
  (0.134%), above the proposed <0.1% budget; no performance pass is claimed. Physical drift remains
  unverified. The connected mixed-media diagnostic also passed from clean commit `610a6cdd` with
  source unchanged. [Retained runtime evidence and limitations](m1/evidence/runtime-20261105/README.md).
  M1 remains In Progress for disconnected operation, full manual keyboard acceptance and physical
  thirty-minute A/V measurement. Physical second-PC qualification remains excluded.

- 2026-11-05 — User confirmed no external phone/camera is available for physical screen/speaker
  timing. The physical A/V portion of MEDIA-004 remains unverified pending capture equipment
  or an explicit scope change; no waiver or passing result is inferred. The completed ten-minute
  audio-counter observation stands. Disconnected/manual keyboard acceptance remains open and
  physical second-PC testing remains excluded. [Acceptance record](m1/IMPLEMENTATION.md).

- 2026-11-05 — User deferred physical display/speaker A/V sync verification beyond M1. The
  measurement remains unverified. The ten-minute audio-counter pass stands. Disconnected
  operation and full manual keyboard acceptance remain M1 exit checks.
  [Updated roadmap](16_MVP_ROADMAP.md).

- 2026-11-05 — Completed the native keyboard portion of UI-001 on this PC, including linked
  move, split, trim, save/history, panel focus and keyboard dialog acceptance/cancellation.
  The broader manual editing worksheet and disconnected mixed-media journey remain open.
  [Native results](m1/IMPLEMENTATION.md).

- 2026-11-05 — User deferred the disconnected mixed-media run beyond M1. The connected
  journey remains recorded as passed. Actual disconnected operation remains unverified.
  The broader manual editing walkthrough remains open. [Updated roadmap](16_MVP_ROADMAP.md).

- 2026-11-05 — Completed the local Windows desktop editing walkthrough. The editor verified
  mixed-rate nesting, linked trim and history, gain/fade, proxy switching, relink, original-bound
  range export, package relocation and working-copy reopen. Four independently decoded exports
  matched at 56 frames and 89,690 stereo sample frames. Native keyboard and visible gain-dialog
  checks complete the local worksheet. [Walkthrough results](m1/IMPLEMENTATION.md).

- 2026-11-05 — Closed M1 under the revised scope on `codex/m1-editing-foundation`. Candidate
  `0d35e24d` passed the single Ubuntu CI job in 8m 31s with 303 Rust tests, formatting and the
  gate helper test. Physical second-PC portability, physical A/V and disconnected verification
  remain deferred. [M1 closeout and evidence](m1/IMPLEMENTATION.md#m1-closeout--2026-11-05).
