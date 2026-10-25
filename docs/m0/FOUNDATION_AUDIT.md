# M0 foundation audit

Audit date: 2026-10-26. Initial qualification platform: Windows x64. Owner: Codex implementation session. Acceptance status and executed checks are recorded in `EVIDENCE.md`; source presence alone is not runtime qualification.

## Source identity

- Fork origin: `https://github.com/damquan1001/shotclutch.git`.
- Inherited source baseline: `3a5f217b` (full hash recorded in the evidence manifest).
- User documentation baseline: `b739515e04b3467ed8b681e6296f7dc5867f43dd` (`quick clean up`). It already committed the 01–17 specifications, moved historical documentation, and removed upstream root documents, logos, and screenshots before M0 started.
- Working tree at M0 branch creation: clean. No existing changes were reset or overwritten.
- Implementation branch: `codex/m0-foundation`. The local upstream remote was newer than the inherited baseline; M0 does not silently incorporate those changes.
- Rust workspace version: 0.13.0; toolchain: 1.95.0; edition: 2024. The product specification version 0.1 is not a file-format or Cargo-version migration.

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

## Dependencies, notices, and distribution

Cargo.lock pins registry versions and the three craft-libs Git revisions used by craft-media, craft-docpath, and craft-ui. They are library dependencies, not an external application suite. Record the resolved dependency graph with `cargo metadata --locked --format-version 1` and checksums with qualification evidence. Do not rewrite pinned revisions solely to make a build pass.

The workspace uses MIT OR Apache-2.0. Preserve upstream copyright, both licenses, per-asset sidecars, and the retained-asset attribution inventory. Bundled Inter, Noto Serif and JetBrains Mono fonts carry OFL texts. Additional craft-fonts bundles require their notices in both package types. Optional speech models have separate manifests/licenses and are not bundled by M0. This inventory is a dependency/legal engineering audit, not a claim of legal certification.

Removed ArtCraft trademarks are no longer embedded. The ShotClutch play/frame icon is original geometric artwork with MIT OR Apache-2.0 provenance. Historical FilmCraft icons remain attributed source assets; the Windows product and native window use ShotClutch artwork.

## Architectural decisions

- DEC-001: Windows x64 first; actual machine profile and any unavailable telemetry belong in the evidence. A clean installation is a separate environment gate.
- DEC-002: retain the inherited project/time/command foundation. No schema/timebase replacement in M0. Document mismatches above for M1.
- Retain internal crate names, command IDs and project compatibility. Rename product surfaces and isolate default user data. Legacy environment aliases remain available; no automatic migration of FilmCraft preferences or recovery journals.
- Replace external moving CI workflows with local jobs and pin third-party actions. Build the unsigned package in CI; publishing is a separate future release operation.
