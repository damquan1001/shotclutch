# ShotClutch — ACP, MCP, CLI, and Skills

Baseline 0.1 · 2026-10-26 · PROTO-001 through PROTO-004.

## Distinct roles, shared capabilities

| Surface | Role | ShotClutch boundary |
|---|---|---|
| ACP — Agent Client Protocol | Coordinate an editor/client with an agent session | Session lifecycle, capability negotiation, progress, supported permission and cancellation interactions |
| MCP — Model Context Protocol | Expose tools/resources to model clients | Scoped editor queries, commands, analysis/inspection jobs, resource handles |
| CLI | Local/headless scripting and diagnostics | Machine-readable requests/results and job control through the same engine service |
| Skills | Reusable instructions, examples, templates, metadata | Agent guidance that cannot bypass permissions or execute implicitly |

ACP is not the editor's transaction engine, MCP is not a project database, and a skill is not an executable plugin. Adapters translate their supported protocol revision into the same editor service contracts. No adapter implements its own trim math or authorization policy.

This document specifies integration behavior, not exact external wire methods. Pin and verify the official protocol revisions and SDKs during implementation, record them in the compatibility manifest, and run conformance tests. Do not assume all clients support the same optional features.

## Common service contract

Expose API version, command schemas, capabilities, active project/revision, supported time domains, limits, and error categories. Use stable IDs and decimal-string ticks. Mutations require expected revision and idempotency where applicable. Long jobs return job handles with status, progress, cancellation, result manifests, and explicit terminal state.

All clients authenticate or bind to a trusted local session. Local transport alone is not proof of authorization: protect endpoints with OS access controls and scoped credentials. Bind to local interfaces by default; remote exposure is a separate explicitly configured feature. Rate-limit requests and isolate sessions/projects.

Queries and events declare their observed revision. Subscription clients handle reconnection and snapshot resynchronization. Large images, transcripts, or video excerpts use bounded resource handles rather than unbounded inline payloads. A resource handle carries origin, content type, scope, expiry, and revision/fingerprint metadata.

## MCP adapter

Discoverable tool families cover project inspection, timeline edits, transaction preview/commit, rough-cut analysis/review/application, semantic search, source/timeline inspection, MotionScene authoring/preview, and export planning/rendering. Resource families expose bounded project snapshots, job results, transcripts, evidence, and fidelity reports.

Classify tools as read-only, derived-data-producing, reversible project mutation, or external side effect. Tool annotations are useful hints, but the engine's policy enforcement is authoritative. A client cannot bypass an approval by invoking a lower-level equivalent command.

Validate schemas and time domains at the boundary. Return structured results plus short human-readable summaries. Tool errors preserve stable internal categories while following the pinned protocol's error conventions. Cancellation and progress use supported mechanisms of that revision; otherwise provide explicit job polling/cancel operations without pretending protocol features exist.

## CLI contract

The executable name is `shotclutch`. Proposed command families include project, media, timeline, preedit, vision, motion, transaction, export, and job. Final flags are an implementation contract to validate, not an installed interface claimed here.

```text
shotclutch project inspect --project ./ProjectFolder --json
shotclutch preedit analyze --project ./ProjectFolder --request ./analysis-request.json --json
shotclutch transaction preview --project ./ProjectFolder --request ./edit-request.json --json
shotclutch transaction commit --project ./ProjectFolder --request ./commit-request.json --json
shotclutch export plan --project ./ProjectFolder --request ./export-request.json --json
shotclutch job status --project ./ProjectFolder --job-id job_example --json
```

Use request files or standard input for complex payloads to avoid shell-escaping hazards. Machine mode writes one complete result or documented JSON Lines events to stdout; diagnostics/progress go to stderr. Define stable exit categories: success, invalid request, permission failure, conflict, unavailable capability/resource, job failure, and cancellation. Exact numeric assignments are versioned with the CLI release.

Noninteractive mode must fail clearly when approval is missing, never wait indefinitely for an invisible dialog. It may consume an existing scoped grant or explicit approved request token. A “yes to all” flag cannot create new global privileges. A running desktop session and headless CLI coordinate through one project service/lock; two independent writers must not corrupt the same project store.

## ACP integration

ShotClutch's primary role is the editor client coordinating a connected agent. Optional future service roles require separate capability negotiation. Session setup selects agent/provider, binds the project, advertises available capabilities, and applies user-configured budgets and grants.

Map supported session events into the agent lifecycle in [10](10_AGENT_ARCHITECTURE.md). Stream readable progress and tool activity, surface permission requests through trusted ShotClutch UI, propagate cancellation, and retain resumable session metadata only as supported by the chosen protocol. Agent text cannot mark its own permission request approved.

ACP-originated tool access uses the same scoped editor API or MCP adapter. Preserve operation IDs across bridges so retries and audit records do not duplicate edits. On disconnect, stop new work, reconcile in-flight job states, and require fresh validation before resuming a stale transaction. Protocol reconnect does not silently restore revoked grants.

## Skill package contract

A skill contains a manifest, instructions, examples, and optional templates/assets. Proposed manifest fields are ID, display name, version, description, author/source, supported ShotClutch API range, required tool capabilities, resource budgets, input/output expectations, and file hashes. The required-capabilities list is a declaration, not a grant.

```text
speech-cleanup/
  skill.json
  SKILL.md
  examples/
  templates/
```

Instruction precedence is application safety/policy → current user instruction and grants → project preferences → selected skill guidance → untrusted source/tool content. Skills cannot change this ordering. Imported project skills are untrusted until the user chooses to enable them. Updates show source/version changes and newly requested capabilities.

Skills may describe a workflow such as reviewing pauses, comparing takes, checking joins, and producing a transaction. They may not silently install dependencies, run scripts, contact providers, or modify files. If executable behavior is needed, it is a separately permissioned plugin or tool invocation. Template extraction must validate paths and reject traversal or executable surprises.

## Compatibility and acceptance

Maintain a matrix of ShotClutch API version, adapter version, protocol revision, tested clients/agents, transports, optional capabilities, and known limits. Schema evolution adds optional fields compatibly or increments versions for breaking changes. Unknown required features fail explicitly; clients cannot reinterpret a field silently.

Run equivalent project queries and mutations through UI/service, CLI, and MCP; test ACP session/permission behavior with a pinned pairing. Include duplicate requests, unknown fields, old versions, malformed resource handles, missing approval in noninteractive mode, disconnect during commit, and cross-project access attempts. Skill tests verify instructional loading and capability enforcement without executing packaged content implicitly.
