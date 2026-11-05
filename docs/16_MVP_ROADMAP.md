# ShotClutch — MVP Roadmap

Baseline 0.1 · 2026-10-26 · Planned sequence, not a schedule or progress report. Live state: [17_DEVELOPMENT_TRACKER.md](17_DEVELOPMENT_TRACKER.md).

## Release definition

The v0.1 milestone is a useful standalone editor that supports a complete local workflow: import and edit real media, manually review and apply Smart Rough Cut, find and inspect footage semantically, accept reversible agent edits, use the shared CLI/MCP/ACP/Skills surfaces, author basic deterministic native MotionScenes, and render final output. Reliable save/recovery, exact time, permissions, and measured performance are release requirements.

The architecture includes web MotionScenes and destination adapters from the start, while their qualified implementations follow in M5. v0.1 includes the interchange planning/fidelity foundation. It does not promise editable exports to every destination before those adapters pass actual import tests.

No dates, staffing estimates, completion percentages, or already-working inherited capabilities are assumed. A repository audit and baseline measurements establish realistic planning inputs.

## Dependency order

```text
M0 Foundation audit and reproducible baseline
  -> M1 Canonical project, transactions, reliable media and local editing
     -> M2 Smart Rough Cut and dedicated review workspace
        -> M3 Semantic understanding and verified agent editing; MCP/CLI
           -> M4 ACP/Skills, native MotionScene, interchange foundation,
                 performance qualification and v0.1 release
              -> M5 Web fallback and qualified destination adapters
```

This is dependency order, not a prohibition on independent work. For example, protocol schema design or scene-runtime experiments may begin earlier, but cannot be marked release-ready before their project, permission, and verification dependencies exist.

## M0 — Foundation audit and reproducible baseline

Scope: CORE-001, CORE-005, PERF-001. Obtain and record the fork revision, preserve legal notices, establish ShotClutch identity/package naming, reproduce builds, inventory capabilities, select initial platform/hardware, document upstream sync policy, and capture benchmark baselines.

Exit: a clean machine launches the standalone application; audit identifies verified/partial/absent/unknown features; build and smoke checks are reproducible; baseline corpus/report exists; core architectural assumptions have been checked against code. If the inherited project/time/command contracts differ from expectations, record a concrete migration decision before M1.

Risk: treating earlier comparison claims as verified implementation. Mitigation: inspect code and run fixtures before planning inherited features as completed.

## M1 — Reliable editing foundation

Scope: CORE-002–CORE-004, UI-001, MEDIA-001–MEDIA-004, AGENT-001–AGENT-002, EXPORT-001, EXPORT-006, SEC-001–SEC-002, TEST-001.

Extend canonical schemas, exact time, stable IDs, durable persistence, source fingerprints, command validation, revision-safe transactions, preview/undo, decoder fallback, proxies, native render/audio paths, local permissions, basic rendered export, and project portability. Preserve usable manual editing while adding headless service access.

Exit: the local mixed-media editing journey passes on a declared platform; save/recovery and grouped undo survive fault injection; proxy/original swaps preserve cuts; a command produces equivalent behavior across service clients; final render has correct frame/sample counts and source bindings.

Scope adjustment (user, 2026-11-04): qualify project collection/relink through local relocation
and integrity checks. A physical second-PC run is deferred beyond M1 and does not block its exit.

Scope adjustment (user, 2026-11-05): defer external display/speaker A/V sync measurement beyond
M1. Retain local playback and audio underrun checks.

Scope adjustment (user, 2026-11-05): defer the network-disconnected mixed-media run beyond M1.
The connected local journey and visible local-only processing status remain M1 evidence.

Risk: codec edge cases, GPU/CPU mismatches, and persistence changes. Mitigation: bounded media qualification and failure fixtures before broader AI work depends on them.

## M2 — Smart Rough Cut

Scope: UI-002, PRE-001–PRE-005. Deliver manual initiation, VAD/ASR integration, reviewable removal candidates, TakeGroups, persistent PreEditAnalysis/PreEditMap revisions, workspace comparison, map preview, editable sequence application, restoration, and safe reapplication rules.

Exit: a user reviews an interview, keeps an intentional pause, chooses a take, restores a phrase, applies the result, saves/reopens, and undoes the group. Source mapping and captions/audio remain correct. Reanalysis cannot erase manual decisions or overwrite an edited timeline.

Risk: shortening footage at the expense of meaning. Mitigation: conservative proposals, visible uncertainty, original-context playback, human evaluation, and restoration metrics.

## M3 — Semantic media and agent workflow

Scope: AI-001–AI-004, AGENT-003–AGENT-005, PROTO-001–PROTO-002, SEC-004. Build hierarchical observations, local indexes, progressive source/timeline inspection, stale-data handling, bounded agent loops, autonomy policies, provenance, hardened MCP/CLI adapters, and secret/retention controls.

Exit: the agent locates a requested moment with evidence, prepares an edit, verifies the composed result, commits within scope, reports what it checked, and offers one-step undo. Concurrent manual edits trigger conflict. Unavailable providers, injection attempts, cancellation, and budgets produce safe outcomes.

Risk: fluent but ungrounded edits and hidden provider disclosure. Mitigation: source-linked evidence, composed verification, local-only testing, and service-enforced grants.

## M4 — Native motion and v0.1 qualification

Scope: MEDIA-005, PROTO-003–PROTO-004, MOTION-001–MOTION-002, MOTION-004, EXPORT-002, SEC-003, PERF-002, TEST-002. Integrate ACP and Skills, choose and isolate the scene runtime, implement the native scene subset/caching, build interchange planning/fidelity reports, harden adaptive preview, and qualify the full release workflow.

Exit: native scenes pass randomized seek and cache-invalidation tests; untrusted code cannot escape its grants; declared ACP pairing and Skills pass contract tests; export plans account for unsupported constructs; benchmarks meet agreed budgets or documented scope revisions; all v0.1 requirements have evidence and no release-blocking defects remain.

Risk: allowing the graphics system or protocol matrix to expand indefinitely. Mitigation: a bounded native feature set, pinned protocol/platform support, explicit unsupported cases, and later extension points.

## M5 — Hybrid graphics and destination delivery

Scope: MOTION-003, EXPORT-003–EXPORT-005. Add isolated web scene rendering, deterministic readiness/seek behavior, resource quotas, caching, alpha/color integration, and qualified Premiere, DaVinci Resolve, and CapCut destination routes where maintainable.

Exit is per capability, not all-or-nothing: each web/runtime or adapter feature has reproducible fixtures and a declared support matrix. An unavailable editable route remains unfulfilled even if a rendered fallback exists. Keep its blocker and fallback visible in the tracker.

## Later investment tracks

After the baseline proves useful, prioritize advanced object/action tracking, native VLM acceleration, larger-project scaling, deeper GPU coverage, richer MotionScene nodes, broader codec/platform support, multicam hardening, richer audio/color tools, and a carefully scoped extension API according to measured user need.

Do not add real-time collaboration, cloud project hosting, a mobile editor, marketplace operations, hosted model infrastructure, enterprise DAM, full grading/compositing parity, or proprietary-project round trips to v0.1 by implication. A new release commitment needs requirements, dependencies, tests, and tracker entries.

## Change and release management

Update this roadmap when release scope or milestone dependencies change. Update the tracker whenever work status, evidence, owner, risk, or a decision changes. The roadmap answers what comes next; the tracker records what is actually known now. A requirement moves to a later milestone through an explicit documented scope change, never because its failing check was omitted from the release run.
