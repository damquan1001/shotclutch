# ShotClutch — Media and Render Engine

Baseline 0.1 · 2026-10-26 · Planned contracts for MEDIA-001 through MEDIA-005 and EXPORT-001.

## Native media foundation

Ordinary source footage is demuxed and decoded through a native media service shared by playback, source monitoring, thumbnails, visual inspection, and export. The browser fallback for graphics does not take ownership of timeline footage or the audio master clock.

```text
Asset locator -> demux/probe -> decoder registry -> timestamped frames/audio
                                              -> shared bounded caches
Sequence snapshot -> time mapping -> render graph -> GPU compositor -> output
Audio source blocks -> sample mapping -> mixer/effects -> device or export
```

MediaSource exposes stream metadata, available time ranges, frame lookup by presentation identity, audio sample-range reads, cancellation, quality/capability information, and explicit errors. Decoder-specific details do not leak into canonical clips.

## Decoder registry and fallback

Preferred selection policy is platform hardware decode when available and qualified, then a qualified native decoder, then a compatibility fallback such as FFmpeg. This is a capability and reliability decision, not a fixed rule that hardware always wins. Unsupported profiles, driver faults, missing alpha, wrong bit depth, or color handling can disqualify a backend.

Probe container, stream, codec/profile, pixel format, dimensions, bit depth, color metadata, timestamps, rotation, channel layout, and expected memory demand. Choose from backends whose advertised capability matches the request and whose configuration passes the qualification suite. Record the chosen backend and reason. Retry through a bounded fallback chain; never loop indefinitely between failing decoders.

Fallback preserves frame identity, orientation, color metadata, and A/V timing. If a switch cannot preserve these, stop and surface the incompatibility. Malformed or unsupported media produces an actionable error with a transcode option. Distribution, licensing, and platform packaging of the fallback remain a release decision; this specification does not assert any codec is already supported.

## Frame contract and seeking

A video frame carries source/stream ID, source fingerprint, presentation timestamp and duration, decoded frame identity, coded/display dimensions, pixel layout, bit depth, color primaries/transfer/matrix/range, alpha representation, orientation, backend generation, and a lifetime-managed buffer or GPU handle.

GOP-aware seeks start at a decodable point and decode forward to the requested presentation frame. Decode order and presentation order are distinct. VFR lookup uses actual presentation intervals; it must not invent a constant-rate frame index. Exact inspection and paused monitor requests identify which source frame was returned. Draft requests may approximate only with a visible quality label.

Backward scrubbing reuses valid GOP/frame caches where possible. Superseding seeks cancel stale work with generation tokens. Results from an earlier playhead request cannot replace a later requested frame. Decoder reset, device loss, and media relink invalidate affected cache entries.

## Proxies and source mapping

Proxies are alternate representations of a source asset, not replacement asset identities. Store original fingerprint, proxy fingerprint, generation settings, mapping between proxy and original presentation times, audio mapping, and quality metadata. Preserve VFR timing or generate an explicit mapping when normalizing it.

Playback can use proxies, half/quarter/eighth-resolution decode, or reduced effect samples. Paused exact inspection requests and final export prefer originals. Missing originals block final export by default. An explicitly chosen proxy-quality export may proceed with a warning and manifest entry; it must not be labeled original-quality.

Relinking validates content identity or presents a changed-content review. Same-name files are insufficient. Time maps, rough-cut analysis, and visual observations are marked stale when source content changes.

## Timeline evaluation and render graph

Evaluate sequence time into clip-local and source time using exact mappings. Traverse nested sequences with cycle guards and bounded depth. Resolve track order, visibility, clips, transforms, clip effects, transitions, track/adjustment effects, masks, and blend/composite operations in the declared order. The same semantic evaluator drives preview, paused inspection, and export.

Render nodes declare inputs, output format, temporal dependencies, quality settings, bounds, and supported GPU/CPU implementations. Native WGPU composition is the planned primary path. CPU implementations serve as qualified fallback and, where appropriate, a reference oracle. Moving a path to GPU must preserve declared numerical/color tolerances, not just improve frame rate.

The render graph must make expensive CPU/GPU transfers visible in diagnostics. Shared textures and decoder surfaces are preferred where platform contracts allow; unsupported zero-copy paths can use measured copies. Effects with temporal history must explicitly request neighboring frames. Cache keys include their full dependency window.

## Color, alpha, and typography

Record input color metadata, project working space, display transform, and output transform. Use an explicitly declared working representation and linear-light operations where the operation requires them. Do not assume all inputs are SDR or all RGBA surfaces use the same transfer function. Missing metadata uses a visible documented default and can be overridden.

Premultiplied versus straight alpha is explicit at every boundary, including MotionScene web output. Convert once at a declared boundary and test edge halos. Color and alpha conversions are part of cache keys. v0.1 qualifies a bounded SDR workflow; HDR and wide-gamut combinations require additional fixtures before support is advertised.

Fonts are resolved by pinned identity with fallback reported. Text shaping must support the declared language set, bidirectional text, and stable layout. A missing font on another machine is a portability issue, not grounds to silently claim identical rendering.

## Audio pipeline

Decode to timestamped PCM blocks; resample using a declared quality mode into the sequence mix rate. Preserve channel layout and explicit downmix policy. Clip gain, fades, linked cuts, track mixing, pan, and supported effects are evaluated at sample precision. Loudness and peaks are measured separately; prevent accidental clipping where policy specifies it and report intentional overload.

During device playback, the audio clock is the synchronization reference. Video selects frames for that clock and may drop late presentation frames. The real-time audio callback performs bounded work using prepared buffers and must not acquire long-held project locks. During export, an offline exact clock generates all required audio samples and video frames; real-time frame dropping is prohibited.

Latency compensation and retimed audio behavior must be explicit. Unsupported pitch/time-stretch combinations fail or require a disclosed bake. Device changes, buffer underruns, and clock discontinuities are observable and recover without changing timeline edit math.

## Scheduler and quality tiers

Priority classes are audio deadline work, current interactive frame, imminent playback prefetch, user-requested preview/export work, thumbnails, indexing, and speculative analysis. Final export scheduling is user-controlled so it can reserve resources without starving the UI. Each class has memory/concurrency budgets and cancellation rules.

Preview offers Full, 1/2, 1/4, 1/8, and Auto where supported. Auto may reduce proxy/decode resolution, effect samples, and MotionScene resolution. It preserves playhead responsiveness and labels the effective mode. Pausing requests a refined exact frame. Export pins final settings rather than inheriting an accidental draft state.

## Cache contract

Separate decoded-frame, audio-block, effect, partial-composite, sequence, and scene caches, with coordinated byte budgets across CPU RAM, GPU memory, and disk. Keys include source content/stream, exact time, render/effect graph digest, parameters, scene/runtime dependencies, quality, dimensions, color/alpha policy, and backend compatibility identity.

Caches are derived and safely evictable. They cannot be the only location of generated source assets or undo data. Reference-count in-use buffers; apply LRU or a measured replacement policy only to evictable entries. Report memory pressure, throttle background work, and degrade preview before allocating unbounded resources.

## Export execution and failure handling

Pin a project revision and resolve originals, fonts, scene dependencies, effects, output codec/container, dimensions, rational frame rate, sample rate, range, and color settings before starting. Emit all required frames in deterministic output order even if work is parallel internally. Write to a temporary artifact and publish only on successful mux/finalization and validation.

On disk exhaustion, worker crash, device loss, or cancellation, preserve the project and report incomplete output. A resumable export needs verified chunk boundaries and a matching manifest; otherwise restart safely. Export does not silently replace unsupported effects with blank frames.

Verification and budgets: [14](14_PERFORMANCE_AND_BENCHMARKS.md), [15](15_TESTING_STRATEGY.md). Destination packages: [12](12_INTERCHANGE_AND_EXPORT.md).
