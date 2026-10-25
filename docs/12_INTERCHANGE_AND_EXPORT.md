# ShotClutch — Interchange and Export

Baseline 0.1 · 2026-10-26 · EXPORT-001 through EXPORT-006.

## Delivery strategy

ShotClutch supports rendered media delivery and a planned interchange/adapter path to Premiere, DaVinci Resolve, and CapCut. These names identify destination applications only. The canonical ShotClutch project remains authoritative; an exported destination project is a derivative delivery artifact.

Use existing interchange structures where they can represent the edit, with destination-specific adapters, sidecar metadata, and baked assets for unsupported constructs. Do not promise native proprietary-project round trips or full feature parity. Formats such as OTIO, FCP XML variants, AAF, and EDL are candidates to evaluate, not universal guarantees of import/export support in every target version.

## Two export classes

| Class | Delivers | Editability |
|---|---|---|
| Rendered export | Video/audio file, image sequence, audio stems, captions where selected | Picture/sound fixed according to output settings |
| Interchange package | Timeline representation, media references or collected media, sidecars, baked elements, fidelity report | Depends on destination capabilities and chosen fallbacks |

Both pin the project revision, input fingerprints, range, settings, and output destination. Final export uses original media by default. Unsupported or missing inputs are resolved before committing the output plan.

## Transient interchange IR

Build a derived representation containing sequence settings, stable clip/track IDs, exact source/timeline ranges, rational frame rates, media references, channel layouts, transforms, keyframes, transitions, captions, effects, nesting, and provenance. Include capability requirements and source-to-export ID mappings.

This IR is an export boundary, not a second persistent project model. Adapters read it and produce destination artifacts. Destination-specific quirks remain in adapters rather than modifying canonical clips. Sidecars preserve extra ShotClutch metadata but cannot make unsupported target behavior work automatically.

## Fidelity ladder

For each construct, choose the highest viable tier under the user's delivery policy:

1. **Exact translation:** destination has a verified equivalent in timing and semantics.
2. **Approximate translation:** preserve intent with a documented parameter/behavior difference.
3. **Sampled native keyframes:** approximate a curve or animation using target keyframes at a declared sampling/error budget.
4. **Render-and-replace:** bake a clip/effect/scene to media, preserving alpha and handles where applicable.
5. **Composite render:** bake a larger region or sequence when interactions cannot be isolated.

If no permitted tier works, block that export. Never silently drop a construct. An approximation may preserve editability while changing appearance; a bake may preserve appearance while losing editability. The report makes that tradeoff explicit. Exactness is scoped to tested semantics and target versions, not a marketing label.

## Planning and user review

`export.plan` resolves the target profile, analyzes every construct, estimates baked media requirements, checks missing sources/fonts, determines time/color conversions, and produces a fidelity report without publishing final output. Each report entry includes canonical IDs/range, target construct, chosen tier, reason, expected loss, required render, and user alternatives.

Illustrative report:

| Construct | Plan | Consequence |
|---|---|---|
| Basic cut with linked audio | Exact if qualified by target profile | Source references and trims remain editable |
| Analytical title spring | Sample keyframes or bake | Editable sampled motion, or fixed appearance with source preserved separately |
| Web MotionScene | Bake with alpha | Visual appearance retained; programmatic scene is not editable in the destination |
| Unsupported adjustment interaction | Composite affected range | Track-level editing within the baked region is lost |
| Missing original | Block or explicit proxy export | Proxy export is labeled lower quality |

Approval, where needed, binds to the concrete plan, destination, overwritten files, and fidelity choices. Changes in project revision or source content invalidate the relevant plan. Required acknowledgments cannot be bypassed through a different adapter.

## Destination profiles

| Destination | Initial adapter investigation | Qualification condition |
|---|---|---|
| Premiere | Evaluate a supported XML/AAF/interchange route and destination-specific relinking/metadata | Import in pinned version; compare cuts, duration, media, audio, transforms, captions, and fallback report |
| DaVinci Resolve | Evaluate supported XML/AAF/OTIO routes according to actual target capability | Same checks plus target color and sequence-setting behavior |
| CapCut | Establish a stable documented or otherwise maintainable authorized adapter route | Editable export remains experimental/unavailable until real import and fidelity tests pass |

Do not infer that a format supported by one destination is supported by another. Record target app version, OS, route, adapter version, required settings, and known omissions. If no qualified editable route is available, offer rendered media, stems, captions, and an edit-decision sidecar with a clear statement that these do not constitute an editable target project.

## Time, effects, and audio mapping

Convert exact source/timeline coordinates once at the adapter boundary with a declared rounding policy. Record any frame/sample quantization. Test mixed rates, drop-frame timecode display, VFR sources, speed changes, negative offsets where supported, and nested timing. Preserve source identity through map-generated clips; a PreEditMap is exported through its committed timeline result, not by relying on the destination to understand ShotClutch-specific analysis objects.

Flatten nesting only when required and report it. Translate effects by semantics, not names alone. Sample curves using a declared maximum error and sample limit. Bake unsupported transitions with sufficient handles and exact overlap duration. Preserve alpha, working/output color transforms, audio channel layout, gains, fades, and sample rate. Bake or stem unsupported audio processing with explicit loss of editability.

## Package and relink manifest

Include export ID, source project/revision, adapter/target profile, output settings, sequence ID/range, canonical-to-target ID mapping, media hashes, relative paths, source/proxy status, baked assets and handles, fonts/licensing constraints, fidelity report, and validation result. A portable ShotClutch project also includes authored MotionScene sources, reviewed maps, user annotations, and essential generated assets according to [04](04_PROJECT_DATA_MODEL.md).

Media collection respects user-selected destinations and rights/permissions. Resolve paths canonically, prevent traversal, detect filename collisions, and avoid embedding credentials or unnecessary private absolute paths. Relink by validated identity with explicit user handling of changed content. Do not silently substitute a similarly named file.

## Completion, validation, and limitations

Write packages to a staging location, validate references and manifests, then publish atomically where supported. Cancellation/failure preserves the source project and marks partial artifacts incomplete. Repeated requests use operation IDs; they do not create duplicate deliveries or overwrite unrelated files.

Automated tests validate generated syntax and internal consistency. Destination qualification additionally requires opening/importing in the declared target version and comparing actual results. Record cut positions, durations, A/V sync, relinking, text/caption appearance, effects, and render comparisons within tolerance. Passing a parser is insufficient to claim destination compatibility.

v0.1 delivers native rendered export and the interchange planning/fidelity foundation. M5 qualifies the destination adapters and web-scene bake path. Native `.prproj`/`.drp` round trips, universal effect translation, and lossless return import are explicitly outside the initial commitment.
