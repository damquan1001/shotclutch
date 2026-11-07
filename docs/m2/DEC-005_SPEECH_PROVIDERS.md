# DEC-005 — M2 speech providers

Selected and implemented on `codex/m2-smart-rough-cut`; updated 2026-11-07. This decision covers M2 ASR. Embeddings and visual model profiles remain M3 work.

The Windows desktop's default local English profile is CPU `whisper-base` (multilingual, 74M parameters). The catalogue in `crates/speech/src/models.rs` pins OpenAI's Hugging Face revision `e37978b90ca9030d5170a5c07aadb050351a65bb`, four artifact sizes and SHA-256 hashes. Weights are installed in the local data directory, outside the project. They are not bundled or committed. The workspace requires an explicit download and displays bytes, phase, cancellation and failure. Installed artifacts are verified before use. Model download accesses the catalogue host; local inference sends no audio to a provider.

Analysis starts only on request. Local recognition uses 20-second audio chunks with up to one second of context at each end, selected stream/channel, absolute source word timing and durable completed coverage. Cancellation preserves completed chunks; resume revalidates the source and settings. Silence-only is an explicit fallback and does not invent transcript words. Unknown cleanup confidence is shown as unknown.

The user's requested AssemblyAI option is an additional cloud provider in Setup. Selecting it and starting transcription uploads bounded mono PCM audio chunks (five minutes plus context), polls the service, maps returned word times to source time, and retains completed coverage. Credentials remain in process memory or the operator's `ASSEMBLYAI_API_KEY` environment variable; they are not project fields. Errors omit the key and provider response bodies. Local Whisper remains the default. Parser/timing/error regressions are automated; a live account run has not been qualified and is not represented by the local inference evidence.

Compatible saved transcripts can drive a fresh local acoustic pass without loading the recognition model or uploading audio. A mismatched source, range, stream, channel, language or model is rejected. Analyzer v3 adds independently measured source-edge silence so overlong Whisper alignments do not hide verified near-silent leading/trailing material.

See [contracts](IMPLEMENTATION.md#project-and-service-contracts), [procedure](QUALIFICATION.md), and [results](EVIDENCE.md). Measured speed and removal quantity are not editorial quality scores; human judgments remain part of acceptance.
