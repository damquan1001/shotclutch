//! Align repeated clauses across sentence breaks and small recognition differences.
use super::{Removal, aligned, hesitation, terminal};
use shotclutch_project::Word;
use shotclutch_time::Tick;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

const MAX_ATTEMPT: usize = 70;
const MAX_ALTERNATIVE: usize = 90;
const LOOKAHEAD: usize = 55;

fn ms(value: i64) -> Tick {
    Tick::from_units(value, 1000)
}

fn glue(key: &str) -> bool {
    matches!(
        key,
        "a" | "an"
            | "the"
            | "of"
            | "to"
            | "and"
            | "but"
            | "that"
            | "in"
            | "at"
            | "on"
            | "is"
            | "are"
            | "was"
            | "were"
            | "so"
            | "this"
            | "it"
            | "they"
            | "its"
            | "it's"
            | "there's"
            | "that's"
            | "i"
            | "you"
            | "he"
            | "she"
            | "we"
            | "as"
            | "be"
            | "been"
    ) || hesitation(key)
}

fn anchor(key: &str) -> bool {
    !key.is_empty()
        && !glue(key)
        && !matches!(
            key,
            "do" | "did"
                | "does"
                | "have"
                | "has"
                | "had"
                | "can"
                | "could"
                | "will"
                | "would"
                | "get"
                | "got"
                | "one"
                | "two"
                | "then"
                | "with"
                | "up"
                | "out"
        )
}

fn protected(key: &str) -> bool {
    key.chars().any(char::is_numeric)
        || key.ends_with("n't")
        || matches!(
            key,
            "no" | "not"
                | "never"
                | "neither"
                | "nor"
                | "without"
                | "cannot"
                | "more"
                | "less"
                | "higher"
                | "lower"
                | "only"
                | "before"
                | "after"
                | "always"
                | "sometimes"
                | "usually"
                | "rarely"
                | "all"
                | "none"
                | "every"
                | "either"
                | "first"
                | "last"
                | "zero"
                | "one"
                | "two"
                | "three"
                | "four"
                | "five"
                | "six"
                | "seven"
                | "eight"
                | "nine"
                | "ten"
                | "eleven"
                | "twelve"
                | "thirteen"
                | "fourteen"
                | "fifteen"
                | "sixteen"
                | "seventeen"
                | "eighteen"
                | "nineteen"
                | "twenty"
                | "thirty"
                | "forty"
                | "fifty"
                | "sixty"
                | "seventy"
                | "eighty"
                | "ninety"
                | "hundred"
                | "thousand"
                | "million"
                | "billion"
        )
}

fn gap(words: &[&Word], edge: usize) -> Tick {
    if edge == 0 || edge == words.len() { ms(1000) } else { words[edge].start - words[edge - 1].end }
}

fn closed(word: &Word) -> bool {
    terminal(word) && !word.text.trim_end_matches(['"', '”']).ends_with("...")
}

fn edges(words: &[&Word], keys: &[String]) -> Vec<usize> {
    let mut edges = BTreeSet::from([0, words.len()]);
    for i in 1..words.len() {
        if gap(words, i) >= ms(300) || closed(words[i - 1]) || words[i - 1].speaker != words[i].speaker {
            edges.insert(i);
        }
    }
    let mut previous = BTreeMap::new();
    for (j, window) in keys.windows(4).enumerate() {
        let phrase = [&window[0], &window[1], &window[2], &window[3]];
        if phrase.iter().copied().filter(|key| anchor(key)).collect::<BTreeSet<_>>().len() < 2 {
            continue;
        }
        if let Some(i) = previous.insert(phrase, j)
            && j - i >= 4
            && words[j].start - words[i].start < ms(45_000)
            && (gap(words, i).max(gap(words, j)) >= ms(80))
            && (edges.contains(&i) || edges.contains(&j) || words[j].start - words[i].start < ms(7000))
        {
            edges.extend([i, j]);
        }
    }
    edges.into_iter().collect()
}

fn appears_named(word: &Word, key: &str, at_sentence_start: bool) -> bool {
    let uppercase = word.text.trim_start_matches(|c: char| !c.is_alphabetic());
    !glue(key) && uppercase.chars().next().is_some_and(char::is_uppercase) && (!at_sentence_start || uppercase.chars().filter(|c| c.is_uppercase()).count() > 1)
}

fn marker_text(word: &Word) -> String {
    // Retain signs and currencies as well as the number itself.
    word.text.trim_matches(|c: char| !c.is_alphanumeric() && !matches!(c, '$' | '€' | '£' | '¥' | '%' | '+' | '-' | '−')).to_lowercase().replace('’', "'")
}

fn shared_subject(old: &[String], fresh: &[String]) -> bool {
    let subject = |keys: &[String]| {
        keys.iter().take(3).find_map(|k| match k.as_str() {
            "i" | "i'm" | "i've" | "i'll" => Some(0),
            "you" | "you're" | "you've" => Some(1),
            "he" | "he's" => Some(2),
            "she" | "she's" => Some(3),
            "we" | "we're" | "we've" => Some(4),
            "they" | "they're" | "they've" => Some(5),
            _ => None,
        })
    };
    subject(old).zip(subject(fresh)).is_none_or(|(a, b)| a == b)
}

struct Matcher<'a> {
    words: &'a [&'a Word],
    keys: &'a [String],
    markers: Vec<Option<String>>,
    edges: Vec<usize>,
    openings: Vec<BTreeSet<&'a str>>,
}

impl Matcher<'_> {
    fn compare(&self, old: Range<usize>, fresh: Range<usize>, end: usize, distinct: &BTreeSet<&str>) -> Option<usize> {
        let a = &self.keys[old.clone()];
        let b = &self.keys[fresh.clone()];
        let closed_old = closed(self.words[old.end - 1]);
        if (a.len() < 6 && (closed_old || gap(self.words, old.end) < ms(350) || end - fresh.start <= a.len()))
            || !shared_subject(a, b)
            || !self.markers[old.clone()].iter().flatten().eq(self.markers[fresh.clone()].iter().flatten())
        {
            return None;
        }
        if a == b && closed_old && (!closed(self.words[fresh.end - 1]) || gap(self.words, old.end) <= gap(self.words, fresh.end)) {
            // Let the original ordered rule settle an equal transcript without a better join.
            return None;
        }
        let later_keys: BTreeSet<_> = b.iter().map(String::as_str).collect();
        if distinct.intersection(&later_keys).count() * 100 < distinct.len() * 62 {
            return None;
        }
        let n = aligned(a, b);
        if n * 100 < a.len() * 78
            || n * 100 < b.len() * 68
            || (a.len() < 6 && n != a.len())
            || aligned(&a[..a.len().min(5)], &b[..b.len().min(6)]) < a.len().min(3)
        {
            return None;
        }
        let last_content = a.iter().rev().take(4).find(|k| !glue(k));
        if (closed_old || a.len() < 9 || self.words[old.end - 1].text.ends_with(','))
            && last_content.is_some_and(|k| !b[b.len().saturating_sub(6)..].contains(k))
        {
            return None;
        }
        let mut unmatched = a.iter().zip(&self.words[old.clone()]).filter(|(key, _)| {
            !glue(key)
                && !later_keys.contains(key.as_str())
                && !b.iter().any(|other| {
                    key.len().min(other.len()) >= 4 && (key.strip_suffix('s') == Some(other.as_str()) || other.strip_suffix('s') == Some(key.as_str()))
                })
        });
        if let Some((_, word)) = unmatched.next()
            && (closed_old || a.len() < 8 || word.confidence >= 0.55 || unmatched.next().is_some())
        {
            return None;
        }
        if closed_old || b.len() <= a.len() {
            let new_unique = b.iter().filter(|k| !glue(k) && !a.contains(k)).count();
            if new_unique > (a.len() / 10).max(1) {
                return None;
            }
        }
        // A long repeated statement cannot carry a separate, unmatched sentence into a cut.
        let mut sentence = old.start;
        for i in old.start..old.end - 1 {
            if closed(self.words[i]) {
                let clause = &self.keys[sentence..=i];
                if aligned(clause, b) * 100 < clause.len() * 78 {
                    return None;
                }
                sentence = i + 1;
            }
        }
        Some(n)
    }

    fn alternative(&self, old: Range<usize>, edge: usize, removed: &[bool]) -> Option<Range<usize>> {
        let distinct: BTreeSet<_> = self.keys[old.clone()].iter().filter(|key| anchor(key)).map(String::as_str).collect();
        let opening = &self.openings[edge];
        if distinct.len() < 2 || opening.is_empty() || self.words[old.clone()].iter().any(|w| w.speaker != self.words[old.start].speaker) {
            return None;
        }
        let mut best = None;
        for j in edge + 1..(edge + 1 + LOOKAHEAD).min(self.edges.len() - 1) {
            let start = self.edges[j];
            if self.words[start].start - self.words[old.end - 1].end > ms(105_000) {
                break;
            }
            if start < old.end
                || removed[start]
                || self.words[start].speaker != self.words[old.start].speaker
                || opening.intersection(&self.openings[j]).count() < opening.len().min(2)
            {
                continue;
            }
            for &end in self.edges.iter().skip(j + 1).take(4) {
                let len = end - start;
                if len > MAX_ALTERNATIVE || len * 10 > old.len() * 18 + 80 || self.words[end - 1].end - self.words[start].start > ms(32_000) {
                    break;
                }
                if len * 100 < old.len() * 69
                    || removed[start..end].contains(&true)
                    || self.words[start..end].iter().any(|w| w.speaker != self.words[old.start].speaker)
                {
                    continue;
                }
                for length in [len, old.len(), old.len() + 2, old.len() + 4] {
                    if length > len || (length < len && closed(self.words[old.end - 1]) && !closed(self.words[start + length - 1])) {
                        continue;
                    }
                    let Some(shared) = self.compare(old.clone(), start..start + length, end, &distinct) else { continue };
                    let score =
                        (shared * 1000 / old.len() + shared * 1000 / length, length == len, closed(self.words[end - 1]), gap(self.words, end) < ms(700));
                    if best.as_ref().is_none_or(|(previous, _)| score > *previous) {
                        best = Some((score, start..end));
                    }
                }
            }
        }
        best.map(|(_, range)| range)
    }
}

pub(in crate::preedit) fn detect(words: &[&Word]) -> Vec<Removal> {
    if words.len() < 6 {
        return Vec::new();
    }
    let keys: Vec<_> = words.iter().map(|w| w.normalized().replace('’', "'")).collect();
    let edges = edges(words, &keys);
    let openings: Vec<BTreeSet<_>> =
        edges.iter().map(|&i| keys[i..(i + 6).min(keys.len())].iter().filter(|k| anchor(k)).map(String::as_str).collect()).collect();
    let names: BTreeSet<_> = words
        .iter()
        .enumerate()
        .filter(|&(i, w)| appears_named(w, &keys[i], i == 0 || closed(words[i - 1]) || gap(words, i) >= ms(300)))
        .map(|(i, _)| keys[i].as_str())
        .collect();
    let markers = words.iter().zip(&keys).map(|(word, key)| (protected(key) || names.contains(key.as_str())).then(|| marker_text(word))).collect();
    let matcher = Matcher { words, keys: &keys, markers, edges, openings };
    let mut removed = vec![false; words.len()];
    let mut retained = vec![false; words.len()];
    let mut result = Vec::new();
    // Work backwards so a discarded attempt can reference a surviving later performance.
    for (i, &start) in matcher.edges.iter().enumerate().rev().skip(1) {
        for &end in matcher.edges[i + 1..].iter().take(3).rev() {
            let range = start..end;
            if !(3..=MAX_ATTEMPT).contains(&range.len())
                || words[end - 1].end - words[start].start > ms(27_000)
                || removed[range.clone()].contains(&true)
                || retained[range.clone()].contains(&true)
            {
                continue;
            }
            if let Some(keep) = matcher.alternative(range.clone(), i, &removed) {
                removed[range.clone()].fill(true);
                retained[keep.clone()].fill(true);
                result.push(Removal { discard: range, keep, rule: "aligned-retake" });
                break;
            }
        }
    }
    for removal in super::detect(words) {
        if !removed[removal.discard.clone()].contains(&true)
            && !retained[removal.discard.clone()].contains(&true)
            && !removed[removal.keep.clone()].contains(&true)
            && matcher.markers[removal.discard.clone()].iter().flatten().eq(matcher.markers[removal.keep.clone()].iter().flatten())
        {
            removed[removal.discard.clone()].fill(true);
            retained[removal.keep.clone()].fill(true);
            result.push(removal);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analyze(attempts: &[&str]) -> Vec<Removal> {
        let mut words = Vec::new();
        let mut time = 0;
        for attempt in attempts {
            for key in attempt.split_whitespace() {
                words.push(Word::new(key, ms(time), ms(time + 220)));
                time += 250;
            }
            time += 550;
        }
        detect(&words.iter().collect::<Vec<_>>())
    }

    #[test]
    fn m2_broader_aligns_across_asr_sentence_breaks_and_opening_variation() {
        let cuts = analyze(&[
            "But they built it with the same optics the older pair didn't,",
            "The case holds a charging cable.",
            "They built it with the same optics. The older pair didn't have those lenses.",
        ]);
        assert_eq!(cuts.len(), 1);
        assert_eq!(cuts[0].discard, 0..12);
        assert_eq!(cuts[0].keep.start, 18);
    }

    #[test]
    fn m2_broader_preserves_changed_values_names_negation_and_additional_facts() {
        for (a, b) in [
            ("The camera records smooth video at thirty frames.", "The camera records smooth video at sixty frames."),
            ("The camera costs around $399 at launch.", "The camera costs around €399 at launch."),
            ("I compared the Sony camera with the other lenses.", "I compared the Canon camera with the other lenses."),
            ("The camera does not support an external microphone.", "The camera does support an external microphone."),
            ("The camera records video with image stabilization and clear detail.", "The camera records video with clear detail."),
            ("The camera records clear video. It leaks.", "The camera records clear video. The case feels light."),
            ("I used the camera on a long hike.", "She used the camera on a long hike."),
            ("We used the red lens and got clear video,", "We used the blue lens and got clear video with better sound."),
        ] {
            let cuts = analyze(&[a, b]);
            assert!(cuts.is_empty() || cuts.iter().all(|cut| cut.discard.end <= 5 && a.contains("It leaks")), "distinct material was removed: {a} / {b}");
        }
    }
}
