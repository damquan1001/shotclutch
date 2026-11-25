use shotclutch_project::{Project, Sequence, TrackItem};
use shotclutch_time::FrameRate;

use super::*;
use crate::edit_verification::affected::{affected, merge};

pub(super) struct Coverage {
    pub ranges: Vec<AffectedRange>,
    pub frames: BTreeMap<Snapshot, BTreeSet<Tick>>,
    pub splits: BTreeSet<Tick>,
}

pub(super) fn derive(
    base_project: &Project,
    base: &Sequence,
    staged_project: &Project,
    staged: &Sequence,
    edits: &[TimelineEdit],
    context: &ReadContext<'_>,
) -> Result<Coverage, ServiceError> {
    let (changed, _) = affected(base, staged, || context.check())?;
    let ranges = |snapshot| changed.iter().filter(move |r| r.snapshot == snapshot).map(|r| r.range);
    let before = ranges(Snapshot::Base).collect::<Vec<_>>();
    let after = ranges(Snapshot::Staged).collect::<Vec<_>>();
    let splits = edits
        .iter()
        .filter_map(|edit| match edit {
            TimelineEdit::Split { time, .. } => time.parse::<i64>().ok().map(Tick),
            _ => None,
        })
        .collect::<BTreeSet<_>>();

    // Sample all affected edges in both compositions. Include bounded comparison
    // ranges where a change exists in only one composition, such as a newly exposed gap.
    let boundaries = before.iter().chain(&after).flat_map(|r| [r.start, r.end()]).chain(splits.iter().copied()).collect::<BTreeSet<_>>();
    let affected = merge(before.iter().chain(&after).copied());
    if affected.len() > 256 {
        return Err(error(ErrorCode::ResourceLimit, "verification reaches too many affected ranges"));
    }
    let mut coverage = Coverage { ranges: Vec::new(), frames: BTreeMap::new(), splits };
    for (snapshot, project, sequence, changes) in [(Snapshot::Base, base_project, base, before), (Snapshot::Staged, staged_project, staged, after)] {
        let ranges = merge(changes);
        if ranges.len() > 128 {
            return Err(error(ErrorCode::ResourceLimit, "verification reaches too many affected ranges"));
        }
        coverage.ranges.extend(ranges.into_iter().map(|range| AffectedRange { snapshot, range }));
        let mut points =
            SamplePoints { rate: sequence.settings.frame_rate, duration: sequence.duration(), times: BTreeSet::new(), remaining_landmarks: 100_000 };
        for &edge in &boundaries {
            context.check()?;
            points.edge(edge)?;
        }
        landmarks(project, sequence, &mut Vec::new(), &affected, &mut points, context)?;
        coverage.frames.insert(snapshot, points.times);
    }
    Ok(coverage)
}

struct SamplePoints {
    rate: FrameRate,
    duration: Tick,
    times: BTreeSet<Tick>,
    remaining_landmarks: usize,
}
impl SamplePoints {
    fn visit(&mut self, context: &ReadContext<'_>) -> Result<(), ServiceError> {
        context.check()?;
        self.remaining_landmarks = self
            .remaining_landmarks
            .checked_sub(1)
            .ok_or_else(|| error(ErrorCode::ResourceLimit, "verification exceeds the animation and transition traversal budget"))?;
        Ok(())
    }

    fn add(&mut self, time: Tick) -> Result<(), ServiceError> {
        if time >= Tick::ZERO && time < self.duration {
            self.times.insert(time);
            if self.times.len() > MAX_FRAME_SAMPLES {
                return Err(error(ErrorCode::ResourceLimit, "verification exceeds the picture sampling budget"));
            }
        }
        Ok(())
    }

    fn edge(&mut self, time: Tick) -> Result<(), ServiceError> {
        self.add(self.rate.snap(Tick(time.0.saturating_sub(1))))?;
        let at = self.rate.frame_at(time);
        let boundary = self.rate.tick_of(at);
        self.add(if boundary == time { boundary } else { self.rate.tick_of(at.saturating_add(1)) })
    }
}

fn from_source(item: &TrackItem, source: Tick) -> Option<Tick> {
    let speed = item.speed.abs();
    if !speed.is_finite() || speed <= 0.0 {
        return None;
    }
    let offset = if item.reverse { item.source_out() - source - Tick(1) } else { source - item.source_in };
    Some(item.start + Tick((offset.0 as f64 / speed).round() as i64))
}

fn projected(mut time: Tick, through: &[&TrackItem]) -> Option<Tick> {
    for item in through.iter().rev() {
        time = from_source(item, time)?;
        if !item.range().contains(time) {
            return None;
        }
    }
    Some(time)
}

fn overlaps_through(range: TimeRange, through: &[&TrackItem], affected: &[TimeRange]) -> bool {
    let mut range = range;
    for item in through.iter().rev() {
        let Some(a) = from_source(item, range.start) else { return false };
        let Some(b) = from_source(item, range.end()) else { return false };
        let Some(mapped) = TimeRange::from_bounds(a.min(b), a.max(b)).intersect(&item.range()) else { return false };
        range = mapped;
    }
    affected.iter().any(|part| part.overlaps(&range))
}

fn landmarks<'a>(
    project: &'a Project,
    sequence: &'a Sequence,
    through: &mut Vec<&'a TrackItem>,
    affected: &[TimeRange],
    points: &mut SamplePoints,
    context: &ReadContext<'_>,
) -> Result<(), ServiceError> {
    if through.len() > shotclutch_render::MAX_NEST_DEPTH as usize {
        return Err(error(ErrorCode::CapabilityUnavailable, "nested composition exceeds the verification depth"));
    }
    for track in sequence.all_tracks() {
        for transition in &track.transitions {
            points.visit(context)?;
            if overlaps_through(transition.range(), through, affected) {
                if let Some(time) = projected(transition.start + Tick(transition.duration.0 / 2), through) {
                    points.add(points.rate.snap(time))?;
                }
            }
        }
        for clip in &track.items {
            points.visit(context)?;
            if !clip.enabled || !overlaps_through(clip.range(), through, affected) {
                continue;
            }
            if clip.frame_hold.is_some() && clip.hold_filters {
                continue;
            }
            for effect in clip.effects.iter().filter(|effect| effect.enabled) {
                for key in effect.params.values().flat_map(|param| &param.keyframes) {
                    points.visit(context)?;
                    if let Some(time) = from_source(clip, key.time)
                        && clip.range().contains(time)
                        && let Some(time) = projected(time, through)
                        && affected.iter().any(|range| range.contains(time))
                    {
                        points.add(points.rate.snap(time))?;
                    }
                }
            }
            if clip.frame_hold.is_none()
                && let Some(nested) = project.sequence(clip.item)
            {
                through.push(clip);
                landmarks(project, nested, through, affected, points, context)?;
                through.pop();
            }
        }
    }
    Ok(())
}
