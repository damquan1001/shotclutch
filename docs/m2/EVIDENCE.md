# M2 verification record

This record retains historical runs below; the latest dated section records current repairs and qualification. Feature name: Smart Cut; canonical `preedit` names remain compatible. [Acceptance mapping](ACCEPTANCE.md) and [reproduction procedure](QUALIFICATION.md).

## Optional Gemini reviewer — 2026-11-24

Added an opt-in Gemini provider for transcript-block, remaining-pair and evidence-based review. Granite 2B remains the default. The owner-side merge, manual protections, media snapping, independent evidence verification, revision checks and Undo commands remain shared. The API contract uses system instructions, transcript-only user content and constrained JSON; credentials stay in session memory or the environment and are excluded from saved state, inspection, reports and request URLs. Authentication/quota/connection failures stop the job rather than issuing another request per group.

`cargo check --offline -p shotclutch-ui-egui` and the locked Windows build passed. All 78 model-independent `preedit::` engine tests passed (four opt-in tests ignored), including five provider tests and a Gemini evidence/checkpoint merge regression. Mock requests cover authentication headers, JSON schemas, blocked/truncated/malformed output, redacted API errors and cancellation while a response is still pending. Windows mocks explicitly use blocking accepted sockets and a synchronization gate for cancellation rather than a tight elapsed-time assertion. The full suite was run with loopback access; no request to Google was made.

The native walkthrough at 1600×1000 and 1024×720 verified provider switching, masked credentials, invalid model settings preserving the checkpoint, save/reopen with no key in the project, and no key in UI inspection. Screenshots were visually inspected. The fixture and restored 877.362-second baseline checksums remain unchanged. The optional Windows preview is `target/m2-qualification/local-take-review/shotclutch-gemini-preview.exe` (77,018,112 bytes; SHA-256 `2fe8fe6c3aa222f825ad5956838394dcbe4b0df19060970454eaf18f9fec8afb`). It also includes the separate Premiere-style menu row. Logs and screenshots are in ignored `target/gemini-*` storage. The frozen M2 tag and local binary remain unchanged.

No Gemini key was configured, so live Google-account compatibility, billing behavior and editorial quality remain unqualified. The mock tests establish integration behavior, not model accuracy. This addition does not promote experimental evidence review or change the failed automatic accuracy gates.

## M2 scoped closeout — 2026-11-24

The user explicitly requested completion of both assessed closeout tasks. M2 is Done as review-assisted Smart Cut with Granite 2B retained. [Completion and frozen checkpoint](M2_COMPLETION.md) pins `m2-closeout-2026-11-24` and the qualified Windows binary; [acceptance](ACCEPTANCE.md) and the tracker now record Done. M3 remains unstarted. Failed unassisted model-quality gates remain failed and unpromoted; no new unattended-editorial or unseen-recording accuracy claim is made.

Routine M2 CI retains its `m2_` coverage and now also runs complete model-independent preedit and renderer mixer suites. The new runner regression confirms that failure in either added suite stops the gate and preserves the child status. Both final local Windows gates passed: M1 in 50.9 seconds and expanded M2 in 21.6 seconds on warm build artifacts. M2 ran the failure-propagation guard, two Python protocol tests, 58 `m2_` engine checks, 72 full-preedit checks, 25 mixer checks, three UI checks, one indexed-boundary check and 20 speech checks. The engine filters overlap. Private/model-dependent opt-in tests remain ignored; no live provider or model download is needed by the headless gate. Logs remain at `target/m2-closeout-m1.log` and `target/m2-closeout-m2.log`. This does not claim a new hosted CI run.

The independently checked reviewed baseline preserves all 11 confirmed keeps (71.486875 seconds), removes all 55.517 seconds of confirmed bad takes and removes the abandoned glasses restart while retaining its later reading. It contains 49 stored overrides, so this is reviewed-result evidence rather than automatic detection credit. All 80 rendered seams pass the de-click check; the 877.362-second source project checksum is unchanged. Existing workflow, source preservation, linked export and durable Undo/redo evidence supports the bounded acceptance. No additional join-review cycle was requested or inferred.

## Smart Cut join polish — 2026-11-23

Granite 2B remains the local reviewer. The Windows UI no longer offers the 8B research candidate. Existing experimental review behavior remains opt-in; this polish does not promote the failed evidence-review accuracy experiment.

One-sided audio fades now include the first and last samples of their actual sample span. The previous duration-based progress left a residual outgoing sample; a one-sample fade-out could retain full amplitude. The regression reproduced that defect before the fix. Fade endpoints now reach exact silence, including sub-millisecond clips, while two-clip crossfade timing is preserved. Smart Cut retains its short, duration-capped 5 ms fades and original linked picture/audio cut positions. Audio-only decision snapping also respects inward/outward direction instead of rounding both ends to the nearest sample.

Validation: 25 mixer regressions passed, including stereo/opposite-polarity joins at 44.1/48 kHz, one/two/eight-sample clips, normal 5 ms fades, render-block independence and existing handle crossfades. All 72 preedit engine regressions passed, including preview/applied audio equality, directional sample rounding, source boundaries, manual decisions and the linked A/V export frame/sample-count check. Four opt-in tests remain ignored in that engine filter.

The read-only `smart_cut_join_audit` example rendered every interior join of the restored recording through the production mixer. All **80 joins** passed the adjacent-sample discontinuity check on the 48 kHz output, with a maximum seam step of **0.0**. Duration remained **877.362 seconds (14:37.362)**. This measures de-click behavior at the splice, with the existing editorial choices retained. Local details are in `target/m2-qualification/smart-cut-polish/joins.json`; the Windows candidate is `target/m2-qualification/local-take-review/shotclutch-smart-cut-polished.exe`. The restored editing project's original checksum is preserved.

## Retained repeated-passage selection — 2026-11-18

Added `scripts/m2/select_retake_windows.py` as a qualification-stage selector. It retrieves ordered phrase matches across nearby clauses, considers both directions, ranks currently retained passages by covered duration, and selects up to ten disjoint windows. It records the counterpart and detected meaning differences for subsequent audio verification. It applies no cuts.

On the unchanged V6 benchmark, ten windows totaling 180.240 seconds covered 8.517 seconds of still-retained labeled bad takes. The previous long-word selector covered zero seconds of those failures. The new selector still missed other labeled failures and can nominate intentional repeats. These are retrieval measurements from the existing recording, not validated automatic removals. All 22 speech benchmark tests passed, including a regression requiring selection of the retained attempt when its counterpart has already been removed. Production integration and broader audio confirmation remain pending.

## Reconciled reviewed cut and duration audit — 2026-11-17

The latest V6 qualification copy and the earlier browser-reviewed copy had different project histories for the same source binding. The V6 copy omitted the earlier 25 removal decisions, leaving 118.452 seconds of those ranges retained. A separate current copy reapplied those decisions through the native review command while protecting the selected takes. Its 39 manual decisions reduced the cut from 1,186.401 to 1,070.700 seconds, a 115.701-second reduction. The remaining 2.751 seconds of older removal ranges were retained around selected takes. Confirmed keep coverage had zero additional loss. These gains restore reviewed decisions and carry no automatic detection credit.

The reference lasts 692.011 seconds, leaving a 378.689-second difference. The reconciled cut contains 534.279 seconds overlapping high-confidence reference source placements, 145.174 seconds between adjacent reference excerpts, and 391.247 seconds with unresolved placement. These categories partition retained source audio. The audit does not assign an editorial deletion reason to unmatched audio. The executable remains the November 16 audio-recheck build.

The new `scripts/m2/audit_cut_gap.py` reports both missing reviewed removals and reference coverage. Its partition regression includes reordered source audio. All 21 speech benchmark tests passed. The private project is `target/m2-qualification/reconciled-review-current/Smart Cut - Reconciled Review.scproj`. A separate native instance reopened it and verified the 1,070.700-second result. Saving this 345 MB history exceeded the control server's response timeout, so qualification waited for the pending save and verified the resulting file independently.

## Local audio recheck — 2026-11-16

Setup offers **Recheck retakes** for saved English speech with Automatic and Context-aware takes enabled. Base English · Fast transcribes up to ten short windows around unusually long words that contain quiet audio. The cut requires a repeated opening with a complete continuation, nearby timing agreement with the saved transcript, and measured quiet at the join. The project stores the resulting ranges and the model fingerprint. Captions still use the primary transcript.

In the native Windows app, the older Tiny English transcript had merged two readings into one. Rechecking ten windows took 58.36 seconds and removed 2.600 seconds of labeled bad-take audio. The cut went from 1,191.486 to 1,188.886 seconds with zero new loss in confirmed keep ranges. The original transcript, original benchmark file and existing manual Keep were preserved. The saved copy reopened with identical cuts. Update cut reused the result in 203 ms.

The newer reviewed V6 copy already cut that restart. Its native recheck found zero additional confirmed restarts across ten windows. The 1,186.401-second cut and all 14 manual decisions were preserved.

The 54 Smart Cut engine tests, 81 M1 engine tests, 3 Setup tests and 20 speech benchmark tests passed. The Windows qualification executable is `target/m2-qualification/shotclutch-smart-cut-audio-recheck.exe`, SHA-256 `b16c22f0ff6b9365401566e261c52e2e453f1f25455b249dd0bed15b5586ab1d`.

## Indexed restart continuations — 2026-11-16

Context-aware V6 searches indexed phrase openings up to 150 seconds apart. The alternative must preserve the entire earlier passage in order and continue through a complete sentence. A distant match also requires evidence that the shorter attempt broke off. A short immediate retry can qualify when the audio is quiet at the seam and the new reading completes the same sentence.

The native recut of the latest Tiny English review copy removed 2.683 additional seconds inside one labeled bad-take range. Confirmed keeps and high-confidence reference audio retained their coverage. The copy saved and reopened at 1,186.401 seconds with all 14 manual decisions. The older Tiny English benchmark returned the V5 result at 1,191.486 seconds. Its transcript merged the two readings at the newly detected restart. Twenty focused matcher and pacing tests and twenty benchmark tests passed.

A separate local Whisper Base English run rechecked 123 seconds across seven windows in 37.11 wall seconds. It recovered both readings in the missed restart and recognized a retry cue misheard in the saved transcript. This was a diagnostic run. The production matcher uses saved words and measured quiet. The Windows qualification executable is `target/m2-qualification/shotclutch-smart-cut-indexed.exe`, SHA-256 `66bbca5ca10fdda497d3a72633e1893d78f5c60cfc6b1194acb3463d60294c9d`.

## Linking unfinished attempts to a complete take — 2026-11-16

Context-aware V5 follows an existing prefix-restart link when its selected attempt also trails off. A nearby complete take can replace both attempts when the opening and continuation agree in order. The matcher allows one internal recognition difference with exact words around it and preserves the numeric and negation guards.

Native recuts of the full V4 benchmark and the separately transcribed review project both removed 4.150 additional seconds. The new removal covered the flagged unfinished opening. No confirmed keep range or high-confidence export match lost coverage. The review copy saved and reopened at 1,189.084 seconds with all 14 manual decisions. Eighteen focused matcher and pacing checks and 20 benchmark tests passed. The Windows qualification executable is `target/m2-qualification/shotclutch-smart-cut-restart-chains.exe`, SHA-256 `e31ae4f53f399152230bc300a9826009ef66d2b56133ec3eefd2c4839c1a4150`.

## Screened Context-aware take matching V4 — 2026-11-10

V4 uses legacy repeat detection to find candidate speech. A discarded fragment needs an exact nearby continuation after removing an opening echo. An existing take link moves only if its previously discarded words all survive in the new keeper. An aborted lead with a repeated modifier also qualifies when the splice falls inside measured quiet. V4 keeps the existing pause settings and preserves earlier saved rule versions.

The native recut used the same complete Tiny English transcript and pause settings as V3. The result fell from 1,202.186 to 1,195.636 seconds. All 6.550 additional removed seconds overlap two previously labeled bad-take clips. There was no additional removal in the confirmed keeps, uncertain clips or the waveform matches at 0.95 or higher. Save and reopen reproduced the result, and recalculating the reviewed project preserved its overrides and earlier map. The recut took 159 ms over the local control channel. This comparison covers one recording and the existing editorial labels. A substantial gap to the reference remains.

The engine checks passed 81 M1 and 50 M2 tests, with one private model rerun ignored. The 20 benchmark tests also passed. The Windows qualification executable is `shotclutch-smart-cut-screened.exe`, SHA-256 `b8f3fa1cab6067893729e683ca0cae5ebef845f2bfad09ff84d7014e7d156690`. The engine and UI were built with development optimization set to zero.

## Context-aware restart matching V3 — 2026-11-10

V3 adds short abandoned openings with an immediate continuation, single-word pickups backed by a broken lead and a surviving matching opening, and isolated retry cues backed by repeated wording nearby. Updating the Context-aware takes preview uses V3. Older saved map revisions retain their stored rule version. An initial native run exposed one frame of audible lead-in loss when a sentence-ending retry cue was removed. The final rule preserves the lead-in padding and has a focused regression test.

The Windows candidate reapplied cleanup to the full saved Tiny English transcript with the same pause settings. The automatic result changed from 1,211.602 to 1,202.186 seconds. Four new spans removed 9.416 seconds, including 8.616 seconds in three labeled bad-take clips. There was zero additional overlap with labeled keeps, uncertain clips, or the reference audio windows matched at 0.95 or higher. The final recut took 76 ms over the control channel. Save and reopen reproduced the result, and updating the separately reviewed project preserved its manual overrides and earlier map. This is one recording and a small improvement over V2. The remaining gap to the reference is open.

The final focused matcher and pacing suite passed 14 tests and the benchmark suite passed 20. The engine regression run before the final lead-in fix passed 81 M1 tests and 46 M2 tests with one private model rerun ignored. The final desktop candidate is `shotclutch-smart-cut-restarts.exe`, SHA-256 `4882c93617d08304353b27d7574e6a63d85e7bd0bf29fdd90f2b03bcfd1fd4e8`. This is a Windows qualification build with engine/UI development optimization set to zero.

## Remaining retake gap review — 2026-11-10

Fresh decodes of the original and reference revalidated 295 waveform placements. At the stricter 0.86 waveform threshold, 41 source skips between adjacent reference excerpts contain 242.093 seconds still retained by the 18:49 reviewed map. The runner excludes all source ranges matched elsewhere in the export and protects 26.852 retained seconds inside explicitly chosen takes.

The six largest eligible regions produced 29 listening clips covering 153.303 seconds. Fifteen clips covering 67.905 seconds have a plausible ordered transcript comparison. The comparisons include full-phrase repeats, partial restarts and cases where wording differs. They are review leads. The reference skips and transcript similarity do not establish editorial removal decisions. Twenty benchmark tests passed. The review audio, transcripts, exact ranges and labels remain local. [Reproduction procedure](../../scripts/m2/SPEECH_BENCHMARK.md#remaining-retake-gaps)

The next listening pass identified ten full bad takes, two partial bad takes, one silent excerpt, eleven keeps and four uncertain clips. Two annotations selected the same phrase, once in its own clip and once in another clip's comparison. That other clip's own audio has no verdict. The reviewed bad-take spans total approximately 55.5 seconds before frame snapping. The earlier automatic candidates were compared on this selected cohort. The broader candidate removed none of the newly labeled bad-take spans. The first ordered candidate removed 3.016 seconds of them and 5.951 seconds of explicit keeps. The second ordered candidate removed none of them and 4.450 seconds of keeps. This cohort was drawn from material retained by the broader candidate. Of the three labeled clips with full ordered transcript coverage against a comparison, the editor marked two keep. These measurements support evaluating take selection separately from retrieval.

The native candidate applied the thirteen labeled removals to a private copy and reopened it at 1,072.084 seconds. The revised map removed 56.784 seconds, with unchanged coverage inside the explicitly protected ranges. Eight local audio previews cover the resulting joins. One partial cut used a low-energy point near the middle of the excerpt. The revised joins await listening review.

## Confirmed retake cuts — 2026-11-10

The native candidate applied the seven confirmed choices as manual range edits to a private project copy. The exact preview duration fell from 1,211.602 to 1,147.803 seconds (20:12 to 19:08 rounded). The seven alternate ranges lost 63.799 seconds of previously retained material, including frame snapping at their edges. All seven chosen ranges stayed fully retained. No timed word outside the alternate ranges lost coverage. The preview segments and duration matched after save and reopen, and the input project hash stayed the same.

Seven local listening clips use the actual preview join positions with three seconds of context on either side. The clips use mono source audio and approximate the native fades. Take choice is confirmed. The new joins still need listening review. The private result provides a reference for comparing future automatic take ranking on this recording. [Reproduction procedure](../../scripts/m2/SPEECH_BENCHMARK.md#confirmed-retake-cuts)

The editor subsequently flagged bad-take material in four clips. Three clips flagged only the lead-in. Two clips shared source audio, so the follow-up used three manual edits. One edit trimmed the newly flagged tail of a previously chosen take. Another preserved the lead-in to a chosen take that also appeared in the adjacent flagged clip. The other chosen takes kept full coverage. The revised private project saved and reopened at 1,136.884 seconds, an additional 10.919 seconds removed. Three revised joins are available for listening. The comments and exact source ranges remain local.

A second listening pass flagged one more restart and a repeated opening word. The follow-up removed the remaining restart segment and cut in the quiet gap between the repeated words. It retained the later word's acoustic onset and saved caption midpoint. The private project saved and reopened at 1,128.868 seconds, an additional 8.016 seconds removed. Two updated join previews are available for local listening.

## Multi-clause retake review set — 2026-11-10

The runner verified the original and edited export hashes and rechecked 295 source placements against fresh waveform decodes. It found 35 possible pairs across runs of up to four adjacent clauses in the 20:12 preview. Six non-overlapping cases have at least two matching excerpts from take B in the export. At least two probes from each earlier take were searched in the export, with no strong matches found in those probes. The current map retains 65.57 seconds across those six earlier spans. Safe removal duration depends on the editorial labels.

The set includes a case with certainty and point-of-view differences and a control with matching export excerpts from both alternatives. An independent Base English pass on all seven pairs reproduced the certainty and point-of-view flags for the sensitive case. Each pair has local source audio, optional saved words and up to three matching export excerpts per side. The 14 main clips total 141.6 seconds. All 36 review audio links were verified.

The editor confirmed a keep choice for all seven pairs, choosing two earlier takes and five later takes. In two of the six cases with matching excerpts from B only, the editor preferred A. The editor chose B for the control that included export audio from both takes. The 20:12 baseline retains all selected take audio and 63.749 seconds across the other alternatives. The labels identify take preference. Exact removal boundaries and audible joins still need qualification. The per-case choices and scoring report are stored in the private review folder.

All 18 benchmark tests passed in both Python environments. New checks verify the waveform at a recorded placement and prevent source audio from appearing in multiple cases. [Reproduction and labeling procedure](../../scripts/m2/SPEECH_BENCHMARK.md#multi-clause-retake-review)

## Uhm acoustic filler trial — 2026-11-10

The approved local benchmark used the published Uhm ONNX model at revision `a0ab78540d724afa6d8caf09090936ffbfbbf3d1` with SHA-256 `fbd19765def76893841e0c70e498bf632d6c68175ceae5a2da8fe3bf7d9d0314`. The graph returned six probabilities per frame. Its bundled label file lists five classes, so the strict screen uses only the documented `uh` and `um` indices. The model and vendor notices are stored in the private qualification folder. [Pinned model and notices](https://huggingface.co/desert-ant-labs/uhm/tree/a0ab78540d724afa6d8caf09090936ffbfbbf3d1) · [Published class mapping](https://huggingface.co/desert-ant-labs/uhm/blob/a0ab78540d724afa6d8caf09090936ffbfbbf3d1/README.md)

CPU inference covered 2,271.53 seconds of verified PCM in 121.25 seconds. With a minimum span of 120 ms, the total filler score flagged five spans at 0.65 and five at 0.75, covering 1.30 and 1.18 seconds respectively. The combined `uh` and `um` score at 0.75 flagged three spans totaling 0.66 seconds. Every flagged span overlapped saved word timings before the 40 ms safety margin. One also overlapped a provisional match to the edited reference. Zero spans passed the overlap screen.

None of the three score variants produced a hypothesis in the 36 voiced gaps, 12 quiet gaps or 24 timed word controls used for the earlier comparison. On the same two-voice synthetic controls, the 0.75 total score flagged 2 of 6 intended filler clips and the strict score flagged 1 of 6. Both returned zero hits on six ordinary word clips. These observations leave natural filler recall and precision unknown. The three strict candidates are available in a private listening page with unreviewed label rows.

All 16 benchmark tests passed in both Python environments. The new tests cover the join between overlapping inference windows, the partial source frame at the end and the saved speech overlap veto. [Reproduction procedure](../../scripts/m2/SPEECH_BENCHMARK.md#uhm-acoustic-filler-trial)

## Selective speech verification — 2026-11-09

The local trial scanned 230 bounded gaps between saved words and found 37 with enough periodic audio to screen. It sampled 36 of those gaps, 12 quiet controls and 20 pairs retained by the broader matcher. Pinned Tiny and Base English each decoded identical excerpts with VAD disabled, both with and without a filler prompt. Both produced zero filler hypotheses inside the sampled gaps. Mean ordered word overlap on the 20 take pairs fell from 0.535 on the saved transcript to 0.473 for Tiny and 0.466 for Base. Each improved one pair by at least 0.15. The complete runs took 79.0 and 302.8 seconds respectively on this host. These cohorts have no verified speech labels. The observed counts leave filler recall and editorial accuracy unknown.

An independent Vosk 0.3.45 and small English 0.15 trial used the same 48 gaps plus 24 high-confidence timed word controls. The restricted vocabulary produced one `ah` hypothesis at 0.44 confidence. It disappeared in the ordinary decode and in a vocabulary with competing speech words. All three modes returned zero hits on the quiet and timed-word controls. On a separate two-voice synthetic check, both restricted modes found 3 of 6 intended filler clips and 0 of 6 ordinary word clips. The Vosk Windows wheel matched PyPI SHA-256 `6994ddc68556c7e5730c3b6f6bad13320e3519b13ce3ed2aa25a86724e7c10ac`. The model archive SHA-256 was `30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498`. [Vosk model catalogue](https://alphacephei.com/vosk/models) · [Vosk wheel metadata](https://pypi.org/project/vosk/0.3.45/)

The quantized DeBERTa xsmall trial compared meaning in both directions. At an entailment floor of 0.85 it accepted all 8 equivalent controls and 0 of 14 controls with changed facts, additional details or shifts in certainty. Two of 20 retained pairs passed on both the saved transcript and Base's second pass. Their later performances overlap provisional reference anchors. One of those pairs changed tentative wording to a firsthand claim despite the high entailment scores. At 0.95, one pair passed on both transcripts. The scores supply candidates for local listening and identify a gap in factual guards before automatic use. [Pinned ONNX model](https://huggingface.co/cross-encoder/nli-deberta-v3-xsmall/tree/03d06b2f6f06023bd596151a16477e2fcec601fb)

The new qualification runners completed with verified artifact hashes and 88 accessible local listening clips. All 14 benchmark regressions passed. Detailed reports and audio remain in ignored storage. [Reproduction procedure](../../scripts/m2/SPEECH_BENCHMARK.md#short-window-verification)

## Broader retake matching — 2026-11-09

The saved V1 result missed repeated attempts when recognition changed sentence boundaries, dropped opening words or produced a few different tokens. V2 compares ordered spans of neighboring clauses and uses surviving later spans or the original ordered matcher to resolve alternatives. It bounds the search by word count, boundary count and time. It checks speaker, protected values and names, leading subject, clause coverage and reliable unmatched words. An unfinished attempt can contain one low-confidence unmatched word when most of the ordered wording is shared. Exact ties can use the earlier complete take when the later performance offers no better join.

Both versions used the same 3,935 saved words from the 2,271.551-second recording with 800 ms pause preservation and 150 ms breathing room. The V2 result is 1,211.602 seconds versus 1,324.622 for V1, a reduction of 113.020 seconds. Take groups increased from 19 to 51. Against the same 297 automatically matched reference anchors, provisional removal overlap fell from 23 windows and 22.207 seconds to 19 windows and 13.364 seconds. Human alignment and editorial labels remain pending. The evidence supports broader lexical recall on this recording, with semantic retakes, filler recovery and audible-join review still open.

All 44 focused engine tests passed, with one private inference case ignored. New fixtures cover shifted sentence breaks, small opening differences and preservation of distinct material, including changed prices, names, numbers, subject and negation. The legacy differential corpus still passes.

The desktop walkthrough opened the V1 project and upgraded it with Update cut. It passed take comparison, pause and phrase restoration, result playback, refresh with manual decisions preserved, save/reopen, apply, undo/redo and edited-target rejection. The input project stayed byte-identical. Saved-transcript recut took 14.40 ms over the control channel. The audio refresh took 40.48 seconds with the same recognized words. Candidate `shotclutch-smart-cut-retakes.exe` has SHA-256 `be0805b4658df61c7c72e2ac4b9295cf8a02461d95f09958a7e0beff92e72b70`.

## Ordered take-matching preview — 2026-11-09

Added explicit `cleanup.takeMatching: "orderedV1"`, exposed as Context-aware takes (preview) under Automatic in Setup. Existing projects and new workspaces retain the legacy default. Update cut uses saved timed words; no inference, model acquisition or upload is needed. Classification uses independent utterance boundaries, ordered prefixes and bounded sequence alignment over up to 48 utterances/120 seconds, excluding artificial splits of long utterances. Changed content-bearing tokens, known speaker differences, short emphasis and unmatched passages remain retained. Only matched attempts are cut; unrelated intervening material is preserved. Completion and recognized hesitation counts precede an earlier-attempt tie; this is not a subjective delivery score. Alternative chains resolve to survivors and review buttons follow source order.

The 10 new M2 regressions cover facts/names/negation/order/complementary details, distant restarts, earlier complete takes, equivalence, restart chains, speakers/emphasis, work bounds, explicit settings, immutable maps, manual restoration, save/reopen, grouped history and pacing independence. The unchanged legacy 196-case corpus still passes. Three headless UI regressions cover saved-setting/undo synchronization, dirty-form preservation and acknowledgement of Update cut; these are included in the M2 CI gate. The native trial exposed an existing stale-Setup issue that could reapply old pause settings; Setup now follows clean saved updates without overwriting pending form edits. The final M2 gate passed (42 engine cases, one private case ignored, three form cases, one codec boundary case, 20 speech cases and two worker-protocol checks). M1 checks passed.

The corrected full-recording comparison reused all 3,935 saved words over eight completed chunks/2,271.551 seconds. Both maps used 800 ms pause preservation and 150 ms breathing room. The legacy result is 778.359 seconds with 123 take groups; ordered matching retains 1,324.622 seconds with 19 groups. Against the same 297 unverified source/export anchors, provisional conflicts change from 28 windows/27.097 seconds to 23 windows/22.207 seconds. This is a large retention/recall tradeoff, not demonstrated editorial improvement: anchors are not verified, windows are not independent editorial errors, and a missing source range has no deletion-category label. The guarded matcher remains opt-in; duration matching and these partial overlap counts do not justify replacing the default. Broader retake recall, acoustic/verbatim filler recovery and semantic adjudication remain open.

The corrected native walkthrough passed the actual Setup toggle/Update cut, pause restoration, take audition/retention, transcript cut/uncut, result transport, refresh with manual decisions/strategy preserved, durable apply/save/reopen/grouped undo/redo/idempotency, edited-target rejection and a fresh generated sequence. Original project bytes stayed unchanged. The explicit ordered recut control-channel round trip was 5.37 ms for the full saved transcript; this includes scheduling/transport and is not a displayed-frame latency measurement. Audio refresh with reused words took 35.52 seconds, not a new recognition run. The checked Setup screenshot was inspected locally. Human labels remain unreviewed.

Desktop candidate: `shotclutch-smart-cut-context.exe`, SHA-256 `bddf4cdb375b6b75131a332d4a893b4c3cdb82cfd85a75e503a016d3e98b67f3`. Build, formatting and whitespace checks passed. The superseded task-built zoom executable was removed after qualification.

This is a guarded lexical preview, not a qualified replacement for the existing automatic rules. It does not recover omitted fillers, resolve paraphrases/corrections or determine the best delivery. Rule confidence remains unknown. Editorial labels and M2 closure remain open.

## Speech accuracy benchmark — 2026-11-09

Implemented the first slice of the [speech accuracy plan](SPEECH_ACCURACY_PLAN.md): private source/reference audio matching, temporally separated review excerpts, identical-input Tiny/Base inference, paired local listening pages and scoring from explicitly verified labels. The Rust example exports the canonical model catalogue; acquisition is explicit, pinned and checksum verified. Inference uses the desktop's unchanged offline worker. [Reproduction and labeling procedure](../../scripts/m2/SPEECH_BENCHMARK.md).

The supplied human export yielded 346 two-second windows: 297 automatic source matches covering 594 seconds, zero ambiguous windows and 49 unmatched windows. Matching took 34.516 seconds including decoding and input verification. These are unverified hypotheses, not a reconstructed human edit list. Of the automatic matches, 28 windows overlap the current cut map by more than 200 ms, with 27.097 seconds of total provisional overlap. Multiple windows may represent one editorial decision; neither these counts nor unmatched audio establish bad-take errors. They provide specific listening targets for investigating take selection.

The final comparison used 24 identical PCM excerpts totaling 479.989 seconds, with 22 development and two holdout excerpts in disjoint 120-second temporal blocks. This small, unbalanced single-recording holdout must expand before generalization or model-selection claims. Tiny returned 914 words in 27.265 seconds of summed inference; Base returned 969 in 66.033 seconds. Initial worker/model loads took 1.281 and 1.141 seconds respectively. Both returned zero lexical filler candidates. Counts do not measure accuracy or establish whether fillers were absent or omitted. These are one local CPU run, ordinary background load and short-excerpt contexts, not full-recording throughput guarantees.

All transcript, event and alignment labels remain unreviewed. Accuracy, filler precision/recall and verified reference-conflict scores therefore remain unknown. The scorer records word substitutions/deletions/insertions, one-to-one filler events and matched boundary error; it does not yet score a new restart/take classifier. Saved inference can be rescored after local labeling without rerunning recognition. Media, audio clips, hypotheses and detailed timing evidence remain ignored and local.

The 14 synthetic benchmark regressions passed with host NumPy 2.5.1 and the pinned speech environment's NumPy 2.2.6. They cover reordered/ambiguous/unmatched audio, temporal separation, source binding, ordered manual overrides, output privacy, unknown labels, filler omissions/false positives, one-to-one matching, boundary error, event bounds and pinned artifact constraints. No production cleanup rules or desktop binary changed in this slice. Human editorial review and M2 closure remain open.

## Editing-style zoom bar — 2026-11-08

Smart Cut now uses a bottom range bar with the Editing workspace's visual/interaction pattern: drag either circular end handle to zoom with the opposite end anchored, drag the middle to scroll, and click the track to reposition the view. It replaces the small toolbar zoom slider. Waveform height reserves only the bar and its spacing. Paused manual scrolling no longer recenters on an offscreen playhead; explicit seeking and playback still follow the cursor.

Native checks passed right-handle zoom, left-handle anchoring, persistent middle scrolling, track repositioning, dragging back to the full source range and compact bottom alignment. Wide and compact screenshots were inspected locally; the saved input project checksum stayed unchanged. Desktop build, formatting and whitespace checks passed. The workspace retains its own view state, independent of the Editing timeline's zoom/scroll state.

## Responsive timeline and neutral UI wording — 2026-11-08

The source timeline previously allocated a fixed 94 px strip inside a taller bottom panel, leaving unused space. It now fills the remaining panel height after controls/status, scales its cut/audio lanes and centers their labels. Dragging, selection, cursor painting and seeking share the expanded bounds. Application status is laid out before the waveform so its actual height is reserved.

Native checks at 1920×1080, 1440×950, 1024×720 and 1440×1080 measured waveform heights of 150, 131.844, 100 and 150 px. Each leaves only the intended 8 px panel padding at the bottom. Clicking near the expanded waveform's lower edge correctly sought the original source. Setup and Review cuts screenshots were inspected locally, and the input project checksum stayed unchanged.

Visible cleanup mode wording is **Automatic**. Help/tooltips, the speech-profile description and rendered legacy detection reasons no longer name the reference application. Existing serialized mode IDs and analysis metadata remain compatible; technical provenance in source/docs is retained. The native desktop build, formatting and whitespace checks passed. No cutting or playback algorithms changed in this UI-only pass.

## Short pause preservation and clean joins — 2026-11-08

The user's overcutting report was reproduced in the saved Tiny English result: exact transcript rules removed entire 520 ms and 640 ms gaps. The supplied human-edited reference was inspected locally (692.011 seconds, 4K H.264 with stereo 48 kHz AAC). At a fixed −42 dBFS threshold it contains 357 quiet intervals of at least 100 ms; median duration is 135 ms and the 95th percentile is 305 ms. These are audio-level observations, not word alignment, human cut labels or an accuracy score. Media and detailed measurements remain private.

New native workspaces enable Preserve natural pauses: keep gaps up to 800 ms between retained words, independently of take detection. Longer gaps keep 150 ms total breathing room by default; sentence gaps keep at least 250 ms. Padding is retained only beside surviving words. Discarded takes and their intervening gaps are joined before frame snapping, avoiding isolated quiet clips and one-frame remnants. Standalone pause removals shorter than 120 ms are skipped. Existing exact settings remain available and saved maps are unchanged until explicit Update cut.

An initial blanket-padding pass lengthened the result to 14:06 and was rejected as too loose. The final context-aware result is **778.359 seconds (12:58.359)**, compared with 750.384 seconds (12:30.384) for the earlier exact result. Both reported short gaps are preserved. All **123 take groups** and word-removal classifications remain unchanged. One previously selected discarded-word fragment, left by separate frame rounding, is now removed correctly. Pause candidates change from 214 to 193; that count includes gaps inside discarded takes and is not an editorial quality score. No new analysis was created and the input project checksum stayed unchanged.

**32 M2 engine regressions passed**, including the unchanged 196-case exact-rule corpus, short/sentence pause preservation, silence-only pacing, tiny-edit suppression, manual restoration, saved settings, adjacent filler/take removals and adjoining video-boundary quantization. The 81 M1 engine checks, five automation checks, and project/format checks passed. Native validation exercised the actual Preserve natural pauses / Update cut controls, wide and compact layouts, save/reopen, durable apply and grouped undo/redo. The primary processing action remains above the scrollable Setup settings. The final settings screenshot was inspected locally.

Candidate: `shotclutch-smart-cut-pacing.exe`, SHA-256 `523addd265d5b09c6f0ee4ea123716ebc5540cfa2af49330f36a40e1bfd24a90`. This is the same development qualification profile as the earlier candidate. The optional `preservePausesMs` field preserves old v18 exact maps when absent. Enable Preserve natural pauses and use Update cut for existing analysis; no transcription or upload is required. Human review of the final automatic result remains open.

## OpenGling transcript and inference parity — 2026-11-08

Audited OpenGling 0.12.0 source and its matching packaged cleanup module. New Smart Cut workspaces explicitly select Tiny English · Fast and Gling-compatible cleanup. Saved profiles/modes are preserved; Update cut reuses saved timed words, while changing recognition profiles requires fresh transcription. Conservative and Legacy transcript modes retain the earlier behavior. [Scope and differences](OPENGLING_CLEANUP_ASSESSMENT.md), [pinned runtime/model decision](DEC-005_SPEECH_PROVIDERS.md).

The same 4,031 saved timed words and 500 ms pause threshold isolated cleanup behavior from recognition differences:

| Cleanup | Retained duration |
| --- | --- |
| Earlier Natural / Conservative | 1,866.464 s (31:06.464) |
| Earlier Transcript / Legacy transcript | 1,537.054 s (25:37.054) |
| OpenGling raw English transcript rules | 1,055.950 s (17:35.950) |
| New native Gling-compatible result | 1,058.523 s (17:38.523) |

All **220 raw removal intervals and types** matched on that transcript before presentation-grid snapping: 86 bad takes, one filler and 133 pauses. The native result rounds removal boundaries inward to certified video/sample points and contains 179 retained intervals. This accounts for the small retained-duration increase. These are raw transcript-rule results, not the reference's optional acoustic refinement and not a second fresh recognition run. The input project was not overwritten.

The synthetic differential regression also matches the reference on **196 cases** without committing private speech. The new restoration regression verifies that an explicit Keep range survives Gling-compatible regeneration and save/reopen. The unchanged M1 runner passed all library/integration checks plus 81 engine and five automation regressions. The expanded M2 runner passed 28 engine regressions, one indexed-boundary check and **20 speech tests**, including fast-worker decoding, invalid timing/confidence, pre-load cancellation, catalogue hashes and AssemblyAI parsing. Full-recording qualification then exposed a stale 20-second waveform validation limit. Decode and project validation now share a typed model-specific checkpoint bound; the expanded **29 engine regressions** include five-minute Tiny/Base English/AssemblyAI coverage and reject oversized or legacy-profile checkpoints. Private real-model checks remain separately invoked.

The longer recognition pass also exposed Windows pipe encoding of non-ASCII transcript characters. Worker output now uses standard ASCII-escaped JSON with explicit UTF-8 process encoding and rejects nonfinite JSON values. Two standard-library-only Python regressions cover Unicode round trips through a legacy Windows encoding and invalid numeric payloads; the M2 runner includes them. Project/time/edit/format and all 81 M1 engine regressions were rechecked after the shared-bound change.

Fresh full-recording inference then completed on the same Windows Ryzen 7 4800H host, selected main audio, English, CPU INT8, beam five, VAD and word timestamps. Both runs used the same pinned Python packages and Tiny English weights, ran serially without compilation, and kept audio/transcripts local:

| Measurement | Smart Cut bounded inference | OpenGling reference whole-file inference |
| --- | --- | --- |
| Recording coverage | 2,271.551 s, eight completed checkpoints | 2,271.531 s reported by its audio decoder |
| Timed words | 3,935 | 3,831 |
| Wrapper wall time | 87.484 s | 73.375 s |
| Sampled peak process-tree working set | 570.4 MiB | 1,541.3 MiB |
| Raw-rule retained duration | 746.140 s | 767.120 s |
| Native frame-aligned result | 750.384 s (12:30.384) | Not a Shotclutch assembly |

Smart Cut's test-reported analysis/verification time was 86.4 seconds. Its run also measures audio/waveform data and writes acknowledged durable checkpoints; the reference run transcribes the whole file and writes its result. Memory was sampled every 0.5 seconds and includes each owned Python worker. These are single local development-build observations with ordinary background host activity, not a controlled release benchmark. The earlier 1,076.5-second Candle base run below is historical, with different model/load conditions.

Running OpenGling's rules on the new 3,935-word Shotclutch transcript matched all **337 raw removal ranges/types** exactly. The native result adds 4.244 seconds through inward presentation-boundary snapping. Fresh whole-file recognition differs from bounded recognition in words/alignment and therefore in heuristic take boundaries. Both fresh raw-rule results are more aggressive than the user's reported 18-minute result. The comparison does not establish that those extra cuts are editorially correct, nor does it target an arbitrary duration; every cut remains available for review/restoration. The saved base transcript can generate the separate 17:38.523 result above without retranscription.

The final native walkthrough passed pause restoration, take comparison/retention, highlighted phrase Cut / Uncut, transport, full audio refresh preserving decisions and preview/control layout, durable apply, save/reopen, grouped undo/redo, duplicate-apply identity, edited-target rejection and fresh-sequence creation. Audio refresh took 38.47 seconds; the sampled cleanup command round trip was 16.49 ms. The source review project checksum remained unchanged and the final workspace screenshot was inspected locally. The new private worksheet has 16 unreviewed rows: human review coverage is 0%, and restoration/adjustment/accepted-removal rates remain null.

Final native candidate: `shotclutch-smart-cut-parity.exe`, SHA-256 `b07746d623767c2d20bd294ab83d656da6d86670996db8fa4ffb30f812d8ce8f`. It is a Windows development qualification build with engine/UI optimization overridden to zero, not a release package. Superseded task executable/debug artifacts are removed after verification. Human editorial acceptance and optional live AssemblyAI qualification remain open.

A bounded private Tiny English excerpt completed recognition and automatic cutting. A fresh native video excerpt produced 34 timed words, persisted the new default model/mode and displayed the result picture, captions and icon transport. Screenshots were inspected locally. Native installation against already cached artifacts displayed progress, verified the pinned runtime/model and reached Installed; this does not claim a new network transfer. During real fast-model analysis, volatile progress left project revision unchanged; cancellation completed in 0.172 seconds with no owned Python worker remaining. The longest sampled control query in that short check was 16 ms, not a rendered-frame measurement. Detailed transcripts, source intervals, native logs and screenshots remain ignored. Removal quantity and exact rule parity do not establish editorial accuracy; the human acceptance gate remains open.

## Transcription responsiveness and preview layout — 2026-11-08

Reproduced Transcribe & cut from the saved full-recording result. Three coupled faults were found: checkpoint publication waited for atomic snapshot writes on the UI thread; each checkpoint's global revision discarded the unchanged result preview; and the processing-state toolbar's fallback layout consumed the viewer's available height. The existing transcript could also leave its selection popup visible when a new run started.

Checkpoint writes now return an acknowledgement receiver. The UI polls it and publishes only acknowledged, current derived data. Review edits, saves, ID allocation and recovery-control changes invalidate a pending base and rebase its analysis event; failed writes publish no unpersisted data. Analysis remains bounded and cancellable. The result cache now follows map/source/render settings instead of every global revision. Preview actions occupy a bounded row at both window widths, media controls stay below the picture, starting analysis clears transient text selection, and the header reports preparation, covered transcription time or finalization.

Local control-channel samples during real CPU Whisper reanalysis of the full recording:

| Measurement | Previous build | Corrected build |
| --- | --- | --- |
| UI query samples | 28 | 113 |
| Median query round trip | 204.60 ms | 102.52 ms |
| 95th percentile query round trip | 3,721.16 ms | 210.25 ms |
| Longest query round trip | 5,012.40 ms | 310.67 ms |
| Invalid viewer/control layouts | 28 | 0 |
| Real completed checkpoints observed | 6 | 5 |
| Preview builds during corrected run | Not instrumented | 1, unchanged across checkpoints |

Both runs used a separate native instance, the same private review project and pinned local model, a roughly 35-second observation window, and wide 1440×950 / compact 1024×720 layouts. Round trips include TCP, scheduling and widget-registry overhead; they are not direct input-to-paint or video-frame measurements. Host load/cache conditions were not isolated as a release benchmark. Both input project checksums stayed unchanged, both jobs were canceled, and screenshots were inspected locally. A separate real-model video excerpt also completed transcription/cutting, produced 35 words, and displayed the generated result picture and transport.

All 26 M2 and 81 M1 engine regressions and 17 autosave regressions passed. New cases hold the recovery worker paused while UI polling and manual review continue, verify rebasing without overwriting decisions or reusing IDs, check cancellation and acknowledged recovery data, reject stale checkpoints after recovery settings change without a project edit, and prove failed writes publish no analysis. The repeatable native workspace harness also checks preview retention and control bounds during audio refresh. Private scripts, media excerpts, transcripts, metrics and screenshots remain ignored. This correctness/performance repair does not replace the outstanding human editorial acceptance. Native candidate: `shotclutch-smart-cut-responsive.exe`. The two superseded task executables were removed after verification.

## Main source audio default — 2026-11-07

Independent local container inspection confirms four encoded stereo AAC tracks in the acceptance MKV. That physical metadata is distinct from the one main audio clip expected in an editing workflow. The prior default expanded every container track, including unused auxiliary tracks, on timeline placement. Video sources now place the main audio only by default, including drag/insert and both sequence-from-clip paths. Audio Channels exposes an explicit Include additional source audio tracks option; the choice is persisted per source, undoable, and applies to future placements. Existing clips and source bytes retain their original stream indices. Audio-only multi-track sources preserve their previous default. The added interpretation field is optional and old v18 documents remain readable.

Smart Cut presents Source audio in ordinary Setup. Track selection is under Advanced source settings for intentional alternate-track analysis. Qualification no longer asks the reviewer to audition unused auxiliary tracks.

Native verification imported the actual MKV into a separate instance, created one linked picture/audio pair by default, enabled the additional-track option through the Audio Channels dialog and confirmed four audio clips, then disabled it and confirmed one audio clip again. Setup's extra-track selector remained hidden in the normal view; screenshots were inspected locally. Nine placement, 12 source-placement, 33 clip-operation, 81 M1 engine and 23 M2 regressions passed, together with project/format persistence checks. The new main-audio regression is included in the M2 CI filter and checks explicit opt-in, channel mapping, invalid parameters, save/reopen, grouped history and sequence creation. Private native artifacts remain ignored. The corrected native candidate is `shotclutch-smart-cut-main-audio.exe`.

## Closeout qualification — 2026-11-07

The final audit added protection for take matches that change negation or numeric tokens. Natural automatic take removal now requires identical words or an unfinished earlier prefix; ambiguous endings remain reviewable. Reject grouping restores every alternative in one undoable revision. The native take panel exposes Keep all and grouping rejection/splitting.

Implementation and qualification tools: `52ad1598`. Native Windows candidate: `shotclutch-m2-qualified.exe`, SHA-256 `d1acd707a3097effb6f77d8a25cc3760fefdcea2b35d5b70f1f1b1f2deeff5a0`. The candidate was linked from the final production code with engine/UI development optimization overridden to zero; this is a qualification executable, not a release package.

The unchanged M1 runner passed on Windows: 191 time/project/edit/format library checks, formatting, 81 engine regressions and five automation regressions. The expanded M2 runner passed **22 engine regressions**, one indexed edit-boundary timestamp test and three AssemblyAI parsing/pre-upload cancellation checks. The added collection/relink case removes the original location, verifies the complete collected bytes, relinks them and restores the removed interval before a fresh application. The permission case rejects owner-only `preedit` commands from an editing-scoped client without changing revision or project state. The local CI runner is wired into the existing workflow; no new hosted CI execution is claimed.

Fresh CPU Whisper base inference covered the entire **2,271.551-second** selected recording on UI stream 1 (command stream 0), with **114 completed checkpoints, 4,031 words, 790 review candidates and 38 take groups**. The private ignored test passed and saved a new review project. Test-reported analysis/source-verification time was **1,076.5 seconds**; wrapper wall time was **1,084.41 seconds**. Sampled peak working set was **747.8 MiB**, sampled every 0.5 seconds using Windows process telemetry. This was a warm-model Windows qualification run on the existing Ryzen 7 4800H host with other compilation running; it is not a controlled release performance benchmark. Candidate/group counts measure detection, not correctness or the number of applied cuts.

The source-verification budget is 8 GiB per phase with streamed hashing; local audio decode/recognition remains bounded to 20-second chunks plus context. Analyzer v3 measures near-silent source edges independently of transcript alignment. The earlier source-edge regression and private source-edge native check passed, including a spanning alignment and a short genuine leading word. The final run uses analyzer v3 throughout.

The native walkthrough passed pause restoration, take comparison/retention, highlighted phrase Cut / Uncut, result transport, full audio refresh preserving manual decisions, Open in editor, idempotent apply, save/reopen, grouped undo/redo, rejection of replacement after a mixer edit, and fresh-sequence creation after rejection. Full-recording audio refresh took **84.84 seconds** and preserved all words and manual decisions. The original input project checksum remained unchanged. The control-channel automatic cleanup request took 29.37 ms on this run; this is command round-trip latency, not a rendered-frame or join-quality measurement. Prior native cut/uncut latency checks and shared-monitor playback measurements remain separate observations.

A separate handoff check saved a new private review-ready project with Smart Cut active and visually inspected its transcript, result picture, icon transport, viewer actions and gray/retained source timeline. The main harness now waits for the UI to consume a background application receipt before returning to review; otherwise that receipt can legitimately switch back to Editing on the next frame. Three superseded generated app executables were removed from the task's build directory after the candidate passed.

The reproducible native walkthrough and private sample worksheet are maintained in `scripts/m2/qualify-workspace.py`; `scripts/m2/editorial-metrics.py` calculates sample-row metrics from human verdicts. The worksheet contains **25 unreviewed rows**, so review coverage is 0% and restoration/adjustment/accepted-removal rates are null. Source media, full transcripts, runtime logs, revealing screenshots and detailed labels remain in ignored local storage. **No final human editorial verdict, listening-based track audition, or labeled restoration rate is inferred from these automated runs.** Optional AssemblyAI live-account qualification remains unexecuted; it is separate from the qualified local model profile.

## Windows local checks

| Check | Result |
| --- | --- |
| Native desktop build with default Whisper feature | Passed |
| Native workspace smoke | Reopened the private v18 project, resolved its original, loaded Smart Rough Cut, and created a review revision in memory. |
| Time, project, format and edit library tests | 191 passed with native filesystem permissions |
| M1 engine regression filter on the M2 build | 81 passed |
| M2 engine regression filter | 7 passed. Private model test runs separately. |
| Pinned Whisper base installation | Four artifacts matched the catalogue size and SHA-256. |

The M2 regressions cover half-open mapping and restoration, explicit invocation, compatible reanalysis, durable cancellation and resume, caption omission, stale application rejection, changed media, save/reopen, undo/redo, and duplicate apply. The linked A/V fixture compares the result frame before and after apply and independently checks the exported container for six video frames and 12,000 audio sample frames. The original bytes are also checked after export. Windows atomic save tests need filesystem permissions that the workspace sandbox does not grant. The hosted M2 step adds the headless filter to the existing M1 gate.

## Private recording

The acceptance source is about 37m 52s, with 4K 60 fps H.264 and four stereo 48 kHz AAC streams. A local level scan used 12-second windows starting at 180, 900 and 1800 seconds. Stream 1 had RMS levels of −34.4, −34.8 and −34.0 dBFS. Streams 2–4 were silent in those windows. Stream 1 is the automated qualification input. The native listening and editorial walkthrough remains open.

The pinned CPU model completed the 180–200 second excerpt on stream 1. It produced 46 words and two suggestions. The disposable result sequence matched the indexed source range duration and retained the selected audio stream. Analysis and source verification took 12.2 seconds in the local test run.

The full local run completed on stream 1 with **2,271.551 seconds of coverage**, 114 checkpoints, 4,031 words, 160 suggestions, and 16 take groups. The test confirmed that the disposable sequence retained the chosen stream and covered the exact input duration. Processing and source verification took 783.7 seconds. Windows reported a 756 MiB peak working set when sampled during the run. The review project saved successfully to the ignored qualification directory. The count of suggestions and groups measures proposal generation. Editorial accuracy still needs listening and labels.

The automated desktop smoke used an isolated native instance and left the saved review project ready for the editorial walkthrough. Human judgment of intentional pauses, take quality, and joins is the remaining acceptance gate. Keep the original, project, transcript, and any detailed editorial labels local.

## Script-led UI redesign — 2026-11-06

The workspace was reworked using the supplied Gling screenshot and the local OpenGling layout as references. Native verification covered the Script, Suggestions, and Files & analysis tabs, original and result pictures, transcript click-to-seek, waveform click-to-seek, and the full-width transport. Screenshots were visually inspected at 1440×950 and 1024×720. The original 4K seek needed additional decode time before its picture appeared. The isolated walkthrough left the saved private project unchanged. Screenshots and the walkthrough script remain in ignored local qualification storage.

`cargo check --locked -p shotclutch -j 6` passed. The standard build compiled dependencies but could not replace the older executable because that app was running. Linking the same app with `cargo rustc --locked -p shotclutch --bin shotclutch -j 6 -- -o <local separate executable>` passed, and the native walkthrough used that new executable. Formatting and diff whitespace checks passed. This UI change does not close the human editorial acceptance gate above.

## Automatic cut review

The native fixture opened on the generated cut and verified result playback, original playback, Space to pause, and opening the edit as a sequence. The saved private transcript generated 136 cuts and a 31m 11s preview from the 37m 52s original. The visual walkthrough used the saved analysis and kept its project and screenshots local.

Eight focused M2 engine tests passed. The new regression covers automatic preview and sequence creation without a separate approval step, then verifies that a restored pause survives another cleanup run. The private ASR qualification test was filtered out of this run.

## OpenGling cleanup adaptation — 2026-11-06

The [implementation comparison](OPENGLING_CLEANUP_ASSESSMENT.md) records the local reference files, adapted rules, acoustic differences, and review behavior.

- 17 focused M2 regressions passed, including adaptive noise-floor detection, preserved audible islands, sentence pacing, exact transcript gaps, guarded filler boundaries, varied take openings, stereo phase cancellation, transcript reuse, manual restoration, and efficient automatic-interval assembly. The long private-model test remains separately invoked.
- All 81 M1 engine regressions passed. All 66 project and 25 format library tests passed, including v17 migration and atomic persistence checks.
- A fresh local Whisper run on a 12-second excerpt produced timed words with no cuts in Transcribe only. Native Update cut reused those words. Switching to Transcript rules and updating the cut persisted the selected mode. Screenshots were inspected locally.
- Full-recording audio refresh completed 114 checkpoints in 113.26 seconds and preserved all 4,031 saved words exactly. These measurements include source verification and native-host overhead. No new Whisper inference was needed for this pass. The final build reused those refreshed measurements with a 500 ms pause threshold and 150 ms padding, producing 675 automatic removal intervals, merging into 314 cuts and removing 812.82 seconds before manual review edits.
- The earlier released cut removed about 400.19 seconds with its older defaults. The final comparison includes changed classification, boundary selection, and the pause threshold. It measures changed removal behavior rather than labeled accuracy. Acoustic measurements also preserved material that a less selective quiet threshold would remove.
- The full-recording native walkthrough filtered cut pauses, played removed material in original context, restored a range, regenerated cleanup while preserving that override, and switched cleanup modes. The original private project checksum stayed unchanged. Only new files in the ignored qualification directory were saved.
- Final native verification compared both takes, retained the earlier take, verified grouped undo/redo, kept both alternatives, and saved/reopened those decisions. The take-review layout was inspected at 1440×950 and 1024×720. The test harness was corrected to order decimal map IDs numerically when identifying the latest revision.

The workspace still cuts automatically before review. Human judgment of take selection, intentional pauses, clipped speech, audible joins, and restoration rates remains necessary for M2 editorial acceptance.
