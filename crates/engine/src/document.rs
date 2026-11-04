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
    pub operations: Vec<shotclutch_format::session::OperationRecord>,
}

impl Snapshot {
    /// Preserve semantic file references when saving into another folder. Prepare only:
    /// the caller publishes this snapshot and adopts paths/caches after the write succeeds.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn for_destination(&mut self, destination: &str, pool: &crate::MediaPool) -> Result<std::collections::BTreeSet<String>, String> {
        use shotclutch_project::{ItemId, MediaRef};
        use std::collections::BTreeSet;
        use std::path::Path;
        let old_document = self.path.as_deref().map(Path::new);
        let mut paths = BTreeSet::new();
        let mut rebase = |project: &Arc<Project>, historical: bool| -> Result<Arc<Project>, String> {
            let mut changes: Vec<(ItemId, bool, String)> = Vec::new();
            for item in project.items.values() {
                let Some(media) = item.as_media() else { continue };
                for (reference, proxy) in std::iter::once((&media.media, false)).chain(media.proxy.as_ref().map(|r| (r, true))) {
                    let MediaRef::File { path } = reference else { continue };
                    // Absolute paths/URLs are already location independent. Do not reinterpret
                    // opaque URL/blob names or refused schemes as local filesystem paths.
                    if !craft_docpath::resolve(path, None).is_ok_and(|p| p.is_relative()) {
                        continue;
                    }
                    // A first save gives an unbound authored relative reference its document
                    // folder. Only an actual user-picked binding has a CWD meaning to preserve.
                    if old_document.is_none() && !pool.is_stored(path) && !pool.has_bound_file(item.id, path, proxy) {
                        continue;
                    }
                    let actual = if historical && old_document.is_some() && !pool.is_stored(path) {
                        crate::doc_path::resolve(path, old_document)
                    } else {
                        pool.follow(path)
                    }
                    .map_err(|e| format!("cannot preserve media reference `{path}`: {e}; relink before saving"))?;
                    let actual = if Path::new(&actual).is_relative() {
                        std::path::absolute(actual).map_err(|e| e.to_string())?.into_os_string().into_string().map_err(|_| "source path is not UTF-8")?
                    } else {
                        actual
                    };
                    let at_destination = crate::doc_path::resolve(path, Some(Path::new(destination)));
                    if at_destination.as_ref().is_ok_and(|p| Path::new(p) == Path::new(&actual)) {
                        continue;
                    }
                    paths.insert(actual.clone());
                    changes.push((item.id, proxy, actual));
                }
            }
            if changes.is_empty() {
                return Ok(project.clone());
            }
            let mut project = (**project).clone();
            for (id, proxy, path) in changes {
                let media = project.item_mut(id).and_then(|i| i.as_media_mut()).ok_or("media disappeared while preparing save")?;
                if proxy {
                    media.proxy = Some(MediaRef::File { path });
                } else {
                    media.media = MediaRef::File { path };
                }
            }
            Ok(Arc::new(project))
        };
        self.project = rebase(&self.project, false)?;
        for (_, project) in self.history.undo.iter_mut().chain(&mut self.history.redo) {
            *project = rebase(project, true)?;
        }
        Ok(paths)
    }

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
            version: 2,
            project_id: self.project_id,
            revision: self.revision,
            next_id: self.next_id.max(self.project.next_id),
            undo: entries(&self.history.undo)?,
            redo: entries(&self.history.redo)?,
            operations: self.operations.clone(),
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
    pub operations: Vec<shotclutch_format::session::OperationRecord>,
}

/// Validate every retained snapshot before changing the live session. Never recover a project
/// while silently discarding corrupt/unsupported history or permitting recursive history files.
pub(crate) fn restore(state: Option<DocumentState>, project: &Project) -> Result<Restored, String> {
    let Some(state) = state else {
        return Ok(Restored {
            id: DocumentId::default(),
            revision: 1,
            next_id: project.next_id,
            history: History { limit: 200, ..Default::default() },
            operations: Vec::new(),
        });
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
    crate::service::validate_records(&state.operations, state.project_id)?;
    Ok(Restored {
        id: state.project_id,
        revision: state.revision,
        next_id,
        history: History { undo, redo, limit: 200, merge_key: None },
        operations: state.operations,
    })
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
            operations: self.operation_records.clone(),
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
