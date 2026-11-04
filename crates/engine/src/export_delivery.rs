//! Native delivery: encode privately beside the destination, verify, then publish complete
//! files. A single-file delivery has one atomic publication point. Multi-file deliveries
//! roll back ordinary failures; they are not a filesystem-wide atomic transaction.

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use shotclutch_export::{ExportSettings, Format, Progress, Report};
use shotclutch_format::session::DocumentId;
use shotclutch_project::{ItemId, Project};

use crate::export_inputs::{Inputs, check_cancel};
use crate::{MediaPool, Services};

static RESERVED: OnceLock<Mutex<BTreeSet<PathBuf>>> = OnceLock::new();

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum FaultPoint {
    InputsBound,
    Encoded,
    Publish(usize),
}
#[cfg(test)]
thread_local! { static FAULT: std::cell::RefCell<Option<Box<dyn FnMut(FaultPoint) -> Result<(), String>>>> = const { std::cell::RefCell::new(None) }; }
#[cfg(test)]
fn fault(point: FaultPoint) -> Result<(), String> {
    FAULT.with(|f| f.borrow_mut().as_mut().map_or(Ok(()), |f| f(point)))
}

fn reservations() -> std::sync::MutexGuard<'static, BTreeSet<PathBuf>> {
    RESERVED.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}
fn key(path: &Path) -> PathBuf {
    if cfg!(windows) { PathBuf::from(path.to_string_lossy().to_lowercase()) } else { path.to_path_buf() }
}

#[derive(Clone, PartialEq, Eq)]
struct Stamp {
    bytes: u64,
    modified: Option<std::time::SystemTime>,
    digest: String,
    permissions: fs::Permissions,
}
fn stamp(path: &Path, progress: &Progress) -> Result<Option<Stamp>, String> {
    let before = match fs::symlink_metadata(path) {
        Ok(m) if m.is_file() && !m.is_symlink() => m,
        Ok(_) => return Err("PERMISSION_DENIED: export destination must be a regular file, not a directory or link".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("export destination metadata: {e}")),
    };
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0; 256 * 1024];
    let mut bytes = 0u64;
    loop {
        check_cancel(progress)?;
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        bytes = bytes.checked_add(n as u64).filter(|n| *n <= 64 * 1024 * 1024 * 1024).ok_or("RESOURCE_LIMIT: export file exceeds 64 GiB")?;
        hash.update(&buffer[..n]);
    }
    let after = file.metadata().map_err(|e| e.to_string())?;
    if before.len() != bytes || after.len() != bytes || before.modified().ok() != after.modified().ok() {
        return Err("REVISION_CONFLICT: export destination changed while checking it".into());
    }
    Ok(Some(Stamp { bytes, modified: before.modified().ok(), digest: format!("{:x}", hash.finalize()), permissions: before.permissions() }))
}

struct Target {
    path: PathBuf,
    before: Option<Stamp>,
}
pub(crate) struct Delivery {
    stage: tempfile::TempDir,
    targets: Vec<Target>,
    reserved: Vec<PathBuf>,
    primary: PathBuf,
    retain: AtomicBool,
}
impl Drop for Delivery {
    fn drop(&mut self) {
        if self.retain.load(Ordering::Acquire) {
            self.stage.disable_cleanup(true);
        }
        let mut reserved = reservations();
        for key in &self.reserved {
            reserved.remove(key);
        }
    }
}

fn filenames(project: &Project, seq: ItemId, settings: &ExportSettings, sidecar: Option<&str>) -> Result<Vec<PathBuf>, String> {
    let q = project.sequence(seq).ok_or("NOT_FOUND: export sequence is unavailable")?;
    let mut paths = if settings.format.is_image_sequence() {
        let range = shotclutch_export::export_range(project, seq, settings).map_err(|e| e.to_string())?;
        let rate = settings.frame_rate.unwrap_or(q.settings.frame_rate);
        let (first, last) = shotclutch_export::frame_span(rate, range);
        let count = u64::try_from(last - first).map_err(|_| "INVALID_ARGUMENT: export frame span is invalid")?;
        if count > 100_000 {
            return Err("RESOURCE_LIMIT: image export exceeds 100000 files".into());
        }
        (0..count).map(|i| PathBuf::from(shotclutch_export::image_sequence_path(&settings.path, i, count))).collect::<Vec<_>>()
    } else {
        vec![PathBuf::from(&settings.path)]
    };
    if settings.format == Format::MxfOpAtom && settings.has_audio() {
        let resolved = settings.resolve(q.settings.width, q.settings.height, q.settings.frame_rate, q.settings.sample_rate);
        paths.extend(shotclutch_export::opatom_audio_paths(&settings.path, resolved.channels as usize).into_iter().map(PathBuf::from));
    }
    if let Some(sidecar) = sidecar {
        paths.push(PathBuf::from(sidecar));
    }
    Ok(paths)
}

impl Delivery {
    pub fn new(
        project: &Project,
        seq: ItemId,
        settings: &ExportSettings,
        sidecar: Option<&str>,
        inputs: &[PathBuf],
        sequences: &[shotclutch_media::sequence::Numbered],
        progress: &Progress,
    ) -> Result<Self, String> {
        settings.validate().map_err(|e| e.to_string())?;
        if settings.output_file.is_some() || settings.sink.is_some() {
            return Err("CAPABILITY_UNAVAILABLE: native atomic delivery requires a path destination".into());
        }
        let output = Path::new(&settings.path);
        if output.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("scproj") || ext.eq_ignore_ascii_case("fcproj")) {
            return Err("PERMISSION_DENIED: rendered export cannot replace a project document".into());
        }
        let parent = output.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|e| format!("I/O: creating export directory: {e}"))?;
        let parent = fs::canonicalize(parent).map_err(|e| e.to_string())?;
        let primary = parent.join(output.file_name().ok_or("INVALID_ARGUMENT: export needs a file name")?);
        let source_paths: BTreeSet<_> = inputs
            .iter()
            .filter_map(|path| {
                fs::canonicalize(path).ok().or_else(|| {
                    let absolute = std::path::absolute(path).ok()?;
                    let parent = fs::canonicalize(absolute.parent()?).ok()?;
                    Some(parent.join(absolute.file_name()?))
                })
            })
            .map(|p| key(&p))
            .collect();
        let sequence_roots: Vec<_> = sequences
            .iter()
            .filter_map(|seq| {
                let dir = Path::new(&seq.dir);
                fs::canonicalize(if dir.as_os_str().is_empty() { Path::new(".") } else { dir }).ok().map(|dir| (key(&dir), seq))
            })
            .collect();
        let mut targets = Vec::new();
        let mut keys = Vec::new();
        for path in filenames(project, seq, settings, sidecar)? {
            let path = parent.join(path.file_name().ok_or("INVALID_ARGUMENT: export needs a file name")?);
            if source_paths.contains(&key(&path)) || fs::canonicalize(&path).ok().is_some_and(|p| source_paths.contains(&key(&p))) {
                return Err("PERMISSION_DENIED: export cannot replace an original source".into());
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let name = if cfg!(windows) { name.to_lowercase() } else { name.into_owned() };
            if sequence_roots
                .iter()
                .any(|(dir, seq)| key(path.parent().unwrap_or_else(|| Path::new("."))) == *dir && seq.number_of(&name).is_some_and(|n| n >= seq.number))
            {
                return Err("PERMISSION_DENIED: export cannot replace numbered original source frames".into());
            }
            keys.push(key(&path));
            targets.push(Target { before: stamp(&path, progress)?, path });
        }
        let unique: BTreeSet<_> = keys.iter().cloned().collect();
        if unique.len() != keys.len() {
            return Err("INVALID_ARGUMENT: export outputs collide with each other".into());
        }
        let stage = tempfile::Builder::new().prefix(".shotclutch-export-").tempdir_in(parent).map_err(|e| e.to_string())?;
        {
            let mut busy = reservations();
            if keys.iter().any(|key| busy.contains(key)) {
                return Err("RESOURCE_LIMIT: another export is using this destination".into());
            }
            busy.extend(keys.iter().cloned());
        }
        Ok(Self { stage, targets, reserved: keys, primary, retain: AtomicBool::new(false) })
    }
    pub fn staged(&self, path: &Path) -> PathBuf {
        self.stage.path().join(path.file_name().unwrap_or_default())
    }

    /// Validate the whole set before its first publication. Ordinary later I/O failures restore
    /// prior files; if restoration fails, retain the backup directory for explicit recovery.
    pub fn publish(&self, progress: &Progress) -> Result<Vec<Value>, String> {
        let backup = self.stage.path().join("previous");
        fs::create_dir(&backup).map_err(|e| e.to_string())?;
        let mut outputs = Vec::new();
        for target in &self.targets {
            check_cancel(progress)?;
            if stamp(&target.path, progress)? != target.before {
                return Err("REVISION_CONFLICT: export destination changed since the job began".into());
            }
            let staged = self.staged(&target.path);
            // FlushFileBuffers requires write access on Windows. Sync before applying an old
            // read-only mode, and close the handle before publication.
            let file = fs::OpenOptions::new().read(true).write(true).open(&staged).map_err(|e| format!("open completed export for sync: {e}"))?;
            file.sync_all().map_err(|e| format!("sync completed export: {e}"))?;
            drop(file);
            if let Some(old) = &target.before {
                fs::set_permissions(&staged, old.permissions.clone()).map_err(|e| e.to_string())?;
            }
            let meta = stamp(&staged, progress)?.ok_or("INTERNAL_ERROR: an encoded output is missing")?;
            outputs.push(json!({"name": target.path.file_name().map(|s| s.to_string_lossy()), "bytes": meta.bytes.to_string(), "sha256": meta.digest}));
            if self.targets.len() > 1 && target.before.is_some() {
                let saved = backup.join(target.path.file_name().unwrap_or_default());
                fs::hard_link(&target.path, &saved).or_else(|_| fs::copy(&target.path, &saved).map(|_| ())).map_err(|e| e.to_string())?;
            }
        }
        let staged_paths = self
            .targets
            .iter()
            .map(|target| tempfile::TempPath::try_from_path(self.staged(&target.path)))
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;
        check_cancel(progress)?;
        for (index, (target, temp)) in self.targets.iter().zip(staged_paths).enumerate() {
            #[cfg(test)]
            let ready = fault(FaultPoint::Publish(index));
            #[cfg(not(test))]
            let ready = {
                let _ = index;
                Ok::<(), String>(())
            };
            let result = ready.and_then(|_| {
                if target.before.is_some() { temp.persist(&target.path) } else { temp.persist_noclobber(&target.path) }.map_err(|e| e.to_string())
            });
            if let Err(e) = result {
                let mut failed = false;
                for old in self.targets[..index].iter().rev() {
                    let restored = if old.before.is_some() {
                        fs::rename(backup.join(old.path.file_name().unwrap_or_default()), &old.path)
                    } else {
                        fs::remove_file(&old.path)
                    };
                    failed |= restored.is_err();
                }
                if failed {
                    self.retain.store(true, Ordering::Release);
                    // Preserve recoverable prior files; ordinary successful/error paths clean up.
                    let retained = self.stage.path().to_path_buf();
                    // The retention flag preserves the directory; the marker explains recovery.
                    let _ = fs::write(self.stage.path().join("RECOVERY_REQUIRED"), b"A publication rollback failed; previous/ contains prior files.");
                    return Err(format!("INTERNAL_ERROR: export rollback needs recovery from {}: {e}", retained.display()));
                }
                return Err(format!("export publication failed: {e}"));
            }
        }
        if let Some(parent) = self.primary.parent()
            && let Ok(dir) = fs::File::open(parent)
        {
            let _ = dir.sync_all();
        }
        Ok(outputs)
    }
}

pub(crate) fn run(
    project: &Arc<Project>,
    seq: ItemId,
    settings: &ExportSettings,
    pool: Arc<MediaPool>,
    services: Arc<dyn Services>,
    progress: &Progress,
    identity: (DocumentId, u64),
    offline: &Mutex<Vec<crate::media_pool::OfflineMedia>>,
    binding: &Mutex<Option<Value>>,
) -> Result<Report, String> {
    *progress.status.lock().unwrap_or_else(|e| e.into_inner()) = "Binding original sources".into();
    let inputs = Inputs::prepare(pool, project, seq, &*services, progress)?;
    #[cfg(test)]
    fault(FaultPoint::InputsBound)?;
    let provider = inputs.pool.full_res_provider(project.clone(), inputs.services.clone()).recording();
    let sidecar = crate::export_tools::caption_sidecar(project, seq, settings).map_err(|e| e.to_string())?;
    let delivery = Delivery::new(project, seq, settings, sidecar.as_ref().map(|s| s.0.as_str()), &inputs.paths, &inputs.sequences, progress)?;
    let mut staged = settings.clone();
    staged.path = delivery.staged(Path::new(&settings.path)).to_string_lossy().into_owned();
    staged.defer_completion = true;
    if let Some((path, bytes)) = sidecar {
        fs::write(delivery.staged(Path::new(&path)), bytes).map_err(|e| e.to_string())?;
    }
    let result = shotclutch_export::export(project, seq, &staged, &provider, progress).map_err(|e| e.to_string());
    *offline.lock().unwrap_or_else(|e| e.into_inner()) = provider.offline_used();
    let mut report = result?;
    #[cfg(test)]
    fault(FaultPoint::Encoded)?;
    *progress.status.lock().unwrap_or_else(|e| e.into_inner()) = "Verifying originals and publishing delivery".into();
    inputs.verify(&*services, progress)?;
    let mut wire = serde_json::to_value(settings).map_err(|e| e.to_string())?;
    wire.sort_all_objects();
    let settings_hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&wire).map_err(|e| e.to_string())?));
    let range = shotclutch_export::export_range(project, seq, settings).map_err(|e| e.to_string())?;
    let outputs = delivery.publish(progress)?;
    report.path.clone_from(&settings.path);
    report.extra_files = report
        .extra_files
        .iter()
        .map(|path| Path::new(&settings.path).with_file_name(Path::new(path).file_name().unwrap_or_default()).to_string_lossy().into_owned())
        .collect();
    *binding.lock().unwrap_or_else(|e| e.into_inner()) = Some(
        json!({"apiVersion": "1", "projectId": identity.0, "revision": identity.1.to_string(), "sequenceId": seq.0.to_string(), "range": {"start": range.start.0.to_string(), "end": range.end().0.to_string()}, "settingsSha256": settings_hash, "sources": inputs.manifest, "outputs": outputs, "publication": if outputs.len() == 1 { "atomicFile" } else { "fileSet" }}),
    );
    *progress.status.lock().unwrap_or_else(|e| e.into_inner()) = "Done".into();
    Ok(report)
}

#[cfg(test)]
#[path = "m1_export_tests.rs"]
mod m1_export_tests;
