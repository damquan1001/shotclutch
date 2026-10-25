# ShotClutch — Testing Strategy

Baseline 0.1 · 2026-10-26 · TEST-001 and TEST-002; acceptance evidence for all requirements in [02](02_PRODUCT_REQUIREMENTS.md).

## Verification principles

Tests protect the single project model, exact time, reversible commands, source identity, render correctness, and permission boundaries. Prefer independent invariants and observable outcomes over tests that restate implementation details. Inherited behavior must be qualified before being marked complete. A successful build or tool response is not proof of correct editing, rendering, or destination import.

Each requirement maps to a test family below and a tracker row. Evidence records tested revision, environment, fixture version, command or procedure, result, and artifact locations. This documentation task creates the test plan; it does not claim that implementation tests have run.

## Test layers

| Layer | Focus | Examples |
|---|---|---|
| Unit | Pure domain logic and validation | Rational time conversion, half-open ranges, effect parameter bounds |
| Property/invariant | Broad input spaces | Map reconstruction, no accumulated drift, undo restores semantic state |
| Component | One subsystem with real dependencies | Decoder fallback, proxy mapping, schema migration, scene isolation |
| Integration | Cross-boundary behavior | UI/CLI/MCP equivalence, transaction plus render preview, index invalidation |
| End-to-end | User-visible workflows | Offline edit/export, rough-cut review, agent verified edit, portable project |
| Qualification | Platform and external combinations | GPU/codec corpus, pinned ACP pairing, real destination import |
| Performance/soak | Latency and resource behavior | Long playback, random seeks, large projects, concurrent indexing |
| Security/fault injection | Adversarial and interrupted work | Malformed files, path escape, stale grant, crash at commit |

## Requirement-to-test family mapping

| Requirements | Test family / minimum evidence |
|---|---|
| CORE-001, CORE-005 | Clean build/install and standalone launch; dependency and legal-notice audit |
| CORE-002, CORE-004, EXPORT-006 | Save/reopen, migrations, unknown extensions, crash recovery, package/relink on second machine |
| CORE-003 | Exact-time property tests and mixed-rate/VFR/retiming/nesting fixtures |
| UI-001, MEDIA-001–MEDIA-004, EXPORT-001 | Manual/automated edit workflow, media corpus, audio sync, preview/export equivalence |
| UI-002, PRE-001–PRE-005 | Trigger policy, workspace review, map arithmetic, take overrides, application/reapplication, undo |
| AI-001–AI-004 | Provenance/freshness, retrieval corpus, inspection coordinates, staged-composition verification |
| AGENT-001, AGENT-002 | Command parity, revision conflict, idempotency, atomicity, undo and recovery |
| AGENT-003–AGENT-005 | Agent state machine, bounded retries, cancellation, mode enforcement, provenance |
| PROTO-001–PROTO-004 | Protocol/CLI schema contracts, pinned conformance, reconnect, noninteractive permissions, skill isolation |
| MOTION-001, MOTION-002, MOTION-004 | Random-order evaluation, native goldens, dependency invalidation, missing-font behavior |
| MOTION-003 | Web isolation, deterministic seeking, readiness, alpha/color, worker failure |
| EXPORT-002–EXPORT-005 | Fidelity coverage, package integrity, target-version import and render comparison |
| SEC-001–SEC-004 | No-egress checks, scoped grants, path and code isolation, secret/retention controls |
| MEDIA-005, PERF-001, PERF-002 | Reproducible benchmark and bounded-resource evidence from document 14 |
| TEST-001, TEST-002 | CI/release gate execution and complete requirement evidence review |

## Critical invariant suites

### Time and project model

Generate valid rational rates, tick ranges, clip offsets, speed maps, and nested transforms. Verify conversions preserve identity where exact representation is possible and otherwise obey the declared boundary rounding. VFR lookup must use presentation timestamps. Check one-frame clips, empty invalid ranges, maximum serialized integers, sequence ends, negative offsets where supported, and audio sample boundaries.

Serialization round-trips preserve semantic state, stable IDs, optional unknown data, and required-extension refusal. Migrations preserve a recoverable original. Nested cycles, dangling references, and non-finite effect parameters fail validation.

### Transactions and history

Apply a group, undo it, and compare canonical semantic state to the initial snapshot while allowing revision/history metadata to advance. Redo reproduces the expected state. Inject failure before/after every durable commit phase. Duplicate idempotent requests cannot duplicate edits. Change the project during preview and assert conflict. Revoke permission before commit and assert denial.

### PreEditMap

Generate retained/removed intervals and check output duration equals the sum of retained durations for the basic map. Forward mappings never enter removed ranges. Inverse mappings return all and only retained placements. Test all-kept, all-removed, repeated source, reordered takes, boundary padding, speech at asset endpoints, and partial analysis. Human overrides survive compatible reanalysis; incompatible content changes become stale.

### Rendering and audio

Use known-pattern sources to verify frame order, transform/crop, alpha edges, transitions, nested evaluation, and color conversion. Compare preview and export at matching settings. Measure exact output frame/sample count and long-form A/V synchronization. Inject late frames and confirm preview degrades while final export remains complete.

## Visual and audio goldens

Store small versioned reference images/audio or hashes where deterministic, with declared tolerance and environment. Use per-pixel/color metrics and perceptual checks appropriate to the feature; report spatial error maps when a visual comparison fails. Cross-platform GPU output may need bounded tolerances rather than exact hashes. Changes to goldens require a human-readable explanation and independent review, not automatic acceptance of new output.

Check text shaping, missing fonts, right-to-left samples, transparent edges, color metadata, effect boundaries, and scene animation extremes. For audio, compare sample counts, alignment markers, peak/RMS or loudness metrics as appropriate, and listen to representative joins for clicks or truncated words. Numerical checks alone cannot certify editorial naturalness.

## AI and editorial evaluation

Use consented labeled footage with clean and difficult speech, intentional pauses, fillers, repeated takes, ambiguous actions, OCR distractors, and hard-negative search queries. Version labels and retain disagreement notes. Keep evaluation material separate from prompt/preset tuning where feasible.

Measure retrieval precision/recall@k, temporal localization error, analysis coverage, removal precision, restoration rate, take-choice agreement, and time to an accepted cut. Human review checks semantic preservation and performance quality. Report per-language/corpus limitations. Stochastic model tests use pinned prompts/settings and repeated trials; they cannot replace deterministic safety/transaction tests.

Agent adversarial cases include wrong source, stale index, misleading transcript instructions, an invisible subject after cropping, failed visual verification, budget exhaustion, canceled generation, and provider disconnect. The safe outcome is often an explicit uncertain result, not an automatic edit.

## Protocol and export qualification

Maintain recorded fixtures for internal command schemas and pinned external protocols. Test malformed arguments, unsupported capabilities, large payload limits, expired handles, reconnect, cancellation, and permission mediation. Compare project effects across clients, not only response shapes.

Export fixtures combine exact, approximate, sampled, and baked constructs. Verify every source construct has a report entry or explicit exact classification. Open the export in the declared target version, relink on a clean machine, compare timing/audio/rendered samples, and retain screenshots/manifests where authorized. A flat render does not satisfy an editable-adapter requirement.

## CI and release gates

Every relevant change runs fast domain/schema/command tests and targeted subsystem tests. Scheduled or release-candidate jobs run the media corpus, platform GPU/codec matrix, fault injection, long-form soak, protocol pairings, and performance benchmarks. External application qualification may require a documented manual procedure when automation is unavailable.

v0.1 release requires: clean standalone install; offline editing and final render; crash-safe save/recovery; reviewed rough cut and grouped undo; evidence-linked semantic/agent workflow; CLI/MCP/ACP/Skills qualification for the declared feature set; deterministic native MotionScene; no known critical data-loss, unauthorized-egress, or arbitrary-execution defect; measured performance on declared hardware; and documented unsupported combinations.

M5 additionally requires isolated web-scene qualification and real target-adapter imports. Do not block v0.1 on features explicitly scheduled for M5, and do not advertise them as completed. Waivers identify risk, owner, affected users, evidence, mitigation, and expiry. A waiver is not a passing test.

## Defects and evidence handling

A defect records minimal reproduction, requirement IDs, build/environment, expected/actual result, severity, source-safe fixture, and regression test where justified. Redact private footage, transcripts, keys, and absolute paths from shared evidence. Preserve artifacts needed to reproduce failures under an explicit retention policy. Tracker Done requires both implementation and appropriate evidence; reopened failures return the requirement to active work.
