# ShotClutch — Product Requirements

Baseline 0.1 · 2026-10-26 · Normative planned behavior; implementation status lives in [17](17_DEVELOPMENT_TRACKER.md).

## Requirement policy

IDs remain stable after creation. Each ID has one authoritative definition in this document, one tracker row, and test evidence before it becomes Done. P0 means required for correctness, safety, or the core workflow; P1 means required for the planned v0.1 experience; P2 means follow-on scope. Priorities do not override dependency ordering. Milestones M0–M4 compose v0.1; M5 is post-v0.1. The roadmap may split tasks without silently weakening acceptance criteria.

Requirements describe the desired ShotClutch behavior. An inherited implementation can satisfy a requirement only after the same acceptance checks as new code. Performance thresholds in [14](14_PERFORMANCE_AND_BENCHMARKS.md) are proposed qualification targets, not measured results.

## Foundation and editing

| ID | Priority / milestone | Required behavior and acceptance evidence |
|---|---|---|
| CORE-001 | P0 / M0 | Build a standalone ShotClutch fork. A clean-machine installation launches and edits without another suite; dependency, license, build, and inherited-capability audits are recorded. |
| CORE-002 | P0 / M1 | Use one canonical project with stable IDs and versioned extensions. Save/reopen preserves clips, map revisions, references, and unknown optional extension data. |
| CORE-003 | P0 / M1 | Use integer ticks and rational rates for edit math. Mixed-rate, VFR, nested, and retimed fixtures preserve declared source-frame/sample mappings without accumulated floating-point drift. |
| CORE-004 | P0 / M1 | Persist changes crash-safely. Fault injection at each commit/save boundary recovers either the previous or complete new revision, never a partial project. |
| CORE-005 | P0 / M0 | Establish reproducible build, automated checks, release packaging plan, and upstream-change policy. Record a baseline revision and clean build evidence for the initial platform. |
| UI-001 | P0 / M1 | Provide usable multitrack editing, exact trim/split/move, linked A/V, nesting, monitor seeking, audio levels, save, relink, and undo. Complete the reference manual editing workflow and keyboard-accessibility checks. |
| UI-002 | P0 / M2 | Provide a dedicated Smart Rough Cut workspace with transcript, waveform, source and result preview, take comparison, reasons, and apply/restore controls. A user completes review without agent or CLI access. |

## Media, playback, and rendering

| ID | Priority / milestone | Required behavior and acceptance evidence |
|---|---|---|
| MEDIA-001 | P0 / M1 | Select a usable hardware, native, or fallback decoder through one source interface. Supported and malformed corpus files produce correct timestamped output or an explicit recoverable error. |
| MEDIA-002 | P0 / M1 | Generate/use proxies while preserving source identity. Switching proxy/original leaves cuts unchanged; final export uses originals by default and reports missing originals. |
| MEDIA-003 | P0 / M1 | Evaluate tracks, effects, transitions, and nested sequences through a shared render graph. Preview and export agree on composition, timing, color policy, and alpha for supported features. |
| MEDIA-004 | P0 / M1 | Mix audio with sample-accurate ranges, linked edits, gain, fades, and device-clock playback. Long-form fixtures meet the A/V drift and underrun qualification criteria. |
| MEDIA-005 | P1 / M4 | Keep interaction responsive under load using bounded caches, prioritized work, cancellation, and adaptive preview. Exact paused/export requests do not reuse an inadequate draft frame. |

## Smart Rough Cut

| ID | Priority / milestone | Required behavior and acceptance evidence |
|---|---|---|
| PRE-001 | P0 / M2 | Start Smart Rough Cut only by explicit invocation with selected sources and settings. Import and background indexing cannot silently generate or apply an editorial cut. |
| PRE-002 | P0 / M2 | Persist versioned PreEditAnalysis and PreEditMap data with source mapping and confidence. Save/reopen and interrupted analysis preserve user decisions and valid completed results. |
| PRE-003 | P0 / M2 | Identify candidate pauses, fillers, restarts, and alternative takes while preserving editorial meaning. Users can compare, override, exclude, and restore suggestions; low-confidence or ambiguous cases remain reviewable. |
| PRE-004 | P0 / M2 | Apply an approved map through one transaction to an editable result. A repeated request is idempotent; source timestamps, selected takes, audio links, and resulting duration are verified. |
| PRE-005 | P0 / M2 | Restore rejected source material and revise a map without mutating originals or silently replacing already-edited timelines. Map updates offer explicit regeneration or revision-aware reapplication and grouped undo. |

## Semantic media and visual AI

| ID | Priority / milestone | Required behavior and acceptance evidence |
|---|---|---|
| AI-001 | P1 / M3 | Store asset/scene/shot/segment/frame observations with provenance, coordinate domains, and confidence. Model changes and source replacement correctly mark dependent results stale. |
| AI-002 | P1 / M3 | Search local transcripts, tags, OCR, captions, and embeddings where available. Results return stable asset IDs, source ranges, retrieval scores, evidence, and freshness; benchmark relevance is measured. |
| AI-003 | P1 / M3 | Support progressive visual inspection from contact sheets to bounded clips and precise frames. Tools distinguish source images from composed timeline renders and honor privacy and resource budgets. |
| AI-004 | P1 / M3 | Verify agent changes against composed timeline output and audio where relevant. Missing evidence or inconclusive models yield an explicit unverified result rather than a pass. |

## Agent and automation

| ID | Priority / milestone | Required behavior and acceptance evidence |
|---|---|---|
| AGENT-001 | P0 / M1 | Route human and automated mutations through the same typed command service. Equivalent UI, CLI, and protocol requests produce equivalent project changes and validation errors. |
| AGENT-002 | P0 / M1 | Support revision-checked atomic transactions with preview, commit, rollback, cancellation, and grouped undo. A stale preview cannot overwrite a newer user edit. |
| AGENT-003 | P1 / M3 | Execute bounded understand/inspect/plan/edit/verify/report loops with progress and cancellation. Failed verification or exhausted budgets stop safely with a visible partial-work summary. |
| AGENT-004 | P0 / M3 | Enforce Ask, Assist, and Autonomous policies independently of client transport. A mode change cannot bypass file, network, code-execution, or export permissions. |
| AGENT-005 | P1 / M3 | Store transaction rationale and generated-asset provenance without secrets. Users can trace outputs to input assets, generator/runtime versions, and the responsible operation. |
| PROTO-001 | P1 / M3 | Expose scoped, discoverable MCP tools/resources over a versioned adapter. A contract suite checks schemas, capabilities, permissions, errors, jobs, and session isolation. |
| PROTO-002 | P1 / M3 | Provide scriptable CLI operations with machine-readable results, nonzero failures, revision controls, and job inspection. Automation never needs to parse presentation text. |
| PROTO-003 | P1 / M4 | Integrate ACP sessions, supported capability negotiation, streamed progress, cancellation, and permission responses. Test against a pinned protocol revision and reference client/agent pairing. |
| PROTO-004 | P1 / M4 | Load versioned Skills as instructions/examples/templates with declared capabilities. A skill cannot grant itself access, install executable code, or outrank user policy. |

## MotionScene

| ID | Priority / milestone | Required behavior and acceptance evidence |
|---|---|---|
| MOTION-001 | P1 / M4 | Evaluate a programmatic scene as `render(t, context) -> SceneIR` independently of request order. Random-access, repeated, and parallel evaluations produce equivalent output in a pinned environment. |
| MOTION-002 | P1 / M4 | Render a declared native subset including groups, text, shapes, images, transforms, opacity, clipping, and keyframe/easing animation. Unsupported nodes return capability errors. |
| MOTION-003 | P2 / M5 | Render explicit web scenes in an isolated runtime into timestamped RGBA surfaces. Arbitrary seek, timeout, alpha, font readiness, resource restrictions, and worker crash recovery pass qualification. |
| MOTION-004 | P1 / M4 | Cache scenes by all observable inputs and preserve provenance. Asset, font, runtime, seed, resolution, color, or code changes invalidate affected entries. |

## Export and portability

| ID | Priority / milestone | Required behavior and acceptance evidence |
|---|---|---|
| EXPORT-001 | P0 / M1 | Render a selected range from a pinned project revision with exact duration, audio, and declared output settings. Failure/cancel never presents a partial output as completed. |
| EXPORT-002 | P1 / M4 | Build a transient interchange representation and per-object fidelity report. Every source construct is translated, approximated, sampled, baked, composited, or explicitly blocked. |
| EXPORT-003 | P2 / M5 | Qualify a Premiere destination adapter using an appropriate interchange route and pinned target version. Imported timing, relinks, audio, and fidelity report match the qualification fixtures. |
| EXPORT-004 | P2 / M5 | Qualify a DaVinci Resolve destination adapter with the same evidence standards; destination-specific mappings remain isolated from canonical project data. |
| EXPORT-005 | P2 / M5 | Qualify a CapCut adapter only after establishing a maintainable supported route. Otherwise expose rendered/stem delivery with explicit loss of editability and leave editable-adapter acceptance unmet. |
| EXPORT-006 | P0 / M1 | Package/relink a project on another machine. Essential project data and generated sources are distinguished from rebuildable caches; missing resources are reported, never guessed. |

## Security, performance, and quality

| ID | Priority / milestone | Required behavior and acceptance evidence |
|---|---|---|
| SEC-001 | P0 / M1 | Complete the core offline workflow with no media or semantic data uploaded. Cloud requests require provider-specific, payload-scoped authorization and a visible local-only setting. |
| SEC-002 | P0 / M1 | Enforce least-privilege grants at service boundaries. Test path traversal, stale approval, cross-project access, and command invocation through every adapter. |
| SEC-003 | P0 / M4 | Isolate untrusted scene code and extension execution. Denied file/network/process access, runaway computation, and worker failure cannot compromise project durability. |
| SEC-004 | P0 / M3 | Keep provider credentials outside projects/logs and support retention/deletion controls. Export, project packaging, skill access, and diagnostics cannot leak secrets. |
| PERF-001 | P0 / M0 | Establish reproducible performance fixtures and baseline captures with hardware, software, media, and cache conditions. No performance claim is accepted without a comparable run. |
| PERF-002 | P1 / M4 | Meet or explicitly renegotiate the release budgets in document 14 on declared hardware. Record regressions, degraded-quality modes, and memory ceilings. |
| TEST-001 | P0 / M1 | Run invariant, command, persistence, and regression checks in CI. Property tests cover exact-time mapping, map reconstruction, and undo/redo. |
| TEST-002 | P0 / M4 | Pass the v0.1 release workflow, security, recovery, and qualification gates. Keep unsupported combinations visible and attach reproducible evidence to each completed requirement. |

## Product acceptance journeys

### Local editing and recovery

Install, disable network, create a project, import mixed-rate media, create a nested multitrack sequence, trim linked audio, generate a proxy, save, reopen, and render. Interrupt a save and recover. Confirm source media is unmodified, undo works, and export settings are explicit. Covers CORE-001 through CORE-005, UI-001, MEDIA-001 through MEDIA-004, EXPORT-001, EXPORT-006, SEC-001, and TEST-001.

### Reviewed speech rough cut

Select an interview, explicitly invoke analysis, compare two repeated takes, preserve an intentional pause, restore a removed phrase, and apply the approved map. Save/reopen the workspace, make a manual timeline change, then revise the map. The app must detect the changed target before reapplication. Covers UI-002 and PRE-001 through PRE-005.

### Evidence-based agent edit

Ask for a B-roll moment, inspect evidence, propose a range, preview the actual composition, and commit within the current grants. Introduce a concurrent human edit and repeat the request. The second transaction must fail stale-state validation or generate a new reviewed preview. One undo restores the first edit's prior project state. Covers AI-001 through AI-004, AGENT-001 through AGENT-005, and SEC-002.

### Deterministic graphics and delivery

Create a native MotionScene, seek out of order, render at final resolution, reopen on a second machine with the same pinned dependencies, and compare results within declared tolerances. Generate an interchange plan containing a deliberately unsupported effect and inspect its fallback report. v0.1 demonstrates the contract and native render; M5 adds qualified destination imports and isolated web scenes.

## Scope-change control

Changes to acceptance, milestone, or priority require an explanation in the tracker and review of affected contracts and tests. Deferred requirements remain listed; an unavailable external adapter must not be relabeled complete because a flat render succeeded. Blocking dependencies should be tracked as such rather than hidden in percentage-complete estimates.
