# ShotClutch — Development Tracker

Baseline 0.1 · Last updated: 2026-10-26 · Authoritative living work record.

## Current state

The documentation baseline is committed at `b739515e04b3467ed8b681e6296f7dc5867f43dd`. M0 implementation is delivered on `codex/m0-foundation`, based on inherited source `3a5f217b348bf0c9dc1678f305c34b6c88617f37`. The initial working tree was clean. Production application revision: `3aeef47218c74067fb4998b0d14e4aacee7dad10`; subsequent commits add test, measurement and evidence refinements. See [the foundation audit](m0/FOUNDATION_AUDIT.md), [executed evidence](m0/EVIDENCE.md), [qualification procedure](m0/QUALIFICATION.md), and [upstream policy](m0/UPSTREAM_POLICY.md).

Current milestone: **M0 — Foundation audit and reproducible baseline, In Review; remote CI rollout in progress.** Initial qualification platform: Windows x64. Owner: Codex implementation session. Local builds, 1,383 targeted tests, desktop workflow, packages and paired CPU benchmark captures have evidence. The user confirms that the 0.13.0 MSI installs cleanly, launches, imports/edits, saves/reopens and exports video successfully. [Manual acceptance](m0/evidence/clean-install-user-confirmation.md) is recorded; hosted CI and native runtime observations have now run with partial passing coverage and explicit failures. See [execution results](m0/CI_RUNTIME_EXECUTION.md).

There are **48 implementation requirements: 44 in v0.1 (M0–M4) and 4 in M5**. CORE-001 and PERF-001 have their bounded M0 acceptance evidence; CORE-005 remains In Review pending automated-check rollout/review. No broader feature or performance-release qualification is implied. M1–M5 remain Planned. Staffing, budget and release dates are unassigned.

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
| M0 | Audit, standalone build, baseline measurements | In Review | Local evidence and user-confirmed clean/core workflow passed; [remote CI rollout](m0/CI_RUNTIME_EXECUTION.md) active; two green same-commit runs outstanding |
| M1 | Durable project, exact commands, native media, local editing | Not started | None |
| M2 | Smart Rough Cut and workspace | Not started | None |
| M3 | Semantic indexing, verified agent, MCP/CLI | Not started | None |
| M4 | ACP/Skills, native scenes, interchange foundation, v0.1 qualification | Not started | None |
| M5 | Web scenes and qualified destination adapters | Planned follow-on | None |

Full entry/exit criteria: [16_MVP_ROADMAP.md](16_MVP_ROADMAP.md).

## Requirement work register

Dependencies below are completion gates, not a requirement to postpone design or test preparation. TEST-002 is the aggregate release gate; “all v0.1 requirements” excludes TEST-002 itself. The common owner/evidence fields above apply until replaced by an active-work record.

| ID | Work item | Priority | Milestone | Status | Depends on |
|---|---|---|---|---|---|
| CORE-001 | Build a standalone ShotClutch fork | P0 | M0 | Done | — |
| CORE-002 | Use one canonical project with stable IDs and versioned extensions | P0 | M1 | Planned | CORE-001 |
| CORE-003 | Use integer ticks and rational rates for edit math | P0 | M1 | Planned | CORE-002 |
| CORE-004 | Persist changes crash-safely | P0 | M1 | Planned | CORE-002 |
| CORE-005 | Establish reproducible build, automated checks, release packaging plan, and upstream-change policy | P0 | M0 | In Review | CORE-001 |
| UI-001 | Provide usable multitrack editing, exact trim/split/move, linked A/V, nesting, monitor seeking, audio levels, save, relink, and undo | P0 | M1 | Planned | CORE-003, AGENT-001, MEDIA-003, MEDIA-004 |
| UI-002 | Provide a dedicated Smart Rough Cut workspace with transcript, waveform, source and result preview, take comparison, reasons, and apply/restore controls | P0 | M2 | Planned | PRE-002, PRE-003 |
| MEDIA-001 | Select a usable hardware, native, or fallback decoder through one source interface | P0 | M1 | Planned | CORE-001, CORE-003 |
| MEDIA-002 | Generate/use proxies while preserving source identity | P0 | M1 | Planned | MEDIA-001 |
| MEDIA-003 | Evaluate tracks, effects, transitions, and nested sequences through a shared render graph | P0 | M1 | Planned | MEDIA-001, CORE-003 |
| MEDIA-004 | Mix audio with sample-accurate ranges, linked edits, gain, fades, and device-clock playback | P0 | M1 | Planned | MEDIA-001, CORE-003 |
| MEDIA-005 | Keep interaction responsive under load using bounded caches, prioritized work, cancellation, and adaptive preview | P1 | M4 | Planned | MEDIA-002, MEDIA-003, MEDIA-004, PERF-001 |
| PRE-001 | Start Smart Rough Cut only by explicit invocation with selected sources and settings | P0 | M2 | Planned | AGENT-001, SEC-002 |
| PRE-002 | Persist versioned PreEditAnalysis and PreEditMap data with source mapping and confidence | P0 | M2 | Planned | CORE-002, CORE-003, CORE-004 |
| PRE-003 | Identify candidate pauses, fillers, restarts, and alternative takes while preserving editorial meaning | P0 | M2 | Planned | PRE-001, PRE-002, MEDIA-004 |
| PRE-004 | Apply an approved map through one transaction to an editable result | P0 | M2 | Planned | PRE-003, UI-002, AGENT-002 |
| PRE-005 | Restore rejected source material and revise a map without mutating originals or silently replacing already-edited timelines | P0 | M2 | Planned | PRE-004 |
| AI-001 | Store asset/scene/shot/segment/frame observations with provenance, coordinate domains, and confidence | P1 | M3 | Planned | CORE-002, MEDIA-001, SEC-001 |
| AI-002 | Search local transcripts, tags, OCR, captions, and embeddings where available | P1 | M3 | Planned | AI-001 |
| AI-003 | Support progressive visual inspection from contact sheets to bounded clips and precise frames | P1 | M3 | Planned | AI-001, MEDIA-003 |
| AI-004 | Verify agent changes against composed timeline output and audio where relevant | P1 | M3 | Planned | AI-003, AGENT-002 |
| AGENT-001 | Route human and automated mutations through the same typed command service | P0 | M1 | Planned | CORE-002, CORE-003, SEC-002 |
| AGENT-002 | Support revision-checked atomic transactions with preview, commit, rollback, cancellation, and grouped undo | P0 | M1 | Planned | AGENT-001, CORE-004 |
| AGENT-003 | Execute bounded understand/inspect/plan/edit/verify/report loops with progress and cancellation | P1 | M3 | Planned | AGENT-002, AI-002, AI-004, AGENT-004 |
| AGENT-004 | Enforce Ask, Assist, and Autonomous policies independently of client transport | P0 | M3 | Planned | SEC-002, AGENT-002 |
| AGENT-005 | Store transaction rationale and generated-asset provenance without secrets | P1 | M3 | Planned | CORE-002, AGENT-002, SEC-004 |
| PROTO-001 | Expose scoped, discoverable MCP tools/resources over a versioned adapter | P1 | M3 | Planned | AGENT-001, AGENT-002, SEC-002 |
| PROTO-002 | Provide scriptable CLI operations with machine-readable results, nonzero failures, revision controls, and job inspection | P1 | M3 | Planned | AGENT-001, AGENT-002, SEC-002 |
| PROTO-003 | Integrate ACP sessions, supported capability negotiation, streamed progress, cancellation, and permission responses | P1 | M4 | Planned | AGENT-003, PROTO-001 |
| PROTO-004 | Load versioned Skills as instructions/examples/templates with declared capabilities | P1 | M4 | Planned | AGENT-004, SEC-003 |
| MOTION-001 | Evaluate a programmatic scene as `render(t, context) -> SceneIR` independently of request order | P1 | M4 | Planned | CORE-003, SEC-003 |
| MOTION-002 | Render a declared native subset including groups, text, shapes, images, transforms, opacity, clipping, and keyframe/easing animation | P1 | M4 | Planned | MOTION-001, MEDIA-003 |
| MOTION-003 | Render explicit web scenes in an isolated runtime into timestamped RGBA surfaces | P2 | M5 | Planned | MOTION-002, MOTION-004, SEC-003 |
| MOTION-004 | Cache scenes by all observable inputs and preserve provenance | P1 | M4 | Planned | MOTION-002, MEDIA-005 |
| EXPORT-001 | Render a selected range from a pinned project revision with exact duration, audio, and declared output settings | P0 | M1 | Planned | MEDIA-002, MEDIA-003, MEDIA-004, SEC-002 |
| EXPORT-002 | Build a transient interchange representation and per-object fidelity report | P1 | M4 | Planned | CORE-003, AGENT-005, EXPORT-001, MOTION-002 |
| EXPORT-003 | Qualify a Premiere destination adapter using an appropriate interchange route and pinned target version | P2 | M5 | Planned | EXPORT-002 |
| EXPORT-004 | Qualify a DaVinci Resolve destination adapter with the same evidence standards; destination-specific mappings remain isolated from canonical project data | P2 | M5 | Planned | EXPORT-002 |
| EXPORT-005 | Qualify a CapCut adapter only after establishing a maintainable supported route | P2 | M5 | Planned | EXPORT-002 |
| EXPORT-006 | Package/relink a project on another machine | P0 | M1 | Planned | CORE-004, MEDIA-002 |
| SEC-001 | Complete the core offline workflow with no media or semantic data uploaded | P0 | M1 | Planned | CORE-001 |
| SEC-002 | Enforce least-privilege grants at service boundaries | P0 | M1 | Planned | CORE-002 |
| SEC-003 | Isolate untrusted scene code and extension execution | P0 | M4 | Planned | SEC-002 |
| SEC-004 | Keep provider credentials outside projects/logs and support retention/deletion controls | P0 | M3 | Planned | SEC-001, SEC-002 |
| PERF-001 | Establish reproducible performance fixtures and baseline captures with hardware, software, media, and cache conditions | P0 | M0 | Done | CORE-001 |
| PERF-002 | Meet or explicitly renegotiate the release budgets in document 14 on declared hardware | P1 | M4 | Planned | PERF-001, MEDIA-005, MOTION-004 |
| TEST-001 | Run invariant, command, persistence, and regression checks in CI | P0 | M1 | Planned | CORE-005, CORE-004, AGENT-002 |
| TEST-002 | Pass the v0.1 release workflow, security, recovery, and qualification gates | P0 | M4 | Planned | TEST-001, PERF-002; all v0.1 requirements |

## Active work and evidence register

M0 implementation and local evidence are ready on `codex/m0-foundation`. CORE-001 now combines executable local evidence with user-confirmed clean/core workflow acceptance; PERF-001 covers baseline captures only. CORE-005 retains its review gate while the hosted CI rollout is stabilized.

| Record | State | Owner | Evidence / next action |
|---|---|---|---|
| DOC-001 | Done | Documentation author | All 17 numbered Markdown documents authored; structural validation and archive packaging accompany this baseline |
| REVIEW-001 | Planned | Unassigned | Review baseline API/schema choices against the selected source revision; record accepted amendments |
| SETUP-001 | Done | Codex | Recorded inherited revision and clean documentation baseline; selected Windows x64 and pinned Rust 1.95.0 |
| CORE-001 | Done | Codex / user acceptance | Production `3aeef472`, installer correction `b5102d3c`; standalone builds, identity/compatibility and capability/legal inventory recorded. Native edit/export has independent frame/sample checks. User confirms clean MSI install and launch/import/edit/save/reopen/export: [acceptance](m0/evidence/clean-install-user-confirmation.md). Next: broaden runtime coverage without changing this bounded M0 conclusion. |
| CORE-005 | In Review | Codex | Locked builds, Rust 1.95, pinned repository-owned CI, unsigned MSI/ZIP and upstream policy verified locally. [Check logs](m0/evidence/README.md). Bootstrap, strict-oracle/test-selection and diagnostic gaps were addressed at 5f697163. First hosted run passed macOS checks and Windows package lifecycle; Linux identity and Windows FXC failures are under repair. Next: two green same-commit runs and review. [Execution](m0/CI_RUNTIME_EXECUTION.md). |
| PERF-001 | Done | Codex | Harness `f35fac68`; seven matching software/CPU sections, identical generated fixture hashes, available hardware/software/cache metadata and raw samples retained. [Comparison](m0/BENCHMARKS.md), [upstream](m0/evidence/upstream/manifest.json), [ShotClutch](m0/evidence/shotclutch/manifest.json). Acceptance is baseline capture, with slow cases and unavailable telemetry disclosed; GPU/second-profile work and release budgets remain separate. |
| MSI-UX-001 | Done | Codex / user acceptance | Fix `b5102d3c` adds a wizard and Finish launch checkbox; artifact validation passes. The user confirms successful clean installation and launched core workflow. [Diagnosis](m0/evidence/msi-wizard/diagnosis.json), [manual acceptance](m0/evidence/clean-install-user-confirmation.md). Finish-checkbox and uninstall paths get separate lifecycle coverage in RT-001. |
| CI-001 | Done | Codex | GitHub CLI/admin access and read-only settings preflight recorded; strict test/oracle/logging helpers deployed at 5f697163. Existing hosted runners only; billing budget not independently verified. [Execution](m0/CI_RUNTIME_EXECUTION.md). |
| CI-002 | In Progress | Codex | Initial packaging lint passed; aggregate CI failed on Linux metadata and Windows FXC. macOS 2,599 passed / 12 ignored; Windows install/render/uninstall passed. Repairs locally validated. Next: two green same-commit runs with all platform release builds. |
| CI-003 | Planned | Repository owner / Codex | Unique aggregate names prepared. Main promotion/required checks/schedules follow reviewed green runs and explicit merge/settings authorization; main remains unchanged. |
| RT-001 | In Progress | Codex / manual tester | CI 5f697163 MSI installed, edited/exported exact 50 frames / 80,000 stereo sample frames, uninstalled and preserved user sentinel on a fresh hosted runner. User Q0 remains accepted. Offline consumer VM and extended interactive lifecycle remain outstanding. |
| RT-002 | In Progress | Codex | Exact CI candidate completed 30-minute NVIDIA Vulkan/Media Foundation Full playback: 99 drops (0.1834%), no audio underruns/missing frames, peak working set 0.887 GiB; exceeds proposed drop target. Seven-section GPU matrix and three interleaved CPU decode pairs retained. Physical A/V drift unavailable. [Raw evidence and limitations](m0/CI_RUNTIME_EXECUTION.md). |
| RT-003 | Planned | Manual Windows tester | A separate physical Windows hardware profile remains unavailable; integrated-GPU library tests on the same laptop do not satisfy this lane. |

For each new active requirement, add owner, change/branch or commit, test/benchmark report, expected versus actual result, review outcome, next action and any blocker ID. Do not substitute prose status updates for reproducible evidence.

## Architecture decisions pending

These are implementation details or proposed baseline choices. They do not reopen the accepted product direction in [01](01_PRODUCT_VISION.md).

| ID | Decision | Baseline / alternatives to evaluate | Needed by | Owner / state |
|---|---|---|---|---|
| DEC-001 | Initial OS and qualification hardware | Windows x64 first; Windows 11 10.0.26200, Ryzen 7 4800H, 16 logical cores, approximately 32 GiB RAM; all adapters/drivers and actual memory in benchmark manifests | M0 | Codex / Profile captured; clean-machine and GPU qualification pending |
| DEC-002 | Canonical schema, tick frequency, storage/migration layout | Extend inherited model; audit integer/rational representation and transaction durability before selecting physical layout | M1 | Unassigned / Open |
| DEC-003 | MotionScene JS runtime and TS/JSX compiler | Compare deterministic evaluation, embedding, sandboxing, startup, memory, licensing, and debugging | M4 | Unassigned / Open |
| DEC-004 | Web runtime embedding/distribution | Chromium worker strategy, headless rendering, GPU sharing, isolation, readiness, platform packaging | M5 | Unassigned / Open |
| DEC-005 | Local ASR, embeddings, and visual model profiles | Evaluate quality/language, hardware cost, licenses, installation size, offline operation; no mandatory cloud fallback | M2 for ASR; M3 for others | Unassigned / Open |
| DEC-006 | Codec fallback distribution and capability matrix | Qualified hardware/native/fallback paths; decide packaging and license obligations from actual dependencies | M1 | Unassigned / Open |
| DEC-007 | ACP/MCP revisions, transports, tested pairings | Pin official specifications and SDKs; define optional capability and authentication support | M3 for MCP; M4 for ACP | Unassigned / Open |
| DEC-008 | Destination versions and interchange routes | Qualify each destination separately; establish CapCut route feasibility before editable-export commitment | M4 planning; M5 qualification | Unassigned / Open |
| DEC-009 | Specialized workspace UI implementation | Retain native shell baseline; compare native panel versus bounded embedded surface only if necessary | M2 | Unassigned / Open |
| DEC-010 | Native scene subset, fonts, color/alpha contract | Qualify bounded SDR/text/2D subset; font redistribution and missing-font behavior; defer unsupported features explicitly | M4 | Unassigned / Open |

Decision records should include context, options, selected choice, rationale, consequences, affected requirement IDs, migration plan, evidence, reviewer, and date. Until settled, downstream APIs are baseline proposals rather than claims of an existing SDK.

## Blockers and prerequisites

The user-confirmed clean installation/core workflow resolves the original clean-install prerequisite. Additional lifecycle, offline/backend/audio/soak and second-profile coverage is planned separately. GitHub CLI access is working and hosted CI has run. The first aggregate run exposed Linux desktop identity and Windows legacy FXC failures; repairs and repeat runs are active. Physical loopback and a second machine remain unavailable.

| ID | Prerequisite | Unblock condition | Affected work |
|---|---|---|---|
| READY-001 | Resolved: source inspected at inherited revision `3a5f217b` | Recorded in M0 foundation audit | M0 |
| READY-002 | Resolved: Windows x64 and actual machine profile captured | Hardware, driver, toolchain and corpus hashes retained in benchmark manifests | M0 baseline |
| CLEAN-001 | Resolved: user confirms clean 0.13.0 MSI installation and launch/import/edit/save/reopen/export | [Direct manual acceptance](m0/evidence/clean-install-user-confirmation.md); exact environment/hash metadata and lifecycle extensions tracked in RT-001 | CORE-001 |
| CI-ACCESS-001 | Resolved for ordinary hosted bootstrap: GitHub CLI has admin access; Actions enabled; private main preserved | First remote run and read-only settings evidence retained; billing budget remains unverified, with no paid infrastructure or schedules enabled | Remote CI activation |
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
