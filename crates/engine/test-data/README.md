# Gling-compatible cleanup corpus

`opengling-cleanup-v2.json` contains 196 synthetic English timed-transcript cases and expected raw removal ranges/types from the local OpenGling 0.12.0 `src/speech-cleanup.js` implementation (`speech-cleanup-2.0.0`). Reference module SHA-256: `bfd5a3f49ccd65fa54ef0e960ee7a178bb973af016253559f53869162c93b0f1`.

Cases cover hesitation vocabulary and punctuation, timing gaps, repeated openings, within-take patterns, cross-take suffixes, overlapping speech and fixed-seed artificial token sequences. These are authored examples, not acceptance-footage transcripts. The fixture is consumed by `m2_gling_compatible_matches_opengling_synthetic_corpus`; running the test requires neither OpenGling nor a speech runtime.

Expected ranges precede Shotclutch's inward frame/sample snapping. Each reference invocation uses English, all three cleanup categories enabled, the case duration and a 0.5-second pause threshold. Paragraph/media grouping and optional acoustic refinement are outside this corpus. Do not replace these expectations with Shotclutch's own output when changing the implementation.
