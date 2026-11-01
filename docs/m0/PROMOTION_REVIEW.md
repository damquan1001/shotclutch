# M0 branch review

Review branch: `codex/m0-foundation`. Preserved main/documentation baseline: `b739515e04b3467ed8b681e6296f7dc5867f43dd`. Inherited upstream source: `3a5f217b348bf0c9dc1678f305c34b6c88617f37`.

## Resulting behavior

The fork builds standalone ShotClutch desktop and CLI executables, with distinct Windows MSI/portable identity and a ShotClutch data location. Existing `.fcproj` serialization, internal crate names and command IDs remain compatible. ShotClutch environment names take precedence over inherited aliases; existing FilmCraft user data is not moved. Native text branding and an original icon replace the removed wordmark dependencies, while upstream and retained-asset notices remain included.

Rust 1.95.0 and locked dependencies underpin repository-owned, pinned-action CI. Windows x64 packages are unsigned qualification artifacts. Windows, Linux and macOS checks include strict fixture tools, logged commands, GPU-adapter probes, library/binary/integration/benchmark-unit/doctest selections, plus platform builds and Linux WASM. A fresh Windows runner installs the MSI, exercises the installed CLI without oracle tools on its PATH, verifies output counts, uninstalls, and checks user-file preservation. No release is published.

## Review evidence

- Complete hosted CI passed at `d0b9a11d`: Windows 2,630 / Linux 2,646 / macOS 2,600 passing executions, with 12 explicitly ignored tests per OS. The GPU probe is reported separately. [Retained run](evidence/rollout/ci-d0b9a11d-1/run.json).
- Repaired source `a7fbcf2f` now has two independent green aggregates: [40834967572](https://github.com/damquan1001/shotclutch/actions/runs/40834967572) and [40860747696](https://github.com/damquan1001/shotclutch/actions/runs/40860747696), plus passing packaging lint. Both retain independent cold attempt-1 test/build results and successful package/install recoveries (attempts 4 and 2). Failed billing starts and install-only artifact mismatches remain historical evidence; these are not two fresh green attempts without retries. CI-002 is Done. [Final evidence](CI_RUNTIME_EXECUTION.md#two-run-ci-acceptance-retained-2026-11-01).
- The successful CI-produced `d0b9a11d` GUI passed isolated 1080p import/trim/undo/redo/save/reopen/export, independently verified at 50 video frames and 80,000 stereo sample frames. [Evidence](evidence/rollout/ci-d0b9a11d-1/native-core-1080/evidence.json).
- Both `369ce617` package sets passed local artifact inspection and the same exact-executable 1080p native workflow, with identical input/render hashes. Their hosted install jobs remain unexecuted because of the account block. Executable machine-code sections match; whole-file hashes differ in recorded linker timestamps/debug GUIDs, so byte-for-byte reproducibility is not claimed. [Detailed bindings and comparison](CI_RUNTIME_EXECUTION.md#external-billing-block-on-repeat-installation-jobs).
- The repaired-source run-40834967572 attempt-4 GUI also passed the isolated native 1080p workflow, and its MSI/ZIP inspection passed. [Exact-build evidence](evidence/rollout/ci-a7fbcf2f-recovery4/native-core-1080/verification-context.json). The second-PC manual return now identifies Windows 10 / i5-4690 / RX 570 / 24 GiB with matching kit hashes and reported PASS. Readable replacements now independently pass packaged-CLI project loading, full ProRes decoding and exact 50-frame / 80,000-stereo-sample counts at 1080p30. [Verified assessment](evidence/second-pc-readable-return-20261101/README.md). Disposable offline VM and measured performance gates remain separate.
- Exact artifacts, hardware/driver profile, all sustained-playback outcomes, benchmark rechecks, failures and reproduction commands are in [the execution record](CI_RUNTIME_EXECUTION.md). Large generated fixtures and binaries remain outside Git.
- The user-confirmed clean 0.13.0 MSI core workflow remains accepted historical evidence, distinct from exact-hash hosted/local reports. [Confirmation](evidence/clean-install-user-confirmation.md).

## Limits that remain visible

This is foundation work, not an M1–M5 feature implementation or v0.1 release qualification. Playback observations vary around the proposed drop-rate target; physical A/V drift remains unqualified. A separate Windows 10 machine has manual PASS evidence, and independent project/output verification now passes for the bounded core workflow. A strict offline consumer-VM lifecycle has not been performed for the exact new CI package. Raw `jobs.cancel` stops work but retains a partial output; MCP cancellation has separate cleanup semantics. The hosted Windows tests use pinned DXC; legacy FXC remains unqualified for consumers. 4K AV1 playback dropped most frames in the quick matrix.

These findings do not become passes through successful compilation or a successful small render. Keep the runtime/release gates open and preserve their measured reports.

## Owner-controlled promotion

Main, repository protection, schedules, billing, signing and release publishing have not been changed. Two independent repaired-source runs are now green. Review the branch and authorize promotion/settings changes separately; the runtime limits above remain open. Opening a PR or merging was excluded from the original implementation scope unless requested. The [proposed required-check value](CI_RUNTIME_EXECUTION.md#ci-003--proposed-required-check-value-not-applied) names `CI required` and `Packaging required` from GitHub Actions app 15368; preserve intervening owner settings and verify private-repository feature availability before applying it. New qualification schedules remain unapplied pending an observed budget and an appropriate runtime environment.
