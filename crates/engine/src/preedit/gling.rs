//! Gling-compatible transcript rules, qualified differentially against OpenGling 0.12.0.
//! All ranges remain source ticks. Acoustic refinement is the separate conservative mode.
use shotclutch_project::{Word, preedit::*};
use shotclutch_time::{Tick, TimeRange};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;

fn ms(value: u32) -> Tick {
    Tick::from_units(i64::from(value), 1000)
}

fn sentence_end(word: &Word) -> bool {
    word.text.trim_end_matches(['"', '\'', ')', '”']).ends_with(['.', '!', '?'])
}

/// Independent of take grouping: preserve conversational gaps and avoid tiny edits.
fn paced_pause(
    start: Tick,
    end: Tick,
    previous: Option<&Word>,
    next: Option<&Word>,
    after_discarded_sentence: bool,
    settings: CleanupSettings,
    handle: Tick,
) -> Option<TimeRange> {
    let floor = ms(settings.preserve_pauses_ms?);
    if previous.is_some() && next.is_some() && end - start <= floor {
        return None;
    }
    let sentence = previous.is_some_and(sentence_end);
    // This pacing control is the total context at a join, rather than per-edge padding.
    let half = Tick(handle.0 / 2);
    let left = if previous.is_some() { half.max(ms(if sentence { 150 } else { 0 })) } else { Tick::ZERO };
    let right = if next.is_some() { half.max(ms(if sentence || after_discarded_sentence { 100 } else { 0 })) } else { Tick::ZERO };
    let (start, end) = (start + left, end - right);
    // Dead space between discarded words joins their removals; it is not a standalone edit.
    (end > start && ((previous.is_none() && next.is_none()) || end - start >= ms(120))).then(|| TimeRange::from_bounds(start, end))
}

#[derive(Clone, Copy)]
struct Mark {
    bad_take: bool,
    rule: &'static str,
    keeper: Option<TimeRange>,
}

fn normalized(text: &str) -> String {
    let text = text.trim();
    text.strip_suffix(|c: char| ".,!?;:".contains(c)).unwrap_or(text).to_lowercase()
}

fn pattern_key(text: &str) -> String {
    normalized(text).chars().filter(|c| !".,/#!$%^&*;:{}=-_`~()".contains(*c)).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

fn bounds(words: &[&Word]) -> TimeRange {
    TimeRange::from_bounds(words[0].start, words.iter().map(|w| w.end).max().unwrap_or(words[0].end))
}

fn opening(a: &[String], b: &[String]) -> bool {
    let (a, b) = (&a[..a.len().min(5)], &b[..b.len().min(5)]);
    if a.len() < 2 || b.len() < 2 {
        return false;
    }
    let keys: BTreeSet<_> = a.iter().collect();
    let (mut run, mut longest) = (0, 0);
    for word in b {
        run = if keys.contains(word) { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    let common: BTreeSet<_> = b.iter().filter(|word| keys.contains(*word)).collect();
    let n = a.len().min(b.len());
    common.len() * 5 >= n * 3 && longest * 2 >= n
}

fn mark(marks: &mut [Option<Mark>], range: Range<usize>, value: Mark) {
    for slot in &mut marks[range] {
        if slot.is_none() || value.bad_take {
            *slot = Some(value);
        }
    }
}

fn split_takes(words: &[&Word], take_threshold: Tick) -> Vec<Range<usize>> {
    let mut takes = Vec::new();
    let mut start = 0;
    let mut end = words.first().map_or(Tick::ZERO, |w| w.end);
    for i in 1..words.len() {
        if words[i].start - end > take_threshold {
            takes.push(start..i);
            start = i;
            end = words[i].end;
        } else {
            end = end.max(words[i].end);
        }
    }
    if !words.is_empty() {
        takes.push(start..words.len());
    }
    takes
}

fn mark_legacy(words: &[&Word], keys: &[String], takes: &[Range<usize>], marks: &mut [Option<Mark>]) {
    let mut retired: Vec<_> = takes.iter().map(|r| marks[r.clone()].iter().all(Option::is_some)).collect();
    for (index, current) in takes.iter().enumerate() {
        let keeper = Some(bounds(&words[current.clone()]));
        for previous in (index.saturating_sub(5)..index).rev() {
            if retired[previous] || !opening(&keys[takes[previous].clone()], &keys[current.clone()]) {
                continue;
            }
            for abandoned in previous..index {
                mark(marks, takes[abandoned].clone(), Mark { bad_take: true, rule: "repeated-take-opening", keeper });
                retired[abandoned] = true;
            }
        }
        let mut patterns = Vec::new();
        let mut first = current.start;
        let mut seen = BTreeSet::new();
        for i in current.clone() {
            let key = pattern_key(&words[i].text);
            if seen.contains(&key) {
                patterns.push(first..i);
                first = i;
                seen.clear();
            }
            seen.insert(key);
            if words[i].text.ends_with('?') && i + 1 < current.end {
                patterns.push(first..i + 1);
                first = i + 1;
                seen.clear();
            }
        }
        if first < current.end {
            patterns.push(first..current.end);
        }
        for pair in patterns.windows(2) {
            let (old, fresh) = (&pair[0], &pair[1]);
            let positions: BTreeMap<_, _> = fresh.clone().enumerate().map(|(i, w)| (pattern_key(&words[w].text), i)).collect();
            let (mut run, mut last) = (0usize, None);
            let (mut best, mut old_at, mut new_at) = (0, 0, 0);
            for (i, w) in old.clone().enumerate() {
                let Some(&p) = positions.get(&pattern_key(&words[w].text)) else {
                    run = 0;
                    last = None;
                    continue;
                };
                run = if last.is_some_and(|last| last + 1 == p) { run + 1 } else { 1 };
                last = Some(p);
                let candidate = (i + 1 - run, p + 1 - run);
                if run > best || (run == best && candidate < (old_at, new_at)) {
                    best = run;
                    old_at = candidate.0;
                    new_at = candidate.1;
                }
            }
            if best >= 4
                || (best >= 2
                    && best * 5 >= old.len() * 2
                    && old_at + best == old.len()
                    && pattern_key(&words[old.start + old_at].text) == pattern_key(&words[fresh.start].text))
            {
                mark(marks, old.clone(), Mark { bad_take: true, rule: "repeated-within-take-pattern", keeper: Some(bounds(&words[fresh.clone()])) });
            }
        }
        retired[index] = marks[current.clone()].iter().all(Option::is_some);
        if index > 0 {
            let previous = &takes[index - 1];
            if let Some(relative) = keys[previous.clone()].iter().rposition(|key| *key == keys[current.start]) {
                let suffix = previous.start + relative..previous.end;
                let membership: BTreeSet<_> = keys[suffix.clone()].iter().collect();
                let matched = keys[current.clone()].iter().take(suffix.len()).filter(|word| membership.contains(*word)).count();
                if matched * 2 >= suffix.len() {
                    mark(marks, suffix, Mark { bad_take: true, rule: "repeated-cross-take-suffix", keeper });
                }
            }
            retired[index - 1] = marks[previous.clone()].iter().all(Option::is_some);
        }
    }
}

pub(super) fn propose(id: u64, analysis: &PreEditAnalysis, settings: CleanupSettings) -> PreEditMap {
    let mut words: Vec<_> = analysis.chunks.iter().filter_map(|c| c.transcript.as_ref()).flat_map(|t| &t.words).filter(|w| w.end > w.start).collect();
    words.sort_by_key(|w| w.start);
    let mut ends: Vec<_> = words.iter().map(|w| w.end).collect();
    ends.sort_unstable();
    let keys: Vec<_> = words.iter().map(|w| normalized(&w.text)).collect();
    let threshold = settings.min_pause_ms.map_or(analysis.settings.min_pause, |v| Tick::from_units(i64::from(v), 1000));
    let handle = settings.handle_ms.map_or(analysis.settings.handle, ms);
    let ordered = settings.take_matching != TakeMatching::Legacy;
    let takes = split_takes(&words, if ordered { ms(700) } else { threshold });
    let mut marks = vec![None; words.len()];
    if settings.cut_fillers && analysis.settings.language == "en" {
        let fillers =
            ["er", "erm", "ermm", "err", "errr", "hm", "hmm", "hmmm", "um", "umm", "ummm", "ah", "uh", "uhh", "uhhh", "mhm", "mmhm", "mm-hm", "mm-hmm"];
        for (i, key) in keys.iter().enumerate() {
            if fillers.contains(&key.as_str()) {
                marks[i] = Some(Mark { bad_take: false, rule: "filler-lexicon", keeper: None });
            }
        }
    }
    if settings.cut_bad_takes && ordered {
        if analysis.settings.language == "en" {
            let removals = match settings.take_matching {
                TakeMatching::OrderedV4 | TakeMatching::OrderedV5 | TakeMatching::OrderedV6 => {
                    let mut hints = marks.clone();
                    mark_legacy(&words, &keys, &split_takes(&words, threshold), &mut hints);
                    let suggested: Vec<_> = hints.iter().map(|mark| mark.is_some_and(|m| m.bad_take)).collect();
                    let quiet = super::cleanup::quiet_ranges(analysis);
                    let cuts = super::ordered_takes::broader::detect_screened(&words, &suggested, &quiet);
                    if settings.take_matching == TakeMatching::OrderedV4 {
                        cuts
                    } else {
                        let cuts = super::ordered_takes::broader::resolve_restart_chains(&words, cuts);
                        if settings.take_matching == TakeMatching::OrderedV6 {
                            super::ordered_takes::broader::indexed::extend(&words, &quiet, cuts)
                        } else {
                            cuts
                        }
                    }
                }
                TakeMatching::OrderedV3 => super::ordered_takes::broader::detect_restarts(&words),
                TakeMatching::OrderedV2 => super::ordered_takes::broader::detect(&words),
                _ => super::ordered_takes::detect(&words),
            };
            for removal in removals {
                mark(&mut marks, removal.discard, Mark { bad_take: true, rule: removal.rule, keeper: Some(bounds(&words[removal.keep])) });
            }
        }
    } else if settings.cut_bad_takes {
        mark_legacy(&words, &keys, &takes, &mut marks);
    }
    let mut candidates = Vec::new();
    let mut groups = Vec::new();
    let mut removed_words = vec![false; words.len()];
    let mut word_spans = Vec::new();
    for take in &takes {
        let mut i = take.start;
        while i < take.end {
            let Some(state) = marks[i] else {
                i += 1;
                continue;
            };
            let first = i;
            while i + 1 < take.end
                && marks[i + 1].is_some_and(|m| {
                    m.bad_take == state.bad_take && (settings.take_matching == TakeMatching::Legacy || (m.keeper == state.keeper && m.rule == state.rule))
                })
            {
                i += 1;
            }
            let start = if first > take.start { words[first - 1].end } else { words[take.start].start };
            let end = if i + 1 < take.end { words[i + 1].start } else { words[take.end - 1].end };
            let own = first..i + 1;
            let intersects = words.partition_point(|w| w.start < end).saturating_sub(ends.partition_point(|e| *e <= start));
            let safe = end > start && words[own.clone()].iter().all(|w| w.start >= start && w.end <= end) && intersects == own.len();
            if safe {
                removed_words[own.clone()].fill(true);
                let candidate = candidates.len();
                word_spans.push((candidate, own));
                candidates.push(Candidate {
                    range: TimeRange::from_bounds(start, end),
                    kind: if state.bad_take { CandidateKind::Take } else { CandidateKind::Filler },
                    reason: match settings.take_matching {
                        TakeMatching::OrderedV6 => format!("Context-aware v6: {}", state.rule),
                        TakeMatching::OrderedV5 => format!("Context-aware v5: {}", state.rule),
                        TakeMatching::OrderedV4 => format!("Context-aware v4: {}", state.rule),
                        TakeMatching::OrderedV3 => format!("Context-aware v3: {}", state.rule),
                        TakeMatching::OrderedV2 => format!("Context-aware v2: {}", state.rule),
                        TakeMatching::OrderedV1 => format!("Context-aware v1: {}", state.rule),
                        TakeMatching::Legacy => format!("Gling-compatible: {}", state.rule),
                    },
                    confidence: None,
                });
                if let Some(keep) = state.keeper {
                    let alternative = candidates.len();
                    candidates.push(Candidate {
                        range: keep,
                        kind: CandidateKind::Take,
                        reason: if ordered { "Retained alternative (context-aware)" } else { "Retained alternative (Gling-compatible)" }.into(),
                        confidence: None,
                    });
                    groups.push(TakeGroup { alternatives: vec![candidate, alternative], explanation: state.rule.into() });
                }
            }
            i += 1;
        }
    }
    if settings.preserve_pauses_ms.is_some() {
        // Classify first, then keep context only beside words that actually survive. Padding
        // both sides of two adjacent removals would resurrect a tiny silent video fragment.
        let half = Tick(handle.0 / 2);
        for (index, own) in word_spans {
            let range = candidates[index].range;
            let start = if own.start > 0 && !removed_words[own.start - 1] { (range.start + half).min(words[own.start].start) } else { range.start };
            let end = if own.end < words.len() && !removed_words[own.end] { (range.end() - half).max(words[own.end - 1].end) } else { range.end() };
            candidates[index].range = TimeRange::from_bounds(start, end);
        }
    }
    if settings.cut_pauses {
        let covered = analysis.chunks.last().map_or(analysis.source.range.start, |c| c.coverage.end());
        let mut cursor = analysis.source.range.start;
        let mut previous: Option<usize> = None;
        for (i, word) in words.iter().enumerate() {
            let kept_previous = previous.filter(|&index| !removed_words[index]).map(|index| words[index]);
            let kept_next = (!removed_words[i]).then_some(*word);
            let pause_threshold = if kept_previous.is_some() && kept_next.is_some() { settings.preserve_pauses_ms.map_or(threshold, ms) } else { threshold };
            if word.start > cursor && (i == 0 || word.start - cursor > pause_threshold) {
                let range = if settings.preserve_pauses_ms.is_some() {
                    // Removing a retry cue must not shave the original lead-in to the next word.
                    let after_discarded_sentence = previous.is_some_and(|index| {
                        removed_words[index]
                            && sentence_end(words[index])
                            && marks[index].is_some_and(|mark| matches!(mark.rule, "isolated-retry-cue" | "short-restart-continuation"))
                    });
                    paced_pause(cursor, word.start, kept_previous, kept_next, after_discarded_sentence, settings, handle)
                } else {
                    Some(TimeRange::from_bounds(cursor, word.start))
                };
                if let Some(range) = range {
                    candidates.push(Candidate {
                        range,
                        kind: CandidateKind::Pause,
                        reason: if settings.preserve_pauses_ms.is_some() {
                            "Natural pacing: longer gap with speech padding"
                        } else {
                            "Gling-compatible: timed speech gap"
                        }
                        .into(),
                        confidence: None,
                    });
                }
            }
            if word.end >= cursor {
                cursor = word.end;
                previous = Some(i);
            }
        }
        if covered > cursor {
            let range = if settings.preserve_pauses_ms.is_some() {
                paced_pause(cursor, covered, previous.filter(|&index| !removed_words[index]).map(|index| words[index]), None, false, settings, handle)
            } else {
                Some(TimeRange::from_bounds(cursor, covered))
            };
            if let Some(range) = range {
                candidates.push(Candidate { range, kind: CandidateKind::Pause, reason: "Trailing speech gap".into(), confidence: None });
            }
        }
    }
    PreEditMap {
        analysis: id,
        previous: None,
        reviewed: false,
        candidates: Arc::new(candidates),
        takes: Arc::new(groups),
        automatic: None,
        cleanup: Some(settings),
        overrides: Vec::new(),
        corrections: Default::default(),
        conflicts: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m2_restart_pause_keeps_the_next_word_lead_in() {
        let cue = Word::new("time.", ms(250), ms(500));
        let next = Word::new("And", ms(2300), ms(2400));
        let settings = CleanupSettings { preserve_pauses_ms: Some(800), ..Default::default() };
        let before = paced_pause(cue.end, next.start, Some(&cue), Some(&next), false, settings, ms(150)).unwrap();
        let after = paced_pause(cue.end, next.start, None, Some(&next), true, settings, ms(150)).unwrap();
        assert_eq!(after.start, cue.end);
        assert_eq!(after.end(), before.end());
    }
}
