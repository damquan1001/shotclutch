//! Evidence and criteria for native staged edit verification.

use serde::{Deserialize, Serialize};
use shotclutch_format::session::DocumentId;
use shotclutch_project::ItemId;
use shotclutch_time::{TICKS_PER_SECOND, Tick, TimeRange};

use crate::service::{ErrorCode, ServiceError, error};

pub(crate) mod affected;
#[cfg(not(target_arch = "wasm32"))]
mod checks;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use checks::{VerificationInput, verify};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Criterion {
    /// Compare composed picture at the sampled split boundaries and mixed PCM around the joins.
    PreserveSplit {
        audio: bool,
    },
    FrameMatches {
        base: Tick,
        staged: Tick,
    },
    /// Measure the mean display luma in a region of the staged render.
    Luminance {
        time: Tick,
        region: Option<Region>,
        minimum: f64,
    },
    AudioMatches {
        base_start: Tick,
        staged_start: Tick,
        duration: Tick,
    },
    AudioLevel {
        range: TimeRange,
        minimum_rms: f64,
        maximum_peak: f64,
    },
    Semantic {
        check: SemanticCheck,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    /// Pixel rectangle in the returned render dimensions.
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SemanticCheck {
    SubjectVisible,
    TitleReadable,
    SpeechIntelligible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Passed,
    Failed,
    Inconclusive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Snapshot {
    Base,
    Staged,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub verifier_version: u32,
    pub project_id: DocumentId,
    pub transaction_id: DocumentId,
    pub sequence_id: ItemId,
    pub base_revision: u64,
    pub request_sha256: String,
    pub staged_sha256: String,
    pub criteria_sha256: String,
    pub sources_sha256: String,
    pub outcome: Outcome,
    pub affected: Vec<AffectedRange>,
    pub render: RenderSettings,
    pub frames: Vec<FrameEvidence>,
    pub audio: Vec<AudioEvidence>,
    pub criteria: Vec<CriterionResult>,
    pub gaps: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AffectedRange {
    pub snapshot: Snapshot,
    pub range: TimeRange,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderSettings {
    pub width: usize,
    pub height: usize,
    pub frame_rate: shotclutch_time::FrameRate,
    pub scale: f32,
    pub sample_rate: u32,
    pub audio_channels: usize,
    pub color: shotclutch_color::ColorPipeline,
    pub captions: bool,
    pub effects: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameEvidence {
    pub snapshot: Snapshot,
    pub time: Tick,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioEvidence {
    pub snapshot: Snapshot,
    pub first_sample: i64,
    pub sample_count: usize,
    pub sha256: String,
    pub rms: f64,
    pub peak: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CriterionResult {
    pub expected: Criterion,
    pub outcome: Outcome,
    pub observed: Assessment,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Assessment {
    Comparison { frames: usize, audio_windows: usize, mismatches: usize },
    Luminance { mean: f64 },
    AudioLevel { rms: f64, peak: f64 },
    Unavailable { reason: String },
}

pub(crate) fn validate(criteria: &[Criterion], edits: &[crate::transactions::TimelineEdit]) -> Result<(), ServiceError> {
    use crate::transactions::TimelineEdit;
    let unit = |value: f64| (0.0..=1.0).contains(&value);
    let duration = |value: Tick| (1..=5 * TICKS_PER_SECOND).contains(&value.0);
    if criteria.is_empty() || criteria.len() > 16 {
        return Err(error(ErrorCode::InvalidArgument, "verified previews require one to sixteen criteria"));
    }
    for criterion in criteria {
        let valid = match criterion {
            Criterion::PreserveSplit { .. } => !edits.is_empty() && edits.iter().all(|edit| matches!(edit, TimelineEdit::Split { .. })),
            Criterion::FrameMatches { base, staged } => *base >= Tick::ZERO && *staged >= Tick::ZERO,
            Criterion::Luminance { time, region, minimum } => *time >= Tick::ZERO && unit(*minimum) && region.is_none_or(|r| r.width > 0 && r.height > 0),
            Criterion::AudioMatches { base_start, staged_start, duration: length } => {
                *base_start >= Tick::ZERO && *staged_start >= Tick::ZERO && duration(*length)
            }
            Criterion::AudioLevel { range, minimum_rms, maximum_peak } => {
                range.start >= Tick::ZERO && duration(range.duration) && unit(*minimum_rms) && maximum_peak.is_finite() && *maximum_peak >= *minimum_rms
            }
            Criterion::Semantic { .. } => true,
        };
        if !valid {
            return Err(error(ErrorCode::InvalidArgument, "invalid composed verification criterion"));
        }
    }
    Ok(())
}
