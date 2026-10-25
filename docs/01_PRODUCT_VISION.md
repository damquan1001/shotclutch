# ShotClutch — Product Vision

Specification baseline: 0.1 · Prepared: 2026-10-26 · Status: development specification, not an implementation report.

## Product definition

ShotClutch is a standalone, local-first, professional nonlinear video editor with an agent that can understand footage, make reversible edits, inspect the resulting picture and sound, and explain its choices. Conventional editing, semantic media search, Smart Rough Cut, and programmatic motion graphics share one project and one editor command system.

The starting point is a fork of an existing native NLE codebase. ShotClutch owns its product architecture and independently implements missing capabilities. Installation, editing, rendering, AI workflows, and motion graphics must not require another application suite. Ordinary libraries, codecs, optional model providers, and export destinations are dependencies or integrations, not alternate product identities.

ShotClutch is the only product and brand name used throughout this set. External editing applications appear only as interchange destinations. Original copyright notices, license obligations, and dependency attribution must be preserved in the implementation's legal notices; branding does not authorize their removal.

## Intended users and outcomes

| User | Work to accomplish | Successful outcome |
|---|---|---|
| Independent editor | Cut interviews, explainers, and social videos | Reach an editable first cut faster without surrendering editorial control |
| Documentary or long-form editor | Find moments across substantial source footage | Retrieve evidence-linked source ranges and assemble a reliable multitrack timeline |
| Creator recording repeated takes | Remove mistakes, pauses, fillers, and redundant attempts | Review a nondestructive rough-cut proposal and restore any rejected material |
| Motion designer or technical creator | Author animated titles, diagrams, and overlays | Seek and render deterministic scenes alongside ordinary footage |
| Agent-assisted production team | Automate repeatable editorial tasks | Reuse skills and tools with scoped permissions, visible changes, and grouped undo |

The professional foundation is an engineering requirement, not a claim of current feature parity with established editors. Complex work means mixed-frame-rate media, exact trims, linked audio, many tracks, nesting, proxies, effects, captions, and long timelines. Inherited feature claims must be audited against an actual source revision.

## Product principles

1. **The project is authoritative.** Humans, agents, the CLI, and protocol clients use the same validated commands and transaction history.
2. **Media stays local by default.** Core editing, saving, rendering, and available local analysis work without uploading footage. Cloud reasoning is optional and explicitly permissioned.
3. **Automation remains inspectable.** Every agent edit exposes its intent, affected objects, supporting evidence, verification result, and undo boundary.
4. **Analysis does not silently edit.** Indexing produces metadata. Smart Rough Cut starts through an explicit user action and creates a reviewable proposal.
5. **Time and source identity survive transformations.** A result can be traced through rough cuts, retiming, and nesting to its source frame or audio sample.
6. **The native media engine owns footage.** Web rendering is a bounded graphics fallback; ordinary timeline video and audio remain in the native pipeline.
7. **Interchange reports losses.** Unsupported constructs are translated, baked, or reported explicitly. Export must not silently discard creative work.

## Core experience

The user imports media into a normal NLE project. They can immediately organize, trim, arrange, mix, and export it. Optional background indexing builds transcripts, shot metadata, and searchable visual observations. Search results lead back to source evidence rather than merely returning model-written summaries.

For speech-led material, the user selects sources and invokes Smart Rough Cut. A dedicated workspace shows the transcript, waveform, candidate removals, take groups, and before/after preview. Decisions persist in a PreEditMap. Applying a reviewed map produces an editable timeline result without altering source files.

The agent receives a bounded brief, inspects relevant sources and timeline state, proposes or executes permitted commands, and verifies the composed result. MotionScene allows the user or agent to author deterministic graphics that integrate as timeline sources. Users can continue manually at any point.

## Decision ledger

“Accepted” below means explicitly preserved by the present user request or the retrieved user decisions. “Baseline” is a specification choice made here to turn that direction into implementable contracts; it is not a claim of prior approval or completed code.

| Decision | State | Consequence |
|---|---|---|
| Standalone app built from a fork; no external suite dependency | Accepted | Audit and adapt the foundation; implement gaps within ShotClutch |
| Professional NLE with agentic visual understanding and semantic indexing | Accepted | Build on a native media/timeline engine and evidence-linked metadata |
| Manually invoked Smart Rough Cut, persistent PreEditMap, dedicated workspace | Accepted | Analysis and application are separate, reversible operations |
| Deterministic programmatic MotionScene with native and web paths | Accepted | Define scene contracts before adding a web rendering worker |
| ACP, MCP, CLI, and Skills | Accepted | Shared service boundary with separate protocol adapters |
| Interchange/adapter export to Premiere, DaVinci Resolve, and CapCut | Accepted | Capability-tested exports with fidelity reports |
| Local-first operation and living development tracker | Accepted | Local core workflow; evidence-based status updates |
| Extend the inherited canonical project rather than add a second timeline model | Baseline | New entities attach to the existing model; audit determines exact storage mapping |
| Retain the inherited native UI initially | Baseline | Specialized workspaces are additive; a frontend rewrite is not an MVP prerequisite |
| Native MotionScene subset before web fallback | Baseline | v0.1 can ship a useful bounded graphics system |
| Ask / Assist / Autonomous modes with action-based grants | Baseline | Autonomy never replaces permission checks |

Runtime selection, database layout, first supported operating systems, codec distribution, protocol revisions, specific model choices, and target export versions remain open in [the tracker](17_DEVELOPMENT_TRACKER.md).

## Scope and boundaries

The first release should demonstrate a reliable local editing loop, reviewed Smart Rough Cut, semantic retrieval, a visually verifying agent, shared automation interfaces, and basic native MotionScene. Quality and durability gates determine release readiness. A large feature inventory alone does not.

Advanced tracking, broader GPU coverage, arbitrary web graphics, and destination-specific project exports are follow-on work. The architecture reserves these paths from the beginning. Real-time collaboration, cloud project hosting, a mobile editor, an extension marketplace, hosted generative models, enterprise asset management, full grading/compositing parity, and native proprietary-project round trips are outside v0.1.

## Success measures

Measure time from import to a user-accepted rough cut, fraction of suggested removals retained or restored, search precision and temporal localization, agent transaction acceptance and rollback rates, and editing performance on a declared machine and corpus. Record baseline manual workflows before claiming improvement. No adoption, accuracy, speed, or completion statistics are asserted by this documentation.

## Documentation map

| File | Owns |
|---|---|
| [01_PRODUCT_VISION.md](01_PRODUCT_VISION.md) | Product direction, accepted decisions, terminology, source limitations |
| [02_PRODUCT_REQUIREMENTS.md](02_PRODUCT_REQUIREMENTS.md) | Requirement IDs, release scope, observable acceptance |
| [03_SYSTEM_ARCHITECTURE.md](03_SYSTEM_ARCHITECTURE.md) | Components, boundaries, ownership, processing flows |
| [04_PROJECT_DATA_MODEL.md](04_PROJECT_DATA_MODEL.md) | Canonical entities, time, persistence, migrations |
| [05_EDITOR_COMMAND_AND_TRANSACTION_MODEL.md](05_EDITOR_COMMAND_AND_TRANSACTION_MODEL.md) | Commands, concurrency, preview, commit, undo |
| [06_MEDIA_AND_RENDER_ENGINE.md](06_MEDIA_AND_RENDER_ENGINE.md) | Decode, audio, render graph, proxies, caches |
| [07_MOTIONSCENE_SPEC.md](07_MOTIONSCENE_SPEC.md) | Authoring, deterministic evaluation, native/web rendering |
| [08_SEMANTIC_MEDIA_AND_VISUAL_AI.md](08_SEMANTIC_MEDIA_AND_VISUAL_AI.md) | Analysis hierarchy, search, inspection, visual evidence |
| [09_SMART_ROUGH_CUT.md](09_SMART_ROUGH_CUT.md) | Manual workflow, take selection, persistent source mapping |
| [10_AGENT_ARCHITECTURE.md](10_AGENT_ARCHITECTURE.md) | Agent lifecycle, evidence, autonomy, verification |
| [11_ACP_MCP_CLI_SKILLS.md](11_ACP_MCP_CLI_SKILLS.md) | Automation interfaces and skill contracts |
| [12_INTERCHANGE_AND_EXPORT.md](12_INTERCHANGE_AND_EXPORT.md) | Export planning, adapters, fidelity and portability |
| [13_SECURITY_PRIVACY_AND_PERMISSIONS.md](13_SECURITY_PRIVACY_AND_PERMISSIONS.md) | Trust boundaries, grants, data flows, isolation |
| [14_PERFORMANCE_AND_BENCHMARKS.md](14_PERFORMANCE_AND_BENCHMARKS.md) | Proposed budgets, corpus, measurement protocol |
| [15_TESTING_STRATEGY.md](15_TESTING_STRATEGY.md) | Test levels, fixtures, correctness and release gates |
| [16_MVP_ROADMAP.md](16_MVP_ROADMAP.md) | Milestone order, dependencies, exit criteria |
| [17_DEVELOPMENT_TRACKER.md](17_DEVELOPMENT_TRACKER.md) | Live work state, evidence, blockers, decisions, change log |

## Terminology and source basis

An **asset** is a source or generated resource. A **clip** is a timeline instance referring to a source. A **sequence** is an editable multitrack timeline. **SceneIR** is a transient graphics representation, not a replacement timeline. **PreEditMap** stores a reviewed mapping from logical rough-cut output back to source ranges. **TakeGroup** groups alternative performances of an editorial unit. **Semantic evidence** is an observation with source coordinates and provenance. A **transaction** is one atomic project change and one undo unit. A **skill** is instructional content; a **plugin** is executable extension code.

The source is the user's current request and the accessible history of “Compare Video Editors,” conversation ID `6adcd8a8-cee4-83ec-a0f1-5e5361113a76`. The retrieval returned five recent turns, no older-page cursor, and one assistant message truncated at 20,000 characters. Earlier inaccessible details cannot be certified as captured. This set covers the requested architecture and available discussion; it does not imply a complete archival transcript was available.

Prior assistant comparisons, feature counts, performance percentages, and sample tracker entries are background proposals, not verified facts about the codebase. No source repository, build, benchmark, or working ShotClutch implementation was supplied or inspected for this documentation task. Technical contracts newly specified here are baseline proposals until implementation review. “MUST” denotes a requirement of this specification, not existing behavior. External protocol and format compatibility must be checked against pinned official specifications during implementation.
