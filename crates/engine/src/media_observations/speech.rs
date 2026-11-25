//! Converts a bounded local speech result to one provenance-backed segment.

use sha2::{Digest, Sha256};
use shotclutch_project::Transcript;
use shotclutch_project::observations::{ConfidenceMeaning, MAX_SUMMARY_BYTES, SourceStream, TextRole, WordTime};
use shotclutch_speech::{Options, SAMPLE_RATE, TICKS_PER_SAMPLE, fast_whisper};
use shotclutch_time::{TICKS_PER_SECOND, TimeRange};

use super::*;

pub const LOCAL_SPEECH_PRODUCER: &str = "shotclutch-local-speech";

/// A bounded source-time window for a pinned, already installed English speech model.
#[derive(Clone, Debug)]
pub struct SpeechObservationRequest {
    pub asset: ItemId,
    pub expected_revision: u64,
    pub expected_fingerprint: SourceFingerprint,
    pub range: TimeRange,
    pub audio_stream: u16,
    pub model: String,
}

/// The active profile identity for a supported local model. Loading separately verifies its files.
pub fn local_speech_profile(model: &str) -> Option<ModelVersion> {
    if !fast_whisper::supports(model) {
        return None;
    }
    let info = shotclutch_speech::models::find(model)?;
    let mut model_digest = Sha256::new();
    for file in info.files {
        model_digest.update(file.sha256.as_bytes());
    }
    let settings = format!("{}\n{SAMPLE_RATE}Hz\nen\nmono-downmix\nno-diarization", fast_whisper::PROFILE);
    Some(ModelVersion {
        producer_version: "1".into(),
        model: info.id.into(),
        model_version: format!("{:x}", model_digest.finalize()),
        configuration_sha256: format!("{:x}", Sha256::digest(settings)),
    })
}

pub(crate) fn record_speech(
    session: &mut Session,
    request: SpeechObservationRequest,
    context: &ReadContext<'_>,
) -> Result<Option<ObservationId>, ServiceError> {
    check_write(session, request.expected_revision, context)?;
    let version = local_speech_profile(&request.model).ok_or_else(|| error(ErrorCode::CapabilityUnavailable, "select a pinned offline speech model"))?;
    let range = request.range;
    let end = range.start.0.checked_add(range.duration.0).ok_or_else(|| error(ErrorCode::InvalidArgument, "speech window overflows source time"))?;
    if range.start < Tick::ZERO || !(1..=30 * TICKS_PER_SECOND).contains(&range.duration.0) {
        return Err(error(ErrorCode::InvalidArgument, "select a nonempty source window of at most 30 seconds"));
    }
    let first_sample = range.start.0 / TICKS_PER_SAMPLE + i64::from(range.start.0 % TICKS_PER_SAMPLE != 0);
    let frames = usize::try_from(end / TICKS_PER_SAMPLE - first_sample)
        .ok()
        .filter(|count| *count > 0)
        .ok_or_else(|| error(ErrorCode::InvalidArgument, "speech window contains no complete audio samples"))?;

    let (media, path) = original(session, request.asset)?;
    let mut bytes = Vec::new();
    verify_source(session, media, path, &request.expected_fingerprint, context, Some(&mut bytes))?;
    let source = open_snapshot(session, path, bytes)?;
    let info = source.info();
    let index = usize::from(request.audio_stream);
    if end > info.duration.0 || !info.audio_streams.get(index).is_some_and(|stream| (1..=32).contains(&stream.channels)) {
        return Err(error(ErrorCode::InvalidArgument, "speech window or audio stream is outside the original's bounds"));
    }

    let audio = context.cancellation.with_media(|| source.audio_stream(index, first_sample, frames, SAMPLE_RATE));
    context.check()?;
    let audio = audio.map_err(|_| error(ErrorCode::VerificationFailed, "original audio could not be decoded"))?;
    let mono = shotclutch_speech::downmix(&audio.channels);
    if mono.len() != frames {
        return Err(error(ErrorCode::VerificationFailed, "source reader returned incomplete speech audio"));
    }
    let models = crate::transcript::models_dir().ok_or_else(|| error(ErrorCode::CapabilityUnavailable, "local speech model directory is unavailable"))?;
    let model =
        shotclutch_speech::load(&models, &request.model).map_err(|_| error(ErrorCode::CapabilityUnavailable, "pinned local speech model is unavailable"))?;
    context.check()?;
    let options = Options { language: Some("en".into()), diarize: false, ..Options::default() };
    let transcript = model.transcribe(&mono, &options, &mut |_, _| context.check().is_ok());
    context.check()?;
    let transcript = transcript.map_err(|_| error(ErrorCode::CapabilityUnavailable, "offline speech inference failed"))?;
    let Some((summary, slice, confidence, mut word_times)) = summarize(&transcript, shotclutch_speech::sample_tick(frames as i64)) else { return Ok(None) };
    let offset = shotclutch_speech::sample_tick(first_sample);
    for time in &mut word_times {
        time.start += offset.0;
        time.end += offset.0;
    }
    let location =
        SourceLocation::SourceRange { stream: SourceStream::Audio(request.audio_stream), start: (slice.start + offset).0, end: (slice.end() + offset).0 };
    let observation = Observation {
        kind: ObservationKind::Segment,
        evidence: source_evidence(request.asset, request.expected_fingerprint, location, info)?,
        provenance: Provenance::Model { producer: LOCAL_SPEECH_PRODUCER.into(), version, confidence },
        created_unix_seconds: crate::autosave::unix_now(),
        summary: Some(summary),
        text_role: Some(TextRole::Transcript),
        word_times,
        speaker: None,
        related: Vec::new(),
    };
    commit_observation(session, observation, context).map(Some)
}

fn summarize(transcript: &Transcript, limit: Tick) -> Option<(String, TimeRange, Confidence, Vec<WordTime>)> {
    let mut text = String::new();
    let mut start = None;
    let mut end = Tick::ZERO;
    let mut score = 0.0f64;
    let mut word_times = Vec::new();
    for word in &transcript.words {
        let word_text = word.text.trim();
        if word_text.is_empty()
            || word_text.chars().any(char::is_control)
            || word.start < end
            || word.end <= word.start
            || word.end > limit
            || !(0.0..=1.0).contains(&word.confidence)
        {
            continue;
        }
        if text.len() + usize::from(!text.is_empty()) + word_text.len() > MAX_SUMMARY_BYTES {
            break;
        }
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(word_text);
        start.get_or_insert(word.start);
        end = word.end;
        score += f64::from(word.confidence);
        word_times.push(WordTime { start: word.start.0, end: word.end.0 });
    }
    let range = TimeRange::from_bounds(start?, end);
    let confidence = Confidence { score: (score / word_times.len() as f64) as f32, meaning: ConfidenceMeaning::ModelScore };
    // Some recognizers time a group of words together. Preserve that text at segment precision.
    if text.split_whitespace().count() != word_times.len() {
        word_times.clear();
    }
    Some((text, range, confidence, word_times))
}

#[cfg(test)]
mod tests {
    use super::*;
    use shotclutch_project::Word;

    #[test]
    fn summary_uses_only_model_words_that_fit_and_keeps_their_exact_bounds() {
        let transcript = Transcript {
            words: vec![
                Word { confidence: 0.5, ..Word::new("First", Tick(1), Tick(6)) },
                Word::new("second.", Tick(7), Tick(13)),
                Word::new("a".repeat(MAX_SUMMARY_BYTES), Tick(15), Tick(18)),
            ],
            ..Transcript::default()
        };
        let (text, span, confidence, _) = summarize(&transcript, Tick(20)).unwrap();
        assert_eq!(text, "First second.");
        assert_eq!(span, TimeRange::from_bounds(Tick(1), Tick(13)));
        assert_eq!(confidence, Confidence { score: 0.75, meaning: ConfidenceMeaning::ModelScore });
        assert!(summarize(&Transcript::default(), Tick(20)).is_none());
        assert!(local_speech_profile("assemblyai").is_none());
        assert_ne!(local_speech_profile(fast_whisper::TINY_EN), local_speech_profile(fast_whisper::BASE_EN));
    }
}
