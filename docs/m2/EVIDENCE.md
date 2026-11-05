# M2 verification record

## Windows local checks

| Check | Result |
| --- | --- |
| Native desktop compile with default Whisper feature | Passed |
| Time, project, format and edit library tests | 191 passed with native filesystem permissions |
| M1 engine regression filter on the M2 build | 81 passed |
| M2 engine regression filter | 6 passed. Private model test runs separately. |
| Pinned Whisper base installation | Four artifacts matched the catalogue size and SHA-256. |

The M2 regressions cover half-open mapping and restoration, explicit invocation, compatible reanalysis, durable cancellation and resume, caption omission, stale application rejection, changed media, save/reopen, undo/redo, and duplicate apply. Windows atomic save tests need filesystem permissions that the workspace sandbox does not grant. The hosted M2 step adds the headless filter to the existing M1 gate.

## Private recording

The acceptance source is about 37m 52s, with 4K 60 fps H.264 and four stereo 48 kHz AAC streams. A local level scan used 12-second windows starting at 180, 900 and 1800 seconds. Stream 1 had RMS levels of −34.4, −34.8 and −34.0 dBFS. Streams 2–4 were silent in those windows. Stream 1 is the automated qualification input. The native listening and editorial walkthrough remains open.

The pinned CPU model completed the 180–200 second excerpt on stream 1. It produced 46 words and two suggestions. The disposable result sequence matched the indexed source range duration and retained the selected audio stream. Analysis and source verification took 12.2 seconds in the local test run.

Record inference and frame-index verification separately from the human judgment of intentional pauses, take quality, and joins. Keep the original, project, transcript, and any detailed editorial labels local.
