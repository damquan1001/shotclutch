//! Host-issued policy and human review for verified timeline transactions.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use shotclutch_format::session::DocumentId;
use shotclutch_project::ItemId;
use shotclutch_time::{Tick, TimeRange};

use crate::edit_verification::{Criterion, Outcome, Report, SemanticCheck, Snapshot, affected::merge};
use crate::service::{ErrorCode, ServiceError, error};
use crate::transactions::{TimelineEdit, TrimMode};

const MAX_RANGES: usize = 128;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Ask,
    Assist,
    Autonomous,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EditFamily {
    Split,
    RegularTrim,
    RippleTrim,
}

impl From<&TimelineEdit> for EditFamily {
    fn from(edit: &TimelineEdit) -> Self {
        match edit {
            TimelineEdit::Split { .. } => Self::Split,
            TimelineEdit::Trim { mode: TrimMode::Regular, .. } => Self::RegularTrim,
            TimelineEdit::Trim { mode: TrimMode::Ripple, .. } => Self::RippleTrim,
        }
    }
}

/// Host configuration for one task. A sequence's ranges cover the full old and new
/// occupied spans of changed items. Pass neighboring ranges for a continuous scope.
#[derive(Clone, Debug)]
pub struct Policy {
    pub mode: Mode,
    pub ranges: BTreeMap<ItemId, Vec<TimeRange>>,
    pub edit_families: BTreeSet<EditFamily>,
    pub max_edits: usize,
    pub max_changed_items: usize,
}

impl Policy {
    pub(crate) fn permits_edits(&self, sequence: ItemId, edits: &[TimelineEdit]) -> Result<(), ServiceError> {
        if !self.ranges.contains_key(&sequence) || edits.iter().any(|edit| !self.edit_families.contains(&EditFamily::from(edit))) {
            return Err(error(ErrorCode::PermissionDenied, "sequence or command family is outside the agent policy"));
        }
        if edits.len() > self.max_edits {
            return Err(error(ErrorCode::ResourceLimit, "transaction exceeds the agent command limit"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReviewReason {
    AskMode,
    OutsideTaskRange,
    InconclusiveSemantics,
}

/// A snapshot of the draft the host can show to the reviewer.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRequest {
    pub project_id: DocumentId,
    pub transaction_id: DocumentId,
    #[serde(with = "crate::service::item_id")]
    pub sequence_id: ItemId,
    #[serde(with = "shotclutch_format::session::decimal_u64")]
    pub base_revision: u64,
    pub expires_at: i64,
    pub mode: Mode,
    pub commands: Vec<TimelineEdit>,
    pub task_ranges: Vec<TimeRange>,
    pub affected: Vec<TimeRange>,
    pub changed_items: usize,
    pub reasons: Vec<ReviewReason>,
    pub verification: Option<Outcome>,
    pub may_approve: bool,
    /// Zero-based indices in the current verification report.
    pub semantic_limitations: Vec<usize>,
    pub review_digest: String,
}

/// A decision attested by the trusted host after displaying a review request.
#[derive(Clone, Debug)]
pub struct HumanApproval {
    pub review_digest: String,
    pub reviewer: String,
    pub accepted_semantic_limitations: Vec<usize>,
    /// Additional timeline authority for this draft alone, from the same host.
    pub additional_ranges: Vec<TimeRange>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RecordedApproval {
    pub reviewer: String,
    pub review_digest: String,
    pub accepted_semantic_limitations: Vec<usize>,
    pub additional_ranges: Vec<TimeRange>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Decision {
    pub mode: Mode,
    pub reasons: Vec<ReviewReason>,
    pub affected: Vec<TimeRange>,
    pub task_ranges: Vec<TimeRange>,
    pub changed_items: usize,
    pub approval: Option<RecordedApproval>,
}

pub(crate) fn valid_ranges(ranges: &[TimeRange]) -> bool {
    ranges.len() <= MAX_RANGES && ranges.iter().all(|r| r.start >= Tick::ZERO && !r.is_empty() && r.start.0.checked_add(r.duration.0).is_some())
}

pub(crate) fn contains_all(scope: &[TimeRange], affected: &[TimeRange]) -> bool {
    let scope = merge(scope.iter().copied());
    affected.iter().all(|r| scope.iter().any(|s| s.start <= r.start && r.end() <= s.end()))
}

/// An incomplete technical check or missing render evidence cannot be accepted by a reviewer.
pub(crate) fn reviewable(report: &Report) -> Option<Vec<usize>> {
    if report.criteria.is_empty() || !report.gaps.is_empty() || report.outcome == Outcome::Failed {
        return None;
    }
    let mut limitations = Vec::new();
    let mut technical_passed = false;
    let staged_ranges = || report.affected.iter().filter(|item| item.snapshot == Snapshot::Staged).map(|item| item.range);
    for (index, result) in report.criteria.iter().enumerate() {
        match (&result.expected, result.outcome) {
            (Criterion::Semantic { check }, Outcome::Inconclusive) => {
                let has_evidence = match check {
                    SemanticCheck::SubjectVisible | SemanticCheck::TitleReadable => {
                        report.frames.iter().any(|frame| frame.snapshot == Snapshot::Staged && staged_ranges().any(|range| range.contains(frame.time)))
                    }
                    SemanticCheck::SpeechIntelligible => report.audio.iter().any(|audio| {
                        if audio.snapshot != Snapshot::Staged || audio.sample_count == 0 || report.render.sample_rate == 0 {
                            return false;
                        }
                        let rate = i64::from(report.render.sample_rate);
                        let end = audio.first_sample.saturating_add(i64::try_from(audio.sample_count).unwrap_or(i64::MAX));
                        let window = TimeRange::from_bounds(Tick::from_units(audio.first_sample, rate), Tick::from_units(end, rate));
                        staged_ranges().any(|range| range.overlaps(&window))
                    }),
                };
                if !has_evidence {
                    return None;
                }
                limitations.push(index);
            }
            (Criterion::Semantic { .. }, Outcome::Passed) => {}
            (_, Outcome::Passed) => technical_passed = true,
            _ => return None,
        }
    }
    match report.outcome {
        Outcome::Passed if limitations.is_empty() => Some(limitations),
        Outcome::Inconclusive if technical_passed && !limitations.is_empty() => Some(limitations),
        _ => None,
    }
}
