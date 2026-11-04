# M1 same-PC offline and A/V qualification

Owner: Codex. Prepared 2026-11-05 for Windows x64 on `codex/m1-editing-foundation`.
These are local acceptance procedures for SEC-001, UI-001 and MEDIA-004, not hosted CI jobs.
The physical second-PC check is excluded from M1 by the user's 2026-11-04 decision.
[Current results and remaining gates](IMPLEMENTATION.md) are authoritative.

## Disconnected mixed-media journey

Prerequisites: clean committed source, cached locked Cargo dependencies, installed ffmpeg/ffprobe,
and the local native toolchain. Existing local test commands are preserved. From the repository:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/m1/check-offline.ps1 -ConnectedDiagnostic
```

This checks the runner while connected, but can only report `passed-connected-diagnostic`.
The exact single required CLI/MCP journey covers mixed-rate/VFR media, nesting, linked retiming,
proxy generation/switching, save/undo/redo/reopen, export, collection, relocation and source hashes.
It fails on missing oracle tools, a skipped/filtered journey, command errors or mismatched media.
Fault-injected save/recovery is covered by the separate M1 engine gate; the journey alone is not
a physical power-loss test or a full manual keyboard-accessibility acceptance.

When ready, disconnect this PC's physical and virtual network adapters, then run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/m1/check-offline.ps1
```

The script does not disconnect anything itself. Any active non-loopback interface blocks offline
acceptance with exit 2, even a virtual adapter without an internet route. The conservative check
can reject an otherwise disconnected PC. Network adapters must remain disconnected throughout;
the script records their state before/after, not continuous packet-capture evidence. Reconnect
normally after testing. `ExecutionPolicy Bypass` applies only to that PowerShell process.

Evidence is written to a new ignored `target/m1/offline-<timestamp>-<pid>/` directory. The JSON
receipt records source/lockfile/executable/oracle and fixture digests, network interface types,
read-only processing status and hashed logs. Only `passed-offline-journey`, exit 0, a clean stable
revision and one passed/zero ignored required journey qualify this run. Do not promote a connected
diagnostic, compiler-only `--offline` run, or blocked/failed receipt to an offline pass.

General Settings and `shotclutch-cli exec privacy.status` show local-only processing for native
built-ins. They do not enforce OS network isolation. Host-supplied speech backends are reported as
unverified, and optional model downloads require an explicit request. No cloud inference provider
is built into this profile; future providers need separate payload-scoped authorization.

## Real device-clock playback and physical drift

Use the existing native runtime runner in `scripts/m0/runtime-soak.py`; its implementation and
historical M0 evidence remain unchanged. It needs Python 3.11 or later (`py -3.12` on this PC),
the built desktop executable, an audio output device, and a visible desktop. The output directory
must be new. It launches an owned editor with separate data/recovery settings and shuts it down
after observing playback. It does not change the default output device or system volume.

Generate new synthetic timing input, without downloading or playing anything:

```powershell
python scripts/m1/prepare-av-soak.py --out target/m1/av-signal-new --seconds 1800
```

The fixture is 1080p30 H.264 with stereo 48 kHz AAC, 1,802 seconds long. After a silent black ten
seconds it presents a three-frame white flash and a 50 ms tone every ten seconds. The manifest
records hashes, tool versions and independent decoded checks of the first two marker boundaries.
Integer frame/sample generation avoids accumulating a floating-point period error. This simple
fixture measures timing/callback behavior; it does not qualify a representative effects workload.

Prepared on this PC: `target/m1/av-signal-long-20261105/sync-1800.mp4`; SHA-256:
`2ce62a8f73f5d1b0f1ca6594c9894aaa033b5deff62e9c6ee86f64de15faba02`.
Its `fixture.json` reports `prepared-only`, not a playback pass.

Run the ten-minute callback/underrun observation:

```powershell
py -3.12 scripts/m0/runtime-soak.py run --fixture target/m1/av-signal-long-20261105/sync-1800.mp4 --bin-dir target/debug --output target/m1/av-ten-minute-new --seconds 600 --resolution Full
```

Require successful normal completion, a real `audio_clock:true` throughout, zero underruns and
zero missing audio frames. Inspect dropped pictures and device/memory counters independently.
No later counter reset, early stop, host-only silent clock or missing sample can count as a pass.
The raw summary intentionally says to evaluate counters separately; exit 0 alone is insufficient.

For the thirty-minute physical check, position an external camera to record both the display and
its audible speaker output before starting the same command with `--seconds 1800` and a fresh
output directory. A 60 fps or faster recording helps measure the one-sequence-frame (33.33 ms)
budget. Retain the recording and note output device, driver, system volume, monitor mode, camera
rate and recording distance; sound propagation and capture A/V offset affect the measurement.
Compare flash and tone onsets near 10 seconds, halfway and near 1,790 seconds, including any fixed
recording offset. Require sync within one sequence frame and no cumulative drift over thirty
minutes. If capture uncertainty cannot resolve that budget, report unverified and improve capture.
Application clock-minus-wall and matching monitor/playhead counters do not measure speaker/display
latency. The runner deliberately leaves `physical_av_drift:null` without external measurement.

## Manual editing and keyboard acceptance worksheet

The full manual reference editing/keyboard-accessibility journey remains a separate UI-001 gate.
Use private generated fixtures and a fresh project/data directory; leave ordinary projects alone.
The existing native smoke and 498 automated UI regressions cover their tested actions, not the
completed operator journey below.

1. Open a fresh native editor using `--empty --no-recover --data-dir target/m1/manual-data-new`.
   Record the executable hash, Windows/display settings and whether networking is disconnected.
2. Import the generated phone VFR and 25 fps camera fixtures from `target/fixtures/cli/workflows/`.
   Create a multitrack sequence, place both sources and linked audio, and seek known source frames
   in the monitor. Record sequence rates and exact clip boundaries before editing.
3. Trim and split linked A/V, move a selection, adjust audio gain/fades, and nest the sequence in
   another rate. Check the monitor, meters and source offsets. Undo/redo must restore the intended
   edits without moving unrelated clips or losing links. Record actual rates/ranges and failures.
4. Generate proxies, wait for confirmed completion, and switch proxy/original. Verify cut positions
   and nesting are unchanged. Save, reopen, verify history and relink a private moved source.
5. Export an explicit range using originals, wait for confirmed completion, and independently
   inspect output frame/sample counts and timing. Collect/verify the project and locally relocate
   it with original paths unavailable. Save As outside the package and reopen the working copy.
   The original package and source hashes must still verify. No second PC is required.
6. Repeat representative trim/split/move, seeking, save/undo/redo, dialog accept/cancel and focus
   changes using the keyboard. Check visible focus, Tab/Shift+Tab navigation, Enter/Escape and
   documented shortcuts. Record any control without an accessible keyboard path; a successful
   mouse action does not count as its keyboard check. Global shortcuts intercepted by another
   application remain a recorded environment limitation until the editor action is checked.

Retain a small acceptance note with actual actions, observed results, ranges/counts, source and
output hashes, any screenshots needed to explain failures, and the native save/reopen artifacts.
Fault-injected durable save/recovery remains separately backed by the M1 engine tests; do not
interrupt unrelated user work or infer a physical power-loss guarantee from these manual actions.
