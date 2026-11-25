use std::collections::BTreeMap;
use std::sync::{Arc, atomic::AtomicBool};

use shotclutch_media::{MediaError, MediaKind, SharedSource, generators::GeneratorSource};
use shotclutch_project::{ItemKind, MediaRef, Project};
use shotclutch_render::{MAX_NEST_DEPTH, RenderOptions, SourceProvider};

use super::*;
use crate::service_sources::MAX_SNAPSHOT_BYTES;

pub(crate) fn timeline(session: &Session, request: &TimelineRequest, context: &ReadContext<'_>) -> Result<TimelineInspection, ServiceError> {
    context.check()?;
    if session.revision != request.expected_revision {
        return Err(error(ErrorCode::RevisionConflict, "timeline changed since the inspection was requested"));
    }
    inspect_snapshot(session, &session.project, request, context)
}

fn inspect_snapshot(session: &Session, project: &Project, request: &TimelineRequest, context: &ReadContext<'_>) -> Result<TimelineInspection, ServiceError> {
    let sequence = project.sequence(request.sequence).ok_or_else(|| error(ErrorCode::NotFound, "sequence not found"))?;
    let duration = sequence.duration();
    let (max_edge, range, audio) = match request.view {
        TimelineView::Frame { time, max_edge } => {
            check_time(time, duration)?;
            (max_edge, TimeRange::new(time, Tick(1)), false)
        }
        TimelineView::Clip { range, max_edge, audio } => {
            range_bounds(range, duration, true)?;
            (max_edge, range, audio)
        }
    };
    check_edge(max_edge)?;
    check_size(sequence.settings.width, sequence.settings.height)?;
    let longest = sequence.settings.width.max(sequence.settings.height);
    let scale = longest.min(max_edge) as f32 / longest as f32;
    let options = RenderOptions { scale, captions: true, ..Default::default() };
    let frame_rate = sequence.settings.frame_rate;
    let first = frame_rate.frame_at(range.start);
    let last = frame_rate.frame_at(Tick(range.end().0 - 1));
    if last.saturating_sub(first) >= MAX_CLIP_FRAMES as i64 {
        return Err(error(ErrorCode::ResourceLimit, "inspection clip exceeds 150 frames"));
    }
    let mut budget = OutputBudget::default();
    let (w, h) = shotclutch_render::output_size(sequence, scale);
    let output_bytes =
        (w * h * 4).checked_mul((last - first + 1) as usize).ok_or_else(|| error(ErrorCode::ResourceLimit, "inspection output exceeds 64 MiB"))?;
    budget.charge(output_bytes)?;
    let rate = sequence.settings.sample_rate;
    let audio_window = if audio {
        check_audio_rate(rate)?;
        let (first, count) = sample_window(range, rate);
        budget.charge(count * shotclutch_render::mixer::width_of(sequence.settings.audio_master) * 4)?;
        Some((first, count))
    } else {
        None
    };
    let provider = prepare_provider(session, &[project], request.sequence, context, audio)?;
    let evidence = media_work(context, || {
        let render = |time| render_frame(project, request.sequence, time, options, &provider, context);
        if matches!(request.view, TimelineView::Frame { .. }) {
            return render(frame_rate.tick_of(first)).map(TimelineResult::Frame);
        }
        let mut frames = Vec::new();
        for index in first..=last {
            let time = frame_rate.tick_of(index);
            let visible = TimeRange::from_bounds(time.max(range.start), frame_rate.tick_of(index.saturating_add(1)).min(range.end()));
            frames.push(ClipFrame { visible, frame: render(time)? });
        }
        let audio = audio_window.map(|(first, count)| render_audio(project, sequence, first, count, &provider, context)).transpose()?;
        Ok(TimelineResult::Clip(Clip { range, frames, audio }))
    })??;
    let originals = provider
        .sources
        .into_iter()
        .filter_map(|(asset, source)| {
            let used = source.used.load(Ordering::Relaxed);
            provider.fingerprints.get(&asset).filter(|_| used).cloned().map(|fingerprint| (asset, fingerprint))
        })
        .collect();
    context.check()?;
    Ok(TimelineInspection { sequence: request.sequence, revision: session.revision, originals, settings: sequence.settings.clone(), scale, evidence })
}

pub(crate) fn render_frame(
    project: &Project,
    sequence: ItemId,
    time: Tick,
    options: RenderOptions,
    provider: &Provider,
    context: &ReadContext<'_>,
) -> Result<TimelineFrame, ServiceError> {
    media_work(context, || {
        let image = shotclutch_render::render_sequence(project, sequence, time, options, provider);
        provider.check()?;
        Ok(TimelineFrame { time, pixels: Pixels { width: image.w as u32, height: image.h as u32, rgba: image.to_rgba8() } })
    })?
}

pub(crate) fn render_audio(
    project: &Project,
    sequence: &shotclutch_project::Sequence,
    first: i64,
    count: usize,
    provider: &Provider,
    context: &ReadContext<'_>,
) -> Result<Pcm, ServiceError> {
    media_work(context, || {
        let buffer = shotclutch_render::mixer::mix_graph(project, sequence, first, count, provider, None);
        provider.check()?;
        pcm(first, count, sequence.settings.sample_rate, buffer)
    })?
}

fn check_audio_rate(rate: u32) -> Result<(), ServiceError> {
    if !(1_000..=192_000).contains(&rate) {
        return Err(error(ErrorCode::ResourceLimit, "sequence sample rate exceeds inspection limits"));
    }
    Ok(())
}

pub(crate) struct Provider {
    /// Maps each occurrence's source item (including subclips) to its original.
    aliases: BTreeMap<ItemId, ItemId>,
    sources: BTreeMap<ItemId, Arc<CheckedSource>>,
    pub(crate) fingerprints: BTreeMap<ItemId, SourceFingerprint>,
    failed: Arc<AtomicBool>,
    limited: Arc<AtomicBool>,
}
impl Provider {
    fn check(&self) -> Result<(), ServiceError> {
        if self.limited.load(Ordering::Relaxed) {
            return Err(error(ErrorCode::ResourceLimit, "composition audio exceeds the working window limit"));
        }
        if self.failed.load(Ordering::Relaxed) {
            return Err(error(ErrorCode::VerificationFailed, "a composition input could not be decoded"));
        }
        Ok(())
    }
}
impl SourceProvider for Provider {
    fn source(&self, item: ItemId) -> Option<SharedSource> {
        let source = self.aliases.get(&item).and_then(|id| self.sources.get(id));
        if source.is_none() {
            self.failed.store(true, Ordering::Relaxed);
        }
        source.map(|s| s.clone() as SharedSource)
    }
    fn isolated_audio(&self) -> bool {
        true
    }
    fn audio_window_allowed(&self, frames: usize, rate: u32) -> bool {
        audio_window_allowed(&self.limited, frames, rate)
    }
}

// Five seconds of output, three seconds of mixer preroll, and resampling padding.
fn audio_window_allowed(limited: &AtomicBool, frames: usize, rate: u32) -> bool {
    let allowed = frames <= 8 * rate as usize + 2;
    if !allowed {
        limited.store(true, Ordering::Relaxed);
    }
    allowed
}

struct CheckedSource {
    inner: SharedSource,
    failed: Arc<AtomicBool>,
    used: AtomicBool,
    limited: Arc<AtomicBool>,
}
impl CheckedSource {
    fn read<T>(&self, run: impl FnOnce() -> shotclutch_media::Result<T>) -> shotclutch_media::Result<T> {
        self.used.store(true, Ordering::Relaxed);
        let result = if shotclutch_media::cancel::cancelled() { Err(MediaError::Cancelled) } else { run() };
        if result.is_err() {
            self.failed.store(true, Ordering::Relaxed);
        }
        result
    }
}
impl MediaSource for CheckedSource {
    fn info(&self) -> &MediaInfo {
        self.inner.info()
    }
    fn video_frame(&self, request: FrameRequest) -> shotclutch_media::Result<Arc<VideoFrame>> {
        self.read(|| {
            let frame = self.inner.video_frame(request)?;
            check_size(frame.width, frame.height).map_err(|e| MediaError::Unsupported(e.message))?;
            Ok(frame)
        })
    }
    fn audio(&self, start: i64, frames: usize, rate: u32) -> shotclutch_media::Result<AudioBuffer> {
        self.audio_stream(0, start, frames, rate)
    }
    fn audio_stream(&self, stream: usize, start: i64, frames: usize, rate: u32) -> shotclutch_media::Result<AudioBuffer> {
        self.read(|| {
            let stream_info = self.inner.info().audio_streams.get(stream).ok_or(MediaError::NoStream("audio"))?;
            if !(1..=32).contains(&stream_info.channels)
                || check_audio_rate(stream_info.sample_rate).is_err()
                || !audio_window_allowed(&self.limited, frames, rate)
            {
                self.limited.store(true, Ordering::Relaxed);
                return Err(MediaError::Unsupported("audio exceeds the inspection working window".into()));
            }
            let audio = self.inner.audio_stream(stream, start, frames, rate)?;
            pcm(start, frames, rate, audio).map(|pcm| pcm.buffer).map_err(|e| MediaError::Decode(e.message))
        })
    }
}

/// Authorize the reachable sequence graph. Transitions, time remapping and mixer preroll
/// can read outside the requested sequence window, so preflight covers the whole graph.
fn dependencies(project: &Project, sequence: ItemId, context: &ReadContext<'_>, audio: bool) -> Result<BTreeMap<ItemId, ItemId>, ServiceError> {
    let mut queue = vec![(sequence, 0)];
    let mut deepest = BTreeMap::new();
    let mut aliases = BTreeMap::new();
    while let Some((id, depth)) = queue.pop() {
        context.check()?;
        if depth > MAX_NEST_DEPTH {
            return Err(error(ErrorCode::CapabilityUnavailable, "inspection nesting exceeds the compositor depth limit"));
        }
        if deepest.get(&id).is_some_and(|previous| *previous >= depth) {
            continue;
        }
        deepest.insert(id, depth);
        if deepest.len() > 512 {
            return Err(error(ErrorCode::ResourceLimit, "inspection reaches too many nested sequences"));
        }
        let seq = project.sequence(id).ok_or_else(|| error(ErrorCode::MediaChanged, "nested sequence is missing"))?;
        check_size(seq.settings.width, seq.settings.height)?;
        if audio {
            check_audio_rate(seq.settings.sample_rate)?;
        }
        for clip in seq.all_tracks().flat_map(|track| &track.items) {
            context.check()?;
            let item = project.item(clip.item).ok_or_else(|| error(ErrorCode::MediaChanged, "composition input is missing"))?;
            match &item.kind {
                ItemKind::Sequence(_) => queue.push((item.id, depth + 1)),
                ItemKind::Media(_) | ItemKind::Subclip { .. } => {
                    let (asset, _, _) = project.resolve_media(item.id).ok_or_else(|| error(ErrorCode::MediaChanged, "subclip original is unavailable"))?;
                    aliases.insert(item.id, asset);
                    if aliases.len() > 512 {
                        return Err(error(ErrorCode::ResourceLimit, "inspection reaches too many source items"));
                    }
                }
                ItemKind::Graphic { width, height, .. } | ItemKind::AdjustmentLayer { width, height, .. } => check_size(*width, *height)?,
            }
        }
    }
    Ok(aliases)
}

pub(crate) fn prepare_provider(
    session: &Session,
    projects: &[&Project],
    sequence: ItemId,
    context: &ReadContext<'_>,
    audio: bool,
) -> Result<Provider, ServiceError> {
    let mut aliases = BTreeMap::new();
    for project in projects {
        for (item, original) in dependencies(project, sequence, context, audio)? {
            if aliases.insert(item, original).is_some_and(|previous| previous != original) {
                return Err(error(ErrorCode::MediaChanged, "composition source identity changed between snapshots"));
            }
        }
    }
    if aliases.len() > 512 {
        return Err(error(ErrorCode::ResourceLimit, "inspection reaches too many source items"));
    }
    let failed = Arc::new(AtomicBool::new(false));
    let limited = Arc::new(AtomicBool::new(false));
    let mut sources = BTreeMap::new();
    let mut fingerprints = BTreeMap::new();
    let mut remaining = context.budget.min(MAX_SNAPSHOT_BYTES);
    for &asset in aliases.values() {
        context.check()?;
        if sources.contains_key(&asset) {
            continue;
        }
        let media =
            session.project.item(asset).and_then(|item| item.as_media()).ok_or_else(|| error(ErrorCode::MediaChanged, "composition original is missing"))?;
        if media.offline {
            return Err(error(ErrorCode::MediaChanged, "composition original is offline"));
        }
        if let Some(video) = &media.info.video {
            check_size(video.width, video.height)?;
        }
        let inner = match &media.media {
            MediaRef::File { path } => {
                if media.info.kind == MediaKind::ImageSequence {
                    return Err(error(ErrorCode::CapabilityUnavailable, "numbered image sets need an inspection manifest"));
                }
                let limited = ReadContext { budget: remaining, ..*context };
                let mut bytes = Vec::new();
                let fingerprint = observation_file(session, path, media.identity, media.offline, &limited, Some(&mut bytes))?;
                remaining -= fingerprint.bytes;
                fingerprints.insert(asset, fingerprint);
                media_work(context, || open_snapshot(session, path, bytes))??
            }
            MediaRef::Generator(generator) => {
                let (width, height, rate) = media.info.video.as_ref().map(|v| (v.width, v.height, v.frame_rate)).unwrap_or((1, 1, Default::default()));
                Arc::new(GeneratorSource::new(generator.clone(), width, height, rate, media.info.duration))
            }
        };
        if let Some(video) = &inner.info().video {
            check_size(video.width, video.height)?;
        }
        sources.insert(asset, Arc::new(CheckedSource { inner, failed: failed.clone(), used: AtomicBool::new(false), limited: limited.clone() }));
    }
    Ok(Provider { aliases, sources, fingerprints, failed, limited })
}
