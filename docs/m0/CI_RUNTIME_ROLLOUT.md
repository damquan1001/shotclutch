# Remote CI and runtime qualification rollout

Plan date: 2026-10-26. Branch: `codex/m0-foundation`. Reviewed source: `4f308ec415ec383b8dfa6fda55af004d5f61b874`. Status: planned; no workflow/settings changes, push, dispatch, release or runner registration performed by this planning session.

## Accepted starting point

The user confirms that the **0.13.0 MSI installs cleanly**, launches, imports/edits, saves/reopens and exports video successfully. Record this as passing manual acceptance, with provenance in [the user confirmation](evidence/clean-install-user-confirmation.md). Exact Windows build, tested package hash, offline conditions and uninstall were not separately reported; they are metadata/coverage to capture in subsequent qualification, rather than reasons to discard the successful test.

Local evidence already covers 1,383 passing targeted tests, three explicitly ignored performance tests, locked release builds, legal/asset checks, package inspection, an authenticated native desktop edit/export workflow with independent frame/sample counts, and paired software/CPU benchmark captures. See [EVIDENCE.md](EVIDENCE.md). M1–M5 implementation and the document 14 release performance budgets remain outside this rollout.

Deploy GitHub-hosted correctness/package checks first. Run interactive and GPU/audio performance qualification on declared Windows 11 x64 hardware. A hosted Windows Server build runner and a consumer Windows desktop provide different evidence.

## Existing workflows and concrete gaps

| Source | Already present | Preparation required before activation |
|---|---|---|
| `.github/workflows/ci.yml` | Windows 2025, Ubuntu 24.04 and macOS 15; format/layers/assets/Clippy/library/CLI integration checks; release build; Linux WASM; reusable Windows package job | Branch push currently matches only `main`; binary unit tests, format fixture integration and benchmark unit tests are omitted; media tools are installed only on Linux and required-oracle mode is absent |
| `.github/workflows/release.yml` | Unsigned x64 MSI/ZIP, artifact validation, fresh-runner install/CLI inspection/uninstall | It produces qualification artifacts, with no release publishing; retain failure logs and package provenance, add packaged render correctness; fresh hosted runner includes development tools and does not prove an offline consumer install |
| `.github/workflows/packaging-lint.yml` | PowerShell parsing, identity checks, actionlint, shell/desktop/AppStream validation | Runs on PR/manual events; add a bootstrap push path; retain diagnostics on failure |
| ARM64 and FreeBSD workflows | Separate build/package/install jobs, schedules and limited PR path triggers | Preserve definitions; confirm runner access and run individually after x64 is green; support remains unqualified until platform evidence exists |
| Nix/static-analysis workflows | Source policy check; explicit absence of a Nix flake; manual Clippy | Nix availability reporting is not a successful Nix build; avoid treating duplicate analysis as a new qualification |
| `scripts/m0/install-check.ps1` | Installs, checks notices/architecture/CLI and uninstalls in a disposable environment | Does not exercise interactive desktop, audio output, full editing or rendered frame/sample correctness |
| `scripts/m0/desktop-smoke.py` | Native desktop bridge workflow and independent render checks | Uses a small generated fixture and external generation/oracle tools; separate fixture staging, offline app execution and later independent output inspection for clean-environment proof |
| `scripts/m0/benchmark.ps1` | Seven measured sections, hashes, hardware, process resources and raw samples | Currently forces software decode and CPU composition for both warmup and measurement; expose validated backend settings before claiming GPU/default-path qualification |

## Phase 0 — Repository access and deployment preflight

Owner: repository owner for access/settings; Codex for implementation and evidence collection when execution is requested.

1. Confirm origin `damquan1001/shotclutch`, actual default branch, visibility, Actions availability, actor write permission, permitted pinned actions, artifact retention and available minutes/concurrency. The connected GitHub lookup returned 404 during planning; this indicates unavailable access through that connection, not proof that the repository is absent. GitHub CLI was unavailable locally. Resolve this before remote execution; do not request signing secrets.
2. Confirm ordinary hosted labels used by the existing workflows are available to this repository. Start with existing hosted runners and manual use of the known Windows desktop. Do not provision paid GPU/larger runners or expose the personal development machine as a general PR runner.
3. Confirm schedules/enabling policy separately. Forked repositories can have disabled workflows/schedules; do not assume nightly jobs will run after a push. See [GitHub workflow enablement](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/disabling-and-enabling-a-workflow).
4. Record the reviewed commit and a clean working tree before deployment. Repository access, billing/spending settings and default-branch contents remain unverified in this plan.

Exit: repository access and runner/budget availability are recorded; a specific commit is eligible to push after authorization.

## Phase 1 — Prepare the first remote run locally

Keep the existing platform checks and Windows-first distribution boundary. Make the following changes in one reviewable CI change, before any push:

1. Add an explicit temporary push trigger for `codex/m0-foundation` to CI and packaging lint. Retain `main`, PR and manual behavior as appropriate. This bootstraps checks without merging an untested workflow or requiring a PR to be opened. Manual dispatch requires the workflow on the default branch; a branch push alone currently does not match CI. See [GitHub dispatch requirements](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/manually-run-a-workflow?tool=webui).
2. Install checksum-verified, version-pinned FFmpeg/ffprobe oracle bundles in each relevant OS test job. Verify required encoders/formats before testing, record tool versions and set `FILMCRAFT_REQUIRE_ORACLES=1`. Missing tools or fixture generation failures must fail qualification instead of silently skipping real-media tests.
3. Keep all workspace library tests; add desktop `--bin shotclutch`, CLI `--bin shotclutch-cli`, format `--test fixtures` and UI `--example bench` tests. Include doctests/other integration suites where applicable after enumerating their tool/platform prerequisites. Count actual outcomes; do not require a permanently fixed test count.
4. Capture full commands, test summaries, skips/ignored tests, build information, runner image/version, disk/memory and exit statuses. Upload diagnostics with `if: always()`; an artifact upload must not mask a failed test. Use failure-preserving PowerShell/native exit handling.
5. Add an artifact-level render check on the fresh Windows install runner. Stage generated inputs separately, run the packaged CLI against them, then independently inspect exact frame/sample counts. Retain the actual MSI install/uninstall logs even on failure. Use `/qn` for automated install; test visible wizard/Finish behavior manually on a desktop.
6. Preserve Rust 1.95.0, locked Cargo resolution, WiX/UI extension 5.0.2, static CRT, commit/hash binding, distinct installer identity and unsigned packages. Keep `contents: read` and immutable action SHAs; qualification requires no publishing credentials. Disable persisted checkout credentials where unneeded.
7. Start with dependency caching keyed by OS/architecture/toolchain/Cargo.lock, separate build profiles/CRT flags and unique artifact names including SHA/run attempt. Do not reuse stale target binaries as evidence. Record cache mode and one cold run. Plan disk use for debug/test and static release targets in separate jobs; standard hosted runners currently have limited SSD capacity. [Runner specifications](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).

Local validation: actionlint, PowerShell/Python syntax, layers/assets, changed test selections and a regenerated Windows package/validator run where the workflow changes affect artifacts. No application feature work is required for CI plumbing.

Exit: the reviewed change makes every required test run or fail explicitly, retains failure evidence and has a working branch-bootstrap trigger.

## Phase 2 — Activate and stabilize hosted CI

After execution/push is authorized:

1. Push the reviewed milestone branch to **origin**. Do not push to upstream. Observe the branch-push events rather than assuming a manual dispatch will discover workflows not yet on the default branch.
2. Run the existing three-OS checks and x64 package/install chain. Classify failures as source, dependency/tooling, workflow, resource, unsupported environment or product defects. Keep failing evidence; rerun only after a change or a documented transient infrastructure failure.
3. Require two green executions of the same commit, including one cold build, before calling the initial CI rollout repeatable. Retain OS/architecture, runner image, fixture/tool hashes, test/skip counts, MSI/ZIP hashes, executable bindings and install/render/uninstall results for each run.
4. Review run duration, disk peaks, cache sizes and minute consumption. Existing main/package timeouts are 120 minutes; adjust only from observed workload/runner limits. Begin without additional paid infrastructure and without enabling extra scheduled workload.
5. After an authorized review/merge, remove the temporary branch push trigger and use main/PR/manual triggers. Configure required checks only after their actual GitHub check names have been observed. Prefer stable final aggregate checks for CI and packaging lint; required checks must run on every PR and fail when a prerequisite fails or unexpectedly skips. Keep extended platform qualification independent of Windows x64 support claims.
6. Enable a modest scheduled media/soak run only after manual runs are green and budget is known. Preserve ARM64/FreeBSD definitions, but activate their runs separately; passing them is not a prerequisite to the initial Windows x64 rollout. No GitHub Release or signing is part of this phase.

Exit: retained green remote runs, a working required-check policy after merge, clear artifact provenance and an observed cost envelope. Owner permission is required for repository-setting changes; none are performed by this plan.

## Phase 3 — Runtime qualification matrix

Use the exact MSI/ZIP downloaded from a successful CI run for new remote-build qualification. Verify its hashes before use; do not rebuild it independently on the test machine.

| Lane | Environment | Work and pass evidence |
|---|---|---|
| Q0 — accepted manual test | User's clean installation, MSI 0.13.0 | **Passed by user confirmation:** install, launch, import/edit, save/reopen and video export. Preserve the statement; add OS/hash details when available without requiring the successful test to be repeated solely for paperwork |
| Q1 — install lifecycle | Disposable Windows 11 x64 VM/desktop; MSI and ZIP from CI | Wizard/Finish launch, notices, Start menu, data location, repeat install/repair/same-version replacement, uninstall preserving user projects, portable data behavior and coexistence with FilmCraft data. Stage inputs before offline testing; inspect rendered output independently afterward |
| Q2 — primary desktop | Known Ryzen 7 4800H/RTX 3060 Laptop Windows 11 profile, freshly captured drivers/power/storage/display | B01–B06 supported subset: edit correctness, long-GOP/VFR, exact export, timeline responsiveness, audio output, software fallback and actual GPU/default backend behavior. Record active adapter/backend and hardware/fallback counters, including hybrid-GPU selection |
| Q3 — reliability and long form | Same primary desktop, interactive session with actual audio device | Ten-minute playback with dropped frames/underruns; 30-minute A/V sync; long project save/reopen and repeated edits/undo; job cancellation, missing-media handling and controlled recovery observations. Log failures as inherited limitations/M1 work where appropriate; do not add transaction architecture in this rollout |
| Q4 — second hardware profile | A separate Windows 11 x64 machine with an integrated GPU or lower-throughput configuration; exact hardware selected before running | Repeat the core correctness and bounded performance subset; report achieved modes and limitations separately. Until this machine exists, second-tier support stays unqualified |

No Linux/macOS/ARM64/FreeBSD runtime support is inferred from cross-platform compilation. Runtime expansion beyond Windows x64 gets its own package adaptation and interactive evidence.

## Phase 4 — Performance and evidence protocol

- Extend the benchmark wrapper to accept supported decoder/compositor/adapter settings and record the measured settings. Apply the same configuration to warmup and measurement. Check counters to distinguish hardware use, decline and software fallback; a requested backend alone is not evidence of use.
- Preserve the existing untouched upstream CPU comparison. Recheck the observed 8–20% slower decode cases with at least three interleaved upstream/fork pairs under matched settings, hashes and cache conditions. Separate thermal/scheduling variance from reproducible changes. Baseline results remain valid descriptive evidence; do not rewrite them to hide slower cases.
- Run full-length scenarios after quick smoke runs. Use deterministic seek seeds, exact frame/sample verification and enough samples to justify percentiles. Host timings are descriptive infrastructure measurements; release GPU/audio budgets require declared desktop hardware.
- Report latency, dropped frames, A/V drift/underruns, CPU/RAM, available GPU memory/usage, disk/cache growth, cancellation and output correctness. Keep unavailable telemetry explicitly unavailable. Match quality, preview resolution, backend and output correctness before comparing speed.
- Apply document 14's repeated >10% regression criterion as an investigation trigger, not automatic proof of a cause. Correctness/data-loss failures block affected qualification regardless of throughput. M4 release budgets remain proposed targets until their declared qualification, rather than new M0 release promises.

Each retained run has a summary, commit and binary/package hashes, OS/runner image or desktop profile, drivers, fixtures and licenses/hashes, tool versions, command/settings/cache state, raw timings/resources, expected/actual frames and samples, logs, failures and rerun links. Store small manifests/reports in the repository; use CI artifacts for large logs/renders and generated fixtures. Proposed initial retention: 14 days for routine diagnostics and 30 days for qualification packages/failure evidence, subject to repository limits. No private footage, authentication tokens or credentials enter artifacts.

Exit: Q1–Q3 have reviewed pass/fail reports for the declared supported subset; Q4 is either qualified or explicitly outstanding. Failures carry reproduction, severity, affected capability and next action. A successful render does not qualify all playback/backend combinations.

## Implementation work order

| Order / ID | Deliverable | Completion evidence |
|---|---|---|
| 1 / CI-001 | Resolve repository access and prepare triggers, strict oracle setup, complete test selection, diagnostics and artifact checks | Read-only preflight record and local workflow validation |
| 2 / CI-002 | Authorized branch push and hosted bootstrap | Two green same-commit runs, including cold build; actual run URLs and package hashes |
| 3 / RT-001 | Qualify the CI-produced MSI/ZIP lifecycle and core workflow on Windows 11 | Q1 report; Q0 remains accepted historical manual evidence |
| 4 / RT-002 | Primary GPU/software/audio/long-form qualification and benchmark reruns | Q2/Q3 raw samples, correctness checks, backend counters and limitations |
| 5 / RT-003 | Second Windows profile and support statement | Q4 report or explicit outstanding hardware dependency |
| 6 / CI-003 | Stabilize required checks and bounded scheduled coverage after authorized merge | Settings/run evidence, measured time/storage/minute use and rollback procedure |

Rollback: retain the last known passing artifact, disable only the failing optional schedule when necessary, and fix/revert workflow changes on a reviewed branch. Do not force-reset the fork, silently relax tests, overwrite benchmark data or modify user projects. Signing, releases, paid runners and new feature milestones remain separate decisions.

The next authorized execution task should implement CI-001 locally and produce a concrete workflow diff before any request to push or change repository settings. This planning session creates the plan and records acceptance only.
