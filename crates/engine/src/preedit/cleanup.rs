//! Transcript classification and independent acoustic cut boundaries.
//! Rules are adapted to source ticks from OpenGling's speech-cleanup and acoustic-cleanup behavior.
use std::collections::BTreeSet;
use std::sync::Arc;

use shotclutch_project::{Word, preedit::*};
use shotclutch_time::{Tick, TimeRange};

use super::analysis::is_filler;

fn ms(n: i64) -> Tick {
    Tick::from_units(n, 1000)
}

/// Trim only near-silent recording edges, with an audible reference in the same chunk.
/// Low-gain continuous recordings cannot qualify through an absolute threshold alone.
pub(super) fn source_edge_quiet(range: TimeRange, source: TimeRange, levels: &[f32]) -> Vec<TimeRange> {
    let mut audible: Vec<_> = levels.iter().copied().filter(|db| db.is_finite() && *db > -100.0).collect();
    audible.sort_by(f32::total_cmp);
    if audible.is_empty() || audible[(audible.len() - 1) * 95 / 100] < -60.0 {
        return Vec::new();
    }
    let mut edges = Vec::new();
    if range.start == source.start {
        let frames = levels.iter().take_while(|db| **db <= -65.0).count();
        let end = (range.start + ms(frames as i64 * 10)).min(range.end());
        if end - range.start >= ms(500) {
            edges.push(TimeRange::from_bounds(range.start, end));
        }
    }
    if range.end() == source.end() {
        let frames = levels.iter().rev().take_while(|db| **db <= -65.0).count();
        let start = (range.end() - ms(frames as i64 * 10)).max(range.start);
        if range.end() - start >= ms(500) {
            edges.push(TimeRange::from_bounds(start, range.end()));
        }
    }
    edges
}

pub(super) fn verified_edge_pause(analysis: &PreEditAnalysis, range: TimeRange) -> bool {
    analysis.chunks.iter().flat_map(|chunk| &chunk.edge_quiet).any(|edge| encloses(*edge, range))
        && (range.start == analysis.source.range.start || range.end() == analysis.source.range.end())
}

/// A rolling four-second noise/speech distribution, in 10 ms frames. Histogram
/// updates are linear in recording length and never require the full recording.
pub(super) fn quiet_intervals(range: TimeRange, levels: &[f32]) -> Vec<TimeRange> {
    let bin = |db: f32| (db.floor() as i32 + 120).clamp(0, 120) as usize;
    let mut histogram = [0usize; 121];
    let mut sorted = levels.to_vec();
    sorted.sort_by(f32::total_cmp);
    let fallback = if sorted.is_empty() {
        -70.0
    } else {
        let noise = sorted[(sorted.len() - 1) / 10];
        let speech = sorted[(sorted.len() - 1) * 95 / 100];
        if speech - noise >= 12.0 { (noise + 6.0).min(speech - 12.0) } else { -70.0 }
    };
    let mut lo = 0;
    let mut hi = 0;
    let mut intervals = Vec::new();
    let mut start = None;
    for (i, level) in levels.iter().enumerate() {
        let end = (i + 201).min(levels.len());
        while hi < end {
            histogram[bin(levels[hi])] += 1;
            hi += 1;
        }
        while lo < i.saturating_sub(200) {
            histogram[bin(levels[lo])] -= 1;
            lo += 1;
        }
        let percentile = |fraction: f32| {
            let target = (((hi - lo) as f32 * fraction).ceil() as usize).max(1);
            let mut count = 0;
            for (j, n) in histogram.iter().enumerate() {
                count += n;
                if count >= target {
                    return j as f32 - 120.0;
                }
            }
            0.0
        };
        let noise = percentile(0.10);
        let speech = percentile(0.95);
        // Preserve constant noise/music and quiet speech when the levels cannot be separated.
        // Digital silence still qualifies even in an entirely silent window.
        let threshold = if speech - noise >= 12.0 { (noise + 6.0).min(speech - 12.0) } else { fallback };
        let time = (range.start + ms(i as i64 * 10)).min(range.end());
        if *level <= threshold {
            start.get_or_insert(time);
        } else if let Some(begin) = start.take()
            && time > begin
        {
            intervals.push(TimeRange::from_bounds(begin, time));
        }
    }
    if let Some(begin) = start
        && begin < range.end()
    {
        intervals.push(TimeRange::from_bounds(begin, range.end()));
    }
    intervals
}

pub(super) fn quiet_ranges(analysis: &PreEditAnalysis) -> Vec<TimeRange> {
    let mut result: Vec<TimeRange> = Vec::new();
    for range in analysis.chunks.iter().flat_map(|chunk| chunk.quiet.iter().copied()) {
        if let Some(last) = result.last_mut()
            && last.end() >= range.start
        {
            *last = TimeRange::from_bounds(last.start, last.end().max(range.end()));
        } else {
            result.push(range);
        }
    }
    result
}

fn opening_matches(a: &[&Word], b: &[&Word]) -> bool {
    if a.len() < 2 || b.len() < 2 || a[0].speaker != b[0].speaker {
        return false;
    }
    let old: BTreeSet<_> = a.iter().take(5).map(|w| w.normalized()).collect();
    let fresh: Vec<_> = b.iter().take(5).map(|w| w.normalized()).collect();
    let denominator = a.len().min(b.len()).min(5);
    let mut longest = 0;
    let mut run = 0;
    for word in &fresh {
        run = if old.contains(word) { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    let common: BTreeSet<_> = fresh.iter().filter(|w| old.contains(*w)).collect();
    common.len() * 5 >= denominator * 3 && longest * 2 >= denominator
}

fn word_range(words: &[&Word]) -> TimeRange {
    TimeRange::from_bounds(words[0].start, words.last().unwrap().end)
}

/// Opening similarity is insufficient evidence when facts or negation change.
pub(super) fn semantic_markers(words: &[String]) -> Vec<String> {
    words
        .iter()
        .filter(|word| {
            word.chars().any(|c| c.is_numeric())
                || matches!(word.as_str(), "no" | "not" | "never" | "neither" | "nor" | "without" | "cannot")
                || word.ends_with("n't")
                || word.ends_with("n’t")
        })
        .cloned()
        .collect()
}

pub(super) fn safe_take_removal(earlier: &[String], later: &[String], natural: bool) -> bool {
    if semantic_markers(earlier) != semantic_markers(later) {
        return false;
    }
    !natural || earlier == later || (earlier.len() >= 2 && later.starts_with(earlier))
}

pub(super) fn propose(id: u64, analysis: &PreEditAnalysis, cleanup: Option<CleanupSettings>) -> PreEditMap {
    let settings = cleanup.unwrap_or_default();
    if settings.mode == CleanupMode::GlingCompatible && analysis.chunks.iter().filter_map(|c| c.transcript.as_ref()).any(|t| !t.words.is_empty()) {
        return super::gling::propose(id, analysis, settings);
    }
    let threshold = settings.min_pause_ms.map_or(analysis.settings.min_pause, |v| ms(i64::from(v)));
    let handle = settings.handle_ms.map_or(analysis.settings.handle, |v| ms(i64::from(v)));
    let words: Vec<_> = analysis.chunks.iter().filter_map(|c| c.transcript.as_ref()).flat_map(|t| &t.words).filter(|w| w.end > w.start).collect();
    let quiet = quiet_ranges(analysis);
    let mut candidates = Vec::new();
    for edge in analysis.chunks.iter().flat_map(|chunk| &chunk.edge_quiet) {
        let leading = edge.start == analysis.source.range.start;
        let start = if leading { edge.start } else { edge.start + handle.max(ms(80)) };
        let end = if leading { edge.end() - handle.max(ms(80)) } else { edge.end() };
        if end <= start {
            continue;
        }
        let range = TimeRange::from_bounds(start, end);
        // Ordinary timed words are protected. Multi-second alignment spans can contain silence.
        let safe = words.iter().filter(|word| word.start < end && word.end > start).all(|word| word.range().duration > ms(2000));
        if safe {
            candidates.push(Candidate {
                range,
                kind: CandidateKind::Pause,
                reason: "Near-silent source edge; an overlong transcript timestamp does not protect it".into(),
                confidence: None,
            });
        }
    }
    let mut takes = Vec::new();
    let mut add_pause = |start: Tick, end: Tick, previous: Option<&Word>, next: Option<&Word>| {
        let boundary = previous.is_none() || next.is_none();
        if end <= start || (!boundary && end - start <= threshold) {
            return;
        }
        let (left, right) = if settings.mode == CleanupMode::Transcript && !words.is_empty() {
            (Tick::ZERO, Tick::ZERO)
        } else {
            let sentence = previous.is_some_and(|w| w.text.trim_end_matches(['"', '\'', ')', '”']).ends_with(['.', '!', '?']));
            let keep = if sentence { 450 } else { 250 };
            (if previous.is_some() { handle.max(ms(keep * 3 / 5)) } else { Tick::ZERO }, if next.is_some() { handle.max(ms(keep * 2 / 5)) } else { Tick::ZERO })
        };
        let (a, b) = (start + left, end - right);
        if b <= a {
            return;
        }
        let mut push = |lo, hi| {
            candidates.push(Candidate {
                range: TimeRange::from_bounds(lo, hi),
                kind: CandidateKind::Pause,
                reason: if settings.mode == CleanupMode::Transcript && !words.is_empty() {
                    "Gap between timed words (transcript rules)"
                } else {
                    "Quiet audio with speech guards and retained pacing"
                }
                .into(),
                confidence: None,
            })
        };
        if settings.mode == CleanupMode::Transcript && !words.is_empty() {
            push(a, b);
        } else {
            let first = quiet.partition_point(|q| q.end() <= a);
            for q in quiet[first..].iter().take_while(|q| q.start < b) {
                let (lo, hi) = (a.max(q.start), b.min(q.end()));
                // Apply the minimum pause to the speech gap, not every quiet island.
                if hi - lo >= ms(20) {
                    push(lo, hi);
                }
            }
        }
    };
    if words.is_empty() {
        // No timed speech: use audio only, never invent a transcript gap.
        for range in &quiet {
            if range.duration > threshold {
                add_pause(range.start, range.end(), None, None);
            }
        }
    } else {
        // Only completed coverage is eligible, including partial/canceled analysis.
        let end = analysis.chunks.last().map_or(analysis.source.range.start, |c| c.coverage.end());
        let mut cursor = analysis.source.range.start;
        let mut previous = None;
        for word in &words {
            add_pause(cursor, word.start, previous, Some(word));
            cursor = cursor.max(word.end);
            previous = Some(*word);
        }
        add_pause(cursor, end, previous, None);
    }
    if analysis.settings.language == "en" {
        for word in &words {
            if is_filler(&word.normalized()) {
                candidates.push(Candidate {
                    range: word.range(),
                    kind: CandidateKind::Filler,
                    reason: "Timed word matches the English hesitation vocabulary".into(),
                    confidence: None,
                });
            }
        }
        // Takes follow pauses/speaker changes rather than punctuation attached by ASR.
        let mut phrases = Vec::new();
        let mut begin = 0;
        for i in 1..=words.len() {
            if i == words.len() || words[i].start - words[i - 1].end > threshold || words[i].speaker != words[i - 1].speaker {
                phrases.push(&words[begin..i]);
                begin = i;
            }
        }
        let mut retired = BTreeSet::new();
        for current in 0..phrases.len() {
            let fresh = phrases[current];
            for previous in (current.saturating_sub(5)..current).rev() {
                let old = phrases[previous];
                if retired.contains(&previous) || fresh[0].start - old.last().unwrap().end > ms(20_000) || !opening_matches(old, fresh) {
                    continue;
                }
                let a = candidates.len();
                for phrase in [old, fresh] {
                    candidates.push(Candidate {
                        range: word_range(phrase),
                        kind: CandidateKind::Take,
                        reason: "Repeated take opening within five nearby attempts; later attempt retained".into(),
                        confidence: None,
                    });
                }
                takes.push(TakeGroup { alternatives: vec![a, a + 1], explanation: "Compare earlier and later attempts in original context".into() });
                retired.insert(previous);
            }
            // Split on repeated tokens, then compare contiguous blocks in adjacent patterns.
            let mut patterns = Vec::new();
            let mut start = 0;
            let mut seen = BTreeSet::new();
            for (i, word) in fresh.iter().enumerate() {
                let key = word.normalized();
                if seen.contains(&key) {
                    patterns.push(&fresh[start..i]);
                    start = i;
                    seen.clear();
                }
                seen.insert(key);
                if word.text.ends_with('?') {
                    patterns.push(&fresh[start..=i]);
                    start = i + 1;
                    seen.clear();
                }
            }
            if start < fresh.len() {
                patterns.push(&fresh[start..]);
            }
            for pair in patterns.windows(2) {
                let (old, new) = (pair[0], pair[1]);
                let positions: std::collections::BTreeMap<_, _> = new.iter().enumerate().map(|(i, w)| (w.normalized(), i)).collect();
                let mut run = 0;
                let mut last = None;
                let mut matched = false;
                for (i, word) in old.iter().enumerate() {
                    let position = positions.get(&word.normalized()).copied();
                    run = match position {
                        Some(p) if last == p.checked_sub(1) && run > 0 => run + 1,
                        Some(_) => 1,
                        None => 0,
                    };
                    last = position;
                    matched |= run >= 4 || (run >= 2 && run * 5 >= old.len() * 2 && i + 1 == old.len() && position == Some(run - 1));
                }
                if matched {
                    candidates.push(Candidate {
                        range: word_range(old),
                        kind: CandidateKind::Restart,
                        reason: "Earlier phrase repeats a contiguous word pattern in the next attempt".into(),
                        confidence: None,
                    });
                }
            }
            if current > 0 {
                let old = phrases[current - 1];
                if fresh[0].speaker == old[0].speaker && fresh[0].start - old.last().unwrap().end <= ms(2000) {
                    if let Some(start) = old.iter().rposition(|w| w.normalized() == fresh[0].normalized()) {
                        let suffix = &old[start..];
                        let matched = suffix.iter().zip(fresh).take_while(|(a, b)| a.normalized() == b.normalized()).count();
                        // Require two words, avoiding the reference's single-word false positives.
                        if matched >= 2 && matched * 2 >= suffix.len() {
                            candidates.push(Candidate {
                                range: word_range(suffix),
                                kind: CandidateKind::Restart,
                                reason: "The preceding phrase ends with the next take's opening words".into(),
                                confidence: None,
                            });
                        }
                    }
                }
            }
        }
    }
    PreEditMap {
        analysis: id,
        previous: None,
        reviewed: false,
        candidates: Arc::new(candidates),
        takes: Arc::new(takes),
        automatic: None,
        cleanup,
        overrides: Vec::new(),
        corrections: Default::default(),
        conflicts: Vec::new(),
    }
}

/// Expand word removals into quiet space, protecting neighboring words. Failure
/// retains the material for review instead of cutting at an unverified boundary.
pub(super) fn natural_word_range(range: TimeRange, words: &[&Word], quiet: &[TimeRange], available: TimeRange) -> Option<TimeRange> {
    let first = words.partition_point(|w| w.end <= range.start);
    let last = words.partition_point(|w| w.start < range.end());
    if first >= last || words[first].start < range.start || words[last - 1].end > range.end() {
        return None;
    }
    let before = first.checked_sub(1).map(|i| words[i]);
    let after = words.get(last).copied();
    let lo = before.map_or(available.start, |w| w.end + ms(120));
    let hi = after.map_or(available.end(), |w| w.start - ms(80));
    if lo > range.start || hi < range.end() {
        return None;
    }
    let start = if before.is_none() {
        available.start
    } else {
        let index = quiet.partition_point(|q| q.end() <= lo);
        quiet[index..].iter().take_while(|q| q.start <= range.start).find(|q| q.end().min(range.start) - q.start.max(lo) >= ms(20))?.start.max(lo)
    };
    let end = if after.is_none() {
        available.end()
    } else {
        let index = quiet.partition_point(|q| q.start < hi);
        quiet[..index].iter().rev().take_while(|q| q.end() >= range.end()).find(|q| q.end().min(hi) - q.start.max(range.end()) >= ms(20))?.end().min(hi)
    };
    Some(TimeRange::from_bounds(start, end))
}

/// Transcript rules own the inter-word gaps inside the same take, matching the
/// reference's word-group cuts even when silence cleanup is disabled.
pub(super) fn transcript_word_range(range: TimeRange, words: &[&Word], pause: Tick) -> TimeRange {
    let first = words.partition_point(|w| w.end <= range.start);
    let last = words.partition_point(|w| w.start < range.end());
    let start = first
        .checked_sub(1)
        .and_then(|i| words.get(i))
        .filter(|w| range.start - w.end <= pause && words.get(first).is_some_and(|own| own.speaker == w.speaker))
        .map_or(range.start, |w| w.end);
    let end = words.get(last).filter(|w| w.start - range.end() <= pause && last > 0 && words[last - 1].speaker == w.speaker).map_or(range.end(), |w| w.start);
    TimeRange::from_bounds(start, end)
}
