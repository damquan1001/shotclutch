# M1 native collected-project contract

Implemented on `codex/m1-editing-foundation`, 2026-11-04. Owner: Codex. The user excluded a physical second-PC check from M1 on 2026-11-04. Local relocation and integrity
checks qualify M1 portability; physical cross-machine checks are deferred beyond this milestone
and do not block M1 exit. Local byte verification does not establish decoder, font, physical A/V
or storage-crash qualification.

## Commands and document modes

`file.projectManager` with `mode: "collect"` builds a native collection. `destination` names a
**new directory whose parent already exists**. The desktop folder picker proposes a new
`CollectedProject` subfolder. Existing files, directories and links are never replaced, including
when `overwrite: true` is supplied. Choose another destination for another delivery.

- Default `excludeUnused: true`: dependency closure of the selected sequences, including nested
  sequences, subclip parents, multicam/merged references and authored graphics. This is an
  explicit `derivedCopy`: new document UUID, revision 1, no undo/redo or operation receipts.
  Retained item IDs and allocator high-water mark survive. Project-level notes and embedded LUTs
  remain authored project data; subset collection is not a redaction tool.
- `excludeUnused: false`, every sequence selected (or omit `sequences`): `fullDocument` by default.
  Preserve document UUID/revision, view, allocator, up to 200 undo and 200 redo states, and retained
  structural-operation receipts. Collect and rewrite file dependencies in **every retained state**,
  including originals only referenced by undo. Conflicting imported identities at one historical
  path fail; today's bytes never silently substitute for a different historical take.
- `includeHistory: false` explicitly requests a new derived identity even for an unpruned copy.
  `includeHistory: true` requires an unpruned complete document. A subset cannot import hidden
  history from unselected sequences. Invalid sequence IDs, malformed booleans and names fail.

`dryRun: true` reports the mode, omitted caches, history/receipt counts, required environment,
relative output mapping and estimated media bytes without creating a directory or job. Project
and manifest overhead are additional to the media estimate. A dry run assigns a fresh proposed
identity; it does not reserve a package or pin source bytes for a later command.

Owner CLI calls wait by default; native desktop calls use a background job by default. Explicit
`wait: true` returns `state: "committed"` only after publication and propagates job failure.
MCP/bridge waited calls use the existing job progress/cancellation path, keeping the session
available to other requests. `jobs.list` is authoritative for an asynchronous result.

## Essential data, caches and environment

`collection.json` version 1 records collection operation UUID, request digest, source document
UUID/revision, collected UUID/revision, document mode, retained history/receipt counts, relative
project name/hash and file assets. Revisions and source item IDs use decimal strings. Each asset
has relative path, source item ID, SHA-256, byte count, role and an explicit `essential` flag:

| Role | Essential | Meaning |
|---|---|---|
| `original` | Yes | Complete encoded original; no trim/transcode or preview decoder reuse |
| `ownedOriginal` | Yes | Immutable encoded bytes imported by the host, materialized into the package |
| `generatedOriginal` | Yes | File-backed authored narration; generating it again is not a substitute |
| `proxy` | No | Explicitly included attached proxy; original remains essential |

Numbered images retain every discovered frame and original numbering, including holes, in a
private per-source folder. Members are not flattened or reduced to the first frame. Separate
folders avoid same-basename and case collisions. Changing sequence membership during copying
aborts; adding an undeclared member to a finished package fails verification.

The native document embeds LUT text, graphics/captions, transcripts, narration metadata, notes
and generator descriptions. Informational LUT import paths are removed. Scratch paths are reset,
ingest is disabled and its destination cleared, in current and historical states. Original source
paths do not appear in the automatic manifest mapping. Authored text/notes are preserved and may
contain private information; collection does not sanitize authored content.

Render previews, waveforms, thumbnails and recovery/auto-save files are explicitly rebuildable
and omitted. `includePreviews: true` fails with `CAPABILITY_UNAVAILABLE` rather than recursively
copying private caches or following links. `includeProxies` defaults true: a requested missing
proxy is an error; `includeProxies: false` deliberately removes proxy attachments in all retained
states. Missing/unreadable originals always fail; no filename-based relinking is attempted.
Intentional-offline flags remain authored state, even when the corresponding original is copied.

System fonts and decoder implementations are environment requirements, not redistributed assets.
The manifest makes that limitation explicit; byte verification cannot promise identical rendering
with different fonts, build capabilities or decoder versions. Unknown optional extensions still
round-trip through normal native save/recovery. Collection fails closed when such an extension is
present because it has no provider to enumerate and validate the extension's external dependencies.

## Source binding, verification and publication

The job retains a project/history snapshot, resolved source context and owned byte buffers.
It fingerprints file sources into complete SHA-256 plus checked 256 KiB blocks before copying;
reads through those blocks, checks imported fast identities where present, and reopens/hashes
originals again before delivery. A stale preview cache or enabled proxy cannot replace an original.
History-only references pass stored-document path policy before host I/O, including network-share
restrictions. No live project, source file or source project document is rewritten by collection.

Files are copied into a private `.shotclutch-collect-*` sibling directory. Each file, the complete
native project and the manifest are synced, then read back and verified. Publication renames the
complete directory exclusively on the same filesystem. Cancellation or ordinary failure before
that point cleans only the private staging directory. Success wins cancellation after publication.
Concurrent occupied targets, including empty directories, cannot be replaced.

On Windows this uses the pinned safe `atomicwrites` 0.4.4 `move_atomic` API, whose Windows
implementation calls MoveFileExW without replacement; the collision regression exercises empty
and nonempty targets. The standard rename was insufficient in the initial Windows test.
Linux/Android and Apple targets use existing locked `rustix` 1.1.5 `renameat_with(NOREPLACE)`.
Other platforms fail explicitly until an exclusive directory primitive is implemented. No
unsafe code or platform-media dependency was added to the engine. References:
[atomicwrites implementation](https://docs.rs/crate/atomicwrites/0.4.4/source/src/lib.rs),
[rustix rename API](https://docs.rs/rustix/1.1.5/rustix/fs/fn.renameat_with.html).

`file.verifyCollection {"path": "<collection directory>"}` is a read-only owner command. It reads
through one retained directory capability, rejects absolute/foreign/traversing/device/link paths
and case collisions, verifies all declared files and the native project's integrity/history,
checks every current/historical file reference against role/source mapping, and rejects undeclared
files. A manifest is content metadata, never authorization. Serialized receipts do not restore host
grants, and scoped editors cannot invoke collection or filesystem verification.

The manifest describes the **delivered snapshot**. Editing/saving that project changes its hash;
verification then truthfully rejects the changed delivery. Keep the received package intact and
Save As to a working copy. Normal `file.open` remains an editable native project workflow and
does not automatically rehash the original delivery manifest on every save/open.

Native Save As and Save a Copy preserve the original resolution of relative file/proxy references
in the current project and retained undo/redo. References that would resolve differently beside
the destination become absolute; same-folder references can remain relative. Save As adopts the
prepared paths/history and retains live decoded/byte-owned sources only after a successful write.
Save a Copy and a failed write leave live state untouched. This creates a working document linked
to its existing media, not another collected package; use collect again to make it self-contained.
For a first save, an unbound authored relative reference still takes its meaning from the new
document's folder. An actual user-picked/cached file or owned byte binding retains its existing
working-directory meaning. No missing file is guessed or materialized by saving.

## Retry and resource boundaries

Supply and retain an `operationId` UUID to retry the same request at the same destination. If the
existing package verifies and its source document/revision and exact request digest match, return
its committed receipt without another job, even if the source media has since disappeared. Changed
arguments, changed source state, a different operation ID or a corrupted package fail without
replacement. This receipt is destination scoped; a different destination is a separate delivery.
If omitted, the engine assigns an operation UUID and returns it in the plan/start response.

Budgets: 64 GiB total copied originals/proxies, 100,000 files, 256 MiB encoded native document,
32 MiB manifest, 200 retained undo and redo states each, and bounded inventory traversal.
Budgets include historical dependencies and proxies. Exceeding a budget fails explicitly.

This is complete-directory publication, not a guarantee against physical storage failure.
A process/power loss may leave an incomplete private staging folder; it is not a published receipt
and is not automatically resumed. Directory-entry persistence across power loss, network-share
semantics, hostile concurrent modification of an owner-controlled output directory, and other
platform runtimes require further qualification. Verification is a snapshot check, not a signature
or a defense against an author who replaces both data and hashes. Legacy `consolidate`/`transcode`
keeps its separate workflow and does not claim this package transaction or portability contract.

## Local M1 acceptance and deferred cross-machine procedure

Local acceptance covers relocation with originals hidden/deleted, identical rendered movie frames
and timeline audio, history-only sources and working undo/redo, saved operation receipts without
authority, byte imports, image holes, proxy/generated/embedded data, corruption/path/junction
rejection, cancellation, source changes and publication collisions. CLI tests cross real process
boundaries. These run without new hosted workflows, network services or oracle tools.

The following physical second-PC procedure is retained for future qualification outside M1. It
is not required by this milestone (user decision, 2026-11-04):

1. Use the same M1 candidate build on both PCs. Keep build ID, OS, decoder and font inventory.
2. Collect the mixed-media qualification project with `excludeUnused:false`, `wait:true`, a fresh
   operation UUID and no existing destination. Retain the returned document/revision and manifest.
3. Put comparison frame/audio renders and acceptance notes **beside**, not inside, the package.
   Transfer the complete collection directory; disconnect network and make the original paths
   unavailable. Run `file.verifyCollection` on the second PC before opening.
4. Open the manifest's relative `.scproj`. Confirm no unexpected missing-media prompt, nested and
   linked edits, original/proxy switching, generated narration/LUTs/titles and independent frame/
   sample comparisons. Qualify fonts/decoders separately from byte identity.
5. Save As outside the delivered package, undo/redo edits that reference historical-only originals,
   reopen the working copy and render again. Confirm source hashes and the original package still
   verify. Retrying the original operation on the first PC must return its existing receipt.
6. Record actual results, failures and limitations in the M1 acceptance register. A same-PC moved
   directory, a build check, or a green headless test is not a second-PC pass.
