# M0 foundation audit

Audit date: 2026-10-26. Initial qualification platform: Windows x64. Owner: Codex implementation session. Acceptance status and executed checks are recorded in `EVIDENCE.md`; source presence alone is not runtime qualification.

## Source identity

- Fork origin: `https://github.com/damquan1001/shotclutch.git`.
- Inherited source baseline: `3a5f217b348bf0c9dc1678f305c34b6c88617f37`.
- User documentation baseline: `b739515e04b3467ed8b681e6296f7dc5867f43dd` (`quick clean up`). It already committed the 01–17 specifications, moved historical documentation, and removed upstream root documents, logos, and screenshots before M0 started.
- Working tree at M0 branch creation: clean. No existing changes were reset or overwritten.
- Implementation branch: `codex/m0-foundation`. The local upstream remote was newer than the inherited baseline; M0 does not silently incorporate those changes.
- Rust workspace version: 0.13.0; toolchain: 1.95.0; edition: 2024. The product specification version 0.1 is not a file-format or Cargo-version migration.

The captured Cargo metadata contains **54 workspace packages**. The compiled engine command inventory contains **710 commands**; this is an inventory count, not an acceptance or feature-parity score. Raw inventories are retained under `target/m0/`.

## Capability inventory

“Partial” means source exists but the full ShotClutch acceptance requirement is not yet qualified. “Absent” means the proposed ShotClutch entity/workflow was not found under its specified contract. Test results can verify narrower capabilities without closing a complete requirement.

| Area | Source evidence | Initial classification and qualification needed |
|---|---|---|
| Canonical project | `crates/project/src/lib.rs` | Partial: serde project, bins/items, linked clips, tracks, nesting, captions, graphics, and stable typed u64 IDs. Audit unknown-extension round trips and ID lifetime rules in M1. |
| Exact time | `crates/time/src/lib.rs` | Partial: i64 ticks at 254,016,000,000/s and rational frame rates. Existing JSON serializes ticks as numbers; the proposed decimal-string API and every retime/VFR boundary still need M1 qualification. |
| Editing and commands | `crates/edit`, `crates/engine/src/commands.rs`, `Session::execute` | Partial: shared command registry, edit algebra, validation, and undo snapshots. No evidence that the complete staged, revision-checked ShotClutch transaction envelope exists. |
| Persistence | `crates/format`, `crates/engine/src/autosave.rs` | Partial: schema v15, chained migrations, atomic write and recovery journal. Preserve `filmcraft.project`; fault injection and durable transaction requirements remain M1. |
| Desktop and automation | `apps/filmcraft`, `apps/filmcraft-cli`, `crates/automation` | Partial: native egui/WGPU shell, headless CLI, stdio MCP, token-protected loopback bridge. These are inherited interfaces, not qualification of the new scoped service contract. |
| Media | `crates/media`, `crates/codecs`, `crates/platform` | Partial: common source abstraction, software codec/container implementations, platform hardware backends and fallback. Qualify profiles, malformed files, VFR and proxy identity on Windows. |
| Rendering | `crates/render`, `crates/gpu`, `crates/golden` | Partial: CPU reference, WGPU compositor/effects, immutable snapshot readers. Preview/export and backend tolerances need corpus evidence. |
| Audio | `crates/audio-dsp`, render mixer, desktop cpal host | Partial: sample mapping, mixing/effects and device output. Long-form sync, device changes and underruns remain runtime qualification. |
| Export and interchange | `crates/export`, `crates/interchange` | Partial: rendered exports and EDL/FCP7 XML/FCPXML/OTIO/AAF/OMF source paths. Actual destination imports and the per-object fidelity ladder remain unqualified. |
| Speech | `crates/speech`, `crates/tts` | Partial: optional Whisper and Kokoro features, local model manifests, word timings and VAD. Default M0 builds do not install models or claim ASR quality. |
| Smart Rough Cut | Numbered specification 09 | Absent under the specified `PreEditAnalysis`, `PreEditMap`, `TakeGroup`, persistent review and application contracts; M2. |
| Semantic agent workflow | Numbered specifications 08 and 10 | Absent under the specified hierarchical evidence store and composed-result verification loop; M3. Existing automation is a foundation. |
| MotionScene / ACP / Skills | Numbered specifications 07 and 11 | Absent under the new contracts; M4/M5. Existing native graphics are not MotionScene qualification. |
| Tests and CI | crate tests, golden tests, xtask | Partial: substantial inherited tests; M0 adds fork-owned checks and Windows package validation. Remote CI results require an actual run. |

The broad capabilities above stay Partial even when a bounded path passes. The following narrower conclusions have executable evidence:

| Capability | Classification | Evidence and limit |
|---|---|---|
| Historical project fixture reading and migrations | Verified | 16 format fixture tests pass, including old schema cases; [test log](evidence/checks/tests-project-fixtures.log). This does not prove arbitrary future-extension preservation. |
| Native desktop import, linked trim, undo/redo, save/reopen and ProRes output | Verified | [Live desktop responses](evidence/desktop/evidence.json): 50 video frames and 80,000 stereo sample frames verified independently. This is a short development-machine workflow. |
| Windows CLI VFR-phone and long-GOP/two-audio-stream workflows | Verified | Both real-media workflow tests pass in the [16-test CLI integration run](evidence/checks/tests-cli-integration-final.log). Broader codec/profile coverage stays Partial. |
| Retained source asset attribution and Windows legal payload | Verified | 55-asset gate and ZIP/MSI payload inspection pass; [asset log](evidence/checks/assets-final.log), [package result](evidence/packages/validation.json). Optional models are not bundled. |
| Clean-machine installation and core editing/export | Verified by user confirmation | User reports clean MSI 0.13.0 install, launch/import/edit/save/reopen/video export passed; [acceptance](evidence/clean-install-user-confirmation.md). Exact OS/hash metadata was not supplied. |
| Strict offline behavior and full install/repair/uninstall lifecycle | Unknown | Additional coverage in [the runtime rollout](CI_RUNTIME_ROLLOUT.md); no assertion that every lifecycle/offline path was exercised in the user-confirmed workflow. |
| GPU throughput, other OS runtime behavior and destination-application imports | Unknown | No qualifying performance/target-application runs; source paths and CI definitions alone are insufficient. |

## Dependencies, notices, and distribution

Cargo.lock pins registry versions and the three craft-libs Git revisions used by craft-media, craft-docpath, and craft-ui. They are library dependencies, not an external application suite. Record the resolved dependency graph with `cargo metadata --locked --format-version 1` and checksums with qualification evidence. Do not rewrite pinned revisions solely to make a build pass.

The captured [Windows-filtered dependency graph](evidence/inventory/dependencies.json) has 335 packages (54 workspace), including development dependencies. Every package supplies a declared license expression; the inventory preserves those expressions rather than collapsing alternatives or compound obligations into a single label. It includes MPL-2.0 media packages, permissive alternatives, and compound font/Unicode expressions. This graph is broader than a shipped-binary inventory. Source/license declarations are verified as inventory data; linked-payload license selection and a comprehensive redistribution-obligation review remain Partial. M0 packages retain the upstream notices and bundled font/asset license texts; no license-certification claim is made.

The workspace uses MIT OR Apache-2.0. Preserve upstream copyright, both licenses, per-asset sidecars, and the retained-asset attribution inventory. Bundled Inter, Noto Serif and JetBrains Mono fonts carry OFL texts. Additional craft-fonts bundles require their notices in both package types. Optional speech models have separate manifests/licenses and are not bundled by M0. This inventory is a dependency/legal engineering audit, not a claim of legal certification.

Removed ArtCraft trademarks are no longer embedded. The ShotClutch play/frame icon is original geometric artwork with MIT OR Apache-2.0 provenance. Historical FilmCraft icons remain attributed source assets; the Windows product and native window use ShotClutch artwork.

## Architectural decisions

- DEC-001: Windows x64 first; actual machine profile and any unavailable telemetry belong in the evidence. A clean installation is a separate environment gate.
- DEC-002: retain the inherited project/time/command foundation. No schema/timebase replacement in M0. Document mismatches above for M1.
- Retain internal crate names, command IDs and project compatibility. Rename product surfaces and isolate default user data. Legacy environment aliases remain available; no automatic migration of FilmCraft preferences or recovery journals.
- Replace external moving CI workflows with local jobs and pin third-party actions. Build the unsigned package in CI; publishing is a separate future release operation.

## Validation findings and bounded repairs

The inherited source builds unchanged. Rust 1.95 Clippy required equivalent boolean/match simplifications in ProRes allocation validation, UI dialog/timeline conditions and desktop audio state handling. These are fork gate repairs, not repairs applied to the untouched benchmark baseline. No warning suppression was added.

Qualification caught and corrected two branding regressions: the DPX creator field must use the new name's actual byte length, and log rotation must consistently use ShotClutch filenames. A DPX header assertion protects the creator field; log-filter coverage includes the renamed executable target. Shipped project migrations and serialized interchange keys retain their historical identity.

The MCP display name and launch configuration use ShotClutch. Existing `filmcraft://document` and `filmcraft://commands` resource URIs remain compatibility identifiers. Platform workflow behavior was checked against upstream craft-common-actions revision `6182ccebfdcec1b76f308ebe187f78d286b3cf39`; the fork owns its job definitions and pins third-party actions. ARM64 MSI installation and FreeBSD build/package jobs are retained; no remote run is claimed.

Windows x64 is the delivered package path. Inherited Linux/macOS/web packaging and icon-generation templates remain unqualified and contain legacy product assumptions; they require adaptation and platform execution before distribution. They are not covered by the Windows artifact acceptance. The retained Unix build checks and updated FreeBSD package job do not establish runtime support. The new Unix metadata references `shotclutch` and omits the removed upstream screenshot; optional Windows signing metadata uses ShotClutch, but no signature was requested or produced.
