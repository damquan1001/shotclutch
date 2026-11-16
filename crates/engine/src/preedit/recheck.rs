use std::sync::mpsc::SyncSender;

use shotclutch_media::SharedSource;
use shotclutch_project::{
    Word,
    preedit::{MAX_RETAKE_RECHECK_WINDOWS, PreEditAnalysis, RecheckedRestart, RetakeRecheck},
};
use shotclutch_speech::{SAMPLE_RATE, TICKS_PER_SAMPLE};
use shotclutch_time::{Tick, TimeRange};

use super::ordered_takes::broader::indexed::{continuation, has_quiet_boundary};
use super::ordered_takes::broader::{marker_text, protected};
use super::{AnalysisProgress, Message, analysis, stopped};
use crate::service::Cancellation;

const MODEL: &str = "faster-whisper-base.en";

fn ms(value: i64) -> Tick {
    Tick::from_units(value, 1000)
}

fn windows(source: TimeRange, words: &[&Word], quiet: &[TimeRange]) -> Vec<(TimeRange, TimeRange)> {
    let mut anomalies: Vec<_> = words
        .iter()
        .filter(|word| {
            let time = word.range();
            (ms(1250)..=ms(4000)).contains(&time.duration)
                && (1..=5).contains(&word.normalized().chars().count())
                && has_quiet_boundary(quiet, time.start + ms(160), time.end() - ms(160))
        })
        .map(|word| word.range())
        .collect();
    anomalies.sort_by_key(|word| (std::cmp::Reverse(word.duration), word.start));
    let mut result: Vec<(TimeRange, TimeRange)> = Vec::new();
    for anomaly in anomalies {
        let range = TimeRange::from_bounds((anomaly.start - ms(6500)).max(source.start), (anomaly.end() + ms(5500)).min(source.end()));
        if !result.iter().any(|(other, _)| other.overlaps(&range)) {
            result.push((range, anomaly));
        }
        if result.len() == MAX_RETAKE_RECHECK_WINDOWS {
            break;
        }
    }
    result.sort_by_key(|(range, _)| range.start);
    result
}

fn keys(words: &[&Word]) -> Vec<String> {
    words.iter().map(|word| word.normalized().replace('’', "'")).collect()
}

fn confirm(original: &[&Word], local: &[&Word], anomaly: TimeRange, quiet: &[TimeRange]) -> Option<RecheckedRestart> {
    let local_keys = keys(local);
    let original_keys = keys(original);
    for split in 9..local.len() {
        let seam = local[split - 1].end;
        let next_start = local[split].start;
        if !anomaly.contains(seam) || next_start - seam > ms(1000) {
            continue;
        }
        let quiet_index = quiet.partition_point(|span| span.end() <= seam);
        let boundary = quiet[quiet_index..].iter().take_while(|span| span.start < next_start).find_map(|span| {
            let lo = span.start.max(seam);
            let hi = span.end().min(next_start);
            (hi - lo >= ms(70)).then(|| lo + ms(35))
        });
        let Some(boundary) = boundary else { continue };
        for len in (9..=split.min(24)).rev() {
            let start = split - len;
            let Some(keep) = continuation(local, &local_keys, start..split, split, true) else { continue };
            let keep_range = TimeRange::from_bounds(local[keep.start].start, local[keep.end - 1].end);
            // The second model can pull the first token backwards over a pause.
            // Match the next three word times and use the original opening.
            let anchor = local[start + 1].start;
            let lo = original.partition_point(|word| word.start < anchor - ms(420)).saturating_sub(1);
            let hi = original.partition_point(|word| word.start <= anchor + ms(420));
            let Some(first) = (lo..hi.min(original.len().saturating_sub(3))).find(|&i| {
                original_keys[i..i + 4] == local_keys[start..start + 4]
                    && (1..4).all(|j| (original[i + j].start - local[start + j].start).0.unsigned_abs() <= ms(420).0 as u64)
            }) else {
                continue;
            };
            let start_time = original[first].start;
            if boundary <= start_time || start_time < anomaly.start - ms(6500) || (first > 0 && !has_quiet_boundary(quiet, original[first - 1].end, start_time))
            {
                continue;
            }
            let lo = original.partition_point(|word| word.start < keep_range.end() - ms(1200));
            let hi = original.partition_point(|word| word.start <= keep_range.end() + ms(900));
            let Some(last) =
                (lo..hi).filter(|&i| original_keys[i] == local_keys[keep.end - 1]).min_by_key(|&i| (original[i].end - keep_range.end()).0.unsigned_abs())
            else {
                continue;
            };
            let end = last + 1;
            let Some(baseline) = original.get(first..end) else { continue };
            if baseline.len() < 9
                || (original[last].end - keep_range.end()).0.unsigned_abs() > ms(900).0 as u64
                || baseline.iter().any(|word| word.speaker != baseline[0].speaker)
            {
                continue;
            }
            // Require agreement around the opening, and preserve all recognized
            // words from the original reading in the surviving performance.
            let mut next = keep;
            if (first..end).all(|i| {
                next.by_ref().any(|j| original_keys[i] == local_keys[j] && (!protected(&original_keys[i]) || marker_text(original[i]) == marker_text(local[j])))
            }) {
                return Some(RecheckedRestart { discard: TimeRange::from_bounds(start_time, boundary), keep: keep_range });
            }
        }
    }
    None
}

pub(super) fn run(original: &SharedSource, saved: &PreEditAnalysis, cancel: &Cancellation, sender: &SyncSender<Message>) -> Result<RetakeRecheck, String> {
    let mut settings = saved.settings.clone();
    settings.model = Some(MODEL.into());
    let model_digest = analysis::model_digest(&settings)?.ok_or("CAPABILITY_UNAVAILABLE: local retake model is unavailable")?;
    let directory = crate::transcript::models_dir().ok_or("CAPABILITY_UNAVAILABLE: speech model directory is unavailable")?;
    let model = shotclutch_speech::load(&directory, MODEL).map_err(|err| format!("Install Base English · Fast to recheck retakes locally. {err}"))?;
    let source = &saved.source;
    let mut words: Vec<_> = saved.chunks.iter().filter_map(|chunk| chunk.transcript.as_ref()).flat_map(|transcript| &transcript.words).collect();
    words.sort_by_key(|word| word.start);
    let quiet = super::cleanup::quiet_ranges(saved);
    let windows = windows(source.range, &words, &quiet);
    let total = windows.iter().fold(Tick::ZERO, |duration, (range, _)| duration + range.duration);
    let mut processed = Tick::ZERO;
    let opts = shotclutch_speech::Options { language: Some("en".into()), diarize: false, ..Default::default() };
    let mut restarts = Vec::new();
    for (i, (range, anomaly)) in windows.iter().enumerate() {
        stopped(cancel)?;
        let start = range.start.0.div_euclid(TICKS_PER_SAMPLE);
        let end = range.end().0.div_euclid(TICKS_PER_SAMPLE);
        let audio = original
            .audio_stream(source.audio_stream, start, (end - start) as usize, SAMPLE_RATE)
            .map_err(|_| "CAPABILITY_UNAVAILABLE: retake audio could not be decoded")?;
        let mono = if let Some(channel) = source.channel {
            audio.channels.get(usize::from(channel)).cloned().ok_or("INVALID_ARGUMENT: audio channel is missing")?
        } else {
            shotclutch_speech::downmix(&audio.channels)
        };
        let phase = format!("Rechecking retakes ({}/{})", i + 1, windows.len());
        let _ = sender.try_send(Message::Progress(AnalysisProgress { processed, total, phase }));
        let mut transcript = model.transcribe(&mono, &opts, &mut |_, _| stopped(cancel).is_ok()).map_err(|err| err.to_string())?;
        let offset = Tick(start * TICKS_PER_SAMPLE);
        for word in &mut transcript.words {
            word.start += offset;
            word.end += offset;
        }
        let local: Vec<_> = transcript.words.iter().filter(|word| shotclutch_project::preedit::encloses(*range, word.range())).collect();
        if let Some(restart) = confirm(&words, &local, *anomaly, &quiet) {
            restarts.push(restart);
        }
        processed += range.duration;
    }
    stopped(cancel)?;
    Ok(RetakeRecheck { model_digest, checked_windows: windows.len() as u8, restarts })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attempt(text: &str, start: i64) -> Vec<Word> {
        text.split_whitespace()
            .enumerate()
            .map(|(i, word)| {
                let at = start + i as i64 * 200;
                Word::new(word, ms(at), ms(at + 200))
            })
            .collect()
    }

    #[test]
    fn m2_audio_recheck_confirms_same_line_across_a_merged_word() {
        let short = "I was able to watch most of the film,";
        let full = "I was able to watch most of the film without touching any settings.";
        let local: Vec<_> = attempt(short, 0).into_iter().chain(attempt(full, 2050)).collect();
        let mut original = attempt(full, 0);
        original[6..].clone_from_slice(&local[15..]);
        let anomaly = TimeRange::from_bounds(original[5].start, original[6].start);
        original[5].end = anomaly.end();
        let quiet = [TimeRange::from_bounds(ms(1820), ms(1990))];
        let local: Vec<_> = local.iter().collect();
        let baseline: Vec<_> = original.iter().collect();
        assert_eq!(windows(TimeRange::from_bounds(Tick::ZERO, ms(12_000)), &baseline, &quiet)[0].1, anomaly);
        assert_eq!(confirm(&baseline, &local, anomaly, &quiet).unwrap().discard.end(), ms(1855));
        assert!(confirm(&baseline, &local, anomaly, &[]).is_none());
        original[4].text = "ignore".into();
        assert!(confirm(&original.iter().collect::<Vec<_>>(), &local, anomaly, &quiet).is_none());
    }
}
