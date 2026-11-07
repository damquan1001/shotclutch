# M2 Smart Cut

Branch `codex/m2-smart-rough-cut` starts from the M1 closeout `b15c6439`. M2 covers UI-002 and PRE-001–PRE-005. The milestone stays open until the Windows editorial walkthrough and real-model qualification are recorded.

[Redacted verification record](EVIDENCE.md), [qualification procedure](QUALIFICATION.md), and [acceptance register](ACCEPTANCE.md). Updated 2026-11-07. The product name is Smart Cut; existing `preedit` commands, schema names and branch name remain compatible.

## Decisions

- **[DEC-005, speech providers](DEC-005_SPEECH_PROVIDERS.md).** Pinned CPU Whisper base is the local English qualification profile. Installation is explicit and shows byte progress, cancellation and errors. Silence-only is labeled and creates no words. Optional AssemblyAI sends audio only when explicitly selected; live service qualification is separate from the local M2 gate.
- **[DEC-009, native workspace](DEC-009_SMART_CUT_WORKSPACE.md).** The left Transcript / Review cuts / Setup pane, right viewer and full-width source waveform follow the requested Gling/OpenGling workflow. Icon transport sits below the viewer. Preview mode, undo/redo, More and Open in editor share the viewer toolbar. The workspace opens on the automatic cut. Skip cuts switches between original and result at the current source position. Both workspaces use the existing desktop frame/audio workers and GPU monitor rendering.

## Project and service contracts

Schema v18 adds a typed `project.preedit` field. Older projects start with empty review state and use the normal migration backup and checksum checks. Analysis records bind a local original by asset ID, full SHA-256, byte length, source time range, stream, channel selection, and model/settings. Checkpoints hold absolute source times and bounded waveform bins. The engine reads audio in 20-second chunks with one second of context on each side. Each completed chunk uses the recovery snapshot boundary. An interrupted analysis can resume after input verification.

Each review command adds an immutable map revision. The workspace requests automatic cleanup after analysis completes. The map stores frame-aligned automatic cuts separately from manual edits. Natural mode requires quiet cutting room and protects adjacent words. Matching take openings establish review groups; automatic Natural removal additionally requires an identical earlier attempt or an unfinished prefix of the retained attempt. Changed negation/numeric tokens remain for review in both cleanup modes. Transcript rules follows timed-word boundaries. These guards do not establish semantic understanding. Manual edits override cleanup and survive compatible reanalysis. If the source binding changes, the map carries incompatible decisions as conflicts.

The owner command registry exposes `preedit.analyze`, `preedit.cancel`, `preedit.inspect`, `preedit.review`, `preedit.preview`, `preedit.apply`, and the desktop background application and cancellation commands. Requests name explicit source/map IDs and use decimal strings for IDs and revision checks. Source range endpoints are decimal tick strings in `{ "start": "...", "end": "..." }`. Stream numbering is zero-based in commands and one-based in the workspace. Scoped service clients retain their M1 command allowlist.

Preview and apply use the same assembly builder. Each retained source interval produces an audio clip and, for video, a linked picture clip. The selected stream is preserved. Short audio fades lie inside the retained ranges. Captions follow retained word midpoints and clamp to the output. Apply verifies the original's content and current project revision. It creates a new editable sequence by default. An explicit replacement requires the original generated sequence digest and unlocked tracks. The committed operation receipt is durable across restart and undo within the existing service retention policy.

The bounded implementation uses one original source per map and an 8 GiB streamed source-verification budget per phase. Local Whisper/audio refresh uses 20-second chunks plus up to one second of context on either side. AssemblyAI uses bounded five-minute chunks plus context. Indexed MP4 and Matroska readers certify supported video cut boundaries. A cut the reader cannot certify fails before application. Ordinary audio-only originals can be cut on their sample grid. The workspace presents a logical result without committing an editorial sequence. Map edits publish immediately through `Session::edit`; recovery serialization runs in the existing background worker. Apply remains one durable transaction after cancellable background verification.

## Qualification

Run the existing M1 gate and the M2 headless tests:

```powershell
python scripts/ci/m1-checks.py
python scripts/ci/m2-checks.py
cargo check --locked -p shotclutch
```

Install the pinned model in the test data directory. Supply the footage path and the auditioned stream through local environment variables. The stream is zero-based. The ignored test fails if inputs or weights are missing. It reports aggregate counts and does not print the transcript. Set `SHOTCLUTCH_M2_EXCERPT_SECONDS` to `start:end` for an excerpt run. Clear it for the full recording.

To open the resulting analysis in the native workspace, set `SHOTCLUTCH_M2_REVIEW_PROJECT` to a private output path outside tracked files. The generated project contains the source locator and transcript. Keep it with the private media. The [qualification procedure](QUALIFICATION.md) drives the native walkthrough and prepares a private human-label worksheet.

```powershell
cargo test --locked -p shotclutch-engine --features whisper --lib m2_private_whisper_qualification -- --ignored --nocapture
```

The native worksheet is to import the original, listen to its main source audio, explicitly start analysis, preserve an intentional pause, compare repeated attempts, remove and restore a phrase, and listen across the joins. Review the result and apply to a new sequence. Save/reopen and verify the map, linked clips, captions, and undo. Make a manual edit and verify replacement fails while a fresh sequence remains available. Reanalyze and verify that the earlier decisions survive.

Measure analysis time and peak memory, coverage, removal precision and restoration rate, take agreement, and review time. Use export frame/sample checks and listen to representative joins. Store only redacted summaries and commands in the evidence index. The private original, transcripts, labels, and revealing screenshots belong in local qualification storage.

## Script-led review layout

The Transcript tab displays the full transcript as readable paragraphs. Click a word or timestamp to seek the original; right-click a word to remove or restore its range. Removed words are struck through and can be hidden with Show cuts. Find in transcript filters paragraphs. Setup retains model installation, stream audition, settings, processing, and resume. Saved analysis from an earlier build can generate a cut from its existing transcript.

The lower waveform always uses source time. Removed intervals and their waveform are gray; retained material uses the accent color. Click either kind of interval to select it, drag an edge to extend/contract it, or right-click for Cut / Uncut. Source marks allow arbitrary ranges. In Result mode seeking into a removed interval advances to the next retained interval. The transport offers play/pause, frame stepping, timecode and playback rate, with Show cuts / Skip cuts and zoom on the timeline. Highlight transcript words to open Cut / Uncut / Correct / Export clip actions. Open in editor creates ordinary editable clips from the current preview. The native theme supplies surfaces and typography; no OpenGling runtime is embedded.

### Speech cleanup controls

Setup offers Transcribe only, Transcribe & cut, and Update cut. The last action uses the existing timed words and quiet audio measurements, so you can change cleanup options without rerunning Whisper. Bad takes, fillers, and silence can be enabled independently. The pause threshold and padding control how much quiet material is retained. Manual decisions still take precedence when regenerating the cut.

Cleanup keeps the later matching attempt, detects repeated openings within an utterance, and uses both the transcript and audio levels to find pauses. Word-based cleanup is tuned for English.

The [OpenGling cleanup assessment](OPENGLING_CLEANUP_ASSESSMENT.md) describes Natural and Transcript rules, adaptive audio measurement, saved-transcript refresh, review controls, and deliberate differences from the reference. Analyzer v3 measures source-edge quiet independently of Whisper timing. Update cut refreshes earlier measurements in the background when Natural is selected. Review cuts provides category/status filters, original-context listening, take comparison, restoration, and boundary marks. Keep all and Reject grouping preserve every alternative; group rejection is one undoable edit. Automatic cuts are ready before this review.
