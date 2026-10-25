# ShotClutch — Project Data Model

Baseline 0.1 · 2026-10-26 · Logical schema; storage engine, file extension, and tick frequency await the foundation audit.

## Canonical ownership

There is one editable project model. Existing compatible core objects are retained and extended. New metadata must refer to canonical stable IDs, not array positions or UI labels. IDs are opaque, persistent, unique within their namespace, and never reused after deletion. Copying an object creates a new ID plus an optional `derivedFrom` reference; relinking an asset preserves its ID only through an explicit validated operation.

```text
Project
  schemaVersion, projectId, revision, timebase, settings
  Assets, Sequences, Tracks, Clips, Effects, Transitions, Markers, Captions
  PreEditAnalyses, PreEditMaps, TakeGroups
  MotionSceneAssets
  SemanticMetadata references
  AgentRuns, TransactionRecords, GeneratedAssetProvenance
  InterchangeMappings, ExtensionData
```

## Core entities

| Entity | Essential fields and invariants |
|---|---|
| Project | ID, schema version, monotonic revision, timebase policy, active sequence, settings, extension manifest |
| Asset | ID, media kind, content fingerprint, locators, stream metadata, duration, source timebase, original/proxy bindings, availability |
| Sequence | ID, rational frame rate, dimensions, working color space, audio sample rate, ordered tracks, duration policy |
| Track | ID, sequence ID, media type, order, mute/solo/lock state, effect references; changes respect lock policy |
| Clip | ID, track ID, source reference, timeline range, source range/time map, linked group, transforms, effects, enabled state |
| Effect/transition | ID, type/version, parameters, rational-time keyframes, supported backend requirements, input references |
| Caption/marker | ID, exact range or point, text, language/type, source reference where applicable |
| Media locator | Relative project path or explicitly granted external URI/path; distinct from content identity |

A clip source is a tagged union: media asset, nested sequence, generated graphic, or MotionScene. Source variants must be explicit. Nested sequence references form a directed acyclic graph; cycles are invalid. Source range validity and timeline overlap rules are validated according to track/transition semantics.

## Exact time contract

Edit math uses signed integer ticks with an explicit positive tick rate and rational rates `numerator/denominator`. Serialized large integers use decimal strings so JavaScript clients do not lose precision. Ranges are half-open `[start, end)`. Empty or negative-duration ranges are rejected unless an operation explicitly permits a point event. Decimal seconds may be used for display and bounded numerical animation, never as canonical trim positions.

The inherited tick frequency is adopted only after verifying supported frame and audio rates. If a requested rate cannot be represented exactly, use rational conversion at the boundary with a documented rounding rule and preserved source timestamp identity; never accumulate rounded frame durations. Do not invent a universal tick constant in this document.

The engine distinguishes:

- Source presentation timestamps and decoded frame identity, especially for VFR media.
- Source audio sample indices and channel layout.
- Clip-local time after trims and speed mapping.
- Sequence time and parent-sequence time through nesting.
- Scene-local time and render sample time for MotionScene.

Every API range identifies its coordinate domain and referenced object. Conversion uses explicit functions with round-down, round-up, nearest, or exact-required policies. Video cut boundaries are snapped to permitted source/sequence frame boundaries; audio edits retain sample precision. Half-open ranges avoid duplicated boundary frames. Timecode is a display convention, not a storage representation.

## Smart Rough Cut entities

| Entity | Stored information |
|---|---|
| PreEditAnalysis | ID/revision, source IDs and fingerprints, analysis configuration, model/tool versions, transcript alignment, detected candidates, confidence, completion/failure state |
| TakeGroup | ID, semantic/editorial unit, alternative source ranges, ordering, quality observations, ranking explanation, selected alternative, manual override |
| PreEditMap | ID/revision, analysis reference, selected source ranges, rejected candidates with reasons, take choices, user overrides, cumulative output mapping, boundary/fade policy, status |
| Map application | ID, map ID/revision, source fingerprints, target sequence/revision, created/affected clip IDs, transaction ID, output mapping digest |

A map is immutable by revision: editing a decision creates a new revision. The current pointer changes atomically. Old revisions referenced by timelines or history remain resolvable. User overrides are authoritative until explicitly changed; reanalysis creates a new proposal rather than erasing them.

Each kept segment has a stable ID, source asset/stream, exact source interval, logical output interval, optional take-group reference, decision provenance, and transition policy. In v0.1 kept intervals play forward at unit speed and concatenate in the approved order. Cross-source assembly and retiming must be explicitly supported before use, not inferred from a transcript. Removed source ranges remain accessible through analysis/decision records. Overlap in output is forbidden for the basic map; transitions are modeled separately so their timing and audio behavior are not hidden inside the mapping.

### Minimal illustrative serialization

This example uses a fixture-only 48,000-tick rate; it does not prescribe the project's production timebase. It represents keeping source seconds 0–2 and 4–5 from a five-second source.

```json
{
  "id": "map_demo",
  "revision": "2",
  "analysisId": "analysis_demo",
  "sourceAssetId": "asset_demo",
  "sourceFingerprint": "fixture-content-v1",
  "timebase": { "ticksPerSecond": "48000" },
  "status": "reviewed",
  "segments": [
    { "id": "seg_a", "source": ["0", "96000"], "output": ["0", "96000"] },
    { "id": "seg_b", "source": ["192000", "240000"], "output": ["96000", "144000"] }
  ],
  "decisions": [
    { "source": ["96000", "192000"], "action": "remove", "reason": "reviewed_pause", "origin": "user" }
  ]
}
```

The output duration is 144,000 ticks. Output tick 120,000 maps to source tick 216,000. Source tick 150,000 has no retained output location. Inverse queries return zero, one, or several output ranges because future repeated source use can create a one-to-many mapping.

## MotionScene and semantic records

MotionSceneAsset stores an ID, revision, explicit backend kind, source-code or declarative-definition reference, language/API version, entry point, duration, dimensions, frame rate, working color policy, parameter schema/values, random seed, dependency lock, fonts, media references, capability requirements, and provenance. A timeline clip refers to a specific scene revision. Rendered caches never replace the authored source.

Semantic metadata follows Asset → Scene → Shot → Segment → FrameObservation. Boundaries may overlap where interpretation demands it; parents and coordinate domains are explicit. Observations contain source fingerprint, range/frame identity, producer and model versions, evidence references, confidence semantics, creation time, and freshness. Human annotations persist independently from regenerable model outputs. Full schema and indexing policy are in [08](08_SEMANTIC_MEDIA_AND_VISUAL_AI.md).

## Agent and generated-asset provenance

AgentRun stores the user intent or a privacy-preserving reference, current autonomy mode, granted scope references, tool/transaction IDs, budgets, verification summaries, and terminal state. Transaction records refer to exact project revisions. Sensitive prompts are retained only according to project settings and are omitted from diagnostics by default.

Generated assets record generator/tool and version, parameters, seed where meaningful, authorized input IDs, creation time, source prompt subject to retention policy, licensing/usage metadata supplied by the provider, and regeneration instructions. Reproduction is best effort for nondeterministic external generation and must not be advertised as exact. Provider secrets are never stored here.

## Persistence and portability

Illustrative package layout; final filename conventions await the storage decision:

```text
ProjectFolder/
  project/        canonical data, schema manifest, recovery journal
  motion/         authored scene sources and dependency locks
  semantic/       durable annotations, analysis/map records, index manifests
  skills/         project instruction packages, untrusted on import
  generated/      authored/generated outputs and provenance
  media/          optional collected originals
  proxies/        regenerable proxy media
  cache/          disposable frames, embeddings, render results, waveforms
```

Physical storage may combine these categories, but their lifecycle must remain distinct. Transcripts and model observations can be rebuilt in principle; user corrections, reviewed maps, generated source assets, and model/version records needed to explain an edit must travel with a portable project. Embeddings and decoded/render caches are disposable. Packages enumerate missing external originals and fonts rather than implying self-containment.

Save uses an atomic snapshot/journal boundary, checksum validation, and recoverable previous state. A migration writes a new version only after validation and preserves a recoverable original. Unknown optional extensions round-trip unchanged; unknown required extensions open read-only or fail with an explicit capability error. Migrations never execute project-supplied code.

## Validation and garbage collection

Validate referential integrity, acyclic nesting, time domains, positive denominators, finite effect parameters, content fingerprints, map coverage, scene capabilities, and revision references. Relinking to changed content marks dependent maps and observations stale pending review. Renaming or moving the same content need not invalidate analysis.

Garbage collection follows reachability from current project state, retained history, saved maps, and export manifests. Cache eviction can be automatic; deletion of authored or provenance-bearing resources requires the project retention policy. Undo must not depend on a cache that may disappear.

Traceability: CORE-002 through CORE-004, PRE-002, PRE-005, AI-001, AGENT-005, MOTION-004, EXPORT-006.
