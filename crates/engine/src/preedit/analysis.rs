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
pub(super) const ANALYZER_VERSION: u32 = 1;
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

pub(super) fn run(original: SharedSource, analysis: &PreEditAnalysis, cancel: &Cancellation, sender: &SyncSender<Message>) -> Result<(), String> {
    let transcriber = if let Some(model) = &analysis.settings.model {
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
        let levels = shotclutch_speech::vad::frame_db(audible);
        let quiet = quiet_intervals(range, &levels);
        let peaks = audible
            .chunks(SAMPLE_RATE as usize / 20)
            .map(|samples| (samples.iter().fold(0.0f32, |peak, sample| peak.max(sample.abs())).min(1.0) * 255.0).round() as u8)
            .collect();
        let transcript = if let Some(model) = &transcriber {
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
        sender.send(Message::Chunk(Arc::new(AnalysisChunk { coverage: range, transcript, quiet, peaks }))).map_err(|_| "CANCELED: project closed")?;
        start = end;
    }
    Ok(())
}

fn quiet_intervals(range: TimeRange, levels: &[f32]) -> Vec<TimeRange> {
    // A conservative energy proxy, not a semantic classification. A constantly loud or
    // low-dynamic-range recording must never be mistaken for all silence by an adaptive threshold.
    let threshold = shotclutch_speech::vad::threshold(levels).min(-42.0);
    let step = Tick(shotclutch_time::TICKS_PER_SECOND / 100);
    let mut quiet = Vec::new();
    let mut start = None;
    for (i, level) in levels.iter().enumerate() {
        let t = (range.start + Tick(i as i64 * step.0)).min(range.end());
        if *level < threshold {
            start.get_or_insert(t);
        } else if let Some(a) = start.take()
            && t > a
        {
            quiet.push(TimeRange::from_bounds(a, t));
        }
    }
    if let Some(a) = start
        && a < range.end()
    {
        quiet.push(TimeRange::from_bounds(a, range.end()));
    }
    quiet
}

pub(super) fn propose(analysis_id: u64, analysis: &PreEditAnalysis) -> PreEditMap {
    let mut candidates = Vec::new();
    let mut takes = Vec::new();
    let mut quiet: Vec<TimeRange> = Vec::new();
    let words: Vec<_> = analysis.chunks.iter().filter_map(|c| c.transcript.as_ref()).flat_map(|t| &t.words).collect();
    for r in analysis.chunks.iter().flat_map(|c| c.quiet.iter().copied()) {
        if let Some(last) = quiet.last_mut()
            && last.end() == r.start
        {
            *last = TimeRange::from_bounds(last.start, r.end());
        } else {
            quiet.push(r);
        }
    }
    let handle = analysis.settings.handle;
    for r in quiet {
        if r.duration < analysis.settings.min_pause || r.duration.0 <= 2 * handle.0 {
            continue;
        }
        let range = TimeRange::from_bounds(r.start + handle, r.end() - handle);
        let i = words.partition_point(|w| w.end <= range.start);
        if words.get(i).is_none_or(|w| w.start >= range.end()) {
            candidates.push(Candidate {
                range,
                kind: CandidateKind::Pause,
                reason: "Low audio energy; listen for quiet speech or an intentional pause".into(),
                confidence: None,
            });
        }
    }
    if analysis.settings.language == "en" {
        for word in &words {
            if matches!(word.normalized().as_str(), "um" | "uh" | "erm" | "hmm") {
                candidates.push(Candidate {
                    range: word.range(),
                    kind: CandidateKind::Filler,
                    reason: "Possible hesitation in the transcript".into(),
                    confidence: None,
                });
            }
        }
        let gap = Tick(shotclutch_time::TICKS_PER_SECOND / 2);
        let mut phrases = Vec::new();
        let mut begin = 0;
        for i in 1..=words.len() {
            if i == words.len() || words[i].start - words[i - 1].end >= gap || words[i - 1].text.ends_with(['.', '!', '?']) {
                phrases.push(&words[begin..i]);
                begin = i;
            }
        }
        let mut grouped = vec![false; phrases.len()];
        for i in 0..phrases.len() {
            let a = phrases[i];
            if a.len() < 2 || grouped[i] {
                continue;
            }
            let left: Vec<_> = a.iter().map(|w| w.normalized()).collect();
            let mut similar = Vec::new();
            for j in i + 1..phrases.len().min(i + 5) {
                let b = phrases[j];
                let right: Vec<_> = b.iter().map(|w| w.normalized()).collect();
                let common = left.iter().zip(&right).take_while(|(a, b)| a == b).count();
                if common >= 3 && common * 4 >= 3 * left.len().max(right.len()) && !grouped[j] {
                    similar.push(j);
                } else if j == i + 1 && common == left.len() && right.len() > left.len() {
                    candidates.push(Candidate {
                        range: TimeRange::from_bounds(a[0].start, a[a.len() - 1].end),
                        kind: CandidateKind::Restart,
                        reason: "The next phrase repeats the opening words and continues".into(),
                        confidence: None,
                    });
                }
            }
            if !similar.is_empty() {
                let alternatives = std::iter::once(i)
                    .chain(similar)
                    .map(|j| {
                        grouped[j] = true;
                        let p = phrases[j];
                        let index = candidates.len();
                        candidates.push(Candidate {
                            range: TimeRange::from_bounds(p[0].start, p[p.len() - 1].end),
                            kind: CandidateKind::Take,
                            reason: "Nearby attempts share an opening phrase".into(),
                            confidence: None,
                        });
                        index
                    })
                    .collect();
                takes.push(TakeGroup { alternatives, explanation: "Compare the full statements before choosing; similar wording can change meaning".into() });
            }
        }
    }
    PreEditMap {
        analysis: analysis_id,
        previous: None,
        reviewed: false,
        candidates: Arc::new(candidates),
        takes: Arc::new(takes),
        overrides: Vec::new(),
        conflicts: Vec::new(),
    }
}
