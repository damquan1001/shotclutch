# M1 native runtime and acceptance preparation — 2026-11-05

These reports concern this Windows development PC. The native processing/status implementation,
UI test fixes and qualification runners are committed at `610a6cdd`. Subsequent evidence-only
updates do not change that executable. The runtime reports identify the executable SHA-256;
the qualification record identifies the committed source and runner digests.

`connected-diagnostic.json` is the exact required CLI/MCP mixed-media journey run from that clean
commit. It reports `passed-connected-diagnostic`, one passing journey, zero ignores, unchanged
source and native local-only processing. **It is not an offline pass.** Network adapter types
are retained without names, addresses or credentials. No network configuration was changed.

`fixture.json` describes the prepared 1,802-second flash/beep source. `smoke.json` records the
initial quiet three-second desktop check. `ten-minute.json` and `samples.jsonl` record the longer
device-clock observation; `qualification.json` evaluates the audio counters separately from
dropped video pictures and physical drift. JSON/JSONL copies normalize line endings to LF without
changing values. The retained-report digests cover those LF bytes.

The ten-minute run completed 600.094 seconds with a real audio clock in all 119 samples, 56,145
callbacks, zero underruns and zero missing audio frames. This passes the audio-counter criterion
for this fixture. Video had 24 dropped pictures (0.134%), above the proposed <0.1% target; it is
not a performance-budget pass. Clock progress minus wall time was -1.327 seconds, which is not a
measurement of physical A/V offset. Ordinary desktop/agent activity included the short connected
CLI diagnostic during this observation; it is not an isolated regression benchmark.

The fixture is intentionally simple, at 1080p30 with stereo 48 kHz audio. It does not qualify a
representative multi-track/effects throughput workload. Device-wide NVIDIA snapshots include
other applications. Application counters cannot measure physical speaker/display latency or
capture offset. Physical A/V drift, the disconnected journey and complete manual keyboard
workflow remain unverified; the physical second-PC test is excluded from M1 by user decision.

Raw media, binaries, desktop logs and private project/data directories remain under ignored
`target/m1/`. No control tokens, user media or project paths are included in these retained reports.
See [the procedure](../../OFFLINE_AND_AV_QUALIFICATION.md) and
[actual acceptance state](../../IMPLEMENTATION.md).
