# DEC-006 — M1 native decoder/fallback selection and local qualification

Decision date: 2026-11-04. Owner: Codex. Platform scope: this Windows x64 PC,
NT build 10.0.26200, 16 logical processors. The report retains the CPU identifier and inventory
of AMD Radeon / NVIDIA RTX 3060 Laptop drivers; it does not attribute every DXVA session to a
particular adapter. Physical second-PC qualification is excluded from M1 by user decision.

## Selected path

Retain the inherited MediaSource/container/GOP interfaces. Auto can choose the qualified OS
Media Foundation/DXVA decoder for supported AVC/HEVC; the in-process pure-Rust codec registry
is the native fallback and the reference when hardware is disabled. A declined backend falls
through; a mid-stream failure uses the bounded HybridDecoder replay path. No mandatory external
FFmpeg runtime fallback, binary download or online service is introduced by M1.

FFmpeg and ffprobe are installed **local qualification tools only**, used to create synthetic
fixtures and independent presentation-time/RGBA/PCM references. They are not linked or shipped
by this change. The actual runtime dependency and notice obligations remain those of the audited
native crates and platform APIs in the M0 inventory; this decision does not assert additional
codec redistribution rights or universal availability of OS extensions. No new dependency
version/package was added for the qualifier; its serde/JSON/SHA dependencies are already locked.

## Reproducible bounded corpus

Run `python scripts/m1/qualify-media.py --out target/m1/<new-evidence-directory>` locally. It builds
`shotclutch-platform --example m1_media` with locked/offline Cargo, requires installed ffmpeg and
ffprobe, generates all nine cases, and runs software, Auto and forced-fallback in separate
processes. A missing fixture/tool, partial decode, wrong timestamp, changed cold/warm result,
wrong sample window or failed oracle comparison fails the run; nothing is silently skipped.
The example and oracle generation are outside routine hosted CI. One small real-container,
oracle-free source-time regression joins the existing engine `m1_` gate.

| Case | Declared input | Independent acceptance |
|---|---|---|
| H.264 MP4 | 1920x1080, 24000/1001, 8-bit 4:2:0, B pictures, 24 frames | Every PTS and frame; warm/cold seek and interval identity |
| HEVC MP4 | 1920x1080, 30000/1001, Main 8-bit 4:2:0, B pictures, 30 frames | Same; original presentation offset preserved |
| ProRes MOV | 640x360, 25 fps, 422 HQ 10-bit SDR, 25 frames | Same; output color conversion compared with reference |
| H.264 VFR MP4 | 320x180, irregular retained 30 fps timestamps, 39 frames | Real presentation intervals; no invented CFR frame IDs |
| PNG | 32x16 RGBA gradient with varying alpha | Byte-exact RGBA and alpha |
| PCM WAV | 44.1 kHz mono, 44,100 frames | Independent f32 reference, max error 1e-6, random windows |
| PCM WAV | 48 kHz stereo, 48,000 frames | Independent f32 reference, channel layout, random windows |
| Invalid MP4 | Unrecognized payload | Recoverable error, no successful frame |
| Truncated HEVC MP4 | 64-byte truncated header | Recoverable error, no successful frame |

Video acceptance: every returned source PTS equals the independent integer-tick oracle; geometry
and frame count match; RGBA PSNR is at least 30 dB against the explicitly nearest-chroma reference.
PNG additionally requires byte equality. Cold and warm queries and intermediate presentation
intervals must return the exact same RGBA hash as the verified source frame. The report separates
verified request counts from internal hardware pictures, because cold seeks/replay decode extra
pictures. Source/oracle, executable, lockfile, source-file and report digests make the run auditable.

Forced fallback wraps a software decoder in the real HybridDecoder and injects an error after
eight input samples. This exercises real replay/merge/reset logic, but it is **simulated hardware
failure**, not evidence of an actual GPU loss. Auto runs separately exercise real DXVA output.

## Measured result and fixes

The bounded nine-case corpus passed in all three modes: 27 case/mode results. Auto recorded
75, 79 and 102 real hardware pictures for the H.264, HEVC and VFR workloads respectively in the
final 2026-11-05 run (256 total, including extra seek/replay pictures); software
recorded no hardware pictures. Forced mode exercised 15 recoveries across those three files and
their cold source opens. Real hardware and software produced identical verified RGBA hashes.
The four compressed-video cases had minimum reference PSNR 45.95, 45.91, 46.44 and 45.39 dB;
PNG was exact. PCM full streams and random windows passed the 1e-6 bound.

The initial run failed all four movie cases because the returned VideoFrame PTS was left at zero.
The shared GOP publication boundary now converts the actual decoded PTS into canonical ticks,
after crop/rotation, for serial and parallel decoding, cache hits, cold seeks and fallback replay.
Native MP4/MOV clocks include edit-list offsets, including B-picture composition delay. MKV,
MXF, AVI, MPEG and APV adapters supply their clocks/origins too; the latter formats are not thereby
advertised as newly oracle-qualified by this nine-case run.

Arbitrary MP4/MKV seeks now use the last track unit whose canonical presentation tick is at or
before the request. This preserves half-open intervals, including clocks that lose a fraction of
one canonical tick when converted. Exact nominal-grid requests retain the inherited coarse-muxer
CFR quantization policy. The small in-process M1 regression remuxes known frames with a coarse
24 Hz clock, nonuniform VFR durations and fractional 24000/1001 timing plus an edit offset. It
checks actual frame identity at boundaries, interval interiors, one tick before the next boundary,
warm cache requests and fresh-source seeks.

Broader regressions caught coarse-clock playback regressions in the first conditional-lookup
implementation. MP4 now retains the grid validated across the entire sample table, independently
of the metadata's median-rate estimate. A standard rate can absorb at most one accumulated track
unit of average-duration quantization before the same full-table validation. All existing
rounded-timestamp and VFR frame-identity cases pass, including short 59.94 fps/1 ms-clock clips.

The inherited decode benchmark no longer reports the nominal complete count/FPS after a decode
failure: it records actual decoded count and an error, and its section exits unsuccessfully.
Do not interpret older benchmark timings as strict decoder qualification.

The [retained final reports](evidence/native-media-20261105/qualification.json) and three mode
reports are described in the [evidence guide](evidence/README.md). All verified video hashes match
across all three modes. The final run passed in 39.14 seconds including its local build/generation
steps; this is not a sustained decoder measurement. Initial failures remain locally under
`target/m1/media-initial/`, with the earlier pass under `media-qualified/` and final generated
inputs/references under `media-final/`. Exact regression, CLI and static-check outcomes are
recorded in IMPLEMENTATION. The benchmark's real corrupt-input check returned exit 1, an explicit
failed row and zero decoded frames, with no fabricated throughput.

## Limits and remaining acceptance

This qualifies the declared small synthetic SDR subset on this PC. It does not qualify HDR,
4:2:2 AVC/HEVC, arbitrary malformed media, camera vendor variants, every container, long-form
performance, physical A/V drift or no-egress behavior on a disconnected machine. The short
`queryFps` measurements include open/query work and exclude reference hashing; they are neither
sustained playback nor compositor/export benchmarks. Hardware selection must remain optional.

DEC-006's implementation/distribution choice is selected and the bounded profile is qualified;
MEDIA-001 and the wider M1 media/audio/render criteria remain In Progress pending the remaining
workflow/performance/security evidence. No second-PC check is required for this milestone.
