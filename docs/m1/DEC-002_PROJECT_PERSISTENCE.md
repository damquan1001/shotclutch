# DEC-002 — Canonical document persistence

Date: 2026-11-02. Owner: Codex. Implementation: `codex/m1-editing-foundation`.
Status: storage choice implemented; milestone qualification remains In Progress.
Requirements: CORE-002–004, AGENT-002, TEST-001.

## Choice

Retain the inherited copy-on-write project and exact integer/rational time model. Keep
254,016,000,000 ticks/second; do not rescale existing edits. Unusual-rate, VFR, nested and retimed
source mappings still require their bounded M1 qualification.

Extend the native JSON envelope to schema v17. The canonical document consists of the editorial
Project plus `edit_state`: version, opaque UUID document identity, monotonic revision, allocation
high-water mark, and retained undo/redo snapshots. Revision and allocation counters use decimal
strings. The engine Session owns this metadata in memory; it is not duplicated inside every
Project snapshot. `project.inspect` exposes `projectId` while retaining its existing reply fields.

Each history entry contains a label and a versioned project envelope without nested edit state.
Every history snapshot passes the ordinary loader/migration path before any document is replaced.
Corrupt or unsupported retained state rejects the load instead of silently losing undo data.
Native persistence retains up to the existing 200 history entries in each direction. A new edit
clears redo; ID allocation does not rewind when undo or history deletion removes an object.

Save As and Save a Copy preserve document identity. Template/subset project writers remain
project-only writers; opening their output adopts a new document identity. Legacy files adopt
identity in memory and acquire it on an explicit native save. An ID identifies a document and
does not authorize access to it.

## Commit and recovery boundary

The current project, retained history, revision, allocator state and recovery location/legacy
protection share one checksummed atomic snapshot. Recovery snapshots also retain the original
file's schema version, so recovery cannot bypass migration backup protection. The separately
written `session.json` supplies advisory display metadata; it is not the commit marker.

The native worker acknowledges an atomic snapshot before the transaction publishes live state
or project events. A failure before promotion leaves the current document unchanged. A crash
after promotion but before publication recovers the complete new revision and its grouped undo.
An advisory metadata failure after promotion reports a warning and preserves successful commit.

`commit_edits_durable` requires an enabled recovery worker. The original `commit_edits` remains
usable by in-memory hosts and acknowledges an attached worker when present. Neither API is yet
exposed as the complete external transaction protocol: scoped grants, idempotent receipts,
expiry, source revalidation and visual verification remain separate M1 service work.

Recovery validates a private document and acknowledges its replacement journal before switching
the live document or removing the old candidate. Failed writes preserve both the current document
and the old candidate for retry. Ordinary grouped commands suppress intermediate persistence,
preview publication and revision events; successful groups publish one revision/history item.

## Compatibility, integrity and limits

Schema v1–v16 and `.fcproj`/`filmcraft.project` readers remain supported. Opening never rewrites a
source file. The first manual overwrite of a migrated native project preserves its original
schema backup; automatic saving cannot overwrite that source before migration is explicitly saved.
Legacy originals retain their Save As protection.

Schema v17 writes SHA-256 integrity over the format/schema, project, edit state and recovery
metadata using canonical JSON ordering and exact floating-point parsing. It hashes the JSON wire
representation rather than widening f32 parameters in memory. Optional UI view state and the
writer label are excluded. A checksum detects corruption; it is not authentication for imported
projects. Existing files without a checksum remain supported through migration.

This chooses correctness using full retained snapshots rather than introducing a database or
new delta-journal dependency. File size and save/recovery work scale with retained history.
Large-project latency, storage budgets, physical power-loss behavior and non-Windows runtime
qualification remain open. Worker serialization runs off the UI thread; the initial internal
durable API waits synchronously for acknowledgement and needs asynchronous host integration
before a responsive external transaction workflow is advertised.


## Receipt extension — 2026-11-02

The same schema-v17 envelope now writes `edit_state.version = 2` with bounded retained
operation receipts (actor/project-bound request digests and committed results). Version 1
remains readable; future versions fail closed. This makes project, inverse/history, revision
and receipt one atomic recovery marker, so retries after restart do not reapply committed
edits. Authorizations, root capabilities and uncommitted previews remain process-local.
Receipt fields are validated during restoration before replacing the live project. Retention,
wire shapes and current limits are specified in [SERVICE_CONTRACT](SERVICE_CONTRACT.md).
