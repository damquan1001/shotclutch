//! Shared save/recovery snapshot encoding. Editorial history is data, not a command replay.

use std::sync::Arc;

use shotclutch_format::session::{DocumentId, DocumentState, HistorySnapshot, RecoveryInfo};
use shotclutch_project::{Project, ProjectView};

use crate::{History, Session};

#[derive(Clone)]
pub(crate) struct Snapshot {
    pub project: Arc<Project>,
    pub project_id: DocumentId,
    pub revision: u64,
    pub next_id: u64,
    pub history: History,
    pub path: Option<String>,
    pub legacy_identity: bool,
    pub loaded_schema: u32,
    pub view: ProjectView,
}

impl Snapshot {
    pub fn encode(&self, view: Option<&ProjectView>, recovery: bool) -> Result<Vec<u8>, String> {
        if self.revision == 0 || self.revision == u64::MAX {
            return Err("RESOURCE_LIMIT: document revision cannot be persisted".into());
        }
        let entries = |states: &[(String, Arc<Project>)]| -> Result<Vec<HistorySnapshot>, String> {
            states
                .iter()
                .rev()
                .take(200)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|(label, project)| {
                    let snapshot = serde_json::from_slice(&shotclutch_format::encode(project, false)).map_err(|e| e.to_string())?;
                    Ok(HistorySnapshot { label: label.clone(), snapshot })
                })
                .collect()
        };
        let state = DocumentState {
            version: 1,
            project_id: self.project_id,
            revision: self.revision,
            next_id: self.next_id.max(self.project.next_id),
            undo: entries(&self.history.undo)?,
            redo: entries(&self.history.redo)?,
        };
        let info = RecoveryInfo { project_path: self.path.clone(), legacy_identity: self.legacy_identity, source_schema: self.loaded_schema };
        let mut project = (*self.project).clone();
        project.next_id = state.next_id;
        let bytes = shotclutch_format::encode_document(&project, view.or(Some(&self.view)), Some(&state), recovery.then_some(&info), false);
        if bytes.is_empty() {
            return Err("could not serialize document snapshot".into());
        }
        Ok(bytes)
    }
}

pub(crate) struct Restored {
    pub id: DocumentId,
    pub revision: u64,
    pub next_id: u64,
    pub history: History,
}

/// Validate every retained snapshot before changing the live session. Never recover a project
/// while silently discarding corrupt/unsupported history or permitting recursive history files.
pub(crate) fn restore(state: Option<DocumentState>, project: &Project) -> Result<Restored, String> {
    let Some(state) = state else {
        return Ok(Restored { id: DocumentId::default(), revision: 1, next_id: project.next_id, history: History { limit: 200, ..Default::default() } });
    };
    let mut next_id = state.next_id.max(project.next_id);
    let mut entries = |snapshots: Vec<HistorySnapshot>| -> Result<Vec<(String, Arc<Project>)>, String> {
        let mut out = Vec::with_capacity(snapshots.len());
        for entry in snapshots {
            let bytes = serde_json::to_vec(&entry.snapshot).map_err(|e| e.to_string())?;
            let loaded = shotclutch_format::decode(&bytes).map_err(|e| format!("invalid history snapshot: {e}"))?;
            if loaded.edit_state.is_some() || loaded.recovery.is_some() {
                return Err("history snapshots cannot contain nested document state".into());
            }
            next_id = next_id.max(loaded.project.next_id);
            out.push((entry.label, Arc::new(loaded.project)));
        }
        Ok(out)
    };
    let undo = entries(state.undo)?;
    let redo = entries(state.redo)?;
    Ok(Restored { id: state.project_id, revision: state.revision, next_id, history: History { undo, redo, limit: 200, merge_key: None } })
}

impl Session {
    pub(crate) fn document_snapshot(&self) -> Snapshot {
        Snapshot {
            project: self.project.clone(),
            project_id: self.document_id,
            revision: self.revision,
            next_id: self.id_watermark.max(self.project.next_id),
            history: self.history.clone(),
            path: self.path.clone(),
            legacy_identity: self.legacy_project_identity,
            loaded_schema: self.loaded_schema,
            view: self.project_view(),
        }
    }
}

#[cfg(test)]
thread_local! {
    pub(crate) static CRASH_AFTER_COMMIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(crate) fn after_durable_commit() {
    if CRASH_AFTER_COMMIT.with(|fault| fault.replace(false)) {
        panic!("injected crash after durable snapshot, before publication");
    }
}
