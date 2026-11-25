//! Owner-only inventory and logical deletion of local project data.
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shotclutch_format::session::DocumentId;
use shotclutch_project::{ItemId, Project};

use crate::autosave;
use crate::service::runs;
use crate::{EngineError, Result, Session};

pub const DELETION_SCOPE: &str = "Logical removal from managed local storage. Originals, exports, independent copies, provider copies, and generated media without verified ownership may remain.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deletion {
    Disposable,
    Analysis { asset: ItemId, include_dependencies: bool },
    Project,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize)]
pub struct Inventory {
    pub transcripts: usize,
    pub observations: usize,
    pub annotations: usize,
    pub rough_cut_analyses: usize,
    pub rough_cut_maps: usize,
    pub reviewed_or_applied_maps: usize,
    pub active_or_resumable_runs: usize,
    pub finished_runs: usize,
    pub undo_redo: usize,
    pub retained_receipts: usize,
    pub recovery_copies: usize,
    pub unverified_snapshot_files: usize,
    pub preview_cache_entries: usize,
    pub generated_narrations: usize,
    pub dependent_caption_tracks: usize,
    pub preserved_caption_tracks: usize,
}

impl Inventory {
    fn analysis(&mut self, project: &Project, asset: Option<ItemId>) {
        let selected = |id| asset.is_none_or(|asset| asset == id);
        self.transcripts += project.transcripts.keys().filter(|id| selected(**id)).count();
        for (_, observation) in project.observations.iter().filter(|(_, obs)| selected(obs.evidence.asset)) {
            self.observations += 1;
            self.annotations += usize::from(matches!(observation.provenance, shotclutch_project::observations::Provenance::Human { .. }));
        }
        let analyses = project.preedit.analyses.iter().filter_map(|(id, analysis)| selected(analysis.source.item).then_some(*id)).collect::<BTreeSet<_>>();
        self.rough_cut_analyses += analyses.len();
        for (id, map) in project.preedit.maps.iter().filter(|(_, map)| analyses.contains(&map.analysis)) {
            self.rough_cut_maps += 1;
            self.reviewed_or_applied_maps += usize::from(map.reviewed || project.preedit.applications.values().any(|application| application.map == *id));
        }
        for (sequence, owned) in caption_tracks(project, &analyses) {
            let tracks = project.sequence(sequence).map_or(0, |seq| seq.caption_tracks.len());
            self.dependent_caption_tracks += tracks;
            self.preserved_caption_tracks += if owned { 0 } else { tracks };
        }
    }

    fn has_analysis(&self) -> bool {
        self.transcripts != 0 || self.observations != 0 || self.rough_cut_analyses != 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ManagedCopy {
    path: PathBuf,
    parent_identity: Option<PathBuf>,
    fingerprint: [u8; 32],
    lock: Option<PathBuf>,
}

#[derive(Serialize, Deserialize)]
struct PendingDeletion {
    project: DocumentId,
    floor: u64,
    saved: ManagedCopy,
    /// Analysis rewrites the primary document before any old copies are deleted.
    /// Project deletion uses the original primary file as the final removal.
    replacement: Option<[u8; 32]>,
    copies: Vec<ManagedCopy>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeletionPreview {
    pub action: Deletion,
    pub current: Inventory,
    pub history: Inventory,
    pub retained: Inventory,
    /// Local desktop metadata only. Diagnostic exports use counts exclusively.
    pub managed_files: Vec<PathBuf>,
    pub blocked: Option<&'static str>,
    project: DocumentId,
    revision: u64,
    saved_file: Option<ManagedCopy>,
    copies: Vec<ManagedCopy>,
}

#[derive(Debug)]
pub struct DeleteResult {
    pub removed_files: usize,
    pub preserved_caption_tracks: usize,
    pub remaining: Inventory,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionPrefs {
    /// Zero keeps completed checkpoints until explicitly removed.
    pub completed_run_days: u16,
}

const PROJECT_FILE_LIMIT: u64 = 256 * 1024 * 1024;
const DELETION_LIMIT: u64 = 256 * 1024;

pub(crate) fn file_digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn failure(message: &'static str) -> EngineError {
    EngineError::Other(message.into())
}

fn regular_bytes(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.len() > limit {
        return Err(io::Error::other("not a bounded regular file"));
    }
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(io::Error::other("file exceeds its inventory limit"));
    }
    Ok(bytes)
}

fn managed_copy(path: PathBuf, lock: Option<PathBuf>, bytes: &[u8]) -> ManagedCopy {
    let path = std::path::absolute(&path).unwrap_or(path);
    ManagedCopy {
        parent_identity: path.parent().and_then(|parent| fs::canonicalize(parent).ok()),
        path,
        fingerprint: Sha256::digest(bytes).into(),
        lock: lock.map(|path| std::path::absolute(&path).unwrap_or(path)),
    }
}

fn remove_copy(copy: &ManagedCopy) -> io::Result<bool> {
    let parent = copy.path.parent().ok_or_else(|| io::Error::other("missing parent"))?;
    let name = copy.path.file_name().ok_or_else(|| io::Error::other("missing file name"))?;
    let real_parent = match fs::canonicalize(parent) {
        Ok(parent) => parent,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if copy.parent_identity.as_ref() != Some(&real_parent) {
        return Err(io::Error::other("the reviewed folder moved"));
    }
    let directory = match cap_std::fs::Dir::open_ambient_dir(&real_parent, cap_std::ambient_authority()) {
        Ok(directory) => directory,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    let meta = match directory.symlink_metadata(name) {
        Ok(meta) => meta,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if !meta.is_file() || meta.len() > PROJECT_FILE_LIMIT {
        return Err(io::Error::other("managed file changed type or size"));
    }
    let mut bytes = Vec::new();
    directory.open(name)?.take(PROJECT_FILE_LIMIT + 1).read_to_end(&mut bytes)?;
    if copy.fingerprint != <[u8; 32]>::from(Sha256::digest(&bytes)) {
        return Err(io::Error::other("the reviewed file changed"));
    }
    directory.remove_file(name)?;
    Ok(true)
}

impl ManagedCopy {
    fn pinned_path(&self) -> io::Result<PathBuf> {
        let parent = self.parent_identity.as_ref().ok_or_else(|| io::Error::other("the reviewed directory is unavailable"))?;
        let name = self.path.file_name().ok_or_else(|| io::Error::other("the reviewed name is unavailable"))?;
        Ok(parent.join(name))
    }
}

fn lock_copy(copy: &ManagedCopy) -> io::Result<Option<File>> {
    copy.lock
        .as_ref()
        .map(|path| {
            if !fs::symlink_metadata(path)?.is_file() {
                return Err(io::Error::other("invalid managed lock"));
            }
            let lock = File::options().read(true).write(true).open(path)?;
            lock.try_lock().map_err(|_| io::Error::other("managed copy is in use"))?;
            Ok(lock)
        })
        .transpose()
}

/// Serialize privacy actions and disk publication across desktop instances sharing recovery storage.
pub(crate) fn storage_lock(data_dir: &Path, id: DocumentId) -> io::Result<File> {
    let dir = data_dir.join("Privacy");
    fs::create_dir_all(&dir)?;
    let file = File::options().create(true).truncate(false).read(true).write(true).open(dir.join(format!("{id}.lock")))?;
    file.lock()?;
    Ok(file)
}

fn floor_path(data_dir: &Path, id: DocumentId) -> PathBuf {
    data_dir.join("Privacy").join(format!("{id}.revision"))
}

fn pending_path(data_dir: &Path, id: DocumentId) -> PathBuf {
    data_dir.join("Privacy").join(format!("{id}.deletion"))
}

fn has_pending(data_dir: &Path, id: DocumentId) -> bool {
    !matches!(fs::symlink_metadata(pending_path(data_dir, id)), Err(error) if error.kind() == io::ErrorKind::NotFound)
}

impl PendingDeletion {
    fn prepare(&self, data_dir: &Path) -> io::Result<()> {
        let bytes = serde_json::to_vec(self)?;
        if bytes.len() as u64 > DELETION_LIMIT {
            return Err(io::Error::other("too many managed files for one deletion"));
        }
        shotclutch_format::atomic_write_new(&pending_path(data_dir, self.project), &bytes)
    }

    /// The caller holds the document publication lock and any individual file locks.
    fn finish(&self, data_dir: &Path) -> io::Result<usize> {
        let marker = pending_path(data_dir, self.project);
        if let Some(replacement) = self.replacement {
            let bytes = regular_bytes(&self.saved.pinned_path()?, PROJECT_FILE_LIMIT)?;
            let current = <[u8; 32]>::from(Sha256::digest(&bytes));
            if current == self.saved.fingerprint {
                // A crash before primary publication left the old project and its copies intact.
                fs::remove_file(marker)?;
                return Ok(0);
            }
            if current != replacement {
                return Err(io::Error::other("the document changed outside the approved deletion"));
            }
        } else if read_floor(data_dir, self.project).map_err(io::Error::other)? != self.floor {
            let bytes = regular_bytes(&self.saved.pinned_path()?, PROJECT_FILE_LIMIT)?;
            if <[u8; 32]>::from(Sha256::digest(&bytes)) != self.saved.fingerprint {
                return Err(io::Error::other("the document changed outside the approved deletion"));
            }
        }
        shotclutch_format::atomic_write(&floor_path(data_dir, self.project), self.floor.to_string().as_bytes())?;
        let mut removed = 0;
        for copy in &self.copies {
            removed += usize::from(remove_copy(copy)?);
        }
        if self.replacement.is_none() {
            removed += usize::from(remove_copy(&self.saved)?);
        }
        fs::remove_file(marker)?;
        Ok(removed)
    }
}

pub(crate) fn resume_pending_deletions(data_dir: &Path) {
    let Ok(entries) = fs::read_dir(data_dir.join("Privacy")) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(id) = path.file_stem().and_then(|stem| stem.to_str()).and_then(parse_document_id) else { continue };
        if path.extension().is_some_and(|extension| extension == "deletion") && resume_pending_deletion(data_dir, id).is_err() {
            log::warn!("A confirmed privacy deletion still has managed files waiting for cleanup.");
        }
    }
}

fn parse_document_id(value: &str) -> Option<DocumentId> {
    let id: DocumentId = serde_json::from_value(serde_json::Value::String(value.into())).ok()?;
    (id.to_string() == value).then_some(id)
}

fn resume_pending_deletion(data_dir: &Path, id: DocumentId) -> io::Result<usize> {
    let _project = storage_lock(data_dir, id)?;
    let pending: PendingDeletion = serde_json::from_slice(&regular_bytes(&pending_path(data_dir, id), DELETION_LIMIT)?)?;
    if pending.project != id {
        return Err(io::Error::other("cleanup record belongs to another project"));
    }
    let _copies = pending.copies.iter().filter(|copy| copy.path.exists()).map(lock_copy).collect::<io::Result<Vec<_>>>()?;
    pending.finish(data_dir)
}

fn read_floor(data_dir: &Path, id: DocumentId) -> std::result::Result<u64, String> {
    match regular_bytes(&floor_path(data_dir, id), 20) {
        Ok(value) => std::str::from_utf8(&value).ok().and_then(|value| value.parse().ok()).ok_or_else(|| "The project privacy revision is unreadable.".into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(0),
        _ => Err("The project privacy revision is unavailable.".into()),
    }
}

pub(crate) fn check_floor(data_dir: &Path, id: DocumentId, revision: u64) -> std::result::Result<u64, String> {
    if has_pending(data_dir, id) {
        return Err("A confirmed privacy deletion is waiting for cleanup. Retry cleanup in Privacy & Data or restart ShotClutch.".into());
    }
    let floor = read_floor(data_dir, id)?;
    if revision < floor {
        return Err("This project snapshot predates a local privacy deletion. Open the current project.".into());
    }
    Ok(floor)
}

pub(crate) fn publication_guard(data_dir: &Path, id: DocumentId, seen: u64) -> std::result::Result<File, String> {
    let lock = storage_lock(data_dir, id).map_err(|_| "Project privacy storage is unavailable.")?;
    check_floor(data_dir, id, seen)?;
    Ok(lock)
}

fn directories(root: &Path) -> impl Iterator<Item = PathBuf> {
    is_directory(root).then(|| fs::read_dir(root).ok()).flatten().into_iter().flatten().filter_map(|entry| {
        let entry = entry.ok()?;
        is_directory(&entry.path()).then(|| entry.path())
    })
}

fn is_directory(path: &Path) -> bool {
    let Ok(meta) = fs::symlink_metadata(path) else { return false };
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const REPARSE_POINT: u32 = 0x400;
        if meta.file_attributes() & REPARSE_POINT != 0 {
            return false;
        }
    }
    meta.is_dir()
}

impl Session {
    pub fn retention_preferences(&self) -> RetentionPrefs {
        self.prefs.retention
    }

    pub fn set_privacy_retention(&mut self, completed_run_days: u16, cache_days: u32) -> Result<()> {
        if completed_run_days > 3650 || cache_days > 3650 {
            return Err(failure("Enter a retention period between zero and 3650 days."));
        }
        let mut prefs = self.prefs.clone();
        prefs.retention.completed_run_days = completed_run_days;
        if cache_days != 0 {
            prefs.media_cache.management = "olderThan".into();
            prefs.media_cache.older_than_days = cache_days;
        } else if prefs.media_cache.management == "olderThan" {
            prefs.media_cache.management = "never".into();
        }
        self.set_prefs(prefs).map_err(|_| failure("Could not save retention preferences."))?;
        self.expire_completed_runs();
        Ok(())
    }

    fn run_copies(&self, asset: Option<ItemId>) -> Vec<(ManagedCopy, runs::Phase)> {
        let Some(storage) = &self.persistence else { return Vec::new() };
        let root = storage.data_dir.join("AgentRuns").join(self.document_id.to_string());
        if !is_directory(&storage.data_dir.join("AgentRuns")) || !is_directory(&root) {
            return Vec::new();
        }
        fs::read_dir(root)
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                if path.extension()?.to_str()? != "json" {
                    return None;
                }
                let id = parse_document_id(path.file_stem()?.to_str()?)?;
                let bytes = regular_bytes(&path, 512 * 1024).ok()?;
                let (phase, input_asset) = runs::retained_checkpoint(&path, self.document_id, id)?;
                if asset.is_some() && input_asset.is_some() && input_asset != asset {
                    return None;
                }
                let lock = path.with_extension("lock");
                Some((managed_copy(path, Some(lock), &bytes), phase))
            })
            .collect()
    }

    pub(crate) fn expire_completed_runs(&self) {
        let days = self.prefs.retention.completed_run_days;
        if days == 0 {
            return;
        }
        let age = std::time::Duration::from_secs(u64::from(days) * 86_400);
        for (copy, phase) in self.run_copies(None) {
            if phase.terminal()
                && fs::metadata(&copy.path).and_then(|meta| meta.modified()).ok().and_then(|time| time.elapsed().ok()).is_some_and(|elapsed| elapsed >= age)
                && let Ok(_lock) = lock_copy(&copy)
            {
                let _ = remove_copy(&copy);
            }
        }
    }

    pub fn privacy_inventory(&self) -> Inventory {
        self.preview_privacy_deletion(Deletion::Project).current
    }

    pub fn privacy_cleanup_pending(&self) -> bool {
        self.persistence.as_ref().is_some_and(|storage| has_pending(&storage.data_dir, self.document_id))
    }

    pub fn retry_privacy_cleanup(&mut self) -> Result<usize> {
        let storage = self.persistence.as_ref().ok_or_else(|| failure("Recovery storage is unavailable."))?;
        let data_dir = storage.data_dir.clone();
        let removed = resume_pending_deletion(&data_dir, self.document_id)
            .map_err(|_| failure("Some managed files are waiting for cleanup. Close other sessions and retry."))?;
        let floor = read_floor(&data_dir, self.document_id).map_err(EngineError::Other)?;
        if floor == u64::MAX {
            self.execute("file.newProject", serde_json::json!({}))?;
        } else if floor > self.revision {
            return Err(failure("Cleanup completed. Reopen the current project."));
        } else {
            self.privacy_floor_seen = floor;
        }
        self.sync_persistence();
        Ok(removed)
    }

    pub fn preview_privacy_deletion(&self, action: Deletion) -> DeletionPreview {
        let asset = match action {
            Deletion::Analysis { asset, .. } => Some(asset),
            _ => None,
        };
        let mut current = Inventory {
            undo_redo: self.history.undo.len() + self.history.redo.len(),
            retained_receipts: self.operation_records.len(),
            generated_narrations: self.project.narrations.len(),
            preview_cache_entries: self.previews.count(),
            ..Default::default()
        };
        current.analysis(&self.project, asset);
        let mut history = Inventory::default();
        for (_, state) in self.history.undo.iter().chain(&self.history.redo) {
            let mut counts = Inventory::default();
            counts.analysis(state, asset);
            history.undo_redo += usize::from(counts.has_analysis());
            history.analysis(state, asset);
        }
        let mut copies = Vec::new();
        let mut blocked = None;
        let mut saved_file = None;
        if action != Deletion::Disposable {
            saved_file = self.path.as_deref().and_then(|path| {
                let path = PathBuf::from(path);
                let bytes = regular_bytes(&path, PROJECT_FILE_LIMIT).ok()?;
                let loaded = shotclutch_format::decode(&bytes).ok()?;
                let state = loaded.edit_state?;
                (!self.legacy_project_identity
                    && state.project_id == self.document_id
                    && state.revision <= self.revision
                    && Some(file_digest(&bytes)) == self.privacy_saved_fingerprint)
                    .then(|| managed_copy(path, None, &bytes))
            });
            if saved_file.is_none() || self.persistence.is_none() {
                blocked = Some(
                    "Permanent deletion requires host recovery storage and the current saved native project. Reopen the project if another writer changed it.",
                );
            }
            let dir = crate::commands::auto_save_dir(self);
            let mut paths = BTreeSet::new();
            if let Some(path) = &self.path {
                for schema in 1..shotclutch_format::SCHEMA_VERSION {
                    let backup = PathBuf::from(crate::commands::schema_backup_path(path, schema));
                    if backup.exists() {
                        paths.insert((backup, None));
                    }
                }
                for dir in [dir, shotclutch_format::autosave::auto_save_dir(Path::new(path))] {
                    if is_directory(&dir) {
                        for path in shotclutch_format::autosave::list_auto_saves(&dir, &crate::commands::auto_save_name(self)) {
                            paths.insert((path, None));
                        }
                    }
                }
            }
            if let Some(storage) = &self.persistence {
                for dir in directories(&autosave::recovery_root(&storage.data_dir)) {
                    let snapshot = autosave::candidate_snapshot(&dir);
                    if snapshot.exists() {
                        let lock = (dir != storage.session_dir).then(|| dir.join("lock"));
                        paths.insert((snapshot, lock));
                    }
                }
            }
            for (path, lock) in paths {
                let loaded = regular_bytes(&path, PROJECT_FILE_LIMIT).ok().and_then(|bytes| Some((shotclutch_format::decode(&bytes).ok()?, bytes)));
                let Some((loaded, bytes)) = loaded else {
                    current.unverified_snapshot_files += 1;
                    continue;
                };
                let Some(state) = loaded.edit_state else {
                    current.unverified_snapshot_files += 1;
                    continue;
                };
                if state.project_id != self.document_id {
                    continue;
                }
                if let Some(asset) = asset
                    && !contains_analysis(&loaded.project, asset)
                    && let Ok(restored) = crate::document::restore(Some(state.clone()), &loaded.project)
                    && !restored.history.undo.iter().chain(&restored.history.redo).any(|(_, project)| contains_analysis(project, asset))
                {
                    continue;
                }
                current.recovery_copies += 1;
                if state.revision > self.revision {
                    blocked = Some("A newer recovery copy exists. Recover the latest project before deleting data.");
                }
                copies.push(managed_copy(path, lock, &bytes));
            }
        }
        for (copy, phase) in self.run_copies(asset) {
            if phase.terminal() {
                current.finished_runs += 1;
            } else {
                current.active_or_resumable_runs += 1;
            }
            if action != Deletion::Disposable || phase.terminal() {
                copies.push(copy);
            }
        }
        if matches!(action, Deletion::Disposable | Deletion::Project) {
            copies.extend(self.preview_cache_copies());
        }
        copies.sort_by(|left, right| left.path.cmp(&right.path));
        let mut retained = Inventory::default();
        if matches!(action, Deletion::Analysis { include_dependencies: false, .. }) {
            if (current.reviewed_or_applied_maps
                + current.annotations
                + history.undo_redo
                + current.recovery_copies
                + current.active_or_resumable_runs
                + current.finished_runs)
                != 0
            {
                retained = current.clone();
                blocked = Some("This analysis has retained copies or authored dependencies. Review the counts and explicitly include them to delete.");
            }
        }
        if self.preedit_job.is_some()
            || self.preedit_apply.is_some()
            || self.jobs.iter().any(|job| !job.progress.finished.load(std::sync::atomic::Ordering::Acquire))
            || !self.media_jobs.is_empty()
            || !self.mask_jobs.is_empty()
            || !self.scene_jobs.is_empty()
            || self.edit_group_depth != 0
        {
            blocked = Some("Wait for project jobs and edits to finish before deleting data.");
        }
        if self.privacy_cleanup_pending() {
            blocked = Some("Finish the pending privacy cleanup before starting another deletion.");
        }
        let managed_files = saved_file.iter().chain(&copies).map(|copy| copy.path.clone()).collect();
        DeletionPreview { action, current, history, retained, managed_files, blocked, project: self.document_id, revision: self.revision, saved_file, copies }
    }

    pub fn apply_privacy_deletion(&mut self, approved: &DeletionPreview) -> Result<DeleteResult> {
        if *approved != self.preview_privacy_deletion(approved.action) {
            return Err(failure("The project or retained copies changed. Review a new deletion preview."));
        }
        if let Some(reason) = approved.blocked {
            return Err(failure(reason));
        }
        let locks = approved
            .copies
            .iter()
            .map(lock_copy)
            .collect::<io::Result<Vec<_>>>()
            .map_err(|_| failure("A managed copy is in use. Close the other session or stop its run and try again."))?;
        let result = self.publish_privacy_deletion(approved);
        if result.is_err() && self.revision == approved.revision && self.is_dirty() {
            if let Some(storage) = &mut self.persistence {
                storage.last_sent = None;
            }
            self.sync_persistence();
        }
        drop(locks);
        result
    }

    fn preview_cache_copies(&self) -> Vec<ManagedCopy> {
        let Some(dir) = self.previews.dir() else { return Vec::new() };
        if !is_directory(&dir) {
            return Vec::new();
        }
        let states = std::iter::once(&self.project).chain(self.history.undo.iter().chain(&self.history.redo).map(|(_, project)| project));
        let mut media_paths = BTreeSet::new();
        for state in states {
            for media in state.items.values().filter_map(|item| item.as_media()) {
                for reference in std::iter::once(&media.media).chain(media.proxy.iter()) {
                    if let shotclutch_project::MediaRef::File { path } = reference
                        && let Ok(path) = crate::doc_path::resolve(path, self.path.as_deref().map(Path::new))
                        && let Ok(path) = fs::canonicalize(path)
                    {
                        media_paths.insert(path);
                    }
                }
            }
        }
        fs::read_dir(dir)
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                if !crate::previews::is_preview_file(entry.file_name().to_str()?) {
                    return None;
                }
                let path = entry.path();
                if media_paths.contains(&fs::canonicalize(&path).ok()?) {
                    return None;
                }
                let bytes = regular_bytes(&path, PROJECT_FILE_LIMIT).ok()?;
                Some(managed_copy(path, None, &bytes))
            })
            .collect()
    }

    fn publish_privacy_deletion(&mut self, approved: &DeletionPreview) -> Result<DeleteResult> {
        let mut files = 0;
        if approved.action == Deletion::Disposable {
            for copy in &approved.copies {
                files += usize::from(remove_copy(copy).map_err(|_| failure("Some run checkpoints remain. Preview the remaining files and retry."))?);
            }
            self.previews.set_dir(self.previews.dir());
            *self.search_cache.get_mut().unwrap_or_else(|err| err.into_inner()) = None;
        } else {
            let saved = approved.saved_file.as_ref().ok_or_else(|| failure("The native project is unavailable."))?;
            let storage = self.persistence.as_ref().ok_or_else(|| failure("Recovery storage is unavailable."))?;
            // Suspend queued recovery writes without removing the current journal until
            // the replacement file is durable.
            storage.forget_pending().map_err(EngineError::Other)?;
            let data_dir = storage.data_dir.clone();
            let lock = publication_guard(&data_dir, self.document_id, self.privacy_floor_seen).map_err(EngineError::Other)?;
            if *approved != self.preview_privacy_deletion(approved.action) {
                if let Some(storage) = &mut self.persistence {
                    storage.last_sent = None;
                }
                self.sync_persistence();
                return Err(failure("Managed copies changed during confirmation. Review a new preview."));
            }
            let next = self.revision.checked_add(1).filter(|n| *n < u64::MAX).ok_or_else(|| failure("Document revision exhausted."))?;
            let replacement = if let Deletion::Analysis { asset, .. } = approved.action {
                let mut snapshot = self.document_snapshot();
                snapshot.revision = next;
                snapshot.loaded_schema = shotclutch_format::SCHEMA_VERSION;
                remove_analysis(&mut snapshot.project, asset);
                for (_, state) in snapshot.history.undo.iter_mut().chain(&mut snapshot.history.redo) {
                    remove_analysis(state, asset);
                }
                let bytes = snapshot.encode(None, false).map_err(EngineError::Other)?;
                Some((snapshot, bytes))
            } else {
                None
            };
            let pending = PendingDeletion {
                project: approved.project,
                floor: if replacement.is_some() { next } else { u64::MAX },
                saved: saved.clone(),
                replacement: replacement.as_ref().map(|(_, bytes)| Sha256::digest(bytes).into()),
                copies: approved.copies.clone(),
            };
            pending.prepare(&data_dir).map_err(|_| failure("Could not persist the approved cleanup."))?;
            if let Some((snapshot, bytes)) = replacement {
                if saved.pinned_path().and_then(|path| shotclutch_format::atomic_write(&path, &bytes)).is_err() {
                    // Keep the intent if its write may have landed. Recovery compares digests.
                    return Err(failure("Could not publish the sanitized project. Retry the pending cleanup."));
                }
                self.project = snapshot.project;
                self.history = snapshot.history;
                self.revision = next;
                self.saved_revision = next;
                self.privacy_saved_fingerprint = pending.replacement;
                self.loaded_schema = snapshot.loaded_schema;
                self.previews.live.publish_project(self.project.clone());
                self.journal.clear();
                self.service.invalidate_project_drafts(self.document_id);
                self.events.push(crate::Event::ProjectChanged { revision: next });
                *self.search_cache.get_mut().unwrap_or_else(|err| err.into_inner()) = None;
            }
            files += pending.finish(&data_dir).map_err(|_| failure("Some managed files remain. Retry the pending cleanup."))?;
            if approved.action == Deletion::Project {
                self.journal.clear();
                self.service.invalidate_project_drafts(self.document_id);
                self.execute("file.newProject", serde_json::json!({}))?;
            } else {
                self.privacy_floor_seen = next;
            }
            drop(lock);
            if let Some(storage) = &mut self.persistence {
                storage.last_sent = None;
            }
            self.sync_persistence();
            if let Some(storage) = &mut self.persistence {
                storage.candidates.retain(|candidate| autosave::candidate_snapshot(&candidate.dir).is_file());
            }
        }
        Ok(DeleteResult {
            removed_files: files,
            preserved_caption_tracks: if matches!(approved.action, Deletion::Analysis { .. }) { approved.current.preserved_caption_tracks } else { 0 },
            remaining: self.privacy_inventory(),
        })
    }
}

fn caption_tracks(project: &Project, analyses: &BTreeSet<u64>) -> std::collections::BTreeMap<ItemId, bool> {
    let mut tracks = std::collections::BTreeMap::new();
    for application in project.preedit.applications.values() {
        if !project.preedit.maps.get(&application.map).is_some_and(|map| analyses.contains(&map.analysis)) {
            continue;
        }
        if let Some(sequence) = project.sequence(application.sequence) {
            let owned = serde_json::to_vec(sequence).is_ok_and(|bytes| format!("{:x}", Sha256::digest(&bytes)) == application.sequence_digest);
            tracks.entry(application.sequence).and_modify(|previous| *previous |= owned).or_insert(owned);
        }
    }
    tracks
}

fn remove_analysis(state: &mut Arc<Project>, asset: ItemId) {
    if !contains_analysis(state, asset) {
        return;
    }
    let state = Arc::make_mut(state);
    state.transcripts.remove(&asset);
    let other = state
        .observations
        .iter()
        .filter_map(|(_, observation)| {
            let source = observation.evidence.asset;
            (source != asset).then_some(source)
        })
        .collect();
    state.observations.retain_sources(&other);
    let analyses = state.preedit.analyses.iter().filter_map(|(id, analysis)| (analysis.source.item == asset).then_some(*id)).collect();
    for (sequence, owned) in caption_tracks(state, &analyses) {
        if owned && let Some(sequence) = state.item_mut(sequence).and_then(|item| item.as_sequence_mut()) {
            sequence.caption_tracks.retain(|track| track.name != "Rough Cut Transcript");
        }
    }
    let items = state.items.keys().copied().filter(|id| *id != asset).collect();
    state.preedit.retain_sources(&items);
}

fn contains_analysis(project: &Project, asset: ItemId) -> bool {
    project.transcripts.contains_key(&asset)
        || project.observations.iter().any(|(_, observation)| observation.evidence.asset == asset)
        || project.preedit.analyses.values().any(|analysis| analysis.source.item == asset)
}

#[derive(Serialize)]
struct Diagnostics {
    version: &'static str,
    os: &'static str,
    architecture: &'static str,
    warnings: usize,
    errors: usize,
    project_items: usize,
    transcript_count: usize,
    observation_count: usize,
    providers: [&'static str; 2],
}

pub const DIAGNOSTIC_ENTRIES: &[&str] =
    &["ShotClutch version, OS and architecture", "Warning and error counts", "Project item, transcript and observation counts", "Supported provider names"];

impl Session {
    pub fn export_privacy_diagnostics(&self, destination: &Path) -> Result<()> {
        let report = Diagnostics {
            version: env!("CARGO_PKG_VERSION"),
            os: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
            warnings: self.log.count(crate::panels::Level::Warning),
            errors: self.log.count(crate::panels::Level::Error),
            project_items: self.project.items.len(),
            transcript_count: self.project.transcripts.len(),
            observation_count: self.project.observations.iter().count(),
            providers: ["AssemblyAI", "Gemini"],
        };
        let bytes = serde_json::to_vec_pretty(&report).map_err(|_| failure("Could not encode diagnostics."))?;
        shotclutch_format::atomic_write_new(destination, &bytes).map_err(|_| failure("Could not create diagnostics. Choose a new file."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deletes_selected_analysis_across_saved_history_autosaves_and_recovery() {
        let (root, source, mut session, asset) = crate::preedit::tests::fixture();
        let path = root.path().join("privacy.scproj");
        let map = crate::preedit::tests::analyze(&mut session, asset, None);
        session
            .execute(
                "preedit.review",
                json!({
                    "map":map.to_string(), "expectedRevision":session.revision.to_string(), "action":{"kind":"markReviewed"}
                }),
            )
            .unwrap();
        session
            .edit("Private transcript", |project, _| {
                project.transcripts.insert(
                    asset,
                    Arc::new(shotclutch_project::Transcript {
                        words: vec![shotclutch_project::Word::new("synthetic-private-transcript", shotclutch_time::Tick(0), shotclutch_time::Tick(1))],
                        ..Default::default()
                    }),
                );
                Ok(())
            })
            .unwrap();
        session.execute("file.saveAs", json!({"path":path})).unwrap();
        let mut stale_writer = Session::default();
        stale_writer.start_autosave(autosave::AutosaveConfig::new(root.path().join("data"))).unwrap();
        stale_writer.execute("file.open", json!({"path":path})).unwrap();

        session
            .edit("Keep this authored note", |project, _| {
                project.notes = "unrelated note".into();
                Ok(())
            })
            .unwrap();
        session.sync_persistence();
        let autosaved = session.persistence.as_ref().unwrap().auto_save_now().unwrap().unwrap();
        let protection = session.preview_privacy_deletion(Deletion::Analysis { asset, include_dependencies: false });
        assert_eq!(protection.current.reviewed_or_applied_maps, 1);
        assert!(protection.current.recovery_copies >= 1);
        assert!(session.apply_privacy_deletion(&protection).is_err());
        let action = Deletion::Analysis { asset, include_dependencies: true };
        let stale = session.preview_privacy_deletion(action);
        session
            .edit("Another note", |project, _| {
                project.notes.push('!');
                Ok(())
            })
            .unwrap();
        assert!(session.apply_privacy_deletion(&stale).is_err());
        session.sync_persistence();
        session.persistence.as_ref().unwrap().flush_checked().unwrap();
        let approved = session.preview_privacy_deletion(action);
        let deleted = session.apply_privacy_deletion(&approved).unwrap();
        assert!(deleted.removed_files >= 1);
        assert!(!autosaved.exists());
        assert!(source.is_file());
        assert!(!fs::read_to_string(&path).unwrap().contains("synthetic-private-transcript"));
        assert_eq!(session.project.notes, "unrelated note!");
        assert!(session.project.transcripts.is_empty());
        assert!(session.project.preedit.is_empty());
        session.execute("edit.undo", json!({})).unwrap();
        assert!(session.project.transcripts.is_empty());
        assert!(session.project.preedit.is_empty());
        session.execute("file.save", json!({})).unwrap();
        stale_writer
            .edit("Diverge", |project, _| {
                project.notes = "stale".into();
                Ok(())
            })
            .unwrap();
        assert!(stale_writer.execute("file.save", json!({})).is_err());
        stale_writer.shutdown();
        session.shutdown();

        let mut reopened = Session::default();
        reopened.start_autosave(autosave::AutosaveConfig::new(root.path().join("data"))).unwrap();
        reopened.execute("file.open", json!({"path":path})).unwrap();
        assert!(reopened.project.transcripts.is_empty());
        assert!(reopened.project.preedit.is_empty());
        let mut newer = Session::default();
        newer.start_autosave(autosave::AutosaveConfig::new(root.path().join("data"))).unwrap();
        newer.execute("file.open", json!({"path":path})).unwrap();
        newer
            .edit("Other session's edit", |project, _| {
                project.notes.push_str(" from another session");
                Ok(())
            })
            .unwrap();
        newer.execute("file.save", json!({})).unwrap();
        newer.shutdown();
        assert!(reopened.preview_privacy_deletion(Deletion::Project).blocked.is_some());
        reopened.execute("file.open", json!({"path":path})).unwrap();
        assert!(reopened.project.notes.ends_with(" from another session"));
        let delete_project = reopened.preview_privacy_deletion(Deletion::Project);
        reopened.apply_privacy_deletion(&delete_project).unwrap();
        assert!(!path.exists());
        assert!(source.is_file());
        reopened.shutdown();
    }

    #[test]
    fn diagnostics_allowlist_excludes_private_fields_and_refuses_overwrite() {
        let mut session = Session::default();
        let root = tempfile::tempdir().unwrap();
        let report = root.path().join("report.json");
        Arc::make_mut(&mut session.project).notes = "synthetic-private-note".into();
        session.log.push(crate::panels::Level::Error, "private", "Authorization: synthetic-key https://private.test/?signature=synthetic");
        session.export_privacy_diagnostics(&report).unwrap();
        let exported = fs::read_to_string(&report).unwrap();
        assert!(!exported.contains("synthetic"));
        assert_eq!(serde_json::from_str::<serde_json::Value>(&exported).unwrap()["errors"], 1);
        assert!(session.export_privacy_diagnostics(&report).is_err());
        assert_eq!(fs::read_to_string(&report).unwrap(), exported);
    }

    #[test]
    fn restart_resumes_confirmed_cleanup_after_primary_publication() {
        let (root, source, mut session, asset) = crate::preedit::tests::fixture();
        let project = root.path().join("restart.scproj");
        crate::preedit::tests::analyze(&mut session, asset, None);
        session.execute("file.saveAs", json!({"path":project})).unwrap();
        let disk = fs::read(&project).unwrap();
        let tombstone = session.document_id;
        let data = root.path().join("data");
        let revision = session.revision + 1;
        let mut clean = session.document_snapshot();
        clean.revision = revision;
        remove_analysis(&mut clean.project, asset);
        for (_, project) in clean.history.undo.iter_mut().chain(&mut clean.history.redo) {
            remove_analysis(project, asset);
        }
        let clean = clean.encode(None, false).unwrap();
        let copy_path = root.path().join("restart-ring.scproj");
        fs::write(&copy_path, &disk).unwrap();
        let recovery_lock = root.path().join("restart-ring.lock");
        let lock = File::options().create_new(true).read(true).write(true).open(&recovery_lock).unwrap();
        lock.lock().unwrap();
        let deletion = PendingDeletion {
            project: tombstone,
            floor: revision,
            saved: managed_copy(project.clone(), None, &disk),
            replacement: Some(Sha256::digest(&clean).into()),
            copies: vec![managed_copy(copy_path.clone(), Some(recovery_lock), &disk)],
        };
        let publication = storage_lock(&data, tombstone).unwrap();
        deletion.prepare(&data).unwrap();
        shotclutch_format::atomic_write(&project, &clean).unwrap();
        drop(publication);
        assert!(resume_pending_deletion(&data, tombstone).is_err());
        assert!(check_floor(&data, tombstone, revision).is_err());
        assert!(copy_path.is_file());
        drop(lock);
        session.shutdown();
        let mut reopened = Session::default();
        reopened.start_autosave(autosave::AutosaveConfig::new(&data)).unwrap();
        reopened.execute("file.open", json!({"path":project})).unwrap();
        assert!(!copy_path.exists());
        assert!(!has_pending(&data, tombstone));
        assert!(reopened.project.preedit.is_empty());
        assert!(source.is_file());
        assert_eq!(reopened.privacy_floor_seen, revision);
        reopened.shutdown();
    }
}
