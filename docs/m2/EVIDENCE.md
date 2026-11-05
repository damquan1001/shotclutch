# M2 verification record

## Windows local checks

| Check | Result |
| --- | --- |
| Native desktop build with default Whisper feature | Passed |
| Native workspace smoke | Reopened the private v18 project, resolved its original, loaded Smart Rough Cut, and created a review revision in memory. |
| Time, project, format and edit library tests | 191 passed with native filesystem permissions |
| M1 engine regression filter on the M2 build | 81 passed |
| M2 engine regression filter | 7 passed. Private model test runs separately. |
| Pinned Whisper base installation | Four artifacts matched the catalogue size and SHA-256. |

The M2 regressions cover half-open mapping and restoration, explicit invocation, compatible reanalysis, durable cancellation and resume, caption omission, stale application rejection, changed media, save/reopen, undo/redo, and duplicate apply. The linked A/V fixture compares the result frame before and after apply and independently checks the exported container for six video frames and 12,000 audio sample frames. The original bytes are also checked after export. Windows atomic save tests need filesystem permissions that the workspace sandbox does not grant. The hosted M2 step adds the headless filter to the existing M1 gate.

## Private recording

The acceptance source is about 37m 52s, with 4K 60 fps H.264 and four stereo 48 kHz AAC streams. A local level scan used 12-second windows starting at 180, 900 and 1800 seconds. Stream 1 had RMS levels of −34.4, −34.8 and −34.0 dBFS. Streams 2–4 were silent in those windows. Stream 1 is the automated qualification input. The native listening and editorial walkthrough remains open.

The pinned CPU model completed the 180–200 second excerpt on stream 1. It produced 46 words and two suggestions. The disposable result sequence matched the indexed source range duration and retained the selected audio stream. Analysis and source verification took 12.2 seconds in the local test run.

The full local run completed on stream 1 with **2,271.551 seconds of coverage**, 114 checkpoints, 4,031 words, 160 suggestions, and 16 take groups. The test confirmed that the disposable sequence retained the chosen stream and covered the exact input duration. Processing and source verification took 783.7 seconds. Windows reported a 756 MiB peak working set when sampled during the run. The review project saved successfully to the ignored qualification directory. The count of suggestions and groups measures proposal generation. Editorial accuracy still needs listening and labels.

The automated desktop smoke used an isolated native instance and left the saved review project ready for the editorial walkthrough. Human judgment of intentional pauses, take quality, and joins is the remaining acceptance gate. Keep the original, project, transcript, and any detailed editorial labels local.

## Script-led UI redesign — 2026-11-06

The workspace was reworked using the supplied Gling screenshot and the local OpenGling layout as references. Native verification covered the Script, Suggestions, and Files & analysis tabs, original and result pictures, transcript click-to-seek, waveform click-to-seek, and the full-width transport. Screenshots were visually inspected at 1440×950 and 1024×720. The original 4K seek needed additional decode time before its picture appeared. The isolated walkthrough left the saved private project unchanged. Screenshots and the walkthrough script remain in ignored local qualification storage.

`cargo check --locked -p shotclutch -j 6` passed. The standard build compiled dependencies but could not replace the older executable because that app was running. Linking the same app with `cargo rustc --locked -p shotclutch --bin shotclutch -j 6 -- -o <local separate executable>` passed, and the native walkthrough used that new executable. Formatting and diff whitespace checks passed. This UI change does not close the human editorial acceptance gate above.

## Automatic cut review

The native fixture opened on the generated cut and verified result playback, original playback, Space to pause, and opening the edit as a sequence. The saved private transcript generated 136 cuts and a 31m 11s preview from the 37m 52s original. The visual walkthrough used the saved analysis and kept its project and screenshots local.

Eight focused M2 engine tests passed. The new regression covers automatic preview and sequence creation without a separate approval step, then verifies that a restored pause survives another cleanup run. The private ASR qualification test was filtered out of this run.
