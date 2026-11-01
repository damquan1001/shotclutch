//! Isolated, revision-checked structural previews of timeline edits.
//!
//! Commands use the existing edit service; the staged session has no persistence worker. A
//! preview is process-local and cannot be deserialized or execute file/network operations.
//! Native durable commits acknowledge the atomic project/history snapshot before publication.
//! The service module adds scoped grants, receipts and source revalidation for the bounded
//! external structural contract. Rendered verification remains a separate policy.

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
    Trim {
        #[serde(with = "wire_clip")]
        clip: ClipId,
        edge: Edge,
        mode: TrimMode,
        delta: String,
        linked: bool,
    },
    Split {
        #[serde(with = "wire_clips")]
        clips: Vec<ClipId>,
        time: String,
    },
}

mod wire_clip {
    use serde::{Deserializer, Serializer};
    use shotclutch_project::{ClipId, ItemId};
    pub fn serialize<S: Serializer>(id: &ClipId, s: S) -> Result<S::Ok, S::Error> {
        crate::service::item_id::serialize(&ItemId(id.0), s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<ClipId, D::Error> {
        crate::service::item_id::deserialize(d).map(|id| ClipId(id.0))
    }
}
mod wire_clips {
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
    use shotclutch_project::ClipId;
    pub fn serialize<S: Serializer>(ids: &[ClipId], s: S) -> Result<S::Ok, S::Error> {
        ids.iter().map(|id| id.0.to_string()).collect::<Vec<_>>().serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<ClipId>, D::Error> {
        Vec::<String>::deserialize(d)?
            .into_iter()
            .map(|text| {
                let id = text.parse::<u64>().map_err(D::Error::custom)?;
                if text != id.to_string() {
                    return Err(D::Error::custom("expected a canonical decimal clip ID string"));
                }
                Ok(ClipId(id))
            })
            .collect()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TransactionError {
    #[error("REVISION_CONFLICT: preview belongs to an earlier project state")]
    RevisionConflict,
    #[error("INVALID_ARGUMENT: {0}")]
    InvalidArgument(String),
    #[error("VERIFICATION_FAILED: {0}")]
    VerificationFailed(String),
    #[error("INTERNAL_ERROR: durable commit failed: {0}")]
    PersistenceFailed(String),
    #[error("CAPABILITY_UNAVAILABLE: {0}")]
    CapabilityUnavailable(&'static str),
    #[error("CANCELED: transaction canceled before commit")]
    Canceled,
    #[error("PERMISSION_DENIED: authorization was revoked or expired")]
    PermissionDenied,
}

/// An immutable structural preview. Drop it to abort. Its project can be rendered using the
/// same snapshot readers as ordinary preview/export. Only the engine can construct one.
pub struct PreparedEdit {
    base: Arc<Project>,
    revision: u64,
    project: Arc<Project>,
    label: String,
    results: Vec<Value>,
    pub(crate) operation: Option<shotclutch_format::session::OperationRecord>,
}

impl PreparedEdit {
    pub(crate) fn is_current(&self, session: &Session) -> bool {
        self.revision == session.revision && Arc::ptr_eq(&self.base, &session.project)
    }
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
        self.prepare_edits_guarded(expected_revision, sequence, label, edits, || Ok(()))
    }

    pub(crate) fn prepare_edits_guarded(
        &self,
        expected_revision: u64,
        sequence: ItemId,
        label: &str,
        edits: &[TimelineEdit],
        check: impl Fn() -> std::result::Result<(), TransactionError>,
    ) -> std::result::Result<PreparedEdit, TransactionError> {
        check()?;
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
        staged.document_id = self.document_id;
        staged.id_watermark = self.id_watermark;
        staged.media = self.media.clone();
        staged.revision = self.revision;
        staged.state = self.state.clone();
        staged.state.active_sequence = Some(sequence);
        let mut results = Vec::with_capacity(edits.len());
        for edit in edits {
            check()?;
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
        Ok(PreparedEdit { base: self.project.clone(), revision: self.revision, project: staged.project.clone(), label: label.into(), results, operation: None })
    }

    /// Compare-and-swap the reviewed snapshot into this session as one undo item. An intervening
    /// edit or project replacement rejects the preview, including a switch to another document.
    /// A configured recovery worker is acknowledged before publication. Without one this is an
    /// in-memory commit; callers needing a durability guarantee must use `commit_edits_durable`.
    pub fn commit_edits(&mut self, prepared: PreparedEdit) -> std::result::Result<Vec<Value>, TransactionError> {
        if self.revision != prepared.revision || !Arc::ptr_eq(&self.project, &prepared.base) {
            return Err(TransactionError::RevisionConflict);
        }
        self.ensure_revision_available().map_err(|e| TransactionError::InvalidArgument(e.to_string()))?;
        if self.edit_group_depth != 0 {
            return Err(TransactionError::InvalidArgument("transaction commits cannot be nested in command groups".into()));
        }
        if self.trim_play.active() {
            return Err(TransactionError::RevisionConflict);
        }
        self.refuse_self_nesting(&prepared.project).map_err(|e| TransactionError::VerificationFailed(e.to_string()))?;
        let mut snapshot = self.document_snapshot();
        if let Some(operation) = prepared.operation {
            snapshot.operations.retain(|r| r.retained_until > crate::autosave::unix_now());
            if snapshot.operations.len() >= 128 {
                return Err(TransactionError::InvalidArgument("RESOURCE_LIMIT: retained operation receipts are full".into()));
            }
            snapshot.operations.push(operation);
        }
        snapshot.project = prepared.project;
        snapshot.revision = self.revision.checked_add(1).ok_or_else(|| TransactionError::InvalidArgument("revision exhausted".into()))?;
        snapshot.next_id = snapshot.next_id.max(snapshot.project.next_id);
        snapshot.history.undo.push((prepared.label, self.project.clone()));
        if snapshot.history.undo.len() > snapshot.history.limit {
            snapshot.history.undo.remove(0);
        }
        snapshot.history.redo.clear();
        snapshot.history.merge_key = None;
        // No canonical state, history item or event is published until the worker acknowledges
        // the atomic recovery snapshot containing both the complete edit and its inverse.
        if let Some(persistence) = &self.persistence {
            persistence.commit_snapshot(snapshot.clone()).map_err(TransactionError::PersistenceFailed)?;
        }
        #[cfg(test)]
        crate::document::after_durable_commit();
        self.project = snapshot.project;
        self.history = snapshot.history;
        self.operation_records = snapshot.operations;
        self.id_watermark = snapshot.next_id;
        self.bump();
        // Keep the user's current view/selection; staging uses an explicit sequence and targets.
        self.fix_state();
        self.sync_persistence();
        self.previews.live.publish_project(self.project.clone());
        Ok(prepared.results)
    }

    /// Fail closed when the host cannot acknowledge durable storage. Suitable for native
    /// service adapters once their independent permission/idempotency policies are implemented.
    pub fn commit_edits_durable(&mut self, prepared: PreparedEdit) -> std::result::Result<Vec<Value>, TransactionError> {
        if self.persistence.is_none() || !self.prefs.auto_save.recovery_journal {
            return Err(TransactionError::CapabilityUnavailable("an enabled recovery worker is required for durable commit"));
        }
        self.commit_edits(prepared)
    }
}

#[cfg(test)]
#[path = "m1_transaction_tests.rs"]
mod m1_transaction_tests;
