//! Isolated, revision-checked structural previews of timeline edits.
//!
//! This is the first M1 transaction slice. Commands use the existing edit service; the staged
//! session has no persistence worker. A preview is process-local and cannot be deserialized or
//! used to execute file/network operations. Durable history and adapter grants remain separate
//! work before this can be exposed as the complete external transaction contract.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use shotclutch_project::{ClipId, ItemId, Project};

use crate::{Result, Session};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    In,
    Out,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrimMode {
    Regular,
    Ripple,
}

/// Exact times are decimal strings at this API boundary, never JSON floating-point numbers.
/// Targets are explicit; the caller's selection and linked-selection preference are not inputs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "camelCase", deny_unknown_fields)]
pub enum TimelineEdit {
    Trim { clip: ClipId, edge: Edge, mode: TrimMode, delta: String, linked: bool },
    Split { clips: Vec<ClipId>, time: String },
}

#[derive(Debug, thiserror::Error)]
pub enum TransactionError {
    #[error("REVISION_CONFLICT: preview belongs to an earlier project state")]
    RevisionConflict,
    #[error("INVALID_ARGUMENT: {0}")]
    InvalidArgument(String),
    #[error("VERIFICATION_FAILED: {0}")]
    VerificationFailed(String),
}

/// An immutable structural preview. Drop it to abort. Its project can be rendered using the
/// same snapshot readers as ordinary preview/export. Only the engine can construct one.
pub struct PreparedEdit {
    base: Arc<Project>,
    revision: u64,
    project: Arc<Project>,
    label: String,
    results: Vec<Value>,
}

impl PreparedEdit {
    pub fn project(&self) -> &Arc<Project> {
        &self.project
    }

    pub fn base_revision(&self) -> u64 {
        self.revision
    }

    pub fn results(&self) -> &[Value] {
        &self.results
    }
}

fn exact_tick(value: &str) -> std::result::Result<i64, TransactionError> {
    let n = value.parse::<i64>().map_err(|_| TransactionError::InvalidArgument("time must be an i64 decimal string".into()))?;
    if n.to_string() != value {
        return Err(TransactionError::InvalidArgument("time must use canonical decimal notation".into()));
    }
    Ok(n)
}

impl Session {
    /// Stage supported timeline edits on a private project and editor state. No history,
    /// persistence notifications, project events or live-preview updates reach the caller.
    pub fn prepare_edits(
        &self,
        expected_revision: u64,
        sequence: ItemId,
        label: &str,
        edits: &[TimelineEdit],
    ) -> std::result::Result<PreparedEdit, TransactionError> {
        if self.revision != expected_revision {
            return Err(TransactionError::RevisionConflict);
        }
        if edits.is_empty() || edits.len() > 128 || label.is_empty() || label.len() > 200 {
            return Err(TransactionError::InvalidArgument("provide 1–128 edits and a label of 1–200 bytes".into()));
        }
        if self.project.sequence(sequence).is_none() {
            return Err(TransactionError::InvalidArgument("sequence not found".into()));
        }
        let mut staged = Session::new(self.services.clone());
        staged.project = self.project.clone();
        staged.media = self.media.clone();
        staged.revision = self.revision;
        staged.state = self.state.clone();
        staged.state.active_sequence = Some(sequence);
        let mut results = Vec::with_capacity(edits.len());
        for edit in edits {
            let r: Result<Value> = match edit {
                TimelineEdit::Trim { clip, edge, mode, delta, linked } => {
                    let delta = exact_tick(delta)?;
                    let q = staged.active_sequence().ok_or_else(|| TransactionError::InvalidArgument("sequence not found".into()))?;
                    let (track, item) = q.find_item(*clip).ok_or_else(|| TransactionError::InvalidArgument("trim clip not found".into()))?;
                    if q.track(track).is_none_or(|t| t.locked) {
                        return Err(TransactionError::InvalidArgument("trim target is locked".into()));
                    }
                    if *linked
                        && let Some(link) = item.link
                        && q.all_tracks().any(|t| t.locked && t.items.iter().any(|i| i.link == Some(link)))
                    {
                        return Err(TransactionError::InvalidArgument("linked trim partner is locked".into()));
                    }
                    staged.execute("timeline.trim", json!({"clip": clip.0, "edge": edge, "mode": mode, "delta": delta, "linked": linked}))
                }
                TimelineEdit::Split { clips, time } => {
                    let time = exact_tick(time)?;
                    if clips.is_empty() || clips.len() > 1024 {
                        return Err(TransactionError::InvalidArgument("split requires 1–1024 explicit clip IDs".into()));
                    }
                    let q = staged.active_sequence().ok_or_else(|| TransactionError::InvalidArgument("sequence not found".into()))?;
                    for id in clips {
                        let (track, item) = q.find_item(*id).ok_or_else(|| TransactionError::InvalidArgument("split clip not found".into()))?;
                        if q.track(track).is_none_or(|t| t.locked) || !(item.start.0 < time && time < item.range().end().0) {
                            return Err(TransactionError::InvalidArgument("split must be inside each unlocked clip".into()));
                        }
                    }
                    staged.state.selection = clips.clone();
                    staged.execute("sequence.addEdit", json!({"time": time}))
                }
            };
            results.push(r.map_err(|e| TransactionError::VerificationFailed(e.to_string()))?);
        }
        Ok(PreparedEdit { base: self.project.clone(), revision: self.revision, project: staged.project.clone(), label: label.into(), results })
    }

    /// Compare-and-swap the reviewed snapshot into this session as one undo item. An intervening
    /// edit or project replacement rejects the preview, including a switch to another document.
    pub fn commit_edits(&mut self, prepared: PreparedEdit) -> std::result::Result<Vec<Value>, TransactionError> {
        if self.revision != prepared.revision || !Arc::ptr_eq(&self.project, &prepared.base) {
            return Err(TransactionError::RevisionConflict);
        }
        let project = (*prepared.project).clone();
        self.edit(&prepared.label, |p, _| {
            *p = project;
            Ok(())
        })
        .map_err(|e| TransactionError::VerificationFailed(e.to_string()))?;
        // Keep the user's current view/selection; staging uses an explicit sequence and targets.
        self.fix_state();
        self.sync_persistence();
        self.previews.live.publish_project(self.project.clone());
        Ok(prepared.results)
    }
}

#[cfg(test)]
#[path = "m1_transaction_tests.rs"]
mod m1_transaction_tests;
