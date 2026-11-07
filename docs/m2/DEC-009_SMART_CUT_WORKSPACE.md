# DEC-009 — Native Smart Cut workspace

Selected and implemented on `codex/m2-smart-rough-cut`; updated 2026-11-07. The user renamed Smart Rough Cut to Smart Cut and requested the Gling/OpenGling cut-first workflow. Existing data and command names remain compatible.

The workspace is native egui: Transcript / Review cuts / Setup on the left, a large preview on the right, and a source-time waveform across the bottom. It uses the same desktop frame/audio workers, device-clock playback and GPU monitor path as Editing. It does not create a separate player. Transcript paragraphs and waveform lookups are cached/indexed so review does not rebuild all text or hash the generated sequence on every frame.

Transcribe & cut creates the automatic logical result immediately. Transcribe only leaves the source intact. Review changes that result directly; there is no candidate approval queue. Open in editor is the explicit durable commit to an ordinary linked-clip sequence. Result preview and application share the assembly builder.

Icon media controls sit below the picture. Preview mode and undo/redo/More/Open in editor share the viewer toolbar. Removed timeline intervals and waveform are gray. Cut and uncut intervals can both be selected, trimmed by their edges, and right-clicked for Cut / Uncut. Skip cuts toggles original/result playback while preserving the source position. Highlighted transcript words expose Cut / Uncut / Correct / Export clip actions.

Review cuts provides category/status filters, original-context listening, take auditions, choosing one or retaining all takes, grouping rejection/splitting, restoration and boundary marks. Unknown confidence stays visible. Reject grouping restores all alternatives in one undoable action. Manual choices remain authoritative through compatible cleanup and reanalysis.

Map edits use the canonical command path with immutable revisions and asynchronous recovery serialization. Full source verification and durable application run outside normal map interaction. Replacement is allowed only while the earlier generated sequence is unchanged; edited results require a new sequence. Scoped clients retain their existing permissions.

See [OpenGling assessment](OPENGLING_CLEANUP_ASSESSMENT.md), [qualification](QUALIFICATION.md), and [acceptance](ACCEPTANCE.md). Automated UI interaction establishes mechanics; intentional pauses, performance choice and audible joins require human review.
