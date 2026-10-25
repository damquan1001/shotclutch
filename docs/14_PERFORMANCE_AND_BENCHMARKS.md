# ShotClutch — Performance and Benchmarks

Baseline 0.1 · 2026-10-26 · PERF-001, PERF-002, MEDIA-005. All numbers below are proposed targets, not measurements.

## Measurement policy

Performance claims must name a build, platform, hardware profile, corpus version, settings, and cache state. The foundation's architecture or published claims do not establish ShotClutch performance. M0 records an untouched baseline; later changes compare against it using the same workloads.

Qualification profiles must include exact CPU, RAM, GPU/VRAM, driver, OS, storage, display resolution/refresh, power mode, decoder/backend, model runtime, and relevant background load. Select at least an entry qualification machine and a higher-capability machine. Their exact configurations and first release OS remain DEC-001; do not publish unqualified promises before they are selected.

## Proposed interaction and throughput budgets

| Metric | Proposed qualification target | Measurement conditions |
|---|---|---|
| Timeline input/layout work | p95 at or below 16 ms per interaction frame | 60 Hz reference display, representative large timeline, no decode on UI thread |
| Warm scrub response | p95 at or below 100 ms from seek input to correct displayed frame | Qualified 1080p or mapped proxy fixture; report approximation separately |
| Cold exact seek | Report separately; initial planning budget p95 at or below 500 ms | Long-GOP fixture on declared storage; no warm-cache substitution |
| 1080p playback | Real time at fixture frame rate; less than 0.1% dropped frames after preroll | Qualified baseline effects/track load; no audio underruns in 10-minute run |
| 4K playback | Real time through declared proxy/adaptive mode | Report actual resolution, effects quality, decoder, and dropped frames |
| Paused exact refinement | p95 at or below 500 ms on baseline fixture | Exact requested frame at full declared preview quality |
| Cached frame inspection | p95 at or below 250 ms | Local frame render/retrieval, excluding external model latency |
| Local semantic retrieval | p95 at or below 1 second | Warm index of 10,000 segments; reranking reported separately |
| Local project open | p95 at or below 5 seconds to editable UI | 1,000-clip fixture on qualified SSD; media analysis not required before editing |
| Cancellation acknowledgement | At or below 250 ms in UI | Stop further dispatch immediately; worker termination measured separately |
| Preview A/V sync | Within one sequence frame, with no cumulative drift | 30-minute continuous fixture; audio sample mapping checked separately |
| Final render correctness | All expected frames and exact declared sample count | No real-time dropping; speed measured independently |

These budgets deliberately separate perceived interaction from decode/model latency. Record p50/p95/p99 and worst case where sample size supports it. Do not claim a p99 from a handful of measurements. Initial numbers are design constraints to validate or revise with evidence, not automatic release waivers.

## Resource ceilings

Use explicit CPU RAM, GPU memory, disk cache, decoder concurrency, model concurrency, and web worker limits. Initial configurable budgets are chosen per qualification machine during M0/M1; no single universal byte limit is specified here. Report peak resident memory, peak GPU allocation, cache occupancy/hit rate, disk growth, eviction rate, and allocation failures.

Stress runs must stabilize after warm-up within configured budgets. A tenfold increase in timeline duration must not require decoding or retaining all frames. Opening a large project may load canonical metadata eagerly where bounded, but waveforms, frames, and deep semantic analysis are lazy or incremental. Memory pressure should throttle background work and reduce preview quality before producing an uncontrolled failure.

## Versioned benchmark corpus

| Fixture | Content | Primary question |
|---|---|---|
| B01 Baseline edit | 1080p constant-rate clips, linked stereo audio, cuts, title, simple transform | Is the common edit loop responsive and correct? |
| B02 Long-GOP | H.264/HEVC across qualified profiles and GOP lengths | How do cold/warm random seek and hardware fallback behave? |
| B03 Intraframe | ProRes or other qualified intraframe media | What are decode bandwidth and storage bottlenecks? |
| B04 Phone/VFR | Rotation, VFR timestamps, discontinuities, mixed rates | Are frame identity, sync, and proxy mappings preserved? |
| B05 Long form | 45–120-minute timelines, 1,000 then 10,000 clip stress variants | Do open, selection, trim, save, and undo scale? |
| B06 Layered edit | 8–10 video tracks, 8–16 audio tracks, nesting, titles, grade/effects | Where do GPU/CPU fallback and transfer costs dominate? |
| B07 Speech review | Long interview, noise, fillers, restarts, multiple takes | What are analysis cost, workspace response, and mapping accuracy? |
| B08 MotionScene | Static/animated native scenes and later web scenes | Are random access, caching, and evaluation deterministic and bounded? |
| B09 Semantic library | 10,000 searchable segments with labeled queries | What are retrieval relevance, latency, and storage cost? |
| B10 Failure/stress | Missing/corrupt media, device loss, full disk, worker crash | Does the editor recover without data loss or runaway memory? |

Use generated or licensed/consented media with immutable hashes and documented redistribution rights. Include mixed 24000/1001, 25, 30000/1001, and 60000/1001 rates plus 48 kHz audio; add other supported rates explicitly. A fixture containing a format is a test obligation, not evidence that it passes.

## Run protocol

1. Record build/commit, dependencies, hardware, drivers, model versions, fixture hashes, and settings.
2. Define cold and warm states precisely: app process, OS file cache, decode cache, render cache, and semantic index are separate layers.
3. Execute deterministic interaction scripts and predefined random-seek sequences with fixed seeds.
4. Warm up only where the scenario calls for it. Run enough repetitions to report meaningful percentiles and variability.
5. Collect UI latency, queue wait, decode/render timings, transfer timings, audio underruns, dropped frames, memory, and disk I/O.
6. Compare output correctness alongside speed. A lower-quality or wrong-frame result cannot count as an optimization.
7. Store raw samples and a concise report with known confounders and pass/fail against the declared profile.

End-to-end model-assisted tasks also report model inference, preprocessing, network/provider wait, retries, and tool overhead separately. External model latency must not be hidden inside a local-render metric. Analysis throughput uses source-duration-to-processing-time ratio and peak resources, plus quality/coverage measures.

## Regression policy

M0 creates the baseline without inventing completion percentages. M1 qualifies correctness and minimum media workflows. M4 compares the complete v0.1 workflow against the budgets. A repeated regression exceeding 10% in a stable benchmark is an initial investigation trigger, not proof by itself; assess variance, output quality, and user impact. Hard correctness, data-loss, or unbounded-resource regressions block regardless of speed.

If a budget is unrealistic on the chosen supported hardware, record the evidence, root cause, changed scope/settings, and approved replacement target in the tracker and this document. Do not silently alter benchmark media or quality to make a result pass.

## Optimization order

Start with actual profiles: decoder selection, unnecessary copies, seek reuse, bounded shared caches, render graph batching, GPU coverage, and proxy correctness. Then optimize expensive effect paths, partial-composite caching, scene caching, model scheduling, and background-index contention. Keep CPU/reference paths for correctness comparisons where useful. Deeper parallelization and web-worker pools follow measured bottlenecks rather than assumed gains.

Every report should distinguish achieved measurements, proposed targets, known failures, and untested combinations. Link qualifying reports from [17](17_DEVELOPMENT_TRACKER.md).
