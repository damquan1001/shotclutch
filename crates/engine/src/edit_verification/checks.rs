use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};
use shotclutch_project::{Project, Sequence};
use shotclutch_render::RenderOptions;

use super::*;
use crate::Session;
use crate::media_inspection::timeline::{Provider, prepare_provider, render_audio, render_frame};
use crate::media_inspection::{MAX_RESULT_BYTES, Pixels, sample_window};
use crate::service_sources::{ReadContext, SourceBinding};
use crate::transactions::TimelineEdit;

mod coverage;

const MAX_FRAME_SAMPLES: usize = 64;
const MAX_AUDIO_WINDOWS: usize = 64;
const MAX_EDGE: u32 = 640;
const VERSION: u32 = 1;

pub(crate) struct VerificationInput<'a> {
    pub transaction_id: DocumentId,
    pub sequence: ItemId,
    pub staged: &'a Project,
    pub sources: &'a [SourceBinding],
    pub edits: &'a [TimelineEdit],
    pub criteria: &'a [Criterion],
    pub request_digest: &'a str,
    pub staged_digest: &'a str,
    pub criteria_digest: &'a str,
    pub sources_digest: &'a str,
}

pub(crate) fn verify(session: &Session, input: VerificationInput<'_>, context: &ReadContext<'_>) -> Result<Report, ServiceError> {
    context.check()?;
    let staged = input.staged.sequence(input.sequence).ok_or_else(|| error(ErrorCode::VerificationFailed, "staged sequence is unavailable"))?;
    let base = session.project.sequence(input.sequence).ok_or_else(|| error(ErrorCode::VerificationFailed, "base sequence is unavailable"))?;
    let longest = staged.settings.width.max(staged.settings.height).max(1);
    let options = RenderOptions { scale: longest.min(MAX_EDGE) as f32 / longest as f32, captions: true, ..Default::default() };
    let (width, height) = shotclutch_render::output_size(staged, options.scale);
    let mut report = Report {
        verifier_version: VERSION,
        project_id: session.document_id,
        transaction_id: input.transaction_id,
        sequence_id: input.sequence,
        base_revision: session.revision,
        request_sha256: input.request_digest.into(),
        staged_sha256: input.staged_digest.into(),
        criteria_sha256: input.criteria_digest.into(),
        sources_sha256: input.sources_digest.into(),
        outcome: Outcome::Inconclusive,
        affected: Vec::new(),
        render: RenderSettings {
            width,
            height,
            frame_rate: staged.settings.frame_rate,
            scale: options.scale,
            sample_rate: staged.settings.sample_rate,
            audio_channels: shotclutch_render::mixer::width_of(staged.settings.audio_master),
            color: staged.settings.color,
            captions: options.captions,
            effects: options.effects,
        },
        frames: Vec::new(),
        audio: Vec::new(),
        criteria: Vec::new(),
        gaps: Vec::new(),
    };
    let coverage = match coverage::derive(&session.project, base, input.staged, staged, input.edits, context) {
        Ok(coverage) => coverage,
        Err(e) => return unavailable(report, input.criteria, e),
    };
    report.affected = coverage.ranges;
    if report.affected.is_empty() {
        report.gaps.push("The prepared edit has no affected sequence range".into());
    }
    let audio = input
        .criteria
        .iter()
        .any(|criterion| matches!(criterion, Criterion::PreserveSplit { audio: true } | Criterion::AudioMatches { .. } | Criterion::AudioLevel { .. }));
    let provider = match prepare_provider(session, &[&session.project, input.staged], input.sequence, context, audio) {
        Ok(provider) => provider,
        Err(e) => return unavailable(report, input.criteria, e),
    };
    if provider.fingerprints.iter().any(|(&id, fp)| !input.sources.iter().any(|source| source.matches_original(id, fp))) {
        return Err(error(ErrorCode::MediaChanged, "an original changed since preview"));
    }
    let mut reader = Reader {
        base: &session.project,
        staged: input.staged,
        provider: &provider,
        sequence: input.sequence,
        context,
        options,
        remaining: MAX_RESULT_BYTES,
        frames: BTreeMap::new(),
        audio: BTreeMap::new(),
    };
    for (&snapshot, times) in &coverage.frames {
        for &time in times {
            if let Err(e) = reader.frame(snapshot, time) {
                if !evidence_error(&e) {
                    return Err(e);
                }
                report.gaps.push(format!("{snapshot:?} frame at tick {}: {}", time.0, e.message));
                break;
            }
        }
    }
    for criterion in input.criteria {
        context.check()?;
        let result = match assess(&mut reader, criterion, &coverage.frames, &coverage.splits) {
            Ok((outcome, observed)) => CriterionResult { expected: criterion.clone(), outcome, observed },
            Err(error) if evidence_error(&error) => incomplete(criterion, error.message),
            Err(error) => return Err(error),
        };
        report.criteria.push(result);
    }
    report.frames = reader.frames.into_values().map(|(evidence, _)| evidence).collect();
    report.audio = reader.audio.into_values().collect();
    if report.criteria.iter().any(|item| item.outcome == Outcome::Failed) {
        report.outcome = Outcome::Failed;
    } else if report.gaps.is_empty() && report.criteria.iter().all(|item| item.outcome == Outcome::Passed) {
        report.outcome = Outcome::Passed;
    }
    context.check()?;
    Ok(report)
}

fn evidence_error(error: &ServiceError) -> bool {
    matches!(error.code, ErrorCode::CapabilityUnavailable | ErrorCode::MediaChanged | ErrorCode::VerificationFailed | ErrorCode::ResourceLimit)
}

fn unavailable(mut report: Report, criteria: &[Criterion], error: ServiceError) -> Result<Report, ServiceError> {
    if !evidence_error(&error) {
        return Err(error);
    }
    report.criteria = criteria.iter().map(|criterion| incomplete(criterion, error.message.clone())).collect();
    report.gaps.push(error.message);
    Ok(report)
}

fn incomplete(criterion: &Criterion, reason: String) -> CriterionResult {
    CriterionResult { expected: criterion.clone(), outcome: Outcome::Inconclusive, observed: Assessment::Unavailable { reason } }
}

struct Reader<'a, 'grant> {
    base: &'a Project,
    staged: &'a Project,
    provider: &'a Provider,
    sequence: ItemId,
    context: &'a ReadContext<'grant>,
    options: RenderOptions,
    remaining: usize,
    frames: BTreeMap<(Snapshot, Tick), (FrameEvidence, Pixels)>,
    audio: BTreeMap<(Snapshot, i64, usize), AudioEvidence>,
}

impl Reader<'_, '_> {
    fn project(&self, side: Snapshot) -> &Project {
        match side {
            Snapshot::Base => self.base,
            Snapshot::Staged => self.staged,
        }
    }

    fn sequence(&self, side: Snapshot) -> Result<&Sequence, ServiceError> {
        self.project(side).sequence(self.sequence).ok_or_else(|| error(ErrorCode::VerificationFailed, "verification sequence is unavailable"))
    }

    fn charge(&mut self, bytes: usize) -> Result<(), ServiceError> {
        self.remaining = self.remaining.checked_sub(bytes).ok_or_else(|| error(ErrorCode::ResourceLimit, "verification output exceeds 64 MiB"))?;
        Ok(())
    }

    fn frame(&mut self, side: Snapshot, time: Tick) -> Result<&(FrameEvidence, Pixels), ServiceError> {
        self.context.check()?;
        let sequence = self.sequence(side)?;
        if time < Tick::ZERO || time >= sequence.duration() {
            return Err(error(ErrorCode::VerificationFailed, "requested picture falls outside the composition"));
        }
        let time = sequence.settings.frame_rate.snap(time);
        let key = (side, time);
        if !self.frames.contains_key(&key) {
            if self.frames.len() >= MAX_FRAME_SAMPLES {
                return Err(error(ErrorCode::ResourceLimit, "verification exceeds 64 picture samples"));
            }
            let (w, h) = shotclutch_render::output_size(sequence, self.options.scale);
            self.charge(w * h * 4)?;
            self.context.cancellation.charge_run_work(crate::service::runs::Work::Frames, 1)?;
            let pixels = render_frame(self.project(side), self.sequence, time, self.options, self.provider, self.context)?.pixels;
            let mut sha = Sha256::new();
            sha.update(pixels.width.to_le_bytes());
            sha.update(pixels.height.to_le_bytes());
            sha.update(&pixels.rgba);
            self.frames.insert(key, (FrameEvidence { snapshot: side, time, sha256: format!("{:x}", sha.finalize()) }, pixels));
        }
        self.frames.get(&key).ok_or_else(|| error(ErrorCode::InternalError, "rendered frame evidence was lost"))
    }

    fn audio(&mut self, side: Snapshot, range: TimeRange) -> Result<&AudioEvidence, ServiceError> {
        self.context.check()?;
        let sequence = self.sequence(side)?;
        let end = range.start.0.checked_add(range.duration.0).map(Tick);
        if range.start < Tick::ZERO || range.is_empty() || end.is_none_or(|end| end > sequence.duration()) {
            return Err(error(ErrorCode::VerificationFailed, "requested audio falls outside the composition"));
        }
        let rate = sequence.settings.sample_rate;
        let (first, count) = sample_window(range, rate);
        if count == 0 {
            return Err(error(ErrorCode::VerificationFailed, "requested audio contains no full sample positions"));
        }
        let key = (side, first, count);
        if !self.audio.contains_key(&key) {
            if self.audio.len() >= MAX_AUDIO_WINDOWS {
                return Err(error(ErrorCode::ResourceLimit, "verification exceeds 64 audio windows"));
            }
            let channels = shotclutch_render::mixer::width_of(sequence.settings.audio_master);
            self.charge(count * channels * 4)?;
            self.context.cancellation.charge_run_work(crate::service::runs::Work::InspectedTicks, range.duration.0 as u64)?;
            let pcm = render_audio(self.project(side), self.sequence(side)?, first, count, self.provider, self.context)?.buffer;
            let mut sha = Sha256::new();
            sha.update(rate.to_le_bytes());
            sha.update((count as u64).to_le_bytes());
            sha.update((pcm.channel_count() as u64).to_le_bytes());
            let mut peak = 0.0_f64;
            let mut squared = 0.0;
            for channel in &pcm.channels {
                self.context.check()?;
                for &sample in channel {
                    if !sample.is_finite() {
                        return Err(error(ErrorCode::VerificationFailed, "mix contains non-finite audio samples"));
                    }
                    sha.update(sample.to_le_bytes());
                    let value = f64::from(sample);
                    peak = peak.max(value.abs());
                    squared += value * value;
                }
            }
            let rms = (squared / (count * pcm.channel_count()) as f64).sqrt();
            self.audio
                .insert(key, AudioEvidence { snapshot: side, first_sample: first, sample_count: count, sha256: format!("{:x}", sha.finalize()), rms, peak });
        }
        self.audio.get(&key).ok_or_else(|| error(ErrorCode::InternalError, "mixed audio evidence was lost"))
    }
}

fn assess(
    reader: &mut Reader<'_, '_>,
    criterion: &Criterion,
    frames: &BTreeMap<Snapshot, BTreeSet<Tick>>,
    splits: &BTreeSet<Tick>,
) -> Result<(Outcome, Assessment), ServiceError> {
    let outcome = |passed| if passed { Outcome::Passed } else { Outcome::Failed };
    let comparison = |frames, audio_windows, mismatches| (outcome(mismatches == 0), Assessment::Comparison { frames, audio_windows, mismatches });
    match criterion {
        Criterion::PreserveSplit { audio } => {
            let times: BTreeSet<_> = frames.values().flatten().copied().collect();
            if times.is_empty() || splits.is_empty() {
                return Err(error(ErrorCode::VerificationFailed, "no sampled split boundaries"));
            }
            let mut mismatches = 0;
            for &time in &times {
                let base = reader.frame(Snapshot::Base, time)?.0.sha256.clone();
                mismatches += usize::from(base != reader.frame(Snapshot::Staged, time)?.0.sha256);
            }
            if *audio {
                let end = reader.sequence(Snapshot::Base)?.duration().min(reader.sequence(Snapshot::Staged)?.duration());
                for &join in splits {
                    let margin = Tick::from_units(20, 1000);
                    let range = TimeRange::from_bounds((join - margin).max(Tick::ZERO), (join + margin).min(end));
                    let base = reader.audio(Snapshot::Base, range)?.sha256.clone();
                    mismatches += usize::from(base != reader.audio(Snapshot::Staged, range)?.sha256);
                }
            }
            Ok(comparison(times.len(), if *audio { splits.len() } else { 0 }, mismatches))
        }
        Criterion::FrameMatches { base, staged } => {
            let expected = reader.frame(Snapshot::Base, *base)?.0.sha256.clone();
            Ok(comparison(1, 0, usize::from(expected != reader.frame(Snapshot::Staged, *staged)?.0.sha256)))
        }
        Criterion::AudioMatches { base_start, staged_start, duration } => {
            let expected = reader.audio(Snapshot::Base, TimeRange::new(*base_start, *duration))?.clone();
            let observed = reader.audio(Snapshot::Staged, TimeRange::new(*staged_start, *duration))?;
            if expected.sample_count != observed.sample_count {
                return Err(error(ErrorCode::VerificationFailed, "audio windows do not cover the same number of samples"));
            }
            Ok(comparison(0, 1, usize::from(expected.sha256 != observed.sha256)))
        }
        Criterion::Luminance { time, region, minimum } => {
            let pixels = &reader.frame(Snapshot::Staged, *time)?.1;
            let Region { x, y, width, height } = region.unwrap_or(Region { x: 0, y: 0, width: pixels.width, height: pixels.height });
            if x.checked_add(width).is_none_or(|end| end > pixels.width) || y.checked_add(height).is_none_or(|end| end > pixels.height) {
                return Err(error(ErrorCode::VerificationFailed, "luminance region is outside the rendered picture"));
            }
            let stride = pixels.width as usize * 4;
            let mut sum = 0.0;
            for row in y..y + height {
                let start = row as usize * stride + x as usize * 4;
                for pixel in pixels.rgba[start..start + width as usize * 4].chunks_exact(4) {
                    let [r, g, b, a] = [pixel[0], pixel[1], pixel[2], pixel[3]].map(f64::from);
                    sum += (0.2126 * r + 0.7152 * g + 0.0722 * b) * a / (255.0 * 255.0);
                }
            }
            let mean = sum / f64::from(width * height);
            Ok((outcome(mean >= *minimum), Assessment::Luminance { mean }))
        }
        Criterion::AudioLevel { range, minimum_rms, maximum_peak } => {
            let evidence = reader.audio(Snapshot::Staged, *range)?;
            Ok((outcome(evidence.rms >= *minimum_rms && evidence.peak <= *maximum_peak), Assessment::AudioLevel { rms: evidence.rms, peak: evidence.peak }))
        }
        Criterion::Semantic { .. } => Ok((Outcome::Inconclusive, Assessment::Unavailable { reason: "A qualified semantic verifier is unavailable".into() })),
    }
}
