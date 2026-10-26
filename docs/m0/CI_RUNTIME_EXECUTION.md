# CI/runtime rollout execution

Started 2026-10-26 (Asia/Bangkok), after the user instructed proceeding with the rollout sequence. Branch: `codex/m0-foundation`. This is the execution record; the original [rollout plan](CI_RUNTIME_ROLLOUT.md) remains the planning baseline.

## Deployment preflight

- GitHub CLI 2.103.2 is installed at `C:\Program Files\GitHub CLI\gh.exe`; it was absent from this session's PATH, not absent from the machine.
- CLI access confirms ADMIN permission on private, non-fork repository `damquan1001/shotclutch`, default branch `main`, not archived. Actions is enabled; allowed actions are unrestricted and default workflow token permissions are read-only. No billing, visibility, signing or runner-registration settings were changed.
- Remote main matched the preserved documentation baseline `b739515e04b3467ed8b681e6296f7dc5867f43dd`. The milestone branch did not previously exist remotely. The user authorized proceeding with the rollout, including its branch-push bootstrap.
- Existing repository rulesets were empty and main was unprotected. Required-check enforcement remains a later step after stable check names and default-branch workflow availability; no protection settings have been applied yet.

## CI-001 — preparation

Implemented and pushed at `5f697163cc5408cc95281c1f1656c881e87fadad`:

- Explicit milestone-branch bootstrap triggers, retained Windows/Linux/macOS checks, and separate release-build jobs to limit combined debug/release disk pressure. Windows release qualification is the static-CRT MSI/ZIP job.
- Version/checksum-locked FFmpeg/ffprobe test tools for each native test architecture and strict required-oracle mode. The Windows bundle uses 8.1.1; Linux/macOS use 9.0.2, with each version recorded rather than treated as an identical cross-platform performance oracle.
- Workspace library, desktop/CLI binary, legacy project fixture, real-media CLI integration, benchmark-unit and doctest coverage; failure-preserving command logs and final aggregate gates.
- Source-download caching only; compiled targets are rebuilt. Read-only workflow permissions, disabled persisted checkout credentials, pinned third-party action revisions, finite timeouts and artifact retention.
- Disposable-runner installed CLI import/edit/undo/redo/save/reopen/ProRes export with oracle tools excluded from the application's PATH. Independent verification checks 50 video frames and 80,000 stereo sample frames. Install/uninstall logs survive failures, and a user-created sentinel must survive uninstall.

Local validation passed actionlint, PowerShell/Python syntax, two helper failure guards, the checksum-locked Windows oracle setup and the actual packaged CLI workflow/output checks. No application feature code changed. The installer harness refuses ordinary local execution, protecting the user's installed application from the disposable-runner test.

## CI-002 — first remote runs

| Workflow | Source / attempt | Run | Current evidence |
|---|---|---|---|
| Packaging lint | `5f697163`, attempt 1 | [40292136108](https://github.com/damquan1001/shotclutch/actions/runs/40292136108) | Passed: PowerShell/Python checks, failure guards, Unix shell/desktop/AppStream checks and workflow lint |
| CI | `5f697163`, attempt 1 | [40292136788](https://github.com/damquan1001/shotclutch/actions/runs/40292136788) | Failed: Linux desktop-entry identity assertion; Windows legacy FXC compiler abort. macOS checks and Windows package/install lifecycle passed |

This is not yet the required pair of green same-commit runs. [Retained first-run records](evidence/rollout/ci-5f697163-1/run.json) include job summaries, failure logs, artifact IDs/digests and package hashes. macOS ran 2,599 passing tests with 12 explicitly ignored tests. Compiled-target caching was disabled. Linux/macOS release compilation was skipped because the initial workflow depended on the failed checks matrix; it is not counted as passed.

The first Windows MSI/ZIP were downloaded and validated locally. MSI SHA-256: `51769179d8da5e9e9cec932c3e542446b44c197cc6ff6db0eb50714517196967`; ZIP SHA-256: `964a88f287287e4874fd05bc9a064f0af977c54d4febdf864c687808213dc5a3`. Clean build provenance binds both to `5f697163`. Fresh hosted install and uninstall returned 0, the user sentinel survived, and the installed CLI produced exactly 50 video frames and 80,000 stereo sample frames. These successful lanes remain evidence even though the aggregate CI run failed. A hosted development image is not an offline consumer Windows VM.

### Repairs for the next run

- Linux-only development icon registration and its metadata test now use the ShotClutch desktop entry and original scalable SVG. The failing test had compared the renamed product against the inherited FilmCraft desktop file. No Windows product source changed.
- Windows GPU tests had 24 failures in legacy FXC `D3DCompile`, including `0x80004005` internal compiler aborts. Added a checksum-locked Microsoft DXC v1.8.2505.1 **test-only** bundle, version/file hash evidence and an actual shader compile probe. GPU unit tests honor explicit wgpu backend/compiler settings, and required-adapter mode fails rather than silently skipping. The hosted profile explicitly selects Direct3D 12/DXC. Local qualification of this harness passed **42/42 compositor tests** on AMD Radeon integrated graphics with Direct3D 12. This is the same physical laptop, not a second machine.
- Legacy FXC remains a known unqualified path; DXC tests do not repair or qualify it for consumers. No compiler DLL is added to application packages. Native desktop measurements below use NVIDIA Vulkan.
- Aggregate names are now distinct: `CI required` and `Packaging required`. Linux/macOS release builds run independently of checks, while the final gate still requires every lane. Documentation-only push filtering avoids unnecessary repeated builds; PR triggers remain unfiltered. Python UTF-8 is explicit on Windows.

## Runtime preparation and provisional observations

- Added a backend/section selector to the existing benchmark wrapper, preserving default software/CPU behavior and using the selected mode for both preparation and measurement. Empty measured sections fail. The inherited export section couples hardware encode/GPU export selection to `--hw`; that limitation is stated in each new manifest.
- A development-binary 1080p H.264 GPU-path smoke produced 96 shown / 0 dropped frames in each of three quick repeats. The first repeat reports 110 hardware-decoded frames and one session; later repeats reuse cached frames. These small runs do not establish sustained performance.
- A 15-second development-binary native desktop observation selected **NVIDIA GeForce RTX 3060 Laptop GPU, Vulkan, NVIDIA 555.97**, with Media Foundation decoding. It reported 459 shown / 0 dropped frames, 1,432 audio callbacks, no underruns/missing frames, and about 0.023 seconds difference between observed playhead and wall-clock progress. Preview resolution was **1/2** in this initial sampler check; the longer candidate run is configured explicitly. This is application telemetry, not physical loopback A/V drift measurement.
- Generated a separate 1,802-second 1080p30 H.264/AAC stereo fixture with brief quiet markers every minute. The large generated file stays under ignored `target/qualification`; preserve its manifest/hash with the actual measured run.

### Measured CI candidate

The exact CI-produced `5f697163` desktop executable (SHA-256 `e82cc4cc97178ff712c64bca471fc6666f66cac64c1ff1fc7f1b277396750d0a`) completed a **30-minute Full-resolution native playback observation** on the Windows 11 / Ryzen 7 4800H / RTX 3060 Laptop profile. It used Vulkan/NVIDIA 555.97 and Media Foundation hardware decoding, with 53,997 hardware-decoded frames, no software fallback, 53,868 shown frames and **99 dropped frames (0.1834%)**. Audio reported 168,611 callbacks, zero underruns and zero missing frames. Peak working set was 952,504,320 bytes (0.887 GiB). It remained alive through controlled shutdown. [Raw counters and summary](evidence/rollout/ci-5f697163-1/full-soak/summary.json), [samples](evidence/rollout/ci-5f697163-1/full-soak/samples.jsonl), [fixture generator/hash](evidence/rollout/runtime/soak-fixture.json).

This Full-resolution observation exceeds document 14's proposed 0.1% drop target. It characterizes an achieved mode and an outstanding performance limitation, not a performance-budget pass. Playhead progress minus observed wall-clock progress was -1.376 seconds over the run; this is internal telemetry, **not measured physical A/V drift**. Physical loopback remains unavailable. The generated 2.57 GB fixture stays outside Git; its SHA-256 is `7637c37e4c487b5a2e3e7759a5d9aa11b8c789dd41bc102ee62279e86dde091b`.

The seven-section auto-decode/GPU quick matrix completed all sections (94 rows). Hardware counters establish actual use where reported; repeated cached frames do not count as fresh hardware decoding. 4K AV1 playback dropped most frames and remains unqualified. The inherited benchmark does not expose the active adapter model; the separate native observation above does. [Raw matrix](evidence/rollout/runtime/measurements/gpu-auto-matrix/manifest.json).

Three interleaved upstream/fork software/CPU decode pairs used U/F, F/U, U/F order. Earlier slower decode results were **not reproduced**: per-pair median FPS changes for H.264 were +17.2%, +2.0%, +23.7%; HEVC +4.2%, +23.1%, +16.6%; VP9 +3.2%, +1.8%, +30.6%; AV1 +4.8%, +5.7%, +13.6%; ProRes +0.9%, +3.6%, +0.8%. This does not establish a speedup or root cause: thermal, clock, OS-cache and scheduling conditions were uncontrolled, and runs are short. The original slower baseline remains intact. [Raw pairs](evidence/rollout/runtime/interleaved/upstream-pair1/manifest.json). Heavy local builds/tests and native soak did not overlap these measurements.

Physical loopback, additional lifecycle scenarios, a successful final-CI candidate and a second hardware profile remain separately reported coverage. The first candidate's measured runtime evidence is bound to its own revision rather than silently relabeled as a later build.
