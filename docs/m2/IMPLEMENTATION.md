# M2 Smart Rough Cut

Branch `codex/m2-smart-rough-cut` starts from the M1 closeout `b15c6439`. M2 covers UI-002 and PRE-001–PRE-005. The milestone stays open until the Windows editorial walkthrough and real-model qualification are recorded.

[Redacted verification record](EVIDENCE.md)

## Decisions

- **DEC-005, local ASR.** The native desktop build enables the existing CPU Whisper backend. M2 uses the pinned multilingual Whisper base package for English acceptance. The model lives in the local data directory, is verified against the catalogue, and can be installed from the workspace after the user confirms the download. The installer receives model artifacts only. Speech inference reads local audio. Silence-only mode is an explicit choice and does not create transcript words.
- **DEC-009, workspace.** Smart Rough Cut is an egui workspace with a dedicated review and result panel alongside the existing Source monitor. Audition and result preview reuse the desktop frame and audio workers.

## Project and service contracts

Schema v18 adds a typed `project.preedit` field. Older projects start with empty review state and use the normal migration backup and checksum checks. Analysis records bind a local original by asset ID, full SHA-256, byte length, source time range, stream, channel selection, and model/settings. Checkpoints hold absolute source times and bounded waveform bins. The engine reads audio in 20-second chunks with one second of context on each side. Each completed chunk uses the recovery snapshot boundary. An interrupted analysis can resume after input verification.

Each review command adds an immutable map revision. Suggestions initially retain all source material. The most recent explicit keep/remove decision wins in an overlap. Reanalysis carries decisions forward when the source binding matches, and presents conflicts when the binding differs. Take suggestions use nearby matching transcript openings. Review the full statements before removing an alternative. ASR word confidence is not calibrated by this backend, so candidate confidence is shown as unknown.

The owner command registry exposes `preedit.analyze`, `preedit.cancel`, `preedit.inspect`, `preedit.review`, `preedit.preview`, `preedit.apply`, and the desktop background application and cancellation commands. Requests name explicit source/map IDs and use decimal strings for IDs and revision checks. Source range endpoints are decimal tick strings in `{ "start": "...", "end": "..." }`. Stream numbering is zero-based in commands and one-based in the workspace. Scoped service clients retain their M1 command allowlist.

Preview and apply use the same assembly builder. Each retained source interval produces an audio clip and, for video, a linked picture clip. The selected stream is preserved. Short audio fades lie inside the retained ranges. Captions follow retained word midpoints and clamp to the output. Apply verifies the original's content and current project revision. It creates a new editable sequence by default. An explicit replacement requires the original generated sequence digest and unlocked tracks. The committed operation receipt is durable across restart and undo within the existing service retention policy.

The bounded implementation uses one original source per map. Indexed MP4 and Matroska readers certify supported video cut boundaries. A cut the reader cannot certify fails before application. Ordinary audio-only originals can be cut on their sample grid. The workspace presents a logical result without committing an editorial sequence.

## Qualification

Run the existing M1 gate and the M2 headless tests:

```powershell
python scripts/ci/m1-checks.py
cargo test --locked -p shotclutch-engine --lib m2_
cargo check --locked -p shotclutch
```

Install the pinned model in the test data directory. Supply the footage path and the auditioned stream through local environment variables. The stream is zero-based. The ignored test fails if inputs or weights are missing. It reports aggregate counts and does not print the transcript. Set `SHOTCLUTCH_M2_EXCERPT_SECONDS` to `start:end` for an excerpt run. Clear it for the full recording.

To open the resulting proposal in the native workspace, set `SHOTCLUTCH_M2_REVIEW_PROJECT` to a private output path outside tracked files. The generated project contains the source locator and transcript. Keep it with the private media.

```powershell
cargo test --locked -p shotclutch-engine --features whisper --lib m2_private_whisper_qualification -- --ignored --nocapture
```

The native worksheet is to import the original, audition all four streams, explicitly start analysis, preserve an intentional pause, compare repeated attempts, remove and restore a phrase, and listen across the joins. Review the result and apply to a new sequence. Save/reopen and verify the map, linked clips, captions, and undo. Make a manual edit and verify replacement fails while a fresh sequence remains available. Reanalyze and verify that the earlier decisions survive.

Measure analysis time and peak memory, coverage, removal precision and restoration rate, take agreement, and review time. Use export frame/sample checks and listen to representative joins. Store only redacted summaries and commands in the evidence index. The private original, transcripts, labels, and revealing screenshots belong in local qualification storage.
