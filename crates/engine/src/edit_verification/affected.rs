use std::collections::BTreeMap;

use shotclutch_project::Sequence;

use super::*;

/// Collect the occupied ranges of every changed timeline item on both sides of the edit.
pub(crate) fn affected(
    base: &Sequence,
    staged: &Sequence,
    mut check: impl FnMut() -> Result<(), ServiceError>,
) -> Result<(Vec<AffectedRange>, usize), ServiceError> {
    let mut before = Vec::new();
    let mut after = Vec::new();
    let mut changed_items = 0;
    let mut changes = Changes { before: &mut before, after: &mut after, count: &mut changed_items, check: &mut check };
    changes.collect(
        base.all_tracks().flat_map(|track| track.items.iter().map(|clip| (clip.id, (track.id, clip), clip.range()))),
        staged.all_tracks().flat_map(|track| track.items.iter().map(|clip| (clip.id, (track.id, clip), clip.range()))),
    )?;
    changes.collect(
        base.all_tracks().flat_map(|track| track.transitions.iter().map(|tr| (tr.id, (track.id, tr), tr.range()))),
        staged.all_tracks().flat_map(|track| track.transitions.iter().map(|tr| (tr.id, (track.id, tr), tr.range()))),
    )?;
    changes.collect(
        base.caption_tracks.iter().flat_map(|track| track.captions.iter().map(|c| (c.id, (track.id, c), c.range()))),
        staged.caption_tracks.iter().flat_map(|track| track.captions.iter().map(|c| (c.id, (track.id, c), c.range()))),
    )?;
    Ok((
        before
            .into_iter()
            .map(|range| AffectedRange { snapshot: Snapshot::Base, range })
            .chain(after.into_iter().map(|range| AffectedRange { snapshot: Snapshot::Staged, range }))
            .collect(),
        changed_items,
    ))
}

struct Changes<'a, F> {
    before: &'a mut Vec<TimeRange>,
    after: &'a mut Vec<TimeRange>,
    count: &'a mut usize,
    check: &'a mut F,
}

impl<F: FnMut() -> Result<(), ServiceError>> Changes<'_, F> {
    fn collect<K: Ord, T: PartialEq>(
        &mut self,
        before: impl Iterator<Item = (K, T, TimeRange)>,
        after: impl Iterator<Item = (K, T, TimeRange)>,
    ) -> Result<(), ServiceError> {
        let before = collect_items(before, self.check)?;
        let after = collect_items(after, self.check)?;
        for (key, (value, range)) in &before {
            (self.check)()?;
            if after.get(key).is_none_or(|new| &new.0 != value) {
                self.before.push(*range);
                *self.count += 1;
            }
        }
        for (key, (value, range)) in &after {
            (self.check)()?;
            match before.get(key) {
                Some(old) if &old.0 == value => {}
                Some(_) => self.after.push(*range),
                None => {
                    self.after.push(*range);
                    *self.count += 1;
                }
            }
        }
        Ok(())
    }
}

fn collect_items<K: Ord, T>(
    items: impl Iterator<Item = (K, T, TimeRange)>,
    check: &mut impl FnMut() -> Result<(), ServiceError>,
) -> Result<BTreeMap<K, (T, TimeRange)>, ServiceError> {
    let mut result = BTreeMap::new();
    for (id, value, range) in items {
        check()?;
        result.insert(id, (value, range));
    }
    Ok(result)
}

pub(crate) fn merge(ranges: impl IntoIterator<Item = TimeRange>) -> Vec<TimeRange> {
    let mut ranges = ranges.into_iter().filter(|r| !r.is_empty()).collect::<Vec<_>>();
    ranges.sort_unstable_by_key(|range| range.start);
    let mut result: Vec<TimeRange> = Vec::new();
    for range in ranges {
        if let Some(previous) = result.last_mut().filter(|previous| previous.end() >= range.start) {
            *previous = TimeRange::from_bounds(previous.start, previous.end().max(range.end()));
        } else {
            result.push(range);
        }
    }
    result
}
