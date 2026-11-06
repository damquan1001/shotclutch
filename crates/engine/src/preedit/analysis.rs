use std::io::Read;
use std::path::Path;
use std::sync::{Arc, mpsc::SyncSender};

use sha2::{Digest, Sha256};
use shotclutch_media::SharedSource;
use shotclutch_project::MediaIdentity;
use shotclutch_project::preedit::*;
use shotclutch_speech::{SAMPLE_RATE, TICKS_PER_SAMPLE};
use shotclutch_time::{Tick, TimeRange};

use super::{Message, stopped};
use crate::service::Cancellation;

const CHUNK_SAMPLES: i64 = 20 * SAMPLE_RATE as i64;
pub(super) const ANALYZER_VERSION: u32 = 2;
pub(super) const SOURCE_BUDGET: u64 = 8 * 1024 * 1024 * 1024;

pub(super) fn fingerprint(path: &Path, imported: Option<MediaIdentity>, cancel: &Cancellation) -> Result<(String, u64), String> {
    stopped(cancel)?;
    let initial = std::fs::metadata(path).map_err(|_| "MEDIA_CHANGED: original source unavailable")?;
    if !initial.is_file() {
        return Err("CAPABILITY_UNAVAILABLE: original is not a regular file".into());
    }
    if initial.len() == 0 || initial.len() > SOURCE_BUDGET {
        return Err("RESOURCE_LIMIT: original must be nonempty and within the source verification budget".into());
    }
    let mut file = std::fs::File::open(path).map_err(|_| "MEDIA_CHANGED: original source could not be opened")?;
    let span = crate::relink::FINGERPRINT_SPAN;
    let mut buffer = vec![0u8; span];
    let mut first = Vec::new();
    let mut last = Vec::new();
    let mut hash = Sha256::new();
    let mut count = 0u64;
    loop {
        stopped(cancel)?;
        let n = file.read(&mut buffer).map_err(|_| "MEDIA_CHANGED: original source read failed")?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > SOURCE_BUDGET {
            return Err("RESOURCE_LIMIT: original source grew beyond the verification budget".into());
        }
        hash.update(&buffer[..n]);
        first.extend_from_slice(&buffer[..n.min(span - first.len())]);
        last.extend_from_slice(&buffer[..n]);
        if last.len() > span {
            last.drain(..last.len() - span);
        }
    }
    let end = std::fs::metadata(path).map_err(|_| "MEDIA_CHANGED: original disappeared during verification")?;
    if count != initial.len() || end.len() != count || end.modified().ok() != initial.modified().ok() {
        return Err("MEDIA_CHANGED: original changed during verification".into());
    }
    if let Some(expected) = imported {
        let tail = if count > 2 * span as u64 { &last[..] } else { &last[last.len() - (count as usize - first.len())..] };
        if expected.size != count || expected.fingerprint != crate::relink::fingerprint(count, &first, tail) {
            return Err("MEDIA_CHANGED: original no longer matches the imported identity".into());
        }
    }
    Ok((format!("{:x}", hash.finalize()), count))
}

pub(super) fn model_digest(settings: &AnalysisSettings) -> Result<Option<String>, String> {
    let Some(model) = &settings.model else { return Ok(None) };
    let info = shotclutch_speech::models::find(model).ok_or("INVALID_ARGUMENT: unknown speech model")?;
    let mut digest = Sha256::new();
    for file in info.files {
        digest.update(file.sha256.as_bytes());
    }
    Ok(Some(format!("{:x}", digest.finalize())))
}

pub(super) fn run(
    original: SharedSource,
    analysis: &mut PreEditAnalysis,
    reuse: Option<&PreEditAnalysis>,
    cancel: &Cancellation,
    sender: &SyncSender<Message>,
) -> Result<(), String> {
    let transcriber = if let Some(model) = analysis.settings.model.as_ref().filter(|_| reuse.is_none()) {
        let dir = crate::transcript::models_dir().ok_or("CAPABILITY_UNAVAILABLE: speech model directory is unavailable")?;
        Some(shotclutch_speech::load(&dir, model).map_err(|e| format!("CAPABILITY_UNAVAILABLE: {e}"))?)
    } else {
        None
    };
    let opts = shotclutch_speech::Options { language: Some(analysis.settings.language.clone()), diarize: false, ..Default::default() };
    let source = &analysis.source;
    let mut start = analysis.chunks.last().map_or(source.range.start, |chunk| chunk.coverage.end());
    while start < source.range.end() {
        stopped(cancel)?;
        let end = (start + Tick(CHUNK_SAMPLES * TICKS_PER_SAMPLE)).min(source.range.end());
        let range = TimeRange::from_bounds(start, end);
        let sample_start = start.0.div_euclid(TICKS_PER_SAMPLE);
        let sample_end = end.0.div_euclid(TICKS_PER_SAMPLE) + i64::from(end.0.rem_euclid(TICKS_PER_SAMPLE) != 0);
        // One second of context on either side reduces alignment artifacts at chunk boundaries.
        let context_start = (sample_start - i64::from(SAMPLE_RATE)).max(0);
        let context_end = (sample_end + i64::from(SAMPLE_RATE)).min(original.info().duration.0.div_euclid(TICKS_PER_SAMPLE) + 1);
        let audio = original
            .audio_stream(source.audio_stream, context_start, (context_end - context_start) as usize, SAMPLE_RATE)
            .map_err(|_| "CAPABILITY_UNAVAILABLE: selected audio stream could not be decoded")?;
        let mono = if let Some(channel) = source.channel {
            audio.channels.get(usize::from(channel)).cloned().ok_or("INVALID_ARGUMENT: audio channel is missing")?
        } else {
            shotclutch_speech::downmix(&audio.channels)
        };
        let lo = (sample_start - context_start) as usize;
        let hi = (sample_end - context_start) as usize;
        let audible = mono.get(lo..hi).ok_or("CAPABILITY_UNAVAILABLE: incomplete decoded audio chunk")?;
        // Measure channel energy separately so phase-inverted stereo cannot cancel to silence.
        let mut levels = vec![f32::NEG_INFINITY; audible.len().div_ceil(SAMPLE_RATE as usize / 100)];
        for (index, channel) in audio.channels.iter().enumerate() {
            if source.channel.is_some_and(|selected| usize::from(selected) != index) {
                continue;
            }
            let channel = channel.get(lo..hi).ok_or("CAPABILITY_UNAVAILABLE: incomplete audio channel")?;
            for (level, db) in levels.iter_mut().zip(shotclutch_speech::vad::frame_db(channel)) {
                *level = level.max(db);
            }
        }
        let quiet = super::cleanup::quiet_intervals(range, &levels);
        let peaks = audible
            .chunks(SAMPLE_RATE as usize / 20)
            .map(|samples| (samples.iter().fold(0.0f32, |peak, sample| peak.max(sample.abs())).min(1.0) * 255.0).round() as u8)
            .collect();
        let transcript = if let Some(saved) = reuse {
            saved
                .chunks
                .iter()
                .find(|chunk| chunk.coverage == range)
                .ok_or("INVALID_ARGUMENT: saved transcript coverage does not match audio refresh")?
                .transcript
                .clone()
        } else if let Some(model) = &transcriber {
            let mut t = model.transcribe(&mono, &opts, &mut |_, _| stopped(cancel).is_ok()).map_err(|e| e.to_string())?;
            let offset = Tick(context_start * TICKS_PER_SAMPLE);
            for w in &mut t.words {
                w.start += offset;
                w.end += offset;
            }
            // The midpoint owns a boundary word exactly once. Clamp its visible bounds to its chunk.
            t.words.retain(|w| range.contains(w.start + Tick((w.end - w.start).0 / 2)));
            for w in &mut t.words {
                w.start = w.start.max(start);
                w.end = w.end.min(end);
            }
            t.words.retain(|w| w.end > w.start);
            t.normalize();
            Some(t)
        } else {
            None
        };
        stopped(cancel)?;
        let chunk = Arc::new(AnalysisChunk { coverage: range, transcript, quiet, peaks });
        sender.send(Message::Chunk(chunk.clone())).map_err(|_| "CANCELED: project closed")?;
        analysis.chunks.push(chunk);
        start = end;
    }
    Ok(())
}

pub(super) fn is_filler(word: &str) -> bool {
    matches!(
        word,
        "er" | "erm"
            | "ermm"
            | "err"
            | "errr"
            | "hm"
            | "hmm"
            | "hmmm"
            | "um"
            | "umm"
            | "ummm"
            | "uhm"
            | "ah"
            | "uh"
            | "uhh"
            | "uhhh"
            | "mhm"
            | "mmhm"
            | "mm-hm"
            | "mm-hmm"
    )
}

pub(super) fn propose(analysis_id: u64, analysis: &PreEditAnalysis, cleanup: Option<CleanupSettings>) -> PreEditMap {
    super::cleanup::propose(analysis_id, analysis, cleanup)
}
