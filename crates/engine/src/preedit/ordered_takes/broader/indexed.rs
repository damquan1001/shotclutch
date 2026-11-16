//! Retrieve repeated openings independently of the saved transcript's sentence
//! boundaries. Cut only a structurally unfinished attempt or an immediate
//! repeat that completes the earlier wording.
use super::{Removal, anchor, closed, edges, gap, marker_text, ms, protected};
use shotclutch_project::Word;
use shotclutch_time::{Tick, TimeRange};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

const OPENING: usize = 5;
const MAX_WORDS: usize = 32;
const MAX_POSTINGS: usize = 32;

pub(in crate::preedit) fn extend(words: &[&Word], quiet: &[TimeRange], mut cuts: Vec<Removal>) -> Vec<Removal> {
    let keys: Vec<_> = words.iter().map(|word| word.normalized().replace('’', "'")).collect();
    let boundaries = edges(words, &keys);
    let mut index = BTreeMap::<&[String], Vec<usize>>::new();
    for &start in &boundaries {
        if let Some(opening) = keys.get(start..start + OPENING) {
            let positions = index.entry(opening).or_default();
            if positions.len() <= MAX_POSTINGS {
                positions.push(start);
            }
        }
    }

    let mut reserved = vec![false; words.len()];
    let mut discarded = vec![false; words.len()];
    for cut in &cuts {
        reserved[cut.discard.clone()].fill(true);
        reserved[cut.keep.clone()].fill(true);
        discarded[cut.discard.clone()].fill(true);
    }
    // Prefer longer, fully covered attempts at the same starting boundary.
    for (i, &start) in boundaries.iter().enumerate() {
        let Some(positions) = keys.get(start..start + OPENING).and_then(|opening| index.get(opening)) else { continue };
        if positions.len() <= 1 || positions.len() > MAX_POSTINGS || gap(words, start) < ms(300) {
            continue;
        }
        for &end in boundaries[i + 1..].iter().take(3).rev() {
            let old = start..end;
            if end == words.len()
                || !(8..=MAX_WORDS).contains(&old.len())
                || words[end - 1].end - words[start].start > ms(24_000)
                || reserved[old.clone()].contains(&true)
                || !has_quiet_boundary(quiet, words[end - 1].end, words[end].start)
                || keys[old.clone()].iter().filter(|key| anchor(key)).collect::<BTreeSet<_>>().len() < 3
            {
                continue;
            }
            let last = &keys[end - 1];
            let dangling = !closed(words[end - 1])
                && matches!(last.as_str(), "and" | "or" | "the" | "a" | "an" | "of" | "with" | "to" | "in" | "on")
                && gap(words, end) >= ms(350)
                && (words[end - 1].text.ends_with(['…', '—', '-'])
                    || words[end - 1].text.ends_with("...")
                    || keys.get(end..end + 3) == keys.get(start..start + 3)
                    || keys.get(end..end + 3).is_some_and(|next| next == ["one", "more", "time"]));
            let direct_retry = words[end - 1].text.ends_with('.') && !words[end - 1].text.ends_with("...") && gap(words, end) <= ms(1600);
            if !dangling && !direct_retry {
                continue;
            }
            let keep = positions
                .iter()
                .copied()
                .filter(|&other| if dangling { (words[start].start - words[other].start).0.unsigned_abs() <= ms(150_000).0 as u64 } else { other == end })
                .filter_map(|other| continuation(words, &keys, old.clone(), other, direct_retry && !dangling))
                .filter(|keep| !discarded[keep.clone()].contains(&true))
                .min_by_key(|keep| (words[start].start - words[keep.start].start).0.unsigned_abs());
            if let Some(keep) = keep {
                reserved[old.clone()].fill(true);
                reserved[keep.clone()].fill(true);
                discarded[old.clone()].fill(true);
                cuts.push(Removal { discard: old, keep, rule: "indexed-continuation" });
                break;
            }
        }
    }
    cuts
}

fn has_quiet_boundary(quiet: &[TimeRange], start: Tick, end: Tick) -> bool {
    let first = quiet.partition_point(|range| range.end() <= start);
    quiet[first..].iter().take_while(|range| range.start < end).any(|range| range.end().min(end) - range.start.max(start) >= ms(70))
}

fn continuation(words: &[&Word], keys: &[String], old: Range<usize>, start: usize, immediate: bool) -> Option<Range<usize>> {
    let repeated_end = start + old.len();
    if repeated_end + 3 > words.len()
        || (start < old.end && old.start < repeated_end + 3)
        || keys[start..repeated_end] != keys[old.clone()]
        || closed(words[repeated_end - 1])
        || gap(words, repeated_end) >= ms(450)
        || (immediate && (!matches!(keys[repeated_end].as_str(), "with" | "without" | "using") || old.len() < 9))
    {
        return None;
    }
    // Preserve currency, signs and other punctuation on protected values.
    let same_prefix = old
        .clone()
        .zip(start..repeated_end)
        .all(|(a, b)| (!protected(&keys[a]) || marker_text(words[a]) == marker_text(words[b])) && (a + 1 == old.end || closed(words[a]) == closed(words[b])));
    if !same_prefix {
        return None;
    }
    let end = (repeated_end..(start + 48).min(words.len())).find(|&i| closed(words[i]))? + 1;
    let speaker = words[old.start].speaker;
    let match_is_continuous = (start + 1..end).all(|i| gap(words, i) < ms(1000));
    (end >= repeated_end + 3
        && (start >= old.end || end <= old.start)
        && match_is_continuous
        && words[old].iter().chain(&words[start..end]).all(|word| word.speaker == speaker))
    .then_some(start..end)
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
    fn m2_indexed_restart_requires_a_quiet_seam_and_a_longer_reading() {
        let opening = "I was able to watch most of the film.";
        let extended = "I was able to watch most of the film without touching any settings.";
        let cuts = |next: &str, quiet: &[TimeRange]| {
            let words: Vec<_> = attempt(opening, 0).into_iter().chain(attempt(next, 2010)).collect();
            extend(&words.iter().collect::<Vec<_>>(), quiet, vec![])
        };
        let quiet = [TimeRange::from_bounds(ms(1830), ms(1990))];
        let result = cuts(extended, &quiet);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].discard, 0..9);
        assert_eq!(result[0].keep, 9..22);
        assert!(cuts(extended, &[]).is_empty());
        assert!(cuts(opening, &quiet).is_empty());
    }

    #[test]
    fn m2_indexed_restart_can_find_a_distant_continuation_around_unique_speech() {
        let opening = "This compact camera records clear footage inside the—";
        let extended = "This compact camera records clear footage inside the waterproof housing overnight.";
        let words: Vec<_> = attempt(opening, 0).into_iter().chain(attempt("The battery lasted two hours.", 2500)).chain(attempt(extended, 104_000)).collect();
        let quiet = [TimeRange::from_bounds(ms(1450), ms(1900))];
        let result = extend(&words.iter().collect::<Vec<_>>(), &quiet, vec![]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].discard, 0..8);
        assert_eq!(result[0].keep, 13..24);

        let mut words = words;
        words[7].text = "the".into();
        assert!(extend(&words.iter().collect::<Vec<_>>(), &quiet, vec![]).is_empty());
    }
}
