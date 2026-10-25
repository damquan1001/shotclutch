# ShotClutch

ShotClutch is a standalone, local-first video editor built from [FilmCraft](https://github.com/storytold/filmcraft). This branch establishes the M0 foundation; the agent, semantic media, reviewed Smart Rough Cut, and MotionScene capabilities described in the specifications are planned work.

## Build on Windows x64

Install the MSVC C++ build tools and Windows SDK, Rust via rustup, and .NET with WiX 5.0.2 for MSI packaging. The checked-in toolchain selects Rust 1.95.0. Build dependencies are downloaded on first use; ordinary editing does not require an external application suite.

```powershell
cargo build --locked --release -p filmcraft -p filmcraft-cli
./target/release/shotclutch.exe --empty
./target/release/shotclutch-cli.exe --version
cargo test --locked -p filmcraft-cli
./packaging/windows/package.ps1 -Arch x64
./scripts/m0/validate-package.ps1 -Dist dist/release
```

Internal Cargo package names remain `filmcraft-*` for upstream maintenance. Projects retain the `.fcproj` extension and `filmcraft.project` format tag. The inherited workspace version is retained; the commit identifies the fork build. Windows x64 is the first qualification target. Other platform compile checks do not establish runtime support.

`SHOTCLUTCH_DATA_DIR` selects the data directory, followed by the legacy `FILMCRAFT_DATA_DIR` alias. Otherwise a portable installation uses `data` beside its executable, and an installed Windows application uses `%APPDATA%\ShotClutch`. Existing FilmCraft data is never moved automatically. Runtime `SHOTCLUTCH_*` environment overrides take precedence over corresponding `FILMCRAFT_*` aliases where an inherited override exists; development-only test controls may retain their inherited names.

See the [specifications](docs/01_PRODUCT_VISION.md), [roadmap](docs/16_MVP_ROADMAP.md), [live tracker](docs/17_DEVELOPMENT_TRACKER.md), and [M0 audit](docs/m0/FOUNDATION_AUDIT.md). Historical FilmCraft documentation is retained in `docs/filmcraft_original_docs`; its feature claims and old paths are historical context.

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
