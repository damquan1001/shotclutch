# M0 completion record

Date: 2026-11-01. Implementation branch: codex/m0-foundation. Status: Done for the original foundation scope in document 16: CORE-001, CORE-005 and PERF-001. This closeout reconciles the retained evidence against those acceptance criteria; no additional feature implementation, merge or release is implied.

## Acceptance mapping

| Requirement / exit | Accepted evidence |
|---|---|
| CORE-001: standalone fork, identity, compatibility and legal inventory | [Foundation audit](FOUNDATION_AUDIT.md), [build/package/workflow evidence](EVIDENCE.md), distinct MSI identity, retained notices, existing .fcproj compatibility and environment aliases |
| Clean Windows installation and core workflow | [User clean-install acceptance](evidence/clean-install-user-confirmation.md); subsequent fresh hosted MSI install/export/uninstall reports and [verified second-PC manual/independent result](evidence/second-pc-readable-return-20261101/README.md), explicitly accepted by the user |
| CORE-005: reproducible build, automated checks, release packaging plan, upstream-change policy | Rust 1.95 locked builds; two independent green repaired-source CI runs at a7fbcf2f with their cold checks and documented package/install recoveries; passing packaging lint; [execution and artifact provenance](CI_RUNTIME_EXECUTION.md); [manual reviewed upstream policy](UPSTREAM_POLICY.md) |
| PERF-001: comparable fixtures and measured baseline | [Paired upstream/fork benchmark report](BENCHMARKS.md), generated fixture hashes and hardware/toolchain/backend/cache/resource metadata; unavailable measurements and slow cases remain explicit |
| Architecture assumptions checked against code | [Verified/partial/absent/unknown inventory and M1 gaps](FOUNDATION_AUDIT.md); inherited project/time/command contracts retained during M0 and pending downstream decisions remain in the tracker |

User documentation baseline: b739515e04b3467ed8b681e6296f7dc5867f43dd. Inherited upstream: 3a5f217b348bf0c9dc1678f305c34b6c88617f37. Independent successful CI IDs: 40834967572 and 40860747696, same repaired source a7fbcf2ffba3156bef1e175d30880670d7ff3bfc. Test/build passes belong to each attempt 1; successful package/install recoveries belong to attempts 4 and 2. This is reproducible locked build/test evidence, not a byte-for-byte binary reproducibility claim.

The accepted second-PC scope is Windows 10 Enterprise / i5-4690 / RX 570 / approximately 24 GiB, online. The readable project loads through the packaged CLI and contains 50-frame edits; the returned ProRes fully decodes to 1920x1080 at 30 fps, 50 video frames and 80,000 stereo sample frames. Earlier zero-filled copies remain recorded with unknown cause; they are superseded by the verified replacements.

## Separately tracked work

The clean-install acceptance above satisfies the original M0 gate. The later rollout's stricter offline consumer-VM matrix is still unperformed; do not claim it passed. Sustained playback/performance budgets, physical A/V drift, extended lifecycle/fault coverage and additional OS/hardware matrices remain separate qualification work. Existing drop-rate variability, 4K AV1 limits, partial output retained by raw cancellation, and consumer FXC limitations remain disclosed. No new architecture or M1-M5 feature is completed by this record.

CI-003 (main promotion, required-check settings and schedules) remains separately authorized repository work. Signing, publishing and a product release were not performed. M0 completion does not authorize a PR, merge, billing/settings change or M1 implementation. Historical reports retain their original status and dates; this closeout supersedes earlier overall M0/CORE-005 pending-review summaries.
