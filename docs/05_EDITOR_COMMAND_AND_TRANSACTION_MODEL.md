# ShotClutch — Editor Command and Transaction Model

Baseline 0.1 · 2026-10-26 · Governs AGENT-001, AGENT-002, CORE-004, PRE-004, and SEC-002.

## One mutation boundary

Human input, agents, CLI scripts, MCP tools, and ACP-mediated agent actions dispatch the same typed commands. Clients may maintain local views but cannot mutate canonical project data directly. The command registry exposes names, schema versions, parameters, required capabilities, affected entity categories, preview support, reversibility, and job behavior.

Separate operations into queries, project mutations, derived-data jobs, and external side effects. Queries do not change editorial state. Analysis jobs publish versioned results through validated completion commands. File export, network submission, and provider spending are side effects with separate permissions and cannot be made reversible simply by wrapping them in project undo.

## Proposed command envelope

Wire names below are ShotClutch application contracts, not claims about existing protocol methods.

```json
{
  "apiVersion": "1",
  "operationId": "op_example",
  "projectId": "project_example",
  "expectedRevision": "42",
  "idempotencyKey": "request_example",
  "command": "timeline.trim",
  "arguments": {
    "clipId": "clip_example",
    "edge": "out",
    "time": { "domain": "sequence", "objectId": "sequence_example", "ticks": "96000" }
  }
}
```

Actor identity, authenticated session, effective grants, and authorization context are assigned by the service, never trusted from arbitrary client fields. Every time value follows [04](04_PROJECT_DATA_MODEL.md). Optional comments or rationale are bounded text and cannot alter command semantics.

Validation order: authenticate → resolve project → check capability → validate schema → check expected revision → resolve IDs and locks → validate domain invariants → compute effects → preview or commit. Large computation may run against a snapshot, but permissions, revision, and referenced content are rechecked at commit.

## Result and error contract

A completed mutation returns operation ID, transaction ID, prior/new revision, affected IDs/ranges, warnings, and undo availability. An asynchronous operation returns a job ID and accepted state; it does not claim completion. Queries declare their observed revision and freshness. Error responses use stable codes, a short human explanation, safe diagnostic details, and retry guidance.

| Error code | Meaning / response |
|---|---|
| `INVALID_ARGUMENT` | Fix schema, range, or time domain before retry |
| `NOT_FOUND` | Object or retained revision is unavailable; refresh state |
| `REVISION_CONFLICT` | Preview/request is stale; refresh and prepare a new transaction |
| `PERMISSION_REQUIRED` | Obtain the specifically requested capability through trusted UI |
| `PERMISSION_DENIED` | Policy forbids the action; do not route around the denial |
| `CAPABILITY_UNAVAILABLE` | Requested backend/feature is unsupported; present a supported alternative |
| `MEDIA_CHANGED` | Source fingerprint differs; relink/reanalyze/review explicitly |
| `VERIFICATION_FAILED` | Declared verification criteria were not met |
| `RESOURCE_LIMIT` | Budget, time, memory, or concurrency ceiling reached |
| `CANCELED` | Operation stopped; response identifies durable work already completed |
| `INTERNAL_ERROR` | Unexpected failure; attach a redacted diagnostic ID |

Idempotency keys are scoped to authenticated actor, project, and operation family. Repeating the same key with the same canonical request returns the existing result or job. Reusing it with different arguments fails. Retention covers the supported retry/recovery window; after expiry clients must query known transaction IDs before issuing a new request. Idempotency is not permission to repeat an external side effect.

## Transaction model

An AgentTransaction includes intent, actor, base revision, ordered commands, read/write sets, affected ranges, source fingerprints, permissions, preview artifacts, verification criteria/results, and commit/rollback state. Ordinary human command groups use the same mechanics. Commands inside a transaction see earlier staged changes, but other clients see only the committed project.

```text
draft -> validated -> preview_ready -> verified -> committed
   |          |              |           |
   +----------+--------------+-----------+-> aborted
```

Verification can be `not_required`, `passed`, `failed`, or `inconclusive` according to operation policy. A structural edit may need invariant checks only; a visual agent edit normally needs rendered evidence. A policy-approved override must record who accepted the limitation. Failed or inconclusive required verification blocks automatic commit.

1. **Begin:** pin base revision, source identities, grant scope, and intended result.
2. **Stage:** apply commands to an isolated view; collect inverse data or a restorable snapshot.
3. **Validate:** enforce locks, referential integrity, timing, media availability, and budget limits.
4. **Preview:** render affected ranges from the staged view; show object/range and duration changes.
5. **Verify:** run applicable structural, temporal, visual, and audio checks against the staged view.
6. **Authorize:** use existing grants or request approval tied to this concrete preview/digest.
7. **Commit:** atomically compare-and-swap the base revision, persist the complete group and history, then publish events.
8. **Abort:** discard staged project changes and release temporary resources. Report any independently completed side effects.

Changing commands, sources, output destinations, preview content, or effective grant scope invalidates the relevant approval and verification. Long-running drafts have a visible expiry and resource limit.

## Concurrency and human interruption

Use project-wide optimistic revision checking for v0.1. Any intervening canonical mutation invalidates a transaction's base revision. More granular conflict handling can be added only after read/write-set correctness is demonstrated. Never auto-rebase an agent edit silently over a human change.

The user can keep editing during analysis or rendering because jobs read snapshots. A new edit may make a planned transaction stale; it does not invalidate a completed render's identity. Locked tracks and clips require explicit unlock commands and applicable permission. An agent cannot treat a UI lock as advisory.

## Undo, redo, rollback, and side effects

Before commit, rollback discards staged work. After commit, undo is a new validated mutation that restores the transaction's previous project state; revisions remain monotonic. One agent transaction appears as one history item with an inspectable command list. Redo replays only when referenced resources remain valid. New edits after undo follow the editor's declared redo-branch policy.

Undo restores project references to generated assets but need not delete the generated files. External uploads, paid generation, sent requests, and exported files are not undone by timeline history. Such actions use separate jobs, permissions, manifests, and explicit cleanup policies. Do not report “rolled back” as though uploaded data has been recalled.

## Crash consistency and job lifecycle

Durable states are `queued`, `running`, `cancel_requested`, `succeeded`, `failed`, and `canceled`. Progress is monotonic within a phase and must distinguish estimated completion from known units. Checkpoints identify source hashes, algorithm versions, and completed chunks. Resume revalidates all inputs.

A transaction writes a durable prepared record and atomic commit marker or equivalent database transaction before events are published. Recovery ignores incomplete staged changes and reconstructs committed revisions once. A crashed client can discover its operation through idempotency/operation IDs. Final artifacts are staged in temporary locations and promoted atomically where supported. Promotion failure is reported and does not corrupt the project.

## Representative operation families

| Family | Examples | Particular constraint |
|---|---|---|
| Project/timeline | `project.inspect`, `timeline.split`, `timeline.trim`, `timeline.move` | Explicit IDs, time domains, revision checks |
| Rough cut | `preedit.analyze`, `preedit.review`, `preedit.selectTake`, `preedit.apply` | Manual initiation and immutable map revisions |
| Visual understanding | `vision.search`, `vision.inspect`, `vision.contactSheet` | Bounded evidence, source/composition distinction |
| Motion | `motion.create`, `motion.update`, `motion.renderPreview` | Sandboxed code and locked dependencies |
| Transactions | `transaction.preview`, `transaction.commit`, `transaction.abort` | Approval binds to the exact staged change |
| Delivery | `export.plan`, `export.render`, `export.package` | Destination permissions and explicit fidelity |

## Required verification

Test stale commits, duplicate requests, cancellation during each phase, crashes before/after commit, permissions revoked during preview, missing resources during redo, failed file promotion, locked tracks, and concurrent UI edits. Compare equivalent commands through all adapters. A transaction must either appear fully committed with its journal entry or not appear at all.
