use super::*;
use crate::media_inspection::{SourceRequest, SourceResult, SourceView};
use crate::media_observations::Freshness;
use crate::media_search::{EvidenceId, EvidenceType, SearchQuery, SearchResults};
use shotclutch_project::observations::{SourceLocation, SourceStream};
use shotclutch_project::{Sequence, Track, TrackItem, TrackKind};
use shotclutch_time::TICKS_PER_SECOND;

const MAX_CANDIDATES: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlannedAction {
    Split { clips: Vec<ClipId>, cut: Tick },
}

/// Planners receive typed, already inspected evidence. The executor checks all proposed targets.
pub trait Planner {
    fn plan(&self, occurrence: &Occurrence, excerpt: &Excerpt, placement: &Placement) -> Result<PlannedAction, ServiceError>;
}

pub struct OfflinePhrasePlanner;

impl Planner for OfflinePhrasePlanner {
    fn plan(&self, _: &Occurrence, _: &Excerpt, placement: &Placement) -> Result<PlannedAction, ServiceError> {
        Ok(PlannedAction::Split { clips: placement.clips.clone(), cut: placement.cut })
    }
}

pub(super) fn query(session: &Session, request: &PhraseSplit) -> Result<SearchQuery, ServiceError> {
    let mut query = SearchQuery::new(&request.phrase);
    query.limit = 100;
    query.filters.evidence_type = Some(EvidenceType::Transcript);
    query.filters.speaker = request.speaker.clone();
    query.filters.asset = request.asset;
    query.include_stale = true;
    query.include_unverified = true;
    if let Some(id) = request.placement {
        let sequence = session.project.sequence(request.sequence).ok_or_else(|| error(ErrorCode::NotFound, "sequence is unavailable"))?;
        let (_, item) = sequence.find_item(id).ok_or_else(|| error(ErrorCode::NotFound, "placement is unavailable in the requested sequence"))?;
        if session.project.item(item.item).and_then(|item| item.as_media()).is_none() || !direct(item) {
            return Err(error(ErrorCode::CapabilityUnavailable, "the specified placement requires a nested, subclip, or retimed mapping"));
        }
        if query.filters.asset.is_some_and(|asset| asset != item.item) {
            return Err(error(ErrorCode::InvalidArgument, "the selected placement belongs to a different original"));
        }
        query.filters.asset = Some(item.item);
    }
    Ok(query)
}

pub(super) fn candidates(results: &SearchResults, cancellation: &Cancellation) -> Result<Vec<Occurrence>, ServiceError> {
    let mut unique = BTreeMap::<_, Occurrence>::new();
    for hit in &results.hits {
        for entry in std::iter::once(&hit.best).chain(&hit.alternatives) {
            cancellation.check()?;
            if entry.evidence_type != EvidenceType::Transcript
                || !entry.exact_phrase
                || !entry.word_timed
                || entry.source_freshness != Freshness::Current
                || entry.producer_freshness != Freshness::Current
            {
                continue;
            }
            let (
                Some(fingerprint),
                EvidenceId::Observation(observation),
                SourceLocation::SourceRange { stream: SourceStream::Audio(audio_stream), start, end },
            ) = (&entry.fingerprint, &entry.evidence, entry.location)
            else {
                continue;
            };
            let key = (entry.asset, audio_stream, start, end);
            let candidate = unique.entry(key).or_insert_with(|| Occurrence {
                asset: entry.asset,
                fingerprint: fingerprint.clone(),
                range: TimeRange::from_bounds(Tick(start), Tick(end)),
                audio_stream,
                observations: Vec::new(),
            });
            if !candidate.observations.contains(observation) && candidate.observations.len() < 16 {
                candidate.observations.push(*observation);
            }
            if unique.len() > MAX_CANDIDATES {
                return Err(error(ErrorCode::ResourceLimit, "too many current phrase occurrences; narrow the search"));
            }
        }
    }
    Ok(unique.into_values().collect())
}

pub(super) fn uncertain(results: &SearchResults, current: &[Occurrence], cancellation: &Cancellation) -> Result<bool, ServiceError> {
    for hit in &results.hits {
        for evidence in std::iter::once(&hit.best).chain(&hit.alternatives) {
            cancellation.check()?;
            if !evidence.exact_phrase
                || !evidence.word_timed
                || (evidence.source_freshness == Freshness::Current && evidence.producer_freshness == Freshness::Current)
            {
                continue;
            }
            let SourceLocation::SourceRange { stream, start, end } = evidence.location else { continue };
            let duplicate = current.iter().any(|known| {
                known.asset == evidence.asset
                    && known.range.start == Tick(start)
                    && known.range.end() == Tick(end)
                    && (stream == SourceStream::All || stream == SourceStream::Audio(known.audio_stream))
                    && evidence.fingerprint.as_ref().is_none_or(|fingerprint| *fingerprint == known.fingerprint)
            });
            if !duplicate {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub(super) fn excerpt(
    session: &Session,
    grant: &Grant,
    occurrence: &Occurrence,
    models: &ActiveModels,
    cancellation: &Cancellation,
) -> Result<Excerpt, ServiceError> {
    let (media, _) = crate::media_observations::original(session, occurrence.asset)?;
    let margin = Tick(TICKS_PER_SECOND / 8);
    let range = TimeRange::from_bounds((occurrence.range.start - margin).max(Tick::ZERO), (occurrence.range.start + margin).min(media.info.duration));
    let request = SourceRequest {
        asset: occurrence.asset,
        expected_fingerprint: occurrence.fingerprint.clone(),
        view: SourceView::Clip { range, max_edge: 256, audio_stream: Some(occurrence.audio_stream) },
    };
    let result = crate::media_inspection::source(session, &request, models, &grant.observation_context(cancellation))?;
    let SourceResult::Clip(clip) = result.evidence else {
        return Err(error(ErrorCode::VerificationFailed, "source inspection returned no excerpt"));
    };
    let audio =
        clip.audio.filter(|audio| audio.buffer.frames() > 0).ok_or_else(|| error(ErrorCode::VerificationFailed, "source excerpt has no audio samples"))?;
    if !clip.frames.iter().any(|frame| frame.visible.contains(occurrence.range.start)) {
        return Err(error(ErrorCode::VerificationFailed, "source excerpt has no picture at the phrase onset"));
    }
    Ok(Excerpt {
        range: clip.range,
        frame_pts: clip.frames.into_iter().map(|frame| frame.frame.pts).collect(),
        first_sample: audio.first_sample,
        sample_count: audio.buffer.frames(),
        sample_rate: audio.buffer.sample_rate,
    })
}

fn direct(item: &TrackItem) -> bool {
    item.speed == 1.0 && !item.reverse && item.frame_hold.is_none() && item.multicam.is_none()
}

fn usable(track: &Track, item: &TrackItem) -> bool {
    track.enabled && !track.muted && !track.locked && item.enabled && direct(item)
}

pub(super) fn placements(
    project: &Project,
    sequence: &Sequence,
    request: &PhraseSplit,
    occurrence: &Occurrence,
    cancellation: &Cancellation,
) -> Result<(Vec<Placement>, bool), ServiceError> {
    let mut result = Vec::new();
    let mut unsupported = false;
    let tracks = sequence.all_tracks();
    let mut linked = BTreeMap::<_, Vec<_>>::new();
    for track in tracks {
        for clip in &track.items {
            cancellation.check()?;
            if let Some(link) = clip.link {
                linked.entry(link).or_default().push((track, clip));
            }
        }
    }
    for track in &sequence.audio_tracks {
        for clip in &track.items {
            cancellation.check()?;
            if clip.item != occurrence.asset || clip.audio_stream != usize::from(occurrence.audio_stream) || !clip.enabled {
                continue;
            }
            let group = clip.link.and_then(|link| linked.get(&link));
            if request.placement.is_some_and(|id| id != clip.id && group.is_none_or(|items| !items.iter().any(|(_, member)| member.id == id))) {
                continue;
            }
            if !direct(clip) {
                unsupported = true;
                continue;
            }
            if occurrence.range.start < clip.source_in || occurrence.range.start >= clip.source_in + clip.duration {
                continue;
            }
            let onset = clip.start + (occurrence.range.start - clip.source_in);
            let rate = sequence.settings.frame_rate;
            let previous = rate.snap(onset);
            let cut = if previous == onset { onset } else { rate.tick_of(rate.frame_at(onset).saturating_add(1)) };
            let Some(group) = group.filter(|group| group.iter().any(|(track, _)| track.kind == TrackKind::Video)) else {
                unsupported = true;
                continue;
            };
            if group.len() > 128 {
                return Err(error(ErrorCode::ResourceLimit, "linked placement exceeds 128 clips"));
            }
            if group.iter().any(|(track, item)| {
                !usable(track, item)
                    || item.item != occurrence.asset
                    || !item.range().contains(onset)
                    || !(item.start < cut && cut < item.end())
                    || item.source_in + (onset - item.start) != occurrence.range.start
                    || (track.kind == TrackKind::Audio && item.audio_stream != clip.audio_stream)
            }) {
                unsupported = true;
                continue;
            }
            let mut clips: Vec<_> = group.iter().map(|(_, item)| item.id).collect();
            clips.sort_unstable();
            let placement = Placement { clips, onset, cut, rounding: cut - onset };
            if !result.contains(&placement) {
                result.push(placement);
            }
            if result.len() > MAX_CANDIDATES {
                return Err(error(ErrorCode::ResourceLimit, "too many timeline placements; select one clip"));
            }
        }
    }
    if request.placement.is_none() {
        // A relevant nested or subclip instance can make a direct match ambiguous.
        let mut pending = eligible_references(sequence, occurrence.asset, cancellation)?;
        let mut visited = BTreeSet::new();
        while let Some(id) = pending.pop_first() {
            cancellation.check()?;
            if !visited.insert(id) {
                continue;
            }
            if visited.len() > 512 {
                return Err(error(ErrorCode::ResourceLimit, "too many referenced sources; select one direct placement"));
            }
            if id == occurrence.asset || project.resolve_media(id).is_some_and(|(original, _, _)| original == occurrence.asset) {
                unsupported = true;
                break;
            }
            if let Some(nested) = project.sequence(id) {
                for track in nested.all_tracks() {
                    for item in &track.items {
                        cancellation.check()?;
                        if item.enabled {
                            pending.insert(item.item);
                        }
                    }
                }
            }
        }
    }
    Ok((result, unsupported))
}

fn eligible_references(sequence: &Sequence, asset: ItemId, cancellation: &Cancellation) -> Result<BTreeSet<ItemId>, ServiceError> {
    let mut ids = BTreeSet::new();
    for track in sequence.all_tracks() {
        for item in &track.items {
            cancellation.check()?;
            if item.enabled && item.item != asset {
                ids.insert(item.item);
            }
        }
    }
    Ok(ids)
}

pub(super) fn verify_cut(session: &Session, draft: &Draft, placement: &Placement, occurrence: &Occurrence) -> Result<(), ServiceError> {
    let fail = || error(ErrorCode::VerificationFailed, "staged cut did not exactly split the intended linked clips");
    let base = session.project.sequence(draft.request.sequence_id).ok_or_else(fail)?;
    let staged = draft.prepared.project().sequence(draft.request.sequence_id).ok_or_else(fail)?;
    let mut changed = BTreeSet::new();
    let mut right_link = None;
    for id in &placement.clips {
        let (track_id, before) = base.find_item(*id).ok_or_else(fail)?;
        let track = staged.track(track_id).ok_or_else(fail)?;
        let left = track.item(*id).ok_or_else(fail)?;
        let right = track.item_at(placement.cut).ok_or_else(fail)?;
        if before.item != occurrence.asset
            || left.item != before.item
            || right.item != before.item
            || left.start != before.start
            || left.source_in != before.source_in
            || left.end() != placement.cut
            || right.id == left.id
            || right.start != placement.cut
            || right.end() != before.end()
            || right.source_in != before.source_in + (placement.cut - before.start)
            || left.link != before.link
            || right.link.is_none()
            || right.link == left.link
            || right_link.is_some_and(|link| right.link != Some(link))
        {
            return Err(fail());
        }
        right_link = right.link;
        changed.extend([left.id.0.to_string(), right.id.0.to_string()]);
    }
    let actual = draft.preview["changes"].as_array().ok_or_else(fail)?;
    if actual.len() != changed.len() || actual.iter().any(|entry| entry["clipId"].as_str().is_none_or(|id| !changed.contains(id))) {
        return Err(fail());
    }
    Ok(())
}
