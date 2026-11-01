# ShotClutch

ShotClutch is a standalone, local-first video editor built from retained open-source components; their copyright and license notices are in `NOTICE` and `legal/` in packaged builds. Development lineage is recorded in [the upstream policy](docs/m0/UPSTREAM_POLICY.md). M0 foundation is complete; the agent, semantic media, reviewed Smart Rough Cut, and MotionScene capabilities described in the specifications are planned work.

## Build on Windows x64

Install the MSVC C++ build tools and Windows SDK, Rust via rustup, and .NET with WiX 5.0.2 for MSI packaging. The checked-in toolchain selects Rust 1.95.0. Build dependencies are downloaded on first use; ordinary editing does not require an external application suite.

```powershell
cargo build --locked --release -p shotclutch -p shotclutch-cli
./target/release/shotclutch.exe --empty
./target/release/shotclutch-cli.exe --version
cargo test --locked -p shotclutch-cli
dotnet tool install wix --tool-path target/m0/tools --version 5.0.2
./packaging/windows/package.ps1 -Arch x64
./scripts/m0/validate-package.ps1 -Dist dist/release
```

Internal Cargo packages and imports use `shotclutch-*` / `shotclutch_*`. New projects use `.scproj` and the `shotclutch.project` format tag. Existing `.fcproj` files remain readable; saving a legacy project requires Save As to a new `.scproj` destination and leaves its source untouched. The inherited workspace version is retained; the commit identifies the fork build. Windows x64 is the first qualification target. Other platform compile checks do not establish runtime support.

Double-click the MSI for the setup wizard, choose the installation folder and finish installation. The final page offers **Launch ShotClutch**; the app is also available from the Start menu. Re-running an installed MSI opens maintenance/repair. Automated installers can use `msiexec /i <package.msi> /qn`; quiet installs never launch the app. Packaging downloads the pinned WiX UI extension 5.0.2 into the checkout's ignored `.wix/` cache on first use.

`SHOTCLUTCH_DATA_DIR` selects the data directory, followed by the legacy `FILMCRAFT_DATA_DIR` alias. Otherwise a portable installation uses `data` beside its executable, and an installed Windows application uses `%APPDATA%\ShotClutch`. Existing user data is never moved automatically. Runtime `SHOTCLUTCH_*` environment overrides take precedence over corresponding `FILMCRAFT_*` aliases where an inherited override exists; development-only test controls may retain their inherited names.

See the [specifications](docs/01_PRODUCT_VISION.md), [roadmap](docs/16_MVP_ROADMAP.md), [live tracker](docs/17_DEVELOPMENT_TRACKER.md), and [M0 audit](docs/m0/FOUNDATION_AUDIT.md). Historical upstream documentation is retained in `docs/filmcraft_original_docs`; its feature claims and old paths are historical context.

## Checks

```powershell
cargo fmt --all -- --check
cargo xtask layers
cargo xtask assets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --lib
```

FFmpeg and ffprobe generate fixtures and independently validate output in tests and benchmarks. They are not runtime requirements of the shipped editor. Local models are optional and are not included in M0 packages.

Licensed under MIT OR Apache-2.0. Upstream copyrights and contributor credits are preserved in [NOTICE](NOTICE), the license files, and [ATTRIBUTION.md](ATTRIBUTION.md).
