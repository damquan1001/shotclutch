# M1 structural service contract

Implemented on `codex/m1-editing-foundation`, 2026-11-02. This is the bounded structural
subset of [05](../05_EDITOR_COMMAND_AND_TRANSACTION_MODEL.md), covering AGENT-001/002
and part of SEC-002. Visual editing, rendered verification, export grants, provider work
and a general permission UI remain outside this subset. M1 remains In Progress.

## Host authority and adapters

Only trusted host Rust code can construct an `EditorScope` and issue an `EditorClient`.
The opaque handle is process-local, revocable, project-bound and expiring. Neither its
nonce nor its granted folders are serialized into a project. Requests cannot supply an
actor, issue a grant, extend a lease, change policy or choose output paths. Actors are
assigned by the host; receipt lookup requires the same actor and current scope.

The existing unrestricted CLI, desktop command registry and authenticated owner bridge
retain owner authority. They are trusted host interfaces, not scoped agent transports.
New transaction commands use the same native trim/split commands and durable commit
implementation. Headless CLI/MCP can explicitly select the scoped profile:

```powershell
shotclutch-cli --project project.scproj --scope-sequence 12 inspect
shotclutch-cli --project project.scproj --scope-sequence 12 --allow-edit --data-dir .local-data mcp
shotclutch-cli --project project.scproj --scope-sequence 12 --allow-edit --data-dir .local-data --save transaction request.json --commit
```

`--scope-sequence` defaults to read-only. `--allow-edit` enables bounded structural edits
and durable recovery for this process without changing saved preferences. `--transactions`
enables durability for the owner profile; `transaction` enables it automatically.
Repeat `--media-root <folder>` to grant additional source folders; the startup project's
folder is also a trusted launch grant. `--source-budget <bytes>` adjusts the per-phase
budget. These are host launch options, never request parameters.

Scoped `--save` requires `--allow-edit` and `--project`. It is explicit host finalization
to that exact startup path. Scoped command calls cannot save, Save As, import, export,
invoke arbitrary engine commands, read files, render frames or control the UI. Scoped
bridge mode is rejected. The owner bridge remains unrestricted and must not be given
to a client intended to have scoped access.

MCP uses `command_run` for the service methods; `doc_inspect` and resources use scoped
queries. Cancellation signals reach staging and source verification. Cancellation after
the durable marker cannot reverse a commit; the engine returns the committed receipt.
The complete MCP protocol hardening/compatibility program remains M3 work.

## Wire contract

`transaction.preview` accepts this versioned request. Project IDs and transaction IDs are
UUIDs. Entity IDs, revisions and exact tick values are canonical decimal strings; numeric
IDs/times, leading zeroes and unknown request fields fail. Times use the inherited
254016000000 ticks/second timebase. Targets are explicit, independent of UI selection.

```json
{
  "apiVersion": "1",
  "operationId": "op_trim_001",
  "idempotencyKey": "request_trim_001",
  "projectId": "aaf380d5-3f81-4c53-b26e-ff3f84787f60",
  "expectedRevision": "42",
  "sequenceId": "12",
  "label": "Trim tail",
  "edits": [
    {"command": "trim", "clip": "35", "edge": "out", "mode": "regular", "delta": "-8467200000", "linked": true}
  ]
}
```

Supported edits are `trim` (in/out, regular/ripple, explicit linked behavior) and
`split` (`clips`: decimal-string array, `time`: exact tick string). A group stages on a
private snapshot, rejects locked targets and invalid ranges, and commits one undo item.
An intervening revision or project replacement invalidates the preview. There is no
automatic rebase. The CLI `transaction` convenience command previews and optionally
commits in one process; a standalone preview expires when that process ends.

Preview returns `preview_ready`, identity/base revision, expiry, changed clip IDs and
before/after track/source ranges, source count and structural verification. It does not
claim rendered visual/audio verification. Commit returns `committed`, prior/new revision,
changes, ordered typed commands, label, source SHA-256/byte bindings (without paths), warnings,
undo availability and receipt retention deadline. Service errors use
the stable codes in [05](../05_EDITOR_COMMAND_AND_TRANSACTION_MODEL.md); the adapters
retain those codes in their error text.

`transaction.commit`, `.abort` and `.inspect` take:

```json
{"apiVersion":"1","projectId":"aaf380d5-3f81-4c53-b26e-ff3f84787f60","transactionId":"8842c739-9508-4caa-9ce5-e3e4d096f46e"}
```

`operation.inspect` takes `apiVersion`, `projectId`, `operationId`, returning the stored
result and current observed revision. Abort discards a draft; committed edits require
undo. `project.inspect` lists only authorized sequence IDs and current revision.
`sequence.inspect` accepts optional `item` as a decimal string and reports exact structural
IDs/ticks plus observed revision. `command.list` lists only service methods, their
parameters, version, required capability, effective enabled state and preview/undo behavior.
Legacy owner query/command shapes remain compatible.

## Source binding, retries and durability

Referenced originals and proxies, including nested sequences and resolved subclips, are
hashed with full SHA-256 during preview and again before commit. Existing imported
head/tail identities are also checked. Missing/offline media, changed bytes, ungranted
folders and exceeded budgets block publication. A same-size change in the middle of a
file is detected even when its inherited fast fingerprint still matches. Numbered image
sequences fail with `CAPABILITY_UNAVAILABLE` until a complete per-frame manifest is implemented;
hashing just the first image would not establish source identity.

Native reads use retained directory capabilities from pinned `cap-std` 4.0.3, rather than
an ambient file open after path validation. Relative traversal and symlink/junction escapes
cannot authorize byte reads outside the root. Network/device path prefixes are rejected.
The [cap-std directory API](https://docs.rs/cap-std/4.0.3/cap_std/fs/struct.Dir.html) and
[upstream implementation](https://github.com/sunfishcode/cap-std) describe this confinement
model. `cap-std` is licensed Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT;
the lockfile pins the resolved dependency graph. This does not establish hostile-decoder
process isolation or eliminate blocking filesystem/device behavior. Browser hosts do not
receive native directory grants.

The service checks scope, current project and cancellation before work, between staged
commands, during each source-read chunk and before durable commit. Expiry/revocation
prevents retries as well as new edits. Session shutdown revokes handles and releases roots.

Idempotency is actor/project scoped within the structural operation family. The digest
covers the full canonical request. Repeating a key/operation ID with identical arguments
returns its preview or committed result; different arguments fail. Replaying a committed
receipt never reapplies the edit, including after undo. A new authorization for the same
host-assigned actor can inspect/replay committed results but cannot take over another
authorization's uncommitted draft.

The complete project, revision, inverse/history and receipt share one checksummed atomic
recovery snapshot. Publication waits for storage acknowledgement. Native save/reopen and
crash recovery retain receipts. `edit_state.version` is now 2; version 1 remains readable.
Readers that only support version 1 reject version 2 rather than silently losing receipts.
Recovered receipt identities/revisions are validated before replacing the live project.
Grants and uncommitted previews are never recovered.

## Limits and remaining qualification

| Resource | Current limit/policy |
|---|---|
| Host grant | Default 15 minutes for a scoped sequence; maximum 1 hour |
| Live authorizations | 16 per session; maximum 32 roots per grant |
| Request/group | 256 KiB JSON; 1–128 edits; label 1–200 bytes |
| IDs/keys/actor | 1–128 ASCII alphanumeric or `_.:-` bytes |
| Pending previews | 8 per authorization, 32 total; expire after 5 minutes or grant expiry |
| Source verification | Default 512 MiB per phase; 512 sources/nested sequences |
| Structural preview | 100,000 clip IDs |
| Durable receipts | 128, retained for 7 days; no unexpired receipt eviction |

Expired receipts are pruned by the next successful commit. After the supported retention
window, refresh/query current state and review a new operation; an old result is no longer
guaranteed retrievable. A full receipt store fails before mutation. Undo history retains
the existing independent 200-snapshot cap.

Local tests cover denial, actor spoofing, readonly scope, cross-project/sequence access,
expiry/revocation, cancellation, preview limits, changed sources, traversal/junction escape,
retries after undo/restart, receipt bounds and crash-marker faults. Real CLI process and MCP
line-protocol tests exercise the adapter boundaries. These checks join the focused M1 gate
where headless; CLI/platform and full regression checks run locally. See
[implementation evidence](IMPLEMENTATION.md).

Outstanding: general scoped mutations, an interactive grant/review UI, rendered verification,
export/original pinning across entire render jobs, storage/latency limits for large histories,
hostile-media worker isolation, all-platform filesystem qualification, complete offline
mixed-media workflow, physical A/V sync and second-machine project collection qualification.
