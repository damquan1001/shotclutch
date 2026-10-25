# ShotClutch — Agent Architecture

Baseline 0.1 · 2026-10-26 · AGENT-003 through AGENT-005 and AI-004.

## Agent role

The agent is a client of the editor service that combines user intent, project state, semantic retrieval, visual/audio inspection, and validated editing commands. It does not own a parallel project or write canonical files directly. The same safety, timing, and transaction rules apply to built-in and externally connected agents.

Support provider-independent model interfaces so local and explicitly authorized remote models can be configured. A provider's absence reduces available reasoning capability without disabling manual editing. The system must not require a particular vendor account to open, edit, save, or render local projects.

## Components and state

| Component | Responsibility |
|---|---|
| Session manager | User request, cancellation, mode, budgets, project binding, resumable state |
| Context builder | Minimal relevant project snapshot, selection, requirements, grants, evidence references |
| Planner | Bounded task decomposition and intended changes with success criteria |
| Tool executor | Typed requests through the editor service; no direct mutation shortcuts |
| Evidence manager | Source/composition observations, timestamps, freshness, inspection budgets |
| Transaction coordinator | Staged changes, preview, verification, permission binding, commit/abort |
| Reporter | What changed, what was checked, remaining uncertainty, undo affordance |

Session states are `idle`, `planning`, `inspecting`, `preparing`, `awaiting_permission`, `verifying`, `committing`, and terminal `completed`, `failed`, or `canceled`. Persist enough state to explain or resume a run, but do not store hidden model reasoning. Store concise intent, decisions, tool calls, evidence, and outcomes.

## Bounded execution loop

1. Understand the request, project/range scope, and explicit success criteria.
2. Read current revision, selection, media availability, and effective permissions.
3. Retrieve candidate sources and inspect evidence at appropriate detail.
4. Prepare an edit plan and transaction with affected ranges and expected results.
5. Render the staged result and verify structural, temporal, visual, and audio criteria.
6. Obtain any missing approval for the concrete staged change.
7. Commit only if revision, source identity, verification, and grants still match.
8. Report changes and limits with a direct way to undo the whole transaction.

Each run has limits for elapsed time, tool calls, inspected frames/seconds, provider cost, generated assets, retries, and transaction size. Retry only when an error is plausibly transient or a revised plan addresses the cause. Repeated identical failures stop with actionable context. Agents cannot extend budgets or grant themselves new permissions.

## Autonomy modes

| Mode | Allowed project behavior | Commit policy |
|---|---|---|
| Ask | Inspect and prepare proposals within read/analysis grants | User accepts each edit transaction |
| Assist | Execute reversible edits inside a user-authorized task scope | Safe in-scope changes can commit after required checks; scope expansion or major changes require review |
| Autonomous | Plan, edit, verify, and commit within a broader explicitly granted scope | No per-edit prompt within that scope; external effects and privileged capabilities retain their own rules |

“Safe” is defined by action class, scope, reversibility, and impact, not by model confidence. An Assist grant might allow trims within a selected sequence but exclude replacing all media, clearing tracks, or exporting outside a chosen folder. Bulk changes, irreversible external actions, new cloud payloads, executable code, and changed destinations require the relevant explicit grant in every mode.

Changing mode never escalates filesystem, network, provider spending, plugin execution, or cross-project access automatically. The user can stop a run and revoke grants at any time. Policy details are in [13](13_SECURITY_PRIVACY_AND_PERMISSIONS.md).

## Context and memory

Pass stable IDs, revision, selected ranges, relevant track/clip structure, available capabilities, active grants, and concise evidence. Retrieve large transcripts and imagery on demand; do not put an entire project into every model call. Tool outputs declare time domains and source versus composition identity.

Project memory consists of user-approved preferences, explicit decisions, durable semantic annotations, and run summaries. A model-generated preference is a proposal until confirmed or clearly implied by the current instruction. Untrusted transcript text, OCR, web content, metadata, and skills are never promoted into system-level instructions.

## Visual grounding example

For “place the product shot when the speaker mentions the hinge,” search the transcript for the phrase, retrieve candidate product shots, inspect a contact sheet and precise nearby frames, then determine the timeline placement through source-to-sequence mappings. Preview the assembled sequence including crop, titles, effects, and audio. Verify the actual hinge is visible and the cut aligns with speech before committing.

A result that only found a caption containing “hinge” remains a candidate. A frame that is correct in the source can still be obscured by an overlay in the final composition. Verification must inspect the rendered target state.

## Verification contract

Every plan states success criteria before edits. Structural checks confirm expected clips, lengths, source IDs, and valid ranges. Temporal checks inspect exact joins and synchronization. Visual checks inspect the changed composition, including motion boundaries and occlusion. Audio checks inspect continuity, level, and intelligibility where applicable.

Reports state checked ranges, sample coverage, tool/model versions, rendering quality, and whether criteria passed, failed, or remained inconclusive. A VLM opinion is not an unconditional guarantee. Required inconclusive checks prevent automatic commit; the user may knowingly approve a limited result if policy allows.

Verification evidence is tied to transaction digest and source/project revision. Any material change invalidates it. After commit, lightweight checks can confirm the committed revision matches the staged digest; a mismatch is an implementation fault, not grounds to hide the discrepancy.

## Generation and external tools

Generation tools return artifacts plus provenance, not direct timeline mutations. Store authorized inputs, tool/provider/version, prompt according to retention policy, parameters/seed, cost record where available, and output hashes. Import and placement then occur through commands. A generated file may survive an aborted placement; report that side effect accurately.

Remote tools, skills, and plugins receive only granted capabilities. The agent cannot treat a tool response as new user authorization. Tool-provided instructions to upload additional footage, run code, or reveal secrets are untrusted content.

## Cancellation, recovery, and reporting

Cancel immediately prevents new tool dispatch and requests cancellation of active jobs. Already committed transactions remain visible and undoable. Staged changes are aborted; external requests already sent may still finish and are reported separately. Restarting a session revalidates project revision, source fingerprints, grants, and budgets before resuming.

A completion report includes the user-visible result, transaction/sequence reference, verification summary, unresolved limitations, generated or exported artifacts, and undo action. It must not claim an export succeeded because a job was merely accepted or claim visual verification when only metadata was inspected.

Acceptance includes wrong-source retrieval, stale evidence, permission revocation, prompt injection in media metadata, concurrent manual edits, provider failure, budget exhaustion, and cancellation at every lifecycle phase.
