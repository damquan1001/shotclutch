# M0 qualification procedure

## Reproducible comparison

Archive inherited revision `3a5f217b` into `target/m0/upstream` using `git archive`; retain the archive hash and full revision. This avoids user-state changes and includes upstream assets required to compile. No build repairs are assumed. If a repair is necessary, keep its patch and classify the result as a repaired baseline.

Run these from the repository root, one after another, with no other builds or workloads running during measurement:

```powershell
./scripts/m0/benchmark.ps1 -Checkout target/m0/upstream -Revision <full-upstream-hash> -Label upstream -Quick
./scripts/m0/benchmark.ps1 -Checkout . -Revision <milestone-commit> -Label shotclutch -Quick
```

The runner calls each existing section directly, so the untouched Windows benchmark needs no Unix launcher repair. The new native orchestrator can also run `cargo run --locked --release -p filmcraft-ui-egui --example bench -- --quick --cpu --hw off --out target/m0/native-bench`.

Record software decode/CPU runs as such. Quick runs establish reproducible baselines; they do not qualify the release budgets in document 14. Run full scenarios and qualified GPU/hardware paths before making performance claims. The manifest records raw section results, fixture SHA-256, repeat count, process peak working set, runtime/toolchain and hardware. Null telemetry means unavailable. Fresh processes do not imply a cold OS filesystem cache. Do not compare quality/backend changes as equivalent speedups.

## Corpus coverage

| Fixture | M0 source | Remaining qualification |
|---|---|---|
| B01 baseline edit | Existing 1080p playback/edit fixtures | Broader effects and GPU qualification |
| B02 long GOP | Existing seek suite and CLI camera fixture | Profile/driver matrix |
| B03 intraframe | Existing ProRes decode cases | Platform bandwidth matrix |
| B04 mixed rate/VFR | Existing CLI phone/camera workflow tests | Larger timestamp/proxy corpus |
| B05 large timeline | Existing timeline/project benchmark (1,000 clips and stress variants) | Long soak and larger workloads |
| B06 layered | Existing playback scenarios | Full 8–10 video / 8–16 audio qualification |
| B07 speech | Planned M2 | Model and editorial-quality corpus |
| B08 scenes | Planned M4/M5 | Determinism and resource qualification |
| B09 semantic library | Planned M3 | Labeled 10,000-segment corpus |
| B10 failures | Existing tests provide partial coverage | Full fault-injection/device/full-disk matrix |

FFmpeg produces generated fixtures and acts as an independent output oracle. Save the generated corpus hashes; regenerate only when the fixture version changes. For required workflow tests, set `FILMCRAFT_REQUIRE_ORACLES=1` so missing tools cannot silently pass.

## Clean Windows installation gate

Use a fresh Windows x64 VM or Windows Sandbox with no FilmCraft/ShotClutch, Rust, FFmpeg, model runtime, or other editing suite installed. Record OS/build, virtual hardware/graphics, package hashes, and whether the environment can reproduce playback.

1. Copy the MSI, portable ZIP, generated input media and their manifests into the VM. Disable network after staging inputs.
2. Install the unsigned MSI, acknowledging Windows' unsigned-development-package prompt. Confirm the product/shortcut is ShotClutch and the installed folder contains both executables and `legal/` files.
3. Launch the desktop. Import media, create a sequence, split/trim linked A/V, undo/redo, save, close/reopen, and export a short range. Check that playback and export work without FFmpeg or an external editor installed.
4. Validate the delivered render independently outside the VM with ffprobe/ffmpeg: expected frames, exact intended sample count, duration, source frames and A/V markers. Preserve screenshots and logs.
5. Confirm preferences/recovery are in `%APPDATA%\ShotClutch`, and existing FilmCraft data is untouched. Separately test the ZIP: portable marker selects its adjacent `data/` directory.
6. Uninstall the MSI; verify it removes only its installed product and leaves user-created media/projects intact. Record residual user-data behavior rather than deleting user data during the test.

Archive the procedure/results and link them from the tracker. Local ZIP/MSI inspection or an unpacked launch does not satisfy this clean-machine gate. M0 remains open until its required evidence exists.
