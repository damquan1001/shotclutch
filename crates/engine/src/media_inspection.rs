//! Read-only inspection of verified originals and the current sequence composition.

use std::sync::atomic::Ordering;

use shotclutch_color::ColorInfo;
use shotclutch_frame::{AudioBuffer, Region, VideoFrame};
use shotclutch_media::{FrameRequest, MediaInfo, MediaSource};
use shotclutch_project::ItemId;
use shotclutch_project::observations::SourceFingerprint;
use shotclutch_time::{TICKS_PER_SECOND, Tick, TimeRange};

use crate::Session;
use crate::media_observations::{ActiveModels, ObservationInspection, open_snapshot, original};
use crate::service::{ErrorCode, ServiceError, error};
use crate::service_sources::{ReadContext, observation_file};

pub(crate) mod timeline;
pub(crate) use timeline::timeline;

const MAX_TILES: u8 = 16;
const MAX_CLIP_FRAMES: usize = 150;
const MAX_CLIP_TICKS: i64 = 5 * TICKS_PER_SECOND;
pub(crate) const MAX_RESULT_BYTES: usize = 64 * 1024 * 1024;
const MAX_PIXELS: u64 = 4096 * 2160;

#[derive(Clone, Debug)]
pub enum Request {
    Source(SourceRequest),
    Timeline(TimelineRequest),
}

#[derive(Debug)]
pub enum Inspection {
    Source(SourceInspection),
    Timeline(TimelineInspection),
}

#[derive(Clone, Debug)]
pub struct SourceRequest {
    pub asset: ItemId,
    pub expected_fingerprint: SourceFingerprint,
    pub view: SourceView,
}

#[derive(Clone, Copy, Debug)]
pub enum SourceView {
    Summary,
    ContactSheet {
        range: TimeRange,
        samples: u8,
        max_edge: u32,
    },
    Frame {
        time: Tick,
        rounding: Rounding,
        max_edge: u32,
        crop: Option<Region>,
    },
    /// Audio is off unless a source audio stream is explicitly selected.
    Clip {
        range: TimeRange,
        max_edge: u32,
        audio_stream: Option<u16>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rounding {
    Exact,
    AtOrBefore,
    AtOrAfter,
}

#[derive(Clone, Debug)]
pub struct TimelineRequest {
    pub sequence: ItemId,
    pub expected_revision: u64,
    pub view: TimelineView,
}

#[derive(Clone, Copy, Debug)]
pub enum TimelineView {
    /// The sequence frame displayed at this tick.
    Frame {
        time: Tick,
        max_edge: u32,
    },
    Clip {
        range: TimeRange,
        max_edge: u32,
        audio: bool,
    },
}

#[derive(Debug)]
pub struct SourceInspection {
    pub asset: ItemId,
    pub revision: u64,
    pub fingerprint: SourceFingerprint,
    pub evidence: SourceResult,
}

#[derive(Debug)]
pub enum SourceResult {
    Summary {
        info: MediaInfo,
        observations: ObservationInspection,
    },
    /// Tiles in increasing source presentation order.
    ContactSheet(Vec<SourceFrame>),
    Frame(SourceFrame),
    Clip(Clip<SourceFrame>),
}

#[derive(Debug)]
pub struct TimelineInspection {
    pub sequence: ItemId,
    pub revision: u64,
    /// Source IDs and fingerprints for originals actually read by picture or audio.
    pub originals: std::collections::BTreeMap<ItemId, SourceFingerprint>,
    pub settings: shotclutch_project::SequenceSettings,
    pub scale: f32,
    pub evidence: TimelineResult,
}

#[derive(Debug)]
pub enum TimelineResult {
    Frame(TimelineFrame),
    Clip(Clip<TimelineFrame>),
}

/// Display-ready, straight-alpha RGBA8.
#[derive(Debug)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug)]
pub struct SourceFrame {
    pub pts: Tick,
    /// Region in the displayed original. An output pixel center (x, y) maps to
    /// (region.x + (x + .5) * region.w / width, region.y + (y + .5) * region.h / height).
    pub region: Region,
    pub original_size: (u32, u32),
    pub source_color: ColorInfo,
    pub pixel_aspect: (u32, u32),
    pub pixels: Pixels,
}

impl SourceFrame {
    /// Maps pixel-edge coordinates from this output into the displayed original.
    pub fn output_to_original(&self) -> shotclutch_geom::Affine {
        use shotclutch_geom::Affine;
        let origin = Affine::translate(self.region.x as f64, self.region.y as f64);
        let scale = Affine::scale(self.region.w as f64 / f64::from(self.pixels.width), self.region.h as f64 / f64::from(self.pixels.height));
        origin.then_apply(&scale)
    }
}

#[derive(Debug)]
pub struct TimelineFrame {
    pub time: Tick,
    pub pixels: Pixels,
}

#[derive(Debug)]
pub struct Clip<F> {
    pub range: TimeRange,
    pub frames: Vec<ClipFrame<F>>,
    pub audio: Option<Pcm>,
}

#[derive(Debug)]
pub struct ClipFrame<F> {
    /// Frame visibility intersected with the requested range. The underlying PTS may precede it.
    pub visible: TimeRange,
    pub frame: F,
}

#[derive(Debug)]
pub struct Pcm {
    /// First sample index in the source or sequence time domain at buffer.sample_rate.
    pub first_sample: i64,
    pub buffer: AudioBuffer,
}

#[derive(Default)]
struct OutputBudget(usize);
impl OutputBudget {
    fn charge(&mut self, bytes: usize) -> Result<(), ServiceError> {
        self.0 = self
            .0
            .checked_add(bytes)
            .filter(|total| *total <= MAX_RESULT_BYTES)
            .ok_or_else(|| error(ErrorCode::ResourceLimit, "inspection output exceeds 64 MiB"))?;
        Ok(())
    }
}

fn range_bounds(range: TimeRange, duration: Tick, clip: bool) -> Result<(Tick, Tick), ServiceError> {
    let start = range.start;
    let end = Tick(start.0.checked_add(range.duration.0).ok_or_else(|| error(ErrorCode::InvalidArgument, "inspection range overflows"))?);
    if start < Tick::ZERO || start >= end || end > duration {
        return Err(error(ErrorCode::InvalidArgument, "inspection range is outside the media bounds"));
    }
    if clip && range.duration.0 > MAX_CLIP_TICKS {
        return Err(error(ErrorCode::ResourceLimit, "inspection clip exceeds five seconds"));
    }
    Ok((start, end))
}

fn check_time(time: Tick, duration: Tick) -> Result<(), ServiceError> {
    if time < Tick::ZERO || time >= duration {
        return Err(error(ErrorCode::InvalidArgument, "inspection time is outside the media bounds"));
    }
    Ok(())
}

fn check_size(width: u32, height: u32) -> Result<(), ServiceError> {
    if width == 0 || height == 0 || width.max(height) > shotclutch_project::MAX_FRAME_SIDE || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(error(ErrorCode::ResourceLimit, "inspection exceeds the decoded pixel budget"));
    }
    Ok(())
}

fn check_edge(max_edge: u32) -> Result<(), ServiceError> {
    if max_edge == 0 {
        return Err(error(ErrorCode::InvalidArgument, "maximum edge must be positive"));
    }
    Ok(())
}

fn media_work<T>(context: &ReadContext<'_>, run: impl FnOnce() -> T) -> Result<T, ServiceError> {
    context.check()?;
    let revoked = context.revoked.clone();
    let expires = context.expires;
    let result = context.cancellation.with_media(|| {
        shotclutch_media::cancel::with_stop(
            move || revoked.load(Ordering::Acquire) || web_time::Instant::now() >= expires,
            || {
                shotclutch_media::cancel::with_strict_decode(|| {
                    shotclutch_media::cancel::with_draft(false, || shotclutch_media::cancel::with_catch_up(None, run))
                })
            },
        )
    });
    context.check()?;
    Ok(result)
}

fn boundary(source: &dyn MediaSource, time: Tick, rounding: Rounding) -> Result<Tick, ServiceError> {
    let pts = source
        .video_edit_boundary(time, rounding == Rounding::AtOrAfter)
        .filter(|pts| *pts >= Tick::ZERO && *pts < source.info().duration)
        .filter(|pts| match rounding {
            Rounding::Exact => *pts == time,
            Rounding::AtOrAfter => *pts >= time,
            Rounding::AtOrBefore => *pts <= time,
        })
        .ok_or_else(|| error(ErrorCode::VerificationFailed, "no certified presentation boundary in the requested direction"))?;
    Ok(pts)
}

fn source_frame(source: &dyn MediaSource, pts: Tick, max_edge: u32, crop: Option<Region>, budget: &mut OutputBudget) -> Result<SourceFrame, ServiceError> {
    let frame = source.video_frame(FrameRequest::full(pts)).map_err(|_| error(ErrorCode::VerificationFailed, "original frame could not be decoded"))?;
    if frame.pts != pts {
        return Err(error(ErrorCode::VerificationFailed, "decoded frame differs from the certified boundary"));
    }
    check_size(frame.width, frame.height)?;
    let original_size = (frame.width, frame.height);
    let (w, h) = (frame.width as usize, frame.height as usize);
    let region = crop.unwrap_or(Region { x: 0, y: 0, w, h });
    if region.w == 0 || region.h == 0 || region.x > w || region.y > h || region.w > w - region.x || region.h > h - region.y {
        return Err(error(ErrorCode::InvalidArgument, "crop is outside the displayed original"));
    }
    let pixels = source_pixels(&frame, region, max_edge, budget)?;
    Ok(SourceFrame { pts, region, original_size, source_color: frame.color, pixel_aspect: frame.par, pixels })
}

fn source_pixels(frame: &VideoFrame, region: Region, max_edge: u32, budget: &mut OutputBudget) -> Result<Pixels, ServiceError> {
    let longest = region.w.max(region.h);
    let edge = longest.min(max_edge as usize);
    let width = (region.w * edge / longest).max(1);
    let height = (region.h * edge / longest).max(1);
    budget.charge(width * height * 4)?;
    let input = frame.to_rgba8();
    let stride = frame.width as usize * 4;
    let rgba = if region.x == 0 && region.y == 0 && width == frame.width as usize && height == frame.height as usize {
        input
    } else {
        let mut result = Vec::with_capacity(width * height * 4);
        for y in 0..height {
            if shotclutch_media::cancel::cancelled() {
                return Err(error(ErrorCode::Canceled, "inspection stopped"));
            }
            let row = (region.y + (2 * y + 1) * region.h / (2 * height)) * stride;
            for x in 0..width {
                let offset = row + (region.x + (2 * x + 1) * region.w / (2 * width)) * 4;
                result.extend_from_slice(&input[offset..offset + 4]);
            }
        }
        result
    };
    Ok(Pixels { width: width as u32, height: height as u32, rgba })
}

pub(crate) fn sample_window(range: TimeRange, rate: u32) -> (i64, usize) {
    let sample = |tick: Tick| ((i128::from(tick.0) * i128::from(rate) + i128::from(TICKS_PER_SECOND) - 1) / i128::from(TICKS_PER_SECOND)) as i64;
    let start = sample(range.start);
    (start, (sample(range.end()) - start) as usize)
}

fn pcm(first_sample: i64, count: usize, rate: u32, buffer: AudioBuffer) -> Result<Pcm, ServiceError> {
    if buffer.sample_rate != rate || buffer.channels.is_empty() || buffer.channels.iter().any(|channel| channel.len() != count) {
        return Err(error(ErrorCode::VerificationFailed, "audio decoder returned an incomplete inspection window"));
    }
    Ok(Pcm { first_sample, buffer })
}

pub(crate) fn source(session: &Session, request: &SourceRequest, models: &ActiveModels, context: &ReadContext<'_>) -> Result<SourceInspection, ServiceError> {
    context.check()?;
    let (media, path) = original(session, request.asset)?;
    let revision = session.revision;
    if matches!(request.view, SourceView::Summary) {
        let observations = crate::media_observations::inspect(session, request.asset, models, context)?;
        if observations.fingerprint.as_ref() != Some(&request.expected_fingerprint) {
            return Err(error(ErrorCode::MediaChanged, "original differs from the requested evidence"));
        }
        return Ok(SourceInspection {
            asset: request.asset,
            revision,
            fingerprint: request.expected_fingerprint.clone(),
            evidence: SourceResult::Summary { info: media.info.clone(), observations },
        });
    }
    let mut bytes = Vec::new();
    let fingerprint = observation_file(session, path, None, media.offline, context, Some(&mut bytes))?;
    if fingerprint != request.expected_fingerprint {
        return Err(error(ErrorCode::MediaChanged, "original differs from the requested evidence"));
    }
    let evidence = media_work(context, || {
        let source = open_snapshot(session, path, bytes)?;
        let video = source.info().video.as_ref().ok_or_else(|| error(ErrorCode::CapabilityUnavailable, "original has no picture"))?;
        check_size(video.width, video.height)?;
        inspect_picture(source.as_ref(), request.view, context.cancellation)
    })??;
    Ok(SourceInspection { asset: request.asset, revision, fingerprint, evidence })
}

fn inspect_picture(source: &dyn MediaSource, view: SourceView, cancellation: &crate::service::Cancellation) -> Result<SourceResult, ServiceError> {
    use crate::service::runs::Work;
    let duration = source.info().duration;
    let mut budget = OutputBudget::default();
    match view {
        SourceView::Frame { time, rounding, max_edge, crop } => {
            check_edge(max_edge)?;
            check_time(time, duration)?;
            let pts = boundary(source, time, rounding)?;
            Ok(SourceResult::Frame(source_frame(source, pts, max_edge, crop, &mut budget)?))
        }
        SourceView::ContactSheet { range, samples, max_edge } => {
            check_edge(max_edge)?;
            let (start, end) = range_bounds(range, duration, false)?;
            if samples == 0 || samples > MAX_TILES {
                return Err(error(ErrorCode::ResourceLimit, "contact sheets require one to sixteen tiles"));
            }
            let mut frames: Vec<SourceFrame> = Vec::new();
            for index in 0..samples {
                let time = Tick(start.0 + ((i128::from(end.0 - start.0) * (2 * i128::from(index) + 1)) / (2 * i128::from(samples))) as i64);
                let pts = boundary(source, time, Rounding::AtOrBefore).or_else(|_| {
                    boundary(source, time, Rounding::AtOrAfter)
                        .and_then(|pts| if pts < end { Ok(pts) } else { Err(error(ErrorCode::VerificationFailed, "range has no certified frame")) })
                })?;
                if frames.last().is_none_or(|frame| frame.pts != pts) {
                    frames.push(source_frame(source, pts, max_edge, None, &mut budget)?);
                }
            }
            Ok(SourceResult::ContactSheet(frames))
        }
        SourceView::Clip { range, max_edge, audio_stream } => {
            check_edge(max_edge)?;
            let (start, end) = range_bounds(range, duration, true)?;
            cancellation.charge_run_work(Work::InspectedTicks, range.duration.0 as u64)?;
            let mut pts = boundary(source, start, Rounding::AtOrBefore)?;
            let mut frames = Vec::new();
            while pts < end {
                if frames.len() == MAX_CLIP_FRAMES {
                    return Err(error(ErrorCode::ResourceLimit, "inspection clip exceeds 150 frames"));
                }
                let next = source.video_edit_boundary(Tick(pts.0 + 1), true).unwrap_or(duration).min(duration);
                if next <= pts {
                    return Err(error(ErrorCode::VerificationFailed, "source frame boundaries did not advance"));
                }
                let visible = TimeRange::from_bounds(pts.max(start), next.min(end));
                cancellation.charge_run_work(Work::Frames, 1)?;
                frames.push(ClipFrame { visible, frame: source_frame(source, pts, max_edge, None, &mut budget)? });
                pts = next;
            }
            let audio = audio_stream
                .map(|stream| {
                    let info = source
                        .info()
                        .audio_streams
                        .get(usize::from(stream))
                        .ok_or_else(|| error(ErrorCode::InvalidArgument, "source audio stream is unavailable"))?;
                    let rate = info.sample_rate;
                    if !(1..=192_000).contains(&rate) || !(1..=32).contains(&info.channels) {
                        return Err(error(ErrorCode::ResourceLimit, "source audio exceeds inspection limits"));
                    }
                    let (first, count) = sample_window(range, rate);
                    budget.charge(count * info.channels as usize * 4)?;
                    let buffer = source
                        .audio_stream(usize::from(stream), first, count, rate)
                        .map_err(|_| error(ErrorCode::VerificationFailed, "source audio could not be decoded"))?;
                    pcm(first, count, rate, buffer)
                })
                .transpose()?;
            Ok(SourceResult::Clip(Clip { range, frames, audio }))
        }
        SourceView::Summary => Err(error(ErrorCode::InternalError, "source summary uses the metadata path")),
    }
}

#[cfg(test)]
mod tests;
