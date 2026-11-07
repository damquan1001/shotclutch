# M2 qualification procedure

Windows x64 / Rust 1.95.0, branch `codex/m2-smart-rough-cut`. Updated 2026-11-08. Run from the repository root. Keep originals, transcripts, projects, screenshots, detailed labels and runtime logs outside tracked files. `target/m2-qualification` is ignored local storage. Commit only redacted aggregate results to [EVIDENCE.md](EVIDENCE.md).

## Correctness and build

```powershell
python scripts/ci/m1-checks.py
python scripts/ci/m2-checks.py
cargo build --locked -p shotclutch
```

The M1 gate retains its original coverage. The M2 runner checks map/analysis/apply behavior, the 196-case synthetic OpenGling differential corpus, edit boundary timestamps, speech catalogue/worker parsing and cancellation, Windows-compatible Unicode JSON transport, and AssemblyAI errors. It needs only the runner's Python standard library, not speech packages, private media, model downloads, credentials or a live provider. CI invokes both runners. Stop on a nonzero result; do not interpret a compile or empty filter as qualification.

If a running app locks the default executable, link the same source to a distinct candidate path using `cargo rustc --locked -p shotclutch --bin shotclutch -- -o <absolute-candidate.exe>`. Do not terminate the user's editor. After successful qualification, delete only superseded task-generated executable files in the verified build folder. Leave locked files for removal after the user closes them.

## Local real-model run

Install Tiny English · Fast explicitly through Setup, including its pinned isolated Python runtime. Use an isolated data directory for qualification. Listen to the main source audio to verify intelligibility. Video placement uses that main audio by default; additional container tracks are an explicit choice in Audio Channels. Smart Cut exposes track selection only under Advanced source settings. Commands use zero-based stream indices; the first track is Main audio in the UI. Empty auxiliary tracks are not a requirement to audition or add to a sequence.

```powershell
$env:SHOTCLUTCH_DATA_DIR = '<private-data-folder>'
$env:SHOTCLUTCH_M2_FOOTAGE = '<private-original-path>'
$env:SHOTCLUTCH_M2_STREAM = '0'
$env:SHOTCLUTCH_M2_REVIEW_PROJECT = '<private-output-folder>\analysis.scproj'
Remove-Item Env:SHOTCLUTCH_M2_EXCERPT_SECONDS -ErrorAction SilentlyContinue
cargo test --locked -p shotclutch-engine --lib m2_private_fast_whisper_qualification -- --ignored --nocapture
```

The original must fit the 8 GiB per-phase verification budget. Hashing streams through a bounded buffer; the budget is bytes read, not an 8 GiB allocation. Fast Whisper decodes five-minute chunks plus context and stores completed coverage. Legacy Candle Whisper and silence-only use 20 seconds; the older ignored `m2_private_whisper_qualification` remains available with the `whisper` feature. The fast test selects Gling-compatible automatic cleanup. It reports aggregate duration, chunks, word/candidate/group counts, elapsed time and result duration, and saves the private project. Record sampled process-tree working set, including the owned speech worker, with sample interval and host/load conditions. For short diagnostics, set `SHOTCLUTCH_M2_EXCERPT_SECONDS` to `start:end`; clear it for full-recording qualification. Never substitute saved-transcript refresh for a claimed real-model run.

For reference comparison, first run both rule engines against identical timed words and settings, compare intervals/types and account for inward frame/sample snapping. Then run fresh recognition serially with the same model, runtime versions, CPU threads, beam size, VAD, language and context settings. Record that the reference reads the whole recording while Shotclutch uses bounded chunks. Separate model loading, source verification and decoding where measured. Keep detailed intervals/transcripts local; output duration alone is not an accuracy metric.

## Native mechanics

```powershell
python scripts/m2/qualify-workspace.py --exe '<absolute-candidate.exe>' --project '<private-output-folder>\analysis.scproj' --output target/m2-qualification/native-final
```

This launches a separate app with isolated data and an authenticated loopback control channel. It uses Gling-compatible cleanup by default; `--cleanup-mode natural` or `transcript` explicitly selects the earlier modes. It exercises original-context listening, pause restoration, take comparison/retention, highlighted phrase Cut / Uncut, result transport, compatible full audio refresh, Open in editor, duplicate application, save/reopen, grouped undo/redo, and rejection of replacement after editing the generated sequence. It verifies the input project checksum. Generated projects, screenshots and logs stay local. The script terminates only its own instance on failure.

The output contains `summary.json`, `reviewed.scproj`, and `editorial-labels.csv`. Automated pause restoration demonstrates the action; it does not assert that the pause was intentional. Mechanical transport checks do not prove smooth video playback or acceptable audible joins. Inspect those during the human pass. The linked A/V regression independently checks preview/application picture agreement, omitted captions, selected stream, and exported frame/sample counts.

During refresh, the harness checks that media controls remain within the viewer, actions stay at the top, and an unchanged map does not rebuild its preview on each analysis checkpoint. For real-model responsiveness, start Transcribe & cut on a saved result, check wide and compact layouts during processing, then cancel after several completed chunks. `perf.stats.smartCut` exposes preview map/revision/build counts for this local diagnostic. Control-channel round trips include transport/scheduling overhead and must not be reported as displayed video frame times.

## Human editorial pass

Open the candidate and private reviewed project. Listen to the selected dialogue track and representative alternatives. Review the worksheet's source-time ranges in original context, including leading/trailing quiet, short pauses, fillers, restart cuts and takes. The deterministic sample includes up to eight candidates per category; it is a structured review sample, not an unbiased estimate of all cuts.

For each row record `accept`, `restore`, or `adjust`, leaving untouched rows `unreviewed`. Record a short local note for any false cut or poor join. `automatic_removed_seconds` distinguishes actually removed candidates from kept review material. Overlapping candidates must not be summed as total removal duration.

In the native UI, deliberately preserve one meaningful pause, compare and choose a take, restore a phrase, preview the joins, apply, save/reopen and undo/redo. Confirm that reanalysis preserves those decisions and an edited generated sequence cannot be silently replaced. Also inspect a fresh-sequence application after replacement rejection. Record review time and take choices/agreement locally.

```powershell
python scripts/m2/editorial-metrics.py target/m2-qualification/native-final/editorial-labels.csv
```

The summary reports reviewed coverage and sample-row restoration/adjustment/accepted-removal rates. Rates remain null when no automatic removals have human labels. They do not certify transcript word accuracy or whole-recording quality. Copy only redacted counts, rates, review time and explicit acceptance/failure to the evidence register. If defects remain, fix and rerun affected checks before closing M2.

## Optional AssemblyAI qualification

Select AssemblyAI in Setup, provide the key locally, and explicitly start transcription on an approved excerpt. This uploads audio to the provider. Check progress, cancellation, resume, word timing, text correction and cleanup. Do not put keys in commands captured to logs, project fields, screenshots or committed evidence. The local M2 profile does not depend on a live cloud account. Record its live status separately; parser tests do not qualify the external service.
