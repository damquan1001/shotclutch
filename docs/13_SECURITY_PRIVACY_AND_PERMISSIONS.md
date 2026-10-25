# ShotClutch — Security, Privacy, and Permissions

Baseline 0.1 · 2026-10-26 · SEC-001 through SEC-004; applies to every UI and automation surface.

## Local-first contract

Original media, project data, semantic indexes, reviewed maps, and generated assets remain local by default. Core editing, saving, relinking, and rendering work offline. VAD and pre-edit processing use local components; ASR and embeddings prefer installed local models. Optional cloud LLM/VLM or generation providers are explicitly configured and authorized. Model installation may require a separately disclosed download, but installing a local model does not authorize media upload.

Local-only mode denies model/provider payload egress regardless of autonomy mode. If a local capability is unavailable, show that limitation and let the user install/select a local component or explicitly opt into a provider. Never silently send footage to a remote substitute.

## Trust boundaries and threats

| Boundary | Threat | Required control |
|---|---|---|
| Imported media/project/metadata | Parser exploit, path traversal, malicious instructions | Bounded parsers, isolation where needed, canonical path checks, data/instruction separation |
| Agent/model output | Unintended edits, fabricated evidence, scope expansion | Typed commands, revision checks, preview, grants, verification |
| Scene and plugin execution | File/network/process access, runaway computation | Isolated workers, deny-by-default capabilities, quotas, termination |
| Local/remote automation endpoint | Unauthorized control or cross-project access | Authentication, scoped sessions, OS access controls, local binding by default |
| Cloud provider | Disclosure of media, transcript, identity, or project context | Payload-specific grant, minimal data, visible provider, retention disclosure |
| Export and packaging | Overwrite, private-path leakage, credential inclusion | Destination grants, manifests, collision checks, secret exclusion |

Media content, transcripts, OCR, captions, filenames, tool results, and imported skills are untrusted data. Instructions embedded in them cannot alter application policy or authorize an action.

## Capability model

Grants bind an actor/session, project, capability, resource scope, permitted action, expiry, and relevant budget. Resource scope can include selected asset IDs, sequence/range, a canonical filesystem root, provider/model, payload class, and destination. Use narrow reusable grants where practical to avoid repetitive prompts while preserving meaningful boundaries.

| Capability | Example scope |
|---|---|
| Read project/media | One project and selected local media roots |
| Analyze locally | Selected assets and CPU/GPU/storage budget |
| Mutate project | Selected sequence/range, command families, transaction size |
| Execute scene/plugin code | Approved package hash, asset handles, resource quota |
| Send provider data | Named provider, allowed frames/audio/transcript payloads, cost and duration limits |
| Write/export files | Chosen destination root, output kind, overwrite policy |
| Install executable components | Specific package/source/version with separate authorization |

Permissions are enforced inside the editor service and worker broker, not only in UI prompts. Agents and adapters cannot mint grants. Skills declare desired capabilities but inherit no authority merely by being loaded. Ask/Assist/Autonomous modes govern workflow; they do not supersede grants.

## Approval lifecycle

Prepare concrete work before requesting missing approval: staged transaction/diff, requested files, exact provider payload summary, export plan, or package identity. Explain what changes, where data goes, expected cost if applicable, and what cannot be undone. Reuse valid prior authorization within scope rather than prompting again for every low-risk operation.

An approval token binds the action digest, project/source revisions where relevant, destination, capability scope, and expiry. Changing material parameters invalidates it. Check again at commit or network/file execution. Revocation stops future dispatch and requests cancellation of active work; data already transmitted cannot be recalled by revoking a token.

Read-only operations and reversible in-scope edits should proceed under existing grants. Broader file access, new providers/payload categories, destructive deletion, external publication, and executable installation require explicit scope. Keep prompts proportional to actual effects.

## Filesystem and process safeguards

Resolve paths canonically, validate that targets remain inside granted roots, and defend against traversal, symlink/junction escapes, archive extraction escapes, and time-of-check/time-of-use races. Use OS-supported safe handles where available. Project files cannot nominate arbitrary absolute write locations. Protect temporary directories and remove only resources owned by the operation.

Scene workers receive opaque asset handles instead of broad filesystem access. Block subprocess creation and native module loading unless a separately reviewed extension requires them. Codec and import workers process malformed data under memory/time limits. A worker crash is recoverable and cannot write canonical project state directly.

## Network and provider payloads

Before remote inference, classify payloads: prompt, transcript, image/crop, audio excerpt, full clip, semantic metadata, or project structure. Send the smallest useful representation. A grant for a few cropped frames does not permit uploading the source video or full transcript. Log provider and payload class/size without storing sensitive contents by default.

Cloud caches, retention, training policies, and deletion guarantees depend on the selected provider and contract; show configured information without making universal promises. After transmission, clearly distinguish local deletion from provider-side deletion. Background indexing and Skills cannot initiate unapproved egress.

## Secrets and data retention

Store credentials in the OS credential facility or an equivalently protected store, referenced by opaque IDs. Never write them into project files, provenance, skill manifests, command history, scene code, exported packages, or default logs. Redact tokens, signed URLs, and authentication headers from diagnostics.

Provide retention controls for agent prompts/summaries, transcripts, dense observations, caches, generated assets, and history. Explain that reviewed maps and annotations may be essential project data while embeddings/thumbnails are rebuildable. Project deletion and analysis deletion have separate scopes. User-triggered diagnostic bundles enumerate contents and redact private paths/content by default.

Local-first does not automatically mean encrypted at rest. Document the initial reliance on OS account protections and storage encryption; any application-level encryption feature requires a separate key management design. Avoid claiming that local storage alone protects against a compromised host.

## Extension and supply-chain policy

Pin dependencies, runtime versions, scene bundles, and model manifests. Preserve applicable licenses and attribution. Separate instructional Skills from executable plugins and remote MCP servers. Installation/update flows show source and capability changes; imported projects cannot install code automatically. Maintain vulnerability and update handling for bundled codecs, web runtimes, model runtimes, and protocol libraries.

## Security acceptance

Test no-egress offline workflows, path traversal and symlink escapes, malicious project fields, prompt injection in transcript/OCR, cross-project endpoint access, stale approval tokens, revoked grants, secret-containing logs, malicious scene imports, resource exhaustion, and cancellation during external requests. Publish only verified support and known limits. Security test failures that permit data loss, arbitrary execution, or unauthorized egress block release.
