# CI qualification helpers

`oracles.lock.json` fixes publisher versioned URLs and SHA-256 for Windows x64, Linux x64 and macOS ARM64. Windows uses Gyan 8.1.1; Linux/macOS use Martin Riedl 9.0.2. These are independent test generators/oracles, never application payloads. Different OS oracle versions do not establish comparable performance between OSes. Upgrade the lock through review and record new fixture hashes when changing generators.

`setup-oracles.py` verifies archives before extracting the selected executables, checks versions/encoders and exports strict discovery variables for subsequent Actions steps. It writes its manifest under `target/ci/oracles`. Publisher references: [Gyan 8.1.1](https://github.com/GyanD/codexffmpeg/releases/tag/8.1.1), [Martin Riedl builds and checksums](https://ffmpeg.martin-riedl.de/).

`run-checks.py` retains every executed command's actual status, wall time and log, including on failure. Standard runner checks and release compilation use separate jobs/disks. Only source dependency downloads are cached; compiled targets are rebuilt. `test_helpers.py` verifies corrupt-download rejection and failing-child status/log preservation.

`package-smoke.py` has separate prepare/exercise/verify modes. It generates a fixture, runs the packaged CLI without FFmpeg/ffprobe on the application child's PATH, trims/undoes/redoes and saves/reopens, exports ProRes, then independently checks 50 video frames and 80,000 stereo sample frames. The install job runs exercise against the installed executable before uninstalling it. The parent oracle phase is distinct from app execution; this is not a claim of a completely bare offline consumer OS.

`scripts/m0/install-check.ps1` refuses ordinary local execution. Use it only on an ephemeral Actions runner or with `-DisposableEnvironment` in a deliberately disposable VM. It preserves installation/uninstallation diagnostics and checks that a user-created sentinel survives uninstall. Do not run it against the developer's installed application.

Bootstrap triggers include `codex/m0-foundation`. All release artifacts are unsigned qualification artifacts. No workflow publishes a GitHub release, uses signing credentials or registers a self-hosted runner. Actual cloud run URLs and runtime conclusions belong in `docs/m0/EVIDENCE.md` and the development tracker.
