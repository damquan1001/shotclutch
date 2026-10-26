# M0 qualification evidence

Implementation branch: `codex/m0-foundation`. Code qualification revision: `3aeef47218c74067fb4998b0d14e4aacee7dad10`. Documentation-only evidence updates may follow that revision.

M0 is **not closed** until the clean-machine installation gate is satisfied. Local package inspection, a development-machine launch, and defined CI jobs do not substitute for that evidence.

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
| Dependency layering | Passed | `target/m0/layers.log` |
| Retained asset attribution | Passed: 55 assets | `target/m0/assets.log` |
| Workflow syntax | Passed with checksum-verified actionlint 1.7.12 | `target/m0/actionlint.log`; local shellcheck integration unavailable; CI defines Unix shell/metadata checks |
| Python tools and comparison guards | Passed | Python syntax compilation; `target/m0/comparison-guard-test/result.txt` uses explicitly synthetic inputs |
| Final Clippy/format refresh | Pending final evidence collection | Full-target Clippy passed after the bounded Rust 1.95 repairs; refresh logs after final metadata changes |
| Real-media CLI and benchmark tests | Pending | Final execution logs will be recorded here |
| Final unsigned MSI and ZIP | Built and validated; clean-manifest refresh pending | WiX 5.0.2, static CRT, both legal inventories and embedded commit/lockfile provenance inspected; ZIP executables match the tested static binaries |
| Comparable performance captures | Pending measurement | Prepared media are not measured baseline evidence |

Library coverage comprises automation, edit algebra, engine, export, format, project, time, and UI library tests. The ignored tests are `essential_sound_tests::perf_full_dialogue_chain_realtime_factor`, `multicam_tests::perf_grid_of_four_1080p_angles`, and `proxies_tests::proxy_playback_perf_4k`. They remain separate performance obligations; no ignored test is counted as passed.

## Desktop workflow

The live desktop smoke passed both initially and against the final static-CRT binary. It generated a two-second H.264/AAC input, imported it through the authenticated desktop command service, made a sequence, ripple-trimmed ten frames, undid/redid, saved/reopened, and exported ProRes. Independent ffprobe/ffmpeg validation found **50 video frames and 80,000 stereo sample frames at 48 kHz**. New port/token environment variables overrode deliberately invalid legacy values. The credential was neither logged nor saved.

Final evidence: `target/m0/desktop-smoke-final/evidence.json`. ZIP executable hashes match the tested static binaries; installer metadata and notice payloads passed inspection. The manifest will be refreshed from a clean documentation tree without rebuilding unchanged code. This development-machine workflow does not establish clean-machine installation or long-form playback performance.

## Findings and remaining gates

Validation found two M0 branding regressions and corrected them: a fixed nine-byte DPX creator slice could not hold the ten-byte name, and two log-rotation assertions still named old files. The DPX preset workflow and targeted library run now pass; the desktop logging run also passes. Rust 1.95 Clippy repairs are equivalent boolean/match simplifications, recorded in the audit; no lint was suppressed.

Clean Windows installation, remote CI execution, other-platform runtime qualification, GPU performance qualification, and a second hardware tier remain unverified. Run the procedure in `QUALIFICATION.md` in a fresh VM/Sandbox or equivalent environment and attach its results. The original application-suite dependency is not required by the native pipeline; strict offline/clean-environment acceptance remains an explicit gate rather than an inference from source.

M1–M5 features and the performance release budgets in document 14 are not completed by these M0 checks.
