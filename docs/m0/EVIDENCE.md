# M0 qualification evidence

Implementation branch: `codex/m0-foundation`. Production application qualification revision: `3aeef47218c74067fb4998b0d14e4aacee7dad10`. Subsequent commits refine tests, the measurement harness and documentation; application binaries are bound to that production revision by package provenance and executable hashes. Retained reports and reproduction commands are in [evidence/README.md](evidence/README.md).

The user has confirmed clean 0.13.0 MSI installation and the launch/import/edit/save/reopen/video-export workflow. [Direct manual acceptance](evidence/clean-install-user-confirmation.md) resolves the prior clean-install/core-workflow gap. Remote CI and extended runtime qualification are now executing: [current rollout results](CI_RUNTIME_EXECUTION.md), including successful lanes, failures, repairs and the 30-minute CI-candidate playback characterization. The historical local evidence below retains its original revision and scope.

## Recorded source and platform

- Inherited baseline: `3a5f217b348bf0c9dc1678f305c34b6c88617f37`, archived unchanged under `target/m0/upstream`. No source repairs were applied to this baseline.
- User documentation baseline: `b739515e04b3467ed8b681e6296f7dc5867f43dd`. The working tree was clean before branch creation.
- Initial qualification platform: Windows x64, Windows 11 Home Single Language 10.0.26200, Ryzen 7 4800H. Available adapters include AMD Radeon graphics, RTX 3060 Laptop GPU, and Meta Virtual Monitor. Complete available telemetry belongs to the measurement manifests.
- Rust 1.95.0 (`59807616e`, 2026-04-14), workspace version 0.13.0. WiX is pinned to 5.0.2 and installed locally under `target/m0/tools`; the host's pre-existing WiX 4 installation is preserved.
- Inventories: 54 workspace packages and 710 compiled engine commands. These counts are not feature-completion or parity estimates.

## Local checks

| Gate | Recorded result | Evidence |
|---|---|---|
| Untouched upstream release build | Passed | `cargo build --release --locked -p filmcraft -p filmcraft-cli`; Cargo reported 16m 37s including first-build work |
| Initial ShotClutch release build | Passed | `target/m0/build-shotclutch.log`; Cargo reported 16m 36s, not a controlled build-speed comparison |
| Targeted libraries | Passed: 1,258 tests; 3 ignored performance tests | `target/m0/tests-libraries-final.log` |
| Legacy project fixtures | Passed: 16 tests | `target/m0/tests-project-fixtures.log` |
| Desktop arguments/control/audio/logging tests | Passed: 75 tests | `target/m0/tests-desktop-final.log` |
| Dependency layering | Passed | [Final log](evidence/checks/layers-final.log) |
| Retained asset attribution | Passed: 55 assets | [Final log](evidence/checks/assets-final.log) |
| Workflow syntax | Passed with checksum-verified actionlint 1.7.12 | `target/m0/actionlint.log`; local shellcheck integration unavailable; CI defines Unix shell/metadata checks |
| Python tools and comparison guards | Passed | Python syntax compilation; `target/m0/comparison-guard-test/result.txt` uses explicitly synthetic inputs |
| Final Clippy/format refresh | Passed | [Full-target release Clippy](evidence/checks/clippy-final.log), [format](evidence/checks/fmt-final.log); no lint suppression |
| CLI unit/integration and benchmark tests | Passed: 16 CLI unit, 16 integration, 2 benchmark tests | `target/m0/tests-cli-final.log` (unit phase), `target/m0/tests-cli-integration-final.log`, `target/m0/tests-benchmark-final.log` |
| Clean MSI installation and core workflow | Passed by user confirmation, version 0.13.0 | [Manual acceptance](evidence/clean-install-user-confirmation.md): launch, import/edit, save/reopen and video export; OS/hash details and extra lifecycle/offline paths not separately supplied |
| Unsigned MSI and ZIP | Passed initial qualification and subsequent wizard correction | Initial clean-manifest reports under `evidence/packages`; [current wizard package checks](evidence/msi-wizard/validation.json) also verify interactive dialogs and launch gating |
| Comparable performance captures | Passed as baseline capture, not release performance qualification | [Checked comparison](BENCHMARKS.md); seven sections, three repeats, 19 identical fixture hashes and 94 raw rows per build; every section exit code 0 |

Library coverage comprises automation, edit algebra, engine, export, format, project, time, and UI library tests. The ignored tests are `essential_sound_tests::perf_full_dialogue_chain_realtime_factor`, `multicam_tests::perf_grid_of_four_1080p_angles`, and `proxies_tests::proxy_playback_perf_4k`. They remain separate performance obligations; no ignored test is counted as passed.

The reports referenced by local `target/m0/` paths above are also retained under [evidence/checks](evidence/checks), with reproduction commands in the [evidence index](evidence/README.md). The dependency inventory contains 54 workspace packages and 335 packages in the Windows-filtered workspace/default-feature graph, including development dependencies. Every package declares a license expression; this does not prove which packages are linked into either executable or certify fulfillment of every dependency's redistribution obligations. See the [locked package/license graph](evidence/inventory/dependencies.json).

## Desktop workflow

The live desktop smoke passed both initially and against the final static-CRT binary. It generated a two-second H.264/AAC input, imported it through the authenticated desktop command service, made a sequence, ripple-trimmed ten frames, undid/redid, saved/reopened, and exported ProRes. Independent ffprobe/ffmpeg validation found **50 video frames and 80,000 stereo sample frames at 48 kHz**. New port/token environment variables overrode deliberately invalid legacy values. The credential was neither logged nor saved.

Final application evidence: [desktop responses and verification](evidence/desktop/evidence.json). ZIP executable hashes match the tested static binaries; installer metadata and notice payloads passed inspection. The initial package manifest records production source `3aeef472` and observed documentation revision `0bd65fe4`, with `dirty: false`. The subsequent wizard correction below retains the same application binaries and records its own packaging provenance. This development-machine workflow does not establish clean-machine installation or long-form playback performance.

Unsigned local artifacts (not published):

- `dist/release/shotclutch-0.13.0-windows-x64.msi`, SHA-256 `01C6E1AF465213C46EFE2F496BEAB74230B17F849C3090F5405BDBB1F803EAAF`.
- `dist/release/shotclutch-0.13.0-windows-x64-portable.zip`, SHA-256 `35D0BEB9F1C1907D6F6A6725A91B0E3466155654A2C578BC92CAE91B6E47C291`.

See [current package inspection](evidence/msi-wizard/validation.json) and [build provenance](evidence/msi-wizard/build-info.json). Initial package inspection/provenance remains in `evidence/packages` as historical qualification evidence. These artifacts retain workspace version 0.13.0; no v0.1 package-version migration was made.

## Installer usability correction

The user reported that the portable executable worked but opening the MSI did nothing. Windows Installer events on this development machine show that the original MSI installed successfully (11707 / 1033, status 0), followed by two successful reconfigurations. Both installed executables and the Start menu shortcut exist, and the installed CLI runs. The original MSI lacked a setup dialog set and did not launch the application after installation; it gave the user no useful completion flow.

Correction source: `b5102d3c46e39bfd8b6cdc11462d95dae37961c8`. The package now includes the version-pinned [WiXUI InstallDir dialog set](https://docs.firegiant.com/wix/tools/wixext/wixui/), actual MIT/Apache license text, destination selection, progress, completion and a Launch ShotClutch checkbox on Finish. Launch is tied to that UI event and is absent from the silent execution sequences. Re-running an already installed package has a maintenance flow. Application version, upgrade identity and tested binaries are retained.

Artifact validation passes checks for the welcome/license/destination/progress/completion/maintenance dialogs, installed executable launch target, checkbox condition, silent-install sequences, product identity and legal payload. A live preview exposes a `ShotClutch Setup` window and the installer log reaches `WelcomeDlg`. The computer-use tool blocks controlling `msiexec.exe`, so no automated Finish click or launch was performed. The user subsequently confirmed clean installation and a successful launched editing/export workflow. Finish-checkbox behavior and uninstall remain separate lifecycle coverage.

[Diagnosis and binary bindings](evidence/msi-wizard/diagnosis.json), [package build log](evidence/msi-wizard/build.log) and [validation](evidence/msi-wizard/validation.json) are retained. Installed, newly packaged and previously tested static executables have identical hashes. The replacement package was built before the fix commit: its manifest truthfully records `dirty: true`, observed parent `9e70433b` and application source `3aeef472`; the corresponding installer-source changes are now committed at `b5102d3c`.

## Measured baseline

The untouched upstream capture uses `3a5f217b348bf0c9dc1678f305c34b6c88617f37`; ShotClutch uses `f35fac68e9ebca6e8d8503210d2f7f750c9058bb`. The same committed orchestration harness ran both existing benchmark binaries directly. No source patch was applied to upstream: [1,415 archived files verified unchanged](evidence/baseline-integrity.json). Both builds use the same Cargo.lock hash. Fixtures were generated and warmed outside measurement; each measured section ran in a fresh child with software decode and CPU composition. Quick mode does not cover the full release corpus. Mixed-rate/VFR media are additionally exercised by the required-oracle CLI workflows, rather than claimed as part of the playback benchmark corpus.

| Measured section | Process wall seconds, upstream / ShotClutch | Peak working set GiB, upstream / ShotClutch |
|---|---:|---:|
| decode | 39.189 / 40.874 | 1.834 / 1.853 |
| seek | 88.565 / 90.999 | 1.320 / 1.321 |
| playback | 247.465 / 247.743 | 2.491 / 2.429 |
| scrub | 133.695 / 134.103 | 2.515 / 2.514 |
| timeline | 8.242 / 8.608 | 0.338 / 0.338 |
| export | 61.320 / 61.918 | 1.800 / 1.818 |
| project | 0.749 / 0.713 | 0.064 / 0.064 |

These are whole-section process resources, including startup and report writing. Per-case timings and available CPU measurements remain in the raw reports. The observer samples peak working set every 100 ms. GPU utilization, temperature, power consumption, cold-cache behavior and upstream Windows load averages are unavailable; empty inherited load strings do not mean an idle machine. The manifest records the active power scheme and installed GPU drivers, without implying GPU workload qualification.

[The full comparison](BENCHMARKS.md) reports all case metrics. Several decode values are slower in the ShotClutch pass (H.264 1080p -8.1%, HEVC 1080p -20.1% best-of-repeat FPS); export values are 1.1–2.3% lower, while 5,000-clip save/open medians are 5.6%/6.8% lower. These sequential quick runs do not isolate code effects from thermal, scheduling or cache variation. Investigate with repeated/interleaved controlled runs before asserting either a regression cause or a speedup. Both builds drop many frames on complex/4K software playback, including zero-frame repeats; successful capture does not qualify those scenarios as real-time.

Raw [upstream manifest](evidence/upstream/manifest.json) and [ShotClutch manifest](evidence/shotclutch/manifest.json) contain complete available hardware/toolchain/settings/corpus data. The comparator accepted matching profiles, cases and sample counts. Explicit synthetic guard checks reject mismatched inputs and preserve the previous report on failure.

## Findings and remaining gates

Validation found two M0 branding regressions and corrected them: a fixed nine-byte DPX creator slice could not hold the ten-byte name, and two log-rotation assertions still named old files. The DPX preset workflow and targeted library run now pass; the desktop logging run also passes. Rust 1.95 Clippy repairs are equivalent boolean/match simplifications, recorded in the audit; no lint was suppressed.

Windows PowerShell benchmark preparation initially treated native stderr as an error, and the first measured attempt rejected an unavailable process exit code. Preparation now uses redirected child processes, and measurement retains the native handle and a per-section exit-status report. Those rejected/preparation attempts are excluded; both retained captures completed all seven sections. No benchmark workload or upstream source was changed to obtain a passing capture.

Clean Windows installation and the core editing/export workflow passed by user confirmation. Hosted CI has run, with macOS checks and the Windows package/install/render/uninstall lanes passing on the first candidate; Linux identity and Windows FXC failures require a new aggregate run. A 30-minute NVIDIA Vulkan observation completed but exceeded the proposed Full-resolution frame-drop target. [Current rollout evidence](CI_RUNTIME_EXECUTION.md) records these bounded results; no two-green-run, offline consumer VM, physical A/V loopback, other-platform runtime or second-machine qualification is claimed. Exact OS/package-hash metadata was not supplied with the historical manual acceptance; preserve that distinction from the automated reports.

M1–M5 features and the performance release budgets in document 14 are not completed by these M0 checks.

CLI integration qualification initially stopped at the renamed MCP display-name assertion. The assertion now expects `shotclutch`; resource URIs remain compatible. The MPEG probe test now uses the shared Windows-capable oracle finder, so required FFmpeg discovery cannot silently skip that test on Windows. Both real-media workflows and all 16 integration tests pass. Combined targeted checks total **1,383 passing tests**, plus the three explicitly ignored performance tests above.
