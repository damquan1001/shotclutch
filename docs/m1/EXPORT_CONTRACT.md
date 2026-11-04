# M1 native export binding and publication

Implemented on `codex/m1-editing-foundation`, 2026-11-04. Covers part of EXPORT-001,
CORE-003/004 and TEST-001. This extends the inherited owner export interfaces; scoped
agent grants still cannot export. M1 qualification remains In Progress.

## Job inputs

Native `file.exportMedia`, Quick Export and queued exports retain the project snapshot,
document ID/revision, sequence, range, settings and destination. A queue item retains its
original document's path resolution even after the live session switches projects.
Original bytes are fingerprinted when the job begins execution, before its first frame;
queueing alone does not retain a strong byte snapshot for later execution.

Each job gets a separate media pool with copied host registrations and stored-path context.
Preview file caches and proxy toggles cannot supply the final export's picture or sound.
Explicit host-injected sources retain their supplied handles and are reported as
`hostProvided`, without claiming a file hash. Byte-imported originals retain their immutable
encoded buffers and import-time SHA-256 as `ownedOriginal`. Each job opens a new decoder from
those buffers, retaining both the original and its identity even if the live media cache clears.
They do not require a filesystem file under the label supplied by the byte-import host. These
buffers and opaque host handles are process-local; saving/reopening and portable collection
still require the host to retain the essential source assets.

For filesystem originals the job computes full SHA-256 and 256 KiB block fingerprints,
including nested sequences and resolved subclips. Their decoder reads use those bound readers:
each requested block is checked before its bytes reach the decoder. A mismatch permanently
invalidates the job, including when a later writer restores the original file contents.
Imported size/head-tail identity is an additional check. Historical projects without that
identity still establish a fresh job binding; this does not reconstruct an import-time hash.
After rendering, every filesystem original is reopened through the host and its complete hash checked
again before publication. Cancellation is checked between fingerprint blocks and before
publication. Source media are never rewritten by these operations.

Numbered-image membership and holes are frozen for the job. Every present original frame
gets its own hash and filename in the binding. A frame appearing later cannot silently fill
a planned hole. The inherited nearest-frame fallback for holes remains in effect. Missing
or intentionally offline media stay offline for the job; the inherited placeholder delivery
and `offlineMedia` report remain available. CLI output with offline media still exits with
failure unless the user selected `--allow-offline`. This is not a complete export-plan
permission or missing-media review UI.

Current native budgets: 512 referenced media items, 512 nested sequences, 100,000 original
files, 64 GiB total originals, and 100,000 numbered output files. Fingerprint memory is bounded
by block counts rather than file contents; the host's existing random-access file pool still
bounds open descriptors. Whole-file decoder fallbacks retain their existing memory needs.
Completed files larger than 64 GiB fail verification before publication. Budget/performance
qualification on large projects remains outstanding.

## Completed outputs

Native exports encode in a private temporary directory beside the destination, preserving
the requested basename/container extension. Caption sidecars are staged there too. Finished
files are synced through writable handles, then any existing destination permissions applied.
Originals, proxies, canonical project filenames and numbered source siblings are protected
from overwrite, including unused and offline project references.

The job reserves its intended output names against other jobs in this process. Before
publication it checks the destination versions captured before encoding. If an earlier
delivery changed, the new export fails and preserves that delivery. Initially absent names
use atomic no-clobber publication, so a concurrent creator is preserved. Existing ordinary
files are atomically replaced after validation; symlinks/directories are refused. Cross-process
conditional replacement of an existing file is not a filesystem compare-and-swap: the final
validation/rename interval remains a qualification limit. Directory sync is best effort on
hosts that cannot flush directory handles.

A single-file delivery has one atomic publication point. Cancellation/failure before that
point preserves the prior file and removes private staging. Cancellation after publication
returns the completed result; it does not delete the delivery. Job completion is deferred
until source verification and publication finish.

Image sequences, sidecars and MXF OP-Atom are file sets. All members are validated before the
first publication; ordinary later promotion failures restore prior members and remove new
ones. If restoration itself fails, the private directory and `previous/` backups are retained
with `RECOVERY_REQUIRED`, and the error names the recovery location. A file set is **not** one
atomic filesystem transaction. Physical interruption can leave a partially published set;
automatic restart recovery and a durable set-level completion manifest remain outstanding.
An interrupted process can also leave private staging directories. Do not assume an orphan
contains only disposable bytes: it may retain prior deliveries needed for recovery.

## Result contract and adapters

Successful native exports return `binding` from waited commands and `jobs.list`:

- `apiVersion`, `projectId`, decimal-string `revision` and `sequenceId`;
- exact decimal-string range endpoints and a canonical settings SHA-256;
- source IDs, original SHA-256/byte counts, numbered-frame names, or explicit offline/host state;
- output basenames, byte counts and SHA-256;
- `publication`: `atomicFile` for one member, `fileSet` for several.

This report is session/job data, not a separately persisted export manifest. A caller needing
an audit artifact must retain the result. Full codec validation uses the independent local
oracle/corpus checks; output hashing alone does not establish semantic codec correctness.

MCP forwards the confirmed engine result and binding. It requests cancellation and waits for
the worker; it no longer deletes final files based on names/timestamps. A missing/incomplete
job cannot claim export success. If cancellation cannot be confirmed within its bounded wait,
the adapter tells the caller to inspect `jobs.list`.

Browser/in-memory delivery and direct `shotclutch-export` library callers retain their existing
host sink/file semantics. The native job wrapper is the publication boundary described here;
it does not claim equivalent atomic delivery for those other hosts. Native callers supplying
an already-open output handle cannot use this path-based atomic wrapper.

## Evidence and remaining work

Deterministic tests cover strong input/output hashes, independent WAV frame/sample reading,
changed/restored decoder bytes, changes after rendering, stale preview caches, frozen image
holes, protected/offline sources, queued relative paths and identity, cancellation around
publication, concurrent destinations, multi-file rollback and retained backups after rollback
failure. A fractional 30000/1001 range checks absolute 48 kHz sample endpoints independently:
frames 1–4 produce 4,805 samples, rather than flooring the duration in isolation.

The real MCP line-protocol cancellation test preserves an existing approved delivery. These
headless `m1_` regressions join the existing single-job gate; full engine/export/MCP and CLI
regressions run locally. Actual results are recorded in [IMPLEMENTATION](IMPLEMENTATION.md).

The bounded Windows SDR decoder/fallback corpus, mixed-rate/VFR/retimed/nested journey,
independent frame/sample agreement and same-PC collection/relocation now have local evidence in
[DEC-006](DEC-006_NATIVE_MEDIA.md) and [IMPLEMENTATION](IMPLEMENTATION.md). Those results apply
to their declared synthetic/native profile, not every export format or platform.

Still open: disconnected acceptance, long-form physical A/V sync and full manual editing
qualification. Wider storage/performance promises, physical interruption and multi-file restart
recovery remain unqualified. No M1 requirement is marked Done by this implementation alone.

Physical second-PC collection/relink is excluded from M1 by user decision (2026-11-04),
not recorded as passed; see the current scope note in IMPLEMENTATION.
