# Retained M0 evidence

These files record execution on the Windows development machine. They do not establish clean-machine installation, remote CI success, other-platform runtime support, or the release performance budgets.

- `checks/`: final test and static-check logs, initial release build log, final package build/inspection logs, and synthetic comparison-guard results. Empty format/actionlint logs indicate successful commands with no diagnostics; command outcomes are recorded in the parent evidence report.
- `desktop/evidence.json`: actual native desktop command responses and independent frame/sample verification. Generated media and private authentication credentials are excluded.
- `packages/`: installer/ZIP inspection result, package build provenance, and hashes binding the packaged executables to the tested static binaries.
- `inventory/`: compiled command inventory and workspace/dependency inventory. An inventory is not a feature acceptance result.
- `upstream/` and `shotclutch/`: raw measured JSON, per-section process wall time and peak working set, stderr workload summaries, and manifests containing hardware, software, settings, fixture hashes and source revisions. Preparation/warmup runs are excluded from these retained measurements.
- `baseline-integrity.json`: SHA-256 verification of every archived upstream source file after builds; no upstream source repair was applied.
- `benchmark-executables.json`: hashes of the two benchmark executables used for the captures.
- `msi-wizard/`: the subsequent installer UX diagnosis, corrected MSI/ZIP hashes, artifact validation, build provenance and binary bindings. Earlier `packages/` records describe the initial package build; the parent evidence report identifies the current artifacts.

## Executed checks

Run commands from the repository root with Rust 1.95.0 and the MSVC toolchain. Independent FFmpeg/ffprobe oracles were available. The required-oracles setting prevents Windows media tests from passing through a missing-oracle skip.

```powershell
cargo fmt --all -- --check
cargo xtask layers
cargo xtask assets
cargo clippy --locked --release --workspace --all-targets -- -D warnings
cargo test --locked --release -p filmcraft-automation -p filmcraft-edit -p filmcraft-engine -p filmcraft-export -p filmcraft-format -p filmcraft-project -p filmcraft-time -p filmcraft-ui-egui --lib
cargo test --locked --release -p filmcraft-format --test fixtures
cargo test --locked --release -p filmcraft --bin shotclutch
cargo test --locked --release -p filmcraft-cli --bin shotclutch-cli
$env:FILMCRAFT_REQUIRE_ORACLES = '1'
cargo test --locked --release -p filmcraft-cli --test it
cargo test --locked --release -p filmcraft-ui-egui --example bench
target/m0/actionlint/bin/actionlint.exe -shellcheck=
python scripts/m0/desktop-smoke.py --bin-dir target/x86_64-pc-windows-msvc/release --output target/m0/desktop-smoke-final
```

The CLI unit log is the successful unit phase extracted from an earlier aggregate run; the subsequent integration phase in that earlier run exposed the renamed server-name expectation. The separate retained integration log supersedes it and includes both real-media editing/export workflows. No failed integration phase is counted as passed.

Package production used `packaging/windows/package.ps1 -Arch x64`, optional signing disabled, WiX 5.0.2, and static CRT. The final provenance-only package refresh used `-SkipBuild` with the unchanged tested binaries. `scripts/m0/validate-package.ps1 -Dist dist/release` inspected the resulting MSI and ZIP. See `packages/build-info.json` for the exact production and observed revisions; later test/harness/documentation commits do not imply a different shipped application binary.

Benchmark reproduction commands and clean-install procedure are in [QUALIFICATION.md](../QUALIFICATION.md). Their full source revisions are recorded in each manifest. Tests, fixture generation and builds were not run concurrently with the measured workloads. OS caches, thermals and background operating-system activity were uncontrolled; results describe this machine and this execution order.

Final metadata-only review also parsed the optional Windows signing helper as PowerShell and the new Unix metadata as XML. The signing description/URL now identify ShotClutch; no signing operation ran. The Unix metadata names the new executable and drops the removed upstream screenshot. Non-Windows distribution templates require further platform-specific adaptation and qualification, as recorded in the audit.
