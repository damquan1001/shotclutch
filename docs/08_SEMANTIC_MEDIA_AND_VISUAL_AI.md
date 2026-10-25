# ShotClutch — Semantic Media and Visual AI

Baseline 0.1 · 2026-10-26 · AI-001 through AI-004; advanced tracking follows v0.1.

## Purpose and separation from editing

Semantic understanding makes footage discoverable and gives agents evidence for editorial decisions. It is a derived interpretation of source media, not a replacement for the media or the timeline. Indexing may run in the background according to user settings, but it cannot apply cuts, choose takes permanently, or rewrite the project without a command.

All observations retain source coordinates and provenance. A natural-language summary is not sufficient evidence for an exact cut. A visual claim about the finished edit must be checked against a composed timeline render, not only against the source frame.

## Hierarchy and observation contract

| Level | Purpose | Typical stored data |
|---|---|---|
| Asset | Whole source identity | Stream metadata, transcript references, broad tags, source fingerprint |
| Scene | Coherent semantic situation | Topic/location/time grouping, range, uncertain boundaries |
| Shot | Visually continuous camera view | Cut boundaries, representative frames, camera/motion observations |
| Segment | Searchable event or speech unit | Transcript words, action/object descriptions, OCR, ranges, embeddings |
| Frame observation | Precise evidence at a presentation timestamp | Frame identity, bounded regions, labels, OCR, model confidence, evidence reference |

Scenes are semantic groupings and may not correspond one-to-one with detected cuts. A segment may span multiple shots; the data model supports explicit relationships instead of forcing incorrect containment. Overlap is allowed and must be visible in the schema. Frame observations always refer to actual decoded presentation frames.

An observation includes ID/type, source asset/stream, source fingerprint, coordinate domain, range or frame identity, optional normalized region coordinates with orientation, producer/model/configuration versions, timestamp, confidence definition, evidence references, and freshness. Human annotations carry user provenance and are not overwritten by machine reindexing. Model confidence is not treated as a calibrated probability unless evaluation supports that interpretation.

## Progressive ingestion and cost control

1. Probe media and create stable assets.
2. Extract inexpensive local metadata, waveform, VAD, and shot-boundary candidates.
3. Run local ASR when installed/available; retain word timing and uncertainty.
4. Generate representative frames/contact sheets and bounded visual captions/OCR.
5. Compute local embeddings where supported and build lexical/vector indexes.
6. Perform deeper inspection only for requested ranges or high-value unresolved queries.

Jobs are incremental, checkpointed, cancelable, and lower priority than interactive editing. Settings control eligible media, frame sampling, model/provider, storage quota, compute budget, and cloud permissions. Missing models produce a clear unavailable state; the editor remains usable offline. No cloud fallback occurs silently.

Persist transcripts, reviewed annotations, shot/segment boundaries, reusable captions, model manifests, and evidence needed to explain an accepted edit. Rebuildable embeddings and contact-sheet thumbnails can live in disposable caches. Avoid storing a rich model response for every frame by default. Apply retention and quota policies to dense observations and temporary crops.

## Index and freshness model

Index keys include source fingerprint, analyzer version/configuration, preprocessing parameters, language, and embedding model identity. New model versions create separate index generations; vectors from incompatible embedding spaces are never mixed. Activate a replacement generation atomically after validation. Partial indexes advertise coverage and do not pretend to search unanalyzed footage.

Renaming or moving identical content updates locators without changing observations. Replacing media content invalidates dependent observations, maps, crops, and embeddings. Sequence edits invalidate composition-level verification for affected ranges but do not invalidate unchanged source observations. Human corrections remain linked to their original source identity and require review when the source changes.

## Search contract

Accept natural language plus structured filters: project/bin, asset, time range, speaker label, media kind, tags, analysis coverage, and available models. Combine transcript/text/OCR lexical matches with embedding retrieval and optional bounded reranking. Returned scores identify their meaning and model; do not compare scores across incompatible models as if they shared a scale.

Each result contains asset ID, source fingerprint, exact source interval, matched observation IDs, representative evidence, score/rank explanation, model versions, and stale/partial-coverage flags. Timeline placements are additional references, not substitutes for source coordinates. Deduplicate overlapping hits while retaining alternatives and uncertainty.

For “find the first frame where a hand touches the product,” retrieval identifies a likely segment. The agent must inspect frames near the event before proposing an exact boundary. A caption mentioning contact is insufficient to claim frame-level precision.

## Progressive inspection tools

| Tool | Input | Output / limit |
|---|---|---|
| Source summary | Asset/range and evidence budget | Existing metadata with coverage and freshness |
| Contact sheet | Source range, bounded sample count, image size | Labeled frames with presentation timestamps and asset identity |
| Source frame | Exact frame identity or timestamp plus rounding policy | Actual frame, returned timestamp, source hash, color/quality metadata |
| Bounded clip | Range, max duration/resolution, audio policy | Inspectable excerpt with synchronized timing |
| Region inspection | Frame and bounded crop coordinates | Crop plus coordinate transform back to original |
| Timeline inspection | Sequence revision/range and render settings | Composed frames/audio including effects, titles, nesting, and scenes |

Inspection progresses from overview to a narrow interval and then precise frames. Enforce image count, duration, resolution, token/provider cost, and retry budgets. Results label proxy/draft quality and cannot be mistaken for full-resolution evidence. For motion, a frame alone may be insufficient; inspect temporal neighborhoods or a bounded clip.

## Visual and audio verification

Before commit, verification renders the staged composition. Check that selected content appears at the intended time, titles remain readable and within bounds, transitions do not expose unintended black frames, aspect ratio/crop is correct, overlays do not obscure required subjects, and relevant audio is continuous and synchronized. Deterministic checks should handle measurable properties; models support contextual judgments and report uncertainty.

Store sequence/project revision, affected range, rendering settings, tool/model versions, checked criteria, evidence hashes, and result `passed`, `failed`, or `inconclusive`. Sampling policies include boundaries, transition interiors, and scene animation extremes. A handful of samples does not prove every frame is correct; reports state what was inspected. High-risk or unverified intervals require additional review rather than an invented pass.

## People, objects, actions, and tracking

v0.1 can store descriptive observations and project-local speaker/person labels where users choose to use them. Identity recognition is not assumed. Separate user-provided identity labels from model-detected visual similarity. Avoid inferring sensitive personal attributes.

Advanced object tracking, persistent identities across shots, action graphs, native VLM acceleration, and dense temporal reasoning are later capabilities. Reserve stable track IDs, time-indexed regions, occlusion/confidence metadata, and model provenance without making the MVP depend on them. Track results must be source-aligned and marked stale after source replacement.

## Evaluation and privacy

Use a consented, versioned corpus with labeled queries, relevant ranges, ambiguous examples, and hard negatives. Measure recall@k, precision@k, temporal localization error, search latency, coverage, and storage/analysis cost. Split evaluation sources to avoid scoring near-duplicates as generalization. Record per-language and per-media-type results where possible.

Keep semantic data local by default; transcripts, embeddings, OCR, and crops can be sensitive even without full footage. Provider grants specify precisely which payload leaves the machine. Deleting analysis removes relevant derived records and caches according to retention policy, while explaining any externally submitted data that cannot be recalled. Credentials never enter the semantic store.

Related: [Smart Rough Cut](09_SMART_ROUGH_CUT.md), [agent architecture](10_AGENT_ARCHITECTURE.md), [security](13_SECURITY_PRIVACY_AND_PERMISSIONS.md), [testing](15_TESTING_STRATEGY.md).
