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
fn quiet_enough(start: Tick, end: Tick, quiet: &[TimeRange]) -> bool {
    let mut total = 0;
    let mut sustained = false;
    for r in quiet[quiet.partition_point(|r| r.end() <= start)..].iter().take_while(|r| r.start < end) {
        let overlap = r.end().min(end) - r.start.max(start);
        total += overlap.0;
        sustained |= overlap >= ms(150);
    }
    sustained && total >= (end - start).0.div_euclid(2)
}

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

fn filler_words(language: &str) -> &'static [&'static str] {
    match language {
        "en" => &["er", "erm", "ermm", "err", "errr", "hm", "hmm", "hmmm", "um", "umm", "ummm", "ah", "uh", "uhh", "uhhh", "mhm", "mmhm", "mm-hm", "mm-hmm"],
        "pt" => &["ãh", "ah", "eh", "éh", "hã", "hm", "hmm"],
        "es" => &["eh", "ehh", "mm", "mmm", "um", "umm", "ah", "ahh"],
        "fr" => &["euh", "euuh", "heu", "heuu", "ah", "oh", "mm", "mmm"],
        "de" => &["äh", "ähm", "ähmm", "eh", "ehm", "hm", "hmm", "mm", "mmm", "oh"],
        "uk" => &["е", "ее", "ем", "емм", "м", "мм", "ммм"],
        "ru" => &["э", "эм", "эмм", "а", "ах", "м", "мм", "ммм"],
        "he" => &["אה", "אהה", "אהם", "אממ", "הממ"],
        "iw" => &["אה", "אהה", "אהם", "אממ", "הממ"],
        "it" => &["eh", "ehh", "mm", "mmm", "ah", "ahh", "oh", "ohh"],
        "nl" => &["eh", "ehh", "mm", "mmm", "ah", "ahh", "oh", "ohh"],
        "da" => &["øh", "øhh", "ah", "ahh", "mm", "mmm", "hm", "hmm"],
        "no" => &["øh", "øhh", "eh", "ehh", "mm", "mmm", "hm", "hmm"],
        "nb" => &["øh", "øhh", "eh", "ehh", "mm", "mmm", "hm", "hmm"],
        "sv" => &["öh", "öhh", "eh", "ehh", "mm", "mmm", "hm", "hmm"],
        "fi" => &["öö", "ööh", "mm", "mmm", "hmm", "hmmm", "no", "noo"],
        "cs" => &["eh", "ehh", "mm", "mmm", "hm", "hmm", "no", "noo"],
        "pl" => &["eee", "eem", "mm", "mmm", "hm", "hmm", "no", "noo"],
        "sk" => &["eh", "ehh", "mm", "mmm", "hm", "hmm"],
        "hu" => &["ö", "öö", "ööö", "öööö", "mm", "mmm", "hm", "hmm"],
        "bg" => &["ъъъ", "ъъ", "ъм", "ъмм", "м", "мм", "ммм"],
        "ro" => &["ăă", "ăăă", "îî", "păi", "mm", "mmm", "hm", "hmm"],
        "tr" => &["ııı", "ıı", "ıh", "ıhh", "mm", "mmm", "hmm", "hmmm"],
        _ => &[],
    }
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

pub(super) fn covered_reading(earlier: &[&Word], later: &[&Word]) -> bool {
    let compact = |words: &[&Word]| {
        let mut out: Vec<String> = vec![];
        for w in words {
            let key = pattern_key(&w.text);
            if !key.is_empty() && out.last() != Some(&key) {
                out.push(key);
            }
        }
        out
    };
    let a = compact(earlier);
    let b = compact(later);
    // Matching the whole earlier phrase prevents a repeated opening from
    // erasing the different facts or conclusion which follow it.
    a.len() >= 3 && a.len() <= b.len() && b.windows(a.len()).any(|w| w == a)
}
fn screen_coverage(words: &[&Word], marks: &mut [Option<Mark>]) {
    let old = marks.to_vec();
    let mut i = 0;
    while i < old.len() {
        let first = i;
        let Some(m) = old[i] else {
            i += 1;
            continue;
        };
        i += 1;
        while i < old.len() && old[i].is_some_and(|v| v.bad_take == m.bad_take && v.keeper == m.keeper && v.rule == m.rule) {
            i += 1
        }
        if !m.bad_take {
            continue;
        }
        let valid = m.keeper.is_some_and(|r| {
            let start = words.partition_point(|w| w.start < r.start);
            let end = words.partition_point(|w| w.start < r.end());
            !sentence_end(words[i - 1]) && start >= i && end > start && covered_reading(&words[first..i], &words[start..end])
        });
        if !valid {
            marks[first..i].fill(None)
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
    let reference_first = matches!(settings.take_matching, TakeMatching::OrderedV7 | TakeMatching::OrderedV8);
    let language = analysis.settings.language.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
    let takes = split_takes(&words, if ordered && !reference_first { ms(700) } else { threshold });
    let mut marks = vec![None; words.len()];
    if settings.cut_fillers {
        let fillers = filler_words(&language);
        for (i, key) in keys.iter().enumerate() {
            if fillers.contains(&key.as_str()) {
                marks[i] = Some(Mark { bad_take: false, rule: "filler-lexicon", keeper: None });
            }
        }
    }
    // Commit the reference classifications before considering supplemental English matches.
    if settings.cut_bad_takes && reference_first {
        mark_legacy(&words, &keys, &takes, &mut marks);
        if settings.take_matching == TakeMatching::OrderedV8 {
            screen_coverage(&words, &mut marks);
        }
    }
    if settings.cut_bad_takes && ordered {
        if language == "en" {
            let removals = match settings.take_matching {
                TakeMatching::OrderedV4 | TakeMatching::OrderedV5 | TakeMatching::OrderedV6 | TakeMatching::OrderedV7 | TakeMatching::OrderedV8 => {
                    let mut hints = marks.clone();
                    if !reference_first || settings.take_matching == TakeMatching::OrderedV8 {
                        mark_legacy(&words, &keys, &split_takes(&words, threshold), &mut hints);
                    }
                    let suggested: Vec<_> = hints.iter().map(|mark| mark.is_some_and(|m| m.bad_take)).collect();
                    let quiet = super::cleanup::quiet_ranges(analysis);
                    let cuts = super::ordered_takes::broader::detect_screened(&words, &suggested, &quiet);
                    if settings.take_matching == TakeMatching::OrderedV4 {
                        cuts
                    } else {
                        let cuts = super::ordered_takes::broader::resolve_restart_chains(&words, cuts);
                        if matches!(settings.take_matching, TakeMatching::OrderedV6 | TakeMatching::OrderedV7 | TakeMatching::OrderedV8) {
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
            let mut keeper_edges = vec![0i32; words.len() + 1];
            if reference_first {
                for keeper in marks.iter().flatten().filter_map(|m| m.keeper) {
                    let first = words.partition_point(|w| w.start < keeper.start);
                    let end = words.partition_point(|w| w.start < keeper.end());
                    keeper_edges[first] += 1;
                    keeper_edges[end] -= 1;
                }
            }
            let mut depth = 0;
            let protected: Vec<_> = keeper_edges
                .iter()
                .map(|d| {
                    depth += d;
                    depth > 0
                })
                .collect();
            for removal in removals {
                if reference_first
                    && (removal.discard.clone().any(|i| protected[i] || marks[i].is_some_and(|m| m.bad_take))
                        || removal.keep.clone().any(|i| marks[i].is_some_and(|m| m.bad_take)))
                {
                    continue;
                }

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
            let start = if settings.take_matching == TakeMatching::OrderedV8 {
                words[first].start
            } else if first > take.start {
                words[first - 1].end
            } else {
                words[take.start].start
            };
            let end = if settings.take_matching == TakeMatching::OrderedV8 {
                words[i].end
            } else if i + 1 < take.end {
                words[i + 1].start
            } else {
                words[take.end - 1].end
            };
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
                        TakeMatching::OrderedV8 => format!("Context-aware v8: {}", state.rule),
                        TakeMatching::OrderedV7 => format!("Context-aware v7: {}", state.rule),
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
        let measured_quiet =
            (reference_first && settings.preserve_pauses_ms.is_some() && analysis.analyzer_version >= 3 && analysis.chunks.iter().all(|c| !c.peaks.is_empty()))
                .then(|| super::cleanup::quiet_ranges(analysis));
        let take_cut: Vec<_> = takes.iter().map(|t| removed_words[t.clone()].iter().all(|r| *r)).collect();
        let mut current_take = 0;
        let covered = analysis.chunks.last().map_or(analysis.source.range.start, |c| c.coverage.end());
        let mut cursor = analysis.source.range.start;
        let mut previous: Option<usize> = None;
        for (i, word) in words.iter().enumerate() {
            while current_take + 1 < takes.len() && i >= takes[current_take].end {
                current_take += 1;
            }
            let audible_gap = current_take > 0
                && takes[current_take].start == i
                && word.start - cursor < ms(2000)
                && !(take_cut[current_take - 1] && take_cut[current_take])
                && measured_quiet.as_ref().is_some_and(|q| !quiet_enough(cursor, word.start, q));
            let kept_previous = previous.filter(|&index| !removed_words[index]).map(|index| words[index]);
            let kept_next = (!removed_words[i]).then_some(*word);
            let pause_threshold = if kept_previous.is_some() && kept_next.is_some() { settings.preserve_pauses_ms.map_or(threshold, ms) } else { threshold };
            if !audible_gap && word.start > cursor && (i == 0 || word.start - cursor > pause_threshold) {
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
    if matches!(settings.take_matching, TakeMatching::OrderedV6 | TakeMatching::OrderedV7 | TakeMatching::OrderedV8)
        && settings.cut_bad_takes
        && let Some(recheck) = &analysis.retake_recheck
    {
        let keepers: BTreeSet<_> = groups.iter().filter_map(|group| group.alternatives.last().copied()).collect();
        for restart in &recheck.restarts {
            if candidates.iter().enumerate().any(|(i, candidate)| {
                if keepers.contains(&i) {
                    candidate.range.overlaps(&restart.discard)
                } else {
                    candidate.kind != CandidateKind::Pause && candidate.range.overlaps(&restart.keep)
                }
            }) {
                continue;
            }
            let discard = candidates.len();
            candidates.push(Candidate { range: restart.discard, kind: CandidateKind::Restart, reason: "Locally rechecked restart".into(), confidence: None });
            let keep = candidates.len();
            candidates.push(Candidate { range: restart.keep, kind: CandidateKind::Take, reason: "Completed reading".into(), confidence: None });
            groups.push(TakeGroup { alternatives: vec![discard, keep], explanation: "Local audio recheck".into() });
        }
    }
    PreEditMap {
        take_review_report: None,
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
    fn a_shared_opening_cannot_discard_unique_facts() {
        let make =
            |text: &str| text.split_whitespace().enumerate().map(|(i, s)| Word::new(s, ms(i as u32 * 250), ms(i as u32 * 250 + 200))).collect::<Vec<_>>();
        let yes = make("we chose the blue screen");
        let completion = make("we chose the blue screen for its contrast");
        assert!(covered_reading(&yes.iter().collect::<Vec<_>>(), &completion.iter().collect::<Vec<_>>()));
        for (old, new) in [
            ("we chose blue for contrast", "we chose blue for comfort"),
            ("skin looks less yellow", "skin looks yellow"),
            ("battery lasts 50 minutes", "battery lasts 40 minutes"),
            ("this does not work", "this does work"),
        ] {
            assert!(!covered_reading(&make(old).iter().collect::<Vec<_>>(), &make(new).iter().collect::<Vec<_>>()));
        }
    }
    #[test]
    fn short_gap_needs_half_quiet_and_one_sustained_run() {
        let ranges = |pairs: &[(u32, u32)]| pairs.iter().map(|&(s, e)| TimeRange::from_bounds(ms(s), ms(e))).collect::<Vec<_>>();
        assert!(quiet_enough(ms(1000), ms(2000), &ranges(&[(950, 1250), (1750, 2050)])));
        assert!(!quiet_enough(ms(1000), ms(2000), &ranges(&[(1000, 1490)])));
        assert!(!quiet_enough(ms(1000), ms(2000), &ranges(&[(1000, 1100), (1150, 1250), (1300, 1400), (1450, 1550), (1600, 1700)])));
    }

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
