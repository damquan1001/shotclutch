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

Follow-up source `7cfbde41` started [CI 40358941028](https://github.com/damquan1001/shotclutch/actions/runs/40358941028); [packaging lint 40358941007](https://github.com/damquan1001/shotclutch/actions/runs/40358941007) passed. Review while compilation was running found that assigning an empty `WGPU_BACKEND` on non-Windows selected no backends in wgpu's parser. That run is superseded rather than accepted as complete coverage. The corrected workflow names explicit Direct3D 12/Vulkan/Metal profiles and requires an adapter probe on every hosted OS, preventing a green result based on silent GPU-test skips.

## Runtime preparation and provisional observations

- Added a backend/section selector to the existing benchmark wrapper, preserving default software/CPU behavior and using the selected mode for both preparation and measurement. Empty measured sections fail. The inherited export section couples hardware encode/GPU export selection to `--hw`; that limitation is stated in each new manifest.
- A development-binary 1080p H.264 GPU-path smoke produced 96 shown / 0 dropped frames in each of three quick repeats. The first repeat reports 110 hardware-decoded frames and one session; later repeats reuse cached frames. These small runs do not establish sustained performance.
- A 15-second development-binary native desktop observation selected **NVIDIA GeForce RTX 3060 Laptop GPU, Vulkan, NVIDIA 555.97**, with Media Foundation decoding. It reported 459 shown / 0 dropped frames, 1,432 audio callbacks, no underruns/missing frames, and about 0.023 seconds difference between observed playhead and wall-clock progress. Preview resolution was **1/2** in this initial sampler check; the longer candidate run is configured explicitly. This is application telemetry, not physical loopback A/V drift measurement.
- Generated a separate 1,802-second 1080p30 H.264/AAC stereo fixture with brief quiet markers every minute. The large generated file stays under ignored `target/qualification`; preserve its manifest/hash with the actual measured run.

### Measured CI candidate

The exact CI-produced `5f697163` desktop executable (SHA-256 `e82cc4cc97178ff712c64bca471fc6666f66cac64c1ff1fc7f1b277396750d0a`) completed a **30-minute Full-resolution native playback observation** on the Windows 11 / Ryzen 7 4800H / RTX 3060 Laptop profile. It used Vulkan/NVIDIA 555.97 and Media Foundation hardware decoding, with 53,997 hardware-decoded frames, no software fallback, 53,868 shown frames and **99 dropped frames (0.1834%)**. Audio reported 168,611 callbacks, zero underruns and zero missing frames. Peak working set was 952,504,320 bytes (0.887 GiB). It remained alive through controlled shutdown. [Raw counters and summary](evidence/rollout/ci-5f697163-1/full-soak/summary.json), [samples](evidence/rollout/ci-5f697163-1/full-soak/samples.jsonl), [fixture generator/hash](evidence/rollout/runtime/soak-fixture.json).

This Full-resolution observation exceeds document 14's proposed 0.1% drop target. It characterizes an achieved mode and an outstanding performance limitation, not a performance-budget pass. Playhead progress minus observed wall-clock progress was -1.376 seconds over the run; this is internal telemetry, **not measured physical A/V drift**. Physical loopback remains unavailable. The generated 2.57 GB fixture stays outside Git; its SHA-256 is `7637c37e4c487b5a2e3e7759a5d9aa11b8c789dd41bc102ee62279e86dde091b`.

The seven-section auto-decode/GPU quick matrix completed all sections (94 rows). Hardware counters establish actual use where reported; repeated cached frames do not count as fresh hardware decoding. 4K AV1 playback dropped most frames and remains unqualified. The inherited benchmark does not expose the active adapter model; the separate native observation above does. [Raw matrix](evidence/rollout/runtime/measurements/gpu-auto-matrix/manifest.json).

The same exact CI executable then completed **ten minutes at Half (1/2) preview resolution**: 18,006 shown / 1 dropped frame (**0.0056%**), 56,271 audio callbacks, zero underruns/missing frames, 925,659,136-byte peak working set (0.862 GiB), 18,037 Media Foundation hardware frames and no fallback. Internal playhead-minus-wall progress was +0.022 seconds. [Summary](evidence/rollout/ci-5f697163-1/half-soak/summary.json), [raw samples](evidence/rollout/ci-5f697163-1/half-soak/samples.jsonl). This meets the proposed drop-rate threshold for this observation, not the whole release matrix. The Full-resolution miss remains distinct. Renderer selection was NVIDIA Vulkan, while decoder logs identify the AMD Radeon Direct3D 11 hardware device: this is a **hybrid adapter path**, not an all-NVIDIA pipeline. Physical A/V drift remains unavailable in both observations.

Each native run used a fresh application data directory. Fixture hash verification read the media before playback; OS caches were not flushed, so these are not cold-media measurements. Default decoder/frame-cache settings and actual counters remain in the samples. No heavy local build, fixture generation or other qualification workload ran concurrently with measured playback; operating-system/background activity remained uncontrolled.

[Fresh primary profile](evidence/rollout/primary-profile.json), captured after the initial observations: Windows 11 Home Single Language 10.0.26200, Ryzen 7 4800H (8 cores / 16 threads), 33,737,945,088 bytes physical memory, AMD driver 31.0.21923.1000, NVIDIA driver 32.0.15.5597 (555.97) and Turbo power scheme. Storage model/capacity is captured without serial numbers. CPU thermals, physical A/V drift and per-process GPU allocation remain unavailable.

Three interleaved upstream/fork software/CPU decode pairs used U/F, F/U, U/F order. Earlier slower decode results were **not reproduced**: per-pair median FPS changes for H.264 were +17.2%, +2.0%, +23.7%; HEVC +4.2%, +23.1%, +16.6%; VP9 +3.2%, +1.8%, +30.6%; AV1 +4.8%, +5.7%, +13.6%; ProRes +0.9%, +3.6%, +0.8%. This does not establish a speedup or root cause: thermal, clock, OS-cache and scheduling conditions were uncontrolled, and runs are short. The original slower baseline remains intact. [Raw pairs](evidence/rollout/runtime/interleaved/upstream-pair1/manifest.json). Heavy local builds/tests and native soak did not overlap these measurements.

Physical loopback, additional lifecycle scenarios, a successful final-CI candidate and a second hardware profile remain separately reported coverage. The first candidate's measured runtime evidence is bound to its own revision rather than silently relabeled as a later build.

### Native lifecycle observations on `5f697163`

[Reproduction and raw command results](evidence/rollout/ci-5f697163-1/lifecycle/summary.json) use a copied generated short fixture, a private application data directory and a child PATH containing only the package directory and Windows System32. Oracle override variables were removed from the child. No installed application, user project or original fixture was moved or overwritten.

- Fifty trim/undo/redo/undo cycles preserved the expected 59/60-frame durations. The saved project survived a controlled quit, reopened with one missing copied media file, relinked to the moved copy and reopened online at 60 frames.
- The 1,802-second source project saved/reopened at 54,060 frames. A live ProRes export was cancelled in 0.578 seconds and reported `error: cancelled`.
- The raw `jobs.cancel` command **left a 6,753,640-byte partial output**. The harness expected removal and therefore records a failure, not an all-pass lifecycle result. Source inspection shows cleanup belongs to the MCP `notifications/cancelled` wrapper (`crates/automation/src/long_job.rs`), while raw `jobs.cancel` only signals the export worker (`crates/engine/src/commands.rs`). These transports must not be reported as having identical cleanup semantics. Partial-output handling remains an explicit export/lifecycle limitation for subsequent qualification; no M1 export architecture was added here. The harness terminated only its own child after the failed expectation.

The first attempt at this observation used an absent downloaded fixture path and stopped before launching an app. The corrected attempt above used `target/ci/local-fixtures/source.mp4`; that preparation error is not classified as a product failure.

### Evidence interpretation

- [Downloaded candidate binding](evidence/rollout/ci-5f697163-1/candidate.json) verifies the outer artifact archive against GitHub's reported SHA-256 before extraction, then records inner MSI/ZIP/executable hashes and clean commit provenance. Test-only DXC DLLs are absent from the portable payload. [Download reproduction](evidence/rollout/download-candidate.py).
- Raw `Cargo.lock` SHA-256 differs between Windows (`3a05488c…`) and Unix (`de03a57a…`) because this Windows checkout uses CRLF. [Normalization proof](evidence/rollout/lockfile-line-endings.json) confirms that LF-normalized bytes equal the Git blob. This is not a dependency-resolution change.
- First-run observed lane durations were approximately 40m33s Linux checks, 28m45s macOS checks, 62m54s Windows checks, 25m59s Windows packaging and 40s install lifecycle. The timing API reported run duration 3,785,000 ms but zero billable duration for all OS buckets; that response is not treated as proof of zero cost or remaining allowance. Account plan/budget remains unavailable through the inspected metadata. No paid runners, signing, release publishing or schedules were enabled.
- Superseded `7cfbde41` completed macOS release compilation in 565.6 seconds before cancellation; [retained source-bound build report](evidence/rollout/ci-7cfbde41-1/release-macos/build/summary.json). All cancelled lanes remain incomplete. Corrected CI `d44a20e6` is running at [40360661267](https://github.com/damquan1001/shotclutch/actions/runs/40360661267); [packaging lint 40360661595](https://github.com/damquan1001/shotclutch/actions/runs/40360661595) passed.

On 2026-10-27 (Asia/Bangkok), `d44a20e6` passed Linux release/WASM compilation, macOS release compilation, all three required adapter probes and the macOS workspace library tests. macOS then failed `graphics::tests::windows_default_leaves_out_gl`: the globally forced Metal profile changed eframe's `NativeOptions::default`, while that test intentionally checks its unmodified default list. This is CI environment leakage, not a product-source repair. [Retained diagnostics](evidence/rollout/ci-d44a20e6-1/checks-macos/evidence/checks/summary.json) identify the failure; the probe names the **Apple Paravirtual device / Metal** adapter.

The runner repair uses CI-specific profile variables, maps them to `WGPU_*` only for library tests/the GPU probe, and records the applied overrides per command. Desktop/CLI default tests receive no forced backend. All **four helper regression guards** pass locally, including a new environment-isolation check; actionlint and Python syntax pass. Remaining jobs on the known-failing source are superseded after retaining completed evidence. No failed/cancelled aggregate is counted toward the two-green-run gate.

Current isolated-profile source: **`d0b9a11dc4ff25f93e855700ca33df865c5532c4`**. [CI 40365205399](https://github.com/damquan1001/shotclutch/actions/runs/40365205399) and [packaging lint 40365205707](https://github.com/damquan1001/shotclutch/actions/runs/40365205707) are the next acceptance candidates. Do not advance CI-002 until two full green executions of one source revision exist. Main promotion and CI-003 remain separate from branch-level stabilization.

### Current candidate results (`d0b9a11d`, attempt 1)

**The complete aggregate passed.** Packaging lint, macOS full checks (**2,600 passing executions / 12 explicitly ignored**), Linux full checks (**2,646 / 12**), Windows full checks (**2,630 / 12**), both non-Windows release builds, Linux WASM and Windows package/install/render/uninstall passed. Each OS also passed its separate required adapter probe. Compiled targets were rebuilt; source-download cache state is retained per lane. Linux's adapter is **llvmpipe / Vulkan**, a CPU software implementation; macOS uses **Apple Paravirtual / Metal**; Windows uses **Microsoft Basic Render Driver / Direct3D 12** with pinned DXC. These establish hosted correctness coverage, not physical GPU performance. [macOS commands](evidence/rollout/ci-d0b9a11d-1/checks-macos/evidence/checks/summary.json), [Linux](evidence/rollout/ci-d0b9a11d-1/checks-ubuntu/evidence/checks/summary.json), [Windows](evidence/rollout/ci-d0b9a11d-1/checks-windows/evidence/checks/summary.json), [run/jobs/artifacts](evidence/rollout/ci-d0b9a11d-1/run.json), [ignored-test inventory](evidence/rollout/ci-d0b9a11d-1/ignored-tests-macos.json).

[Current downloaded candidate](evidence/rollout/ci-d0b9a11d-1/candidate.json) passed the outer archive digest check and local MSI/ZIP validation. MSI SHA-256 is `ca4aec9da86d6344f8792bb807fc4e21800f099c501449ca42dc00ac0f1877a9`, ZIP `77391268f8e61c7ce2e01dda035c9e83037aa2c18b2705ada523dc53304845e3`, GUI executable `bbbccc48f9b2f4345b42782c7ee55eec06091d2fb973e7c6d53a230f9f852504`. The [fresh hosted install](evidence/rollout/ci-d0b9a11d-1/install-evidence/install/summary.json) confirms the same MSI, install/uninstall exit 0 and preserved user sentinel. Installed CLI output has [50 frames / 80,000 stereo sample frames](evidence/rollout/ci-d0b9a11d-1/install-evidence/install/workflow/verification.json).

The downloaded GUI then passed isolated native workflows at 320×180 and **1920×1080**, with the app child restricted to its package directory and Windows System32, oracle overrides removed, and parent-only generation/verification. Both trim/undo/redo/save/reopen/export workflows produced 50 video frames and 80,000 stereo sample frames. Independent 1080p inspection also confirms 1920×1080, 30/1 fps, 48 kHz stereo. [1080p command evidence](evidence/rollout/ci-d0b9a11d-1/native-core-1080/evidence.json), [fixture/oracle context](evidence/rollout/ci-d0b9a11d-1/native-core-1080/verification-context.json). This does not represent a local MSI installation; the developer's installed application was preserved.

Sustained observations on the same Windows 11 profile and fixture:

| Candidate / preview | Duration | Shown / dropped | Drop rate | Audio underruns / missing frames | Peak working set |
|---|---:|---:|---:|---:|---:|
| `5f697163` / Full | 30 min | 53,868 / 99 | 0.1834% | 0 / 0 | 0.887 GiB |
| `5f697163` / Half | 10 min | 18,006 / 1 | 0.0056% | 0 / 0 | 0.862 GiB |
| `d0b9a11d` / Full | 30 min | 53,976 / 35 | 0.0648% | 0 / 0 | 0.874 GiB |
| `d0b9a11d` / Half | 10 min | 17,975 / 32 | 0.1777% | 0 / 0 | 0.858 GiB |

The latest Full observation meets the proposed 0.1% threshold; the latest Half observation misses it. Neither replaces the earlier result. No Windows playback optimization was implemented between these candidates, and conditions were not controlled enough to establish a cause or a stable performance guarantee. **Release performance qualification remains open.** Current Full hardware decoding reports 54,041 frames, one Media Foundation session, zero fallback/software frames; Half reports 18,037. Both selected NVIDIA Vulkan rendering and AMD hardware decoding. Internal clock-minus-wall differences were +0.0233 s (Full) and +0.0037 s (Half); neither is physical A/V drift. [Full raw observation](evidence/rollout/ci-d0b9a11d-1/full-soak/summary.json), [Half](evidence/rollout/ci-d0b9a11d-1/half-soak/summary.json), [sampled drop increments](evidence/rollout/ci-d0b9a11d-1/half-soak/drop-observations.json).

The current candidate also completed 50 trim/history cycles at 1080p, offline/relink/reopen and a 54,060-frame long-project save/reopen. Active export cancellation took 1.032 seconds, returned `cancelled` after 16 frames, and retained the same 6,753,640-byte partial file behavior. Its report is explicitly `completed_with_limitations`, despite the reproduction script's generic completion message. [Lifecycle evidence](evidence/rollout/ci-d0b9a11d-1/lifecycle/summary.json). Physical loopback, strict offline consumer-VM lifecycle, a second physical machine and broader fixture/runtime qualification remain outstanding. Do not mark those lanes Done from these observations.

### Measured headroom and two-run repeatability

The successful Windows check job took **117.22 minutes against 120**, including 14m29s for the separate cold GPU probe and repeated feature-graph compilation across the selected test commands. That is too little runner-variation margin for a stable gate. Commit **`369ce617891c42fcf9c9307795339283e3d68dc4`** raises only Windows checks to 180 minutes; all workloads, pinned tools, cache policy and other job limits are unchanged. Its concurrency group separates push and manual events so two independent same-commit qualification executions can coexist. There are **no product/dependency/packaging source changes** from the successfully tested `d0b9a11d` candidate; runtime numbers remain bound to that original executable rather than relabeled.

| Qualification source | Event | Run | Interim state |
|---|---|---|---|
| `369ce617` | Push | [40403013899](https://github.com/damquan1001/shotclutch/actions/runs/40403013899) | Failed: Windows scheduling assertion; install startup blocked |
| `369ce617` | Manual | [40403015233](https://github.com/damquan1001/shotclutch/actions/runs/40403015233) | All checks/builds passed; install startup blocked |
| `369ce617` | Packaging lint | [40403013833](https://github.com/damquan1001/shotclutch/actions/runs/40403013833) | Passed |

Default-branch inspection confirmed the existing CI file already supports `workflow_dispatch`; dispatching the milestone branch required no main-branch change. Both runs resolve the identical full source SHA above. Both rebuild compiled targets on fresh ordinary hosted runners; no paid/larger runners or self-hosted registration were added. CI-002 remains open until both aggregate results and artifacts have been retained. The earlier successful complete `d0b9a11d` run remains valid evidence, but is not miscounted as a second execution of `369ce617`.

### External billing block on repeat installation jobs

Both `369ce617` runs built their Windows packages, but GitHub refused to start the fresh install jobs. Each failed with **zero executed steps** and an annotation about recent failed account payments or a spending limit. This does not identify which account condition applies. [Push startup annotation](evidence/rollout/ci-369ce617-push/billing-annotation.json), [manual annotation](evidence/rollout/ci-369ce617-manual/billing-annotation.json). No installer step ran in those jobs, so these are account/infrastructure blocks rather than MSI test results.

The account owner was asked to review GitHub Settings → Billing & plans. No payment, budget, repository visibility or spending-limit change was made. All already-running checks have finished and their reports are retained below; no new hosted retry will start until the billing block is resolved. The two-green aggregate acceptance gate remains **outstanding**.

Retry care: package/fixture artifact names include `github.run_attempt`. After the warning is resolved, rerun the **package producer and its dependent install/aggregate jobs**, or rerun the whole workflow. Rerunning only an install job on a new attempt would request a new-attempt artifact that the previous package job did not produce. Preserve the successful cold check reports and both startup failures before retrying; do not manufacture an aggregate pass or change test selection to bypass the account block.

Both already-produced `369ce617` package sets were independently downloaded, verified against their own GitHub artifact digests and validated locally. Both exact GUI executables passed the isolated native 1080p trim/history/save/reopen/export workflow, with independently checked 1920×1080 at 30/1 fps, 50 video frames and 80,000 stereo sample frames at 48 kHz. Input and rendered-output hashes match between the two local executions. No MSI was installed on the development host. [Push artifact binding](evidence/rollout/ci-369ce617-push/candidate.json), [manual binding](evidence/rollout/ci-369ce617-manual/candidate.json), [push native verification](evidence/rollout/ci-369ce617-push/native-core-1080/verification-context.json), [manual verification](evidence/rollout/ci-369ce617-manual/native-core-1080/verification-context.json).

Build repeatability is **not claimed as whole-file byte reproducibility**. The two GUI binaries differ in 20 bytes and the CLI binaries in 24 bytes. Inspection confines every differing byte to COFF/debug timestamps and the CodeView debug GUID; `.text` and all other sections except `.rdata` match. [Binary comparison](evidence/rollout/repeat-binary-comparison.json). Locked build procedures, identical machine-code sections and matching functional output establish bounded repeat evidence; deterministic linker/package metadata remains an explicit future improvement rather than a fabricated identical hash.

### Completed repeat results and test repair

Both attempt-1 aggregates finished with failure. The manual run passed Windows **2,630**, Linux **2,646** and macOS **2,600** test executions, with 12 explicitly ignored per OS; adapter probes, release builds, Linux WASM and Windows packaging passed. Its only non-aggregate failure was the install job startup block. [Manual run and lane summaries](evidence/rollout/ci-369ce617-manual/summary.json), [jobs](evidence/rollout/ci-369ce617-manual/jobs.json).

The push run also passed Linux/macOS checks, all adapter probes, release builds/WASM and packaging. Windows stopped during workspace library tests: `determinism_tests::max_threads_caps_the_threads_an_export_runs_on` observed three participating workers and failed its `threads > 3` assertion. The preceding eight-thread pool assertion passed. This is a scheduling-dependent test expectation: Rayon need not involve four workers for a short export on a busy two-core runner. It is separate from the install startup block. The failed export group had 126 passes / one failure; the completed Windows command groups total 1,374 passes / one failure / seven ignored, and later commands did not run. [Push summary](evidence/rollout/ci-369ce617-push/summary.json), [failure log](evidence/rollout/ci-369ce617-push/checks-windows/evidence/checks/05.log).

The test repair checks that the uncapped export runs on the eight-thread pool with a nonzero participant count bounded by that pool. It preserves the one-/three-thread cap assertions and the separate same-file and never-adds-threads tests. No production export behavior, workload, dependency, packaging source or GPU profile changes. Repair commit `7d32d7aebabf0c71fae59210ab1e041d029db063` passed formatting, all 13 export determinism tests, 20 two-CPU repetitions of all four thread-cap tests (80 passing executions), and export all-target Clippy with warnings denied. Local validation and its exact source binding are retained in [test-repair evidence](evidence/rollout/export-thread-test-repair/validation.json). The repair requires new full hosted qualification on its own source revision after the account block is resolved; neither failed aggregate counts as a green repeat.

Repair/evidence commits use `[skip ci]` while the account block is unresolved, to avoid starting new jobs. A skipped push is not acceptance evidence. Once the owner resolves CI-BILLING-001, run two complete push/manual or manual/manual executions of the same repaired revision, including their package producers and fresh install jobs. The preserved `369ce617` artifacts and native reports remain bound to their original revision; their measured results are not relabeled as repaired-build results.

## CI-003 — proposed required-check value (not applied)

After green repeat runs and an authorized review/promotion, the proposed `required_status_checks` value is:

```json
{
  "strict": true,
  "contexts": [],
  "checks": [
    {"context": "CI required", "app_id": 15368},
    {"context": "Packaging required", "app_id": 15368}
  ]
}
```

The observed `Packaging required` check is supplied by GitHub Actions app 15368. Distinct aggregate names avoid the former ambiguous `required` contexts. PR triggers are unfiltered, so documentation-only PRs still produce required checks. The proposal binds the supplier app and requires an up-to-date branch; it does not select unrelated review/bypass/admin policies. Re-read existing protection before applying changes and preserve any intervening owner configuration. Private-repository protection also depends on the account plan, which the inspected API did not disclose. [GitHub protection API](https://docs.github.com/en/rest/branches/branch-protection). Main remains unchanged and unprotected; no policy, merge, PR, schedule or paid plan has been applied by this rollout.

## Budget extension and repaired-source qualification (2026-10-29)

The owner reported an extended GitHub Actions budget and authorized continuing CI. No account billing setting was changed by Codex. Repaired source `a7fbcf2ffba3156bef1e175d30880670d7ff3bfc` is being qualified through two sequential complete manual runs; the workflow cancels overlapping manual runs on one branch, so the second dispatch waits for the first complete success. Failures stop the sequence for diagnosis. Both executions must retain this same source revision; no branch promotion or release publishing is included.

First full run: [40834967572](https://github.com/damquan1001/shotclutch/actions/runs/40834967572). Packaging lint: [40834971676](https://github.com/damquan1001/shotclutch/actions/runs/40834971676). The six initial CI jobs started successfully; fresh-install startup is checked after package production rather than inferred from initial startup. This is running work, not acceptance evidence of completion.

[Sequence state](evidence/rollout/ci-a7fbcf2f-sequence/state.json) retains dispatched IDs and actual results. The local supervisor retains completed run/job/artifact/timing metadata and lane logs, and dispatches the second run only after a green first run. The complete CI repeat gate and existing runtime limitations remain open until their required evidence exists. No third run or automatic retry is authorized by this two-run sequence.

### Attempt-4 package/install recovery

Run 40834967572 passed all three OS test lanes and release builds, including Windows packaging, in attempt 1. The original install job was blocked at startup by the provider billing annotation. Later install-only retries requested attempt-2/3 artifacts, although only attempt-1 package/fixture artifacts existed. Attempt 3 therefore failed before MSI execution with `Artifact not found`; this was not an installer test failure.

On user authorization, the package producer (job 123357686877) and its dependent jobs were rerun together using `gh run rerun 40834967572 --repo damquan1001/shotclutch --job 123357686877`. Attempt 4 is rebuilding packages, with previously successful test/build lanes preserved. Its reports are retained separately under `ci-a7fbcf2f-recovery4`, preserving attempt-1 evidence. The second full same-source run remains conditional on successful recovery; no install-only retry or unrelated full rebuild was started. Acceptance remains pending actual results, not the dispatch itself.

### Recovery passed; second full run dispatched

Run **40834967572, attempt 4, passed**. Successful cold test/build jobs from attempt 1 remain preserved (Windows 2,630 / Linux 2,646 / macOS 2,600 passing executions; 12 explicitly ignored per OS). Attempt 4 rebuilt the Windows package and passed fresh hosted MSI installation, installed-CLI editing/export, independent verification of 50 video frames / 80,000 stereo sample frames, uninstallation, and preservation of the user sentinel. [Install summary](evidence/rollout/ci-a7fbcf2f-recovery4/install/install/summary.json), [output verification](evidence/rollout/ci-a7fbcf2f-recovery4/install/install/workflow/verification.json), [attempt-4 jobs](evidence/rollout/ci-a7fbcf2f-recovery4/jobs.json). This is a successful combined-attempt run, not four independent green runs or a strict offline consumer environment.

MSI SHA-256: `34cfe7c13c873a94909ff5268d4ab99efafd9e9b762e6e0f94c6bc6d67cadf65`. Packaging lint 40834971676 also passed at the same repaired revision. The original billing block is now resolved for observed job execution; failed prior attempts remain retained as history.

The supervisor dispatched **[second full run 40860747696](https://github.com/damquan1001/shotclutch/actions/runs/40860747696)** on the identical `a7fbcf2ffba3156bef1e175d30880670d7ff3bfc` source. It rebuilds compiled targets and exercises all matrix/build/package/install lanes. The two-run acceptance gate remains open until this independent run completes successfully and its reports are retained. Existing runtime/performance/physical A/V/second-machine limits remain unchanged.

The recovered artifact download for additional local exact-package verification was started but has not completed; no local GUI pass is claimed for this new package. Hosted install/output verification above is complete. Earlier exact-package native reports retain their original hashes and revisions.

### Second-run installation recovery (2026-11-01)

The owner reported Actions usage cleared and authorized continuing the second validation. Run 40860747696 had already passed all three OS checks and release builds at the repaired source; its install job 123418543560 had zero steps and the same provider billing/spending-limit annotation. The package producer 123403045442 and dependent jobs were rerun together, preserving successful cold checks and matching package/fixture names to install attempt 2. Attempt-1 reports remain in `ci-a7fbcf2f-repeat2`; recovery evidence is retained separately in `ci-a7fbcf2f-repeat2-recovery2`. No third full run or install-only retry was dispatched. Acceptance remains pending observed installation/aggregate success.

## Two-run CI acceptance retained (2026-11-01)

**CI-002 is Done.** Independent runs [40834967572](https://github.com/damquan1001/shotclutch/actions/runs/40834967572) and [40860747696](https://github.com/damquan1001/shotclutch/actions/runs/40860747696) both have successful aggregates on identical source `a7fbcf2ffba3156bef1e175d30880670d7ff3bfc`. Packaging lint 40834971676 passed on that source too. Each run independently rebuilt compiled targets and passed Windows 2,630 / Linux 2,646 / macOS 2,600 test executions, with 12 explicitly ignored per OS, separate required adapter probes, release builds and Linux WASM.

Acceptance uses each run's preserved attempt-1 checks and successful package/install recovery (first run attempt 4; second run attempt 2). These are two independent green workflow runs with documented recovery, not two fresh green attempts without retries. Earlier billing startup failures and install-only artifact mismatches remain in the historical record; no failed result was overwritten or counted as passing.

Both recovered MSI packages installed and uninstalled with exit 0, preserved the user sentinel, and produced independently verified 50 video frames / 80,000 stereo sample frames. [First install](evidence/rollout/ci-a7fbcf2f-recovery4/install/install/summary.json), [first output](evidence/rollout/ci-a7fbcf2f-recovery4/install/install/workflow/verification.json), [second install](evidence/rollout/ci-a7fbcf2f-repeat2-recovery2/install/install/summary.json), [second output](evidence/rollout/ci-a7fbcf2f-repeat2-recovery2/install/install/workflow/verification.json). Second MSI SHA-256: `5eee5e812b9486051a648ba7b259237051773a23037253b73afbb01fcfa1f666`. Reports truthfully mark `clean_consumer_environment: false`: fresh hosted runners are development environments, not strict offline consumer VMs.

[Final sequence state](evidence/rollout/ci-a7fbcf2f-sequence/state.json) binds both successful IDs. No additional hosted run is needed for this CI gate. M0/CORE-005 remain In Review for review and the documented qualification limits: strict offline consumer environment, second physical Windows profile, physical A/V drift, variable playback drop rates and broader lifecycle coverage. Raw export cancellation still retains partial output. No new runtime pass is inferred from hosted compilation or CLI rendering. Main promotion, required-check settings, schedules, signing and publishing remain separately authorized work and were not performed.

The first recovered package download subsequently completed: [digest/provenance and executable bindings](evidence/rollout/ci-a7fbcf2f-recovery4/candidate.json). Its MSI hash matches the hosted install report. This closes the earlier download-pending observation only; no additional local GUI pass is claimed for that package.

## Exact recovered desktop and remaining environment gates (2026-11-01)

The downloaded run-40834967572 attempt-4 desktop passed isolated native 1080p import/trim/undo/redo/save/reopen/export, with independent output checks at 1920x1080, 30/1 fps, 50 video frames and 80,000 stereo sample frames at 48 kHz. The app child PATH contained only its package directory and Windows System32; oracle overrides were removed. [Commands/logs](evidence/rollout/ci-a7fbcf2f-recovery4/native-core-1080/evidence.json), [hashes and independent output verification](evidence/rollout/ci-a7fbcf2f-recovery4/native-core-1080/verification-context.json), [reproduction](evidence/rollout/ci-a7fbcf2f-recovery4/native-core-1080/reproduce.py). Local MSI/ZIP inspection passed [validation](evidence/rollout/ci-a7fbcf2f-recovery4/local-package-validation.json); no installation/uninstallation was performed on the development host.

The initial sandbox attempt reached the app's control server but the client socket was denied with WinError 10013 and timed out. Its own child was terminated by the harness. The subsequent permitted host-side run passed; no application source change or suppression was used. Initial package inspection through sandboxed Windows PowerShell was also denied; permitted PowerShell 7 inspection passed. These are execution-environment failures, not product defects or new clean-install evidence.

[Bounded environment inventory](evidence/rollout/ci-a7fbcf2f-recovery4/native-core-1080/environment-availability.json): Windows 11 Home Single Language; VMware Workstation is installed, no VMs are running, and no VMX file was found in the checked standard Documents folder. Windows Sandbox and default-path VirtualBox executables were not found. This does not prove that no VM exists elsewhere. No VM was started or modified. The owner was asked to identify a disposable Windows 11 VM path and/or separate physical PC. Q1 and Q4 remain pending that environment; physical A/V drift also needs appropriate measurement equipment. Existing Full/Half drop-rate variability and cancellation findings remain unchanged; the new small native render does not qualify those scenarios.

### Second-PC handoff prepared

The owner confirmed that a separate PC can be provided. Hardware/access details and executed results remain pending; this does not itself satisfy Q4. A self-contained transfer kit was prepared at `target/qualification/second-pc-a7fbcf2f.zip` (large local artifact, not committed): the verified run-40834967572 attempt-4 MSI/portable packages, original generated 1080p fixture, SHA-256 manifest, manual workflow checklist and optional read-only PowerShell profile collector. [Manifest](evidence/second-pc-kit/manifest.json), [procedure](evidence/second-pc-kit/README.md), [collector](evidence/second-pc-kit/collect-profile.ps1), [observation form](evidence/second-pc-kit/observations.md). Collector syntax passed; it was not run as second-PC evidence. No VM, system policy or existing installation was changed. Returned render/project/profile files will be checked before a bounded correctness pass is recorded; performance and physical A/V tests remain separate.
