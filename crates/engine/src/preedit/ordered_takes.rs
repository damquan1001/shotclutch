//! Bounded, ordered transcript matching. This classifies textual redundancy, not delivery
//! quality or omitted audio. Unknown differences stay retained. No intervening span is cut.
use shotclutch_project::Word;
use shotclutch_time::Tick;
use std::ops::Range;

const MAX_WORDS: usize = 96;
const LOOKBACK: usize = 48;

pub(super) struct Removal {
    pub discard: Range<usize>,
    pub keep: Range<usize>,
    pub rule: &'static str,
}

struct Utterance {
    range: Range<usize>,
    keys: Vec<String>,
    complete: bool,
    speaker: Option<u32>,
    mixed_speakers: bool,
    fillers: usize,
}

fn terminal(word: &Word) -> bool {
    word.text.trim_end_matches(['"', '\'', ')', '”']).ends_with(['.', '?', '!'])
}

fn hesitation(key: &str) -> bool {
    matches!(key, "um" | "umm" | "ummm" | "uh" | "uhh" | "uhhh" | "er" | "erm" | "err")
}

fn content(key: &str) -> bool {
    // Only articles can disappear in a textual-equivalence match. Tense, pronouns,
    // conjunctions, negation, names, numbers and comparisons all carry information.
    !matches!(key, "a" | "an" | "the")
}

fn utterance(words: &[&Word], range: Range<usize>) -> Utterance {
    let raw: Vec<_> = words[range.clone()].iter().map(|w| w.normalized().replace('’', "'")).collect();
    let known: std::collections::BTreeSet<_> = words[range.clone()].iter().filter_map(|w| w.speaker).collect();
    Utterance {
        complete: terminal(words[range.end - 1]),
        speaker: known.first().copied(),
        mixed_speakers: known.len() > 1,
        fillers: raw.iter().filter(|s| hesitation(s)).count(),
        keys: raw.into_iter().filter(|s| !s.is_empty() && !hesitation(s)).collect(),
        range,
    }
}

fn spans(words: &[&Word]) -> Vec<Utterance> {
    let mut ranges = Vec::new();
    let mut first = 0;
    let mut natural_start = true;
    let mut end = words.first().map_or(Tick::ZERO, |w| w.end);
    for i in 1..words.len() {
        let speaker = words[i - 1].speaker.zip(words[i].speaker).is_some_and(|(a, b)| a != b);
        let natural = speaker || words[i].start - end >= Tick::from_units(700, 1000) || (i - first >= 4 && terminal(words[i - 1]));
        if natural || i - first >= MAX_WORDS {
            ranges.push((first..i, natural_start && natural));
            first = i;
            natural_start = natural;
            end = words[i].end;
        } else {
            end = end.max(words[i].end);
        }
    }
    if first < words.len() {
        ranges.push((first..words.len(), natural_start));
    }
    // Split an unfinished repeated opening only when a measurable hesitation separates
    // attempts. Tiny word repetitions alone are emphasis, not abandoned utterances.
    let mut result = Vec::new();
    for (range, eligible) in ranges {
        if !eligible || range.len() >= MAX_WORDS {
            continue;
        }
        let mut first = range.start;
        for i in range.start + 3..range.end {
            if i - first < 3 || terminal(words[i - 1]) || words[i].start - words[i - 1].end < Tick::from_units(350, 1000) {
                continue;
            }
            let old = utterance(words, first..i);
            let fresh = utterance(words, i..range.end);
            if prefix(&old, &fresh) {
                result.push(old);
                first = i;
            }
        }
        result.push(utterance(words, first..range.end));
    }
    result
}

/// LCS retains order and repeated-token multiplicity, unlike a bag of opening words.
fn aligned(a: &[String], b: &[String]) -> usize {
    let mut row = vec![0; b.len() + 1];
    for x in a {
        let mut diagonal = 0;
        for (j, y) in b.iter().enumerate() {
            let previous = row[j + 1];
            row[j + 1] = if x == y { diagonal + 1 } else { row[j + 1].max(row[j]) };
            diagonal = previous;
        }
    }
    row[b.len()]
}

fn prefix(old: &Utterance, fresh: &Utterance) -> bool {
    !old.complete
        && old.keys.len() >= 3
        && old.keys.iter().filter(|s| content(s)).collect::<std::collections::BTreeSet<_>>().len() >= 2
        && fresh.keys.len() > old.keys.len()
        && fresh.keys.starts_with(&old.keys)
}

fn equivalent(a: &Utterance, b: &Utterance) -> bool {
    a.keys.len().min(b.keys.len()) >= 6
        && a.keys.iter().filter(|s| content(s)).eq(b.keys.iter().filter(|s| content(s)))
        && aligned(&a.keys, &b.keys) * 100 >= a.keys.len().max(b.keys.len()) * 85
}

pub(super) fn detect(words: &[&Word]) -> Vec<Removal> {
    let units = spans(words);
    let mut parents: Vec<Option<(usize, &'static str)>> = vec![None; units.len()];
    for (j, fresh) in units.iter().enumerate() {
        if fresh.range.len() >= MAX_WORDS || fresh.mixed_speakers {
            continue;
        }
        for i in (j.saturating_sub(LOOKBACK)..j).rev() {
            let old = &units[i];
            if words[fresh.range.start].start - words[old.range.end - 1].end > Tick::from_units(120, 1) {
                break;
            }
            if parents[i].is_some() || old.range.len() >= MAX_WORDS || old.mixed_speakers || old.speaker != fresh.speaker {
                continue;
            }
            let (discard, keep, rule) = if prefix(old, fresh) {
                (i, j, "ordered-prefix-restart")
            } else if prefix(fresh, old) {
                (j, i, "ordered-prefix-restart")
            } else if words[fresh.range.start].start - words[old.range.end - 1].end >= Tick::from_units(700, 1000) && equivalent(old, fresh) {
                // Completeness and fewer recognized hesitations are observable evidence.
                // ASR probability/loudness are not delivery scores. A tie keeps the earlier
                // performance; a later attempt does not win merely because it is last.
                if (fresh.complete, std::cmp::Reverse(fresh.fillers)) > (old.complete, std::cmp::Reverse(old.fillers)) {
                    (i, j, "ordered-equivalent-take")
                } else {
                    (j, i, "ordered-equivalent-take")
                }
            } else {
                continue;
            };
            parents[discard] = Some((keep, rule));
            if discard == j {
                break;
            }
        }
    }
    parents
        .iter()
        .enumerate()
        .filter_map(|(i, parent)| {
            let &(mut keep, rule) = parent.as_ref()?;
            // Every keeper is live when linked, so links form an acyclic forest. Resolve a
            // later supersession before emitting groups: alternatives must end at a survivor.
            while let Some((next, _)) = parents[keep] {
                keep = next;
            }
            Some(Removal { discard: units[i].range.clone(), keep: units[keep].range.clone(), rule })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(takes: &[&str]) -> Vec<Word> {
        let mut words = Vec::new();
        let mut at = 0;
        for take in takes {
            for key in take.split_whitespace() {
                words.push(Word::new(key, Tick::from_units(at, 1000), Tick::from_units(at + 200, 1000)));
                at += 250;
            }
            at += 1000;
        }
        words
    }

    fn run(words: &[Word]) -> Vec<Removal> {
        detect(&words.iter().collect::<Vec<_>>())
    }

    #[test]
    fn m2_ordered_takes_only_remove_the_matching_attempt_not_intervening_content() {
        let words = words(&["this camera records video", "the battery lasts two hours.", "this camera records video at sixty frames."]);
        let cuts = run(&words);
        assert_eq!(cuts.len(), 1);
        assert_eq!(cuts[0].discard, 0..4);
        assert_eq!(cuts[0].keep, 9..16);
    }

    #[test]
    fn m2_ordered_takes_protect_facts_names_order_and_complementary_details() {
        for (a, b) in [
            ("this camera records video at 30 frames.", "this camera records video at 60 frames."),
            ("this camera records video at thirty frames.", "this camera records video at sixty frames."),
            ("this camera does not support external microphones.", "this camera does support external microphones."),
            ("I prefer the Sony camera for this job.", "I prefer the Canon camera for this job."),
            ("Alice photographed Bob beside the red camera.", "Bob photographed Alice beside the red camera."),
            ("this camera records video with good detail.", "this camera records video with good detail and sound."),
            ("this camera recorded video with good detail.", "this camera records video with good detail."),
        ] {
            assert!(run(&words(&[a, b])).is_empty(), "different meaning must survive: {a} / {b}");
        }
    }

    #[test]
    fn m2_ordered_takes_rank_completion_without_a_last_attempt_bias() {
        let cuts = run(&words(&["this camera records video with good detail.", "this camera records video"]));
        assert_eq!(cuts.len(), 1);
        assert_eq!(cuts[0].discard, 7..11);
        assert_eq!(cuts[0].keep, 0..7);
        let cuts = run(&words(&["this camera records video with good detail.", "this camera records video with good detail."]));
        assert_eq!(cuts.len(), 1);
        assert_eq!(cuts[0].discard, 7..14);
        assert_eq!(cuts[0].keep, 0..7);
        let cuts = run(&words(&["this camera records video with good detail", "this camera records video with good detail."]));
        assert_eq!(cuts[0].discard, 0..7);
    }

    #[test]
    fn m2_ordered_takes_match_articles_and_known_fillers_without_losing_content() {
        let cuts = run(&words(&["this um camera records the video with good detail.", "this camera records video with good detail."]));
        assert_eq!(cuts.len(), 1);
        assert_eq!(cuts[0].discard, 0..9);
        assert_eq!(cuts[0].keep, 9..16);
    }

    #[test]
    fn m2_ordered_takes_follow_the_surviving_alternative_through_restarts() {
        let cuts = run(&words(&["this camera records video", "this camera records video with good", "this camera records video with good detail."]));
        assert_eq!(cuts.len(), 2);
        assert_eq!(cuts[0].keep, 10..17);
        assert_eq!(cuts[1].keep, 10..17);
    }

    #[test]
    fn m2_ordered_takes_preserve_emphasis_and_different_speakers() {
        assert!(run(&words(&["very very very good.", "Thank you.", "Thank you."])).is_empty());
        let mut phrase = words(&["this camera records video with good detail.", "this camera records video with good detail."]);
        for w in &mut phrase[7..] {
            w.speaker = Some(1);
        }
        assert!(run(&phrase).is_empty());
        let mut phrase = words(&["this camera records video with good detail.", "this camera records video with good detail."]);
        for w in &mut phrase[7..] {
            w.start = w.start - Tick::from_units(1000, 1000);
            w.end = w.end - Tick::from_units(1000, 1000);
        }
        assert!(run(&phrase).is_empty(), "immediate sentence emphasis stays retained");
    }

    #[test]
    fn m2_ordered_takes_split_measurable_within_utterance_restarts() {
        let mut phrase = words(&["this camera records this camera records video with good detail."]);
        for w in &mut phrase[3..] {
            w.start = w.start + Tick::from_units(400, 1000);
            w.end = w.end + Tick::from_units(400, 1000);
        }
        let cuts = run(&phrase);
        assert_eq!(cuts.len(), 1);
        assert_eq!(cuts[0].discard, 0..3);
        assert!(run(&words(&["this camera records this camera records video with good detail."])).is_empty());
    }

    #[test]
    fn m2_ordered_takes_context_and_work_are_bounded() {
        let mut phrase = words(&["this camera records video", "this camera records video with good detail."]);
        for w in &mut phrase[4..] {
            w.start = w.start + Tick::from_units(121, 1);
            w.end = w.end + Tick::from_units(121, 1);
        }
        assert!(run(&phrase).is_empty());
        let long = (0..200).map(|n| format!("token{n}")).collect::<Vec<_>>().join(" ");
        assert!(run(&words(&[&long, &long])).is_empty());
    }

    #[test]
    fn m2_ordered_takes_find_distant_restarts_without_removing_other_statements() {
        let mut text = vec!["this camera records video".to_owned()];
        text.extend((0..8).map(|i| format!("unrelated sentence number {i} ends.")));
        text.push("this camera records video with good detail.".to_owned());
        let refs: Vec<_> = text.iter().map(String::as_str).collect();
        let cuts = run(&words(&refs));
        assert_eq!(cuts.len(), 1);
        assert_eq!(cuts[0].discard, 0..4);
        assert_eq!(cuts[0].keep, 44..51);
    }
}
