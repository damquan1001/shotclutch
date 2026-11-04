//! Native, verified, relocatable collection. Publication is an exclusive directory rename.
//! The manifest describes a delivery snapshot, not a mutable project's future saves.

use crate::commands::{CommandSpec, always, bad, bool_p, str_p};
use crate::document::Snapshot;
use crate::export_inputs::{BoundReader, check_cancel, reader};
use crate::{EngineError, History, Result, Services, Session};
use cap_std::fs::Dir;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use shotclutch_export::Progress;
use shotclutch_format::session::DocumentId;
use shotclutch_media::{ByteReader, MediaKind, SharedReader};
use shotclutch_project::{ItemId, ItemKind, MediaIdentity, MediaRef, Project};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, atomic::Ordering};

const MAX_BYTES: u64 = 64 * 1024 * 1024 * 1024;
const MAX_FILES: usize = 100_000;
const MAX_DOCUMENT: u64 = 256 * 1024 * 1024;
const MAX_MANIFEST: u64 = 32 * 1024 * 1024;
const MANIFEST: &str = "collection.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Asset {
    path: String,
    role: String,
    source_id: String,
    essential: bool,
    bytes: u64,
    sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    version: u32,
    operation_id: DocumentId,
    request_sha256: String,
    source_project_id: DocumentId,
    #[serde(with = "shotclutch_format::session::decimal_u64")]
    source_revision: u64,
    project_id: DocumentId,
    #[serde(with = "shotclutch_format::session::decimal_u64")]
    revision: u64,
    document_mode: String,
    undo_states: usize,
    redo_states: usize,
    operation_receipts: usize,
    project: String,
    project_sha256: String,
    assets: Vec<Asset>,
    embedded: Vec<String>,
    rebuildable_caches: Vec<String>,
    requirements: Vec<String>,
}

// Stricter than the current platform's Path so a package also opens on Windows.
fn component(s: &str) -> bool {
    let base = s.split('.').next().unwrap_or("").to_ascii_uppercase();
    !s.is_empty()
        && s.len() <= 180
        && s != "."
        && s != ".."
        && !s.ends_with(['.', ' '])
        && !s.chars().any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        && !matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        && !(base.len() == 4 && (base.starts_with("COM") || base.starts_with("LPT")) && base.as_bytes()[3].is_ascii_digit())
}
fn relative(s: &str) -> bool {
    s.len() <= 1024 && s.split('/').all(component)
}
pub(crate) fn project_name(s: &Session, p: &Value) -> Result<String> {
    let name = str_p(p, "projectName")
        .map(str::to_string)
        .or_else(|| s.path.as_deref().and_then(|p| Path::new(p).file_stem()).map(|p| p.to_string_lossy().into_owned()))
        .unwrap_or_else(|| s.project.name.clone());
    if !component(&name) {
        return Err(bad("file.projectManager", "projectName must be one portable filename component"));
    }
    Ok(name)
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn io(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn hash(mut f: impl Read, limit: u64, progress: &Progress) -> std::result::Result<(u64, String), String> {
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buf = vec![0; 256 * 1024];
    loop {
        check_cancel(progress)?;
        let n = f.read(&mut buf).map_err(io)?;
        if n == 0 {
            break;
        }
        total = total.checked_add(n as u64).filter(|n| *n <= limit).ok_or("RESOURCE_LIMIT: collection byte budget exceeded")?;
        hash.update(&buf[..n]);
    }
    Ok((total, format!("{:x}", hash.finalize())))
}
fn read_limited(mut file: impl Read, limit: u64) -> std::result::Result<Vec<u8>, String> {
    let mut out = Vec::new();
    file.by_ref().take(limit + 1).read_to_end(&mut out).map_err(io)?;
    if out.len() as u64 > limit {
        return Err("RESOURCE_LIMIT: collection metadata is too large".into());
    }
    Ok(out)
}
fn open(root: &Dir, path: &str) -> std::result::Result<cap_std::fs::File, String> {
    if !relative(path) {
        return Err("VERIFICATION_FAILED: nonportable or escaping collection path".into());
    }
    let mut prefix = PathBuf::new();
    for part in path.split('/') {
        prefix.push(part);
        if root.symlink_metadata(&prefix).map_err(io)?.file_type().is_symlink() {
            return Err("VERIFICATION_FAILED: collection paths cannot contain links".into());
        }
    }
    let file = root.open(path).map_err(io)?;
    if !file.metadata().map_err(io)?.is_file() {
        return Err("VERIFICATION_FAILED: collection member is not a regular file".into());
    }
    Ok(file)
}
fn root(path: &Path) -> std::result::Result<Dir, String> {
    if fs::symlink_metadata(path).map_err(io)?.file_type().is_symlink() {
        return Err("VERIFICATION_FAILED: collection root cannot be a link".into());
    }
    Dir::open_ambient_dir(path, cap_std::ambient_authority()).map_err(io)
}

/// A retained directory capability bounds reads; a manifest never grants filesystem access.
fn verify(path: &Path, progress: &Progress) -> std::result::Result<Manifest, String> {
    let dir = root(path)?;
    let bytes = read_limited(open(&dir, MANIFEST)?, MAX_MANIFEST)?;
    let m: Manifest = serde_json::from_slice(&bytes).map_err(io)?;
    if m.version != 1
        || m.assets.len() > MAX_FILES
        || !relative(&m.project)
        || m.project.contains('/')
        || !m.project.ends_with(".scproj")
        || !matches!(m.document_mode.as_str(), "fullDocument" | "derivedCopy")
    {
        return Err("VERIFICATION_FAILED: unsupported or invalid collection manifest".into());
    }
    let bytes = read_limited(open(&dir, &m.project)?, MAX_DOCUMENT)?;
    if digest(&bytes) != m.project_sha256 {
        return Err("VERIFICATION_FAILED: project hash mismatch".into());
    }
    let loaded = shotclutch_format::decode(&bytes).map_err(io)?;
    let restored = crate::document::restore(loaded.edit_state, &loaded.project)?;
    if restored.id != m.project_id
        || restored.revision != m.revision
        || restored.history.undo.len() != m.undo_states
        || restored.history.redo.len() != m.redo_states
        || restored.operations.len() != m.operation_receipts
    {
        return Err("VERIFICATION_FAILED: document identity or history differs from manifest".into());
    }
    if m.document_mode == "fullDocument" && (m.project_id != m.source_project_id || m.revision != m.source_revision)
        || m.document_mode == "derivedCopy"
            && (m.project_id == m.source_project_id || m.revision != 1 || m.undo_states + m.redo_states + m.operation_receipts != 0)
    {
        return Err("VERIFICATION_FAILED: collection document mode mismatch".into());
    }
    let mut paths = BTreeMap::new();
    let mut names = BTreeSet::from([m.project.to_lowercase(), MANIFEST.to_string()]);
    let mut remaining = MAX_BYTES;
    for asset in &m.assets {
        if !relative(&asset.path)
            || !names.insert(asset.path.to_lowercase())
            || !matches!(asset.role.as_str(), "original" | "ownedOriginal" | "generatedOriginal" | "proxy")
            || asset.essential == (asset.role == "proxy")
            || asset.source_id.parse::<u64>().is_err()
        {
            return Err("VERIFICATION_FAILED: invalid or colliding collection asset".into());
        }
        if asset.bytes > remaining {
            return Err("RESOURCE_LIMIT: collection exceeds 64 GiB".into());
        }
        let (size, sha) = hash(open(&dir, &asset.path)?, remaining, progress)?;
        remaining -= size;
        if size != asset.bytes || sha != asset.sha256 {
            return Err("VERIFICATION_FAILED: collected asset hash mismatch".into());
        }
        paths.insert(asset.path.clone(), asset);
    }
    verify_inventory(&dir, &names, progress)?;
    for project in std::iter::once(&loaded.project).chain(restored.history.undo.iter().chain(&restored.history.redo).map(|(_, p)| &**p)) {
        portable(project)?;
        for item in project.items.values() {
            let Some(media) = item.as_media() else { continue };
            for (reference, proxy) in std::iter::once((&media.media, false)).chain(media.proxy.as_ref().map(|p| (p, true))) {
                if let MediaRef::File { path } = reference {
                    let asset = paths.get(path).ok_or("VERIFICATION_FAILED: project references unlisted media")?;
                    if (asset.role == "proxy") != proxy || asset.source_id != item.id.0.to_string() {
                        return Err("VERIFICATION_FAILED: media role/source mapping mismatch".into());
                    }
                    if !proxy && media.info.kind == MediaKind::ImageSequence {
                        let numbered = shotclutch_media::sequence::Numbered::parse(path).ok_or("VERIFICATION_FAILED: invalid collected image sequence")?;
                        let parent = Path::new(path).parent().ok_or("VERIFICATION_FAILED: missing image folder")?;
                        for entry in dir.read_dir(parent).map_err(io)? {
                            let entry = entry.map_err(io)?;
                            let name = entry.file_name().to_string_lossy().into_owned();
                            if numbered.number_of(&name).is_some() {
                                let member_path = format!("{}/{}", parent.to_string_lossy().replace('\\', "/"), name);
                                let member = paths.get(&member_path).ok_or("VERIFICATION_FAILED: unlisted image sequence member")?;
                                if member.role == "proxy" || !member.essential || member.source_id != asset.source_id {
                                    return Err("VERIFICATION_FAILED: image sequence role/source mapping mismatch".into());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    check_cancel(progress)?;
    Ok(m)
}
fn verify_inventory(dir: &Dir, names: &BTreeSet<String>, progress: &Progress) -> std::result::Result<(), String> {
    let mut todo = vec![String::new()];
    let mut seen = BTreeSet::new();
    let mut count = 0;
    while let Some(folder) = todo.pop() {
        let path = if folder.is_empty() { "." } else { &folder };
        for entry in dir.read_dir(path).map_err(io)? {
            check_cancel(progress)?;
            count += 1;
            if count > MAX_FILES * 4 + 2 {
                return Err("RESOURCE_LIMIT: collection inventory is too large".into());
            }
            let entry = entry.map_err(io)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = if folder.is_empty() { name } else { format!("{folder}/{name}") };
            if !relative(&path) || !seen.insert(path.to_lowercase()) {
                return Err("VERIFICATION_FAILED: invalid/colliding inventory path".into());
            }
            let kind = entry.file_type().map_err(io)?;
            if kind.is_dir() {
                todo.push(path);
            } else if !kind.is_file() || !names.contains(&path.to_lowercase()) {
                return Err("VERIFICATION_FAILED: unlisted file or link in collection inventory".into());
            }
        }
    }
    Ok(())
}
fn portable(p: &Project) -> std::result::Result<(), String> {
    if !p.extensions.is_empty() {
        return Err("CAPABILITY_UNAVAILABLE: collection needs dependency providers for optional project extensions".into());
    }
    if p.settings.scratch != Default::default()
        || p.settings.ingest.destination.is_some()
        || p.settings.ingest.enabled
        || p.luts.iter().any(|l| l.source_path.is_some())
    {
        return Err("VERIFICATION_FAILED: collection retains external cache/ingest/LUT source paths".into());
    }
    Ok(())
}

struct Copy {
    source: String,
    owned: Option<Arc<[u8]>>,
    asset: Asset,
    identity: Option<MediaIdentity>,
}
struct Sequence {
    first: String,
    frames: Vec<Option<String>>,
}
struct Plan {
    snapshot: Snapshot,
    copies: Vec<Copy>,
    sequences: Vec<Sequence>,
    excluded: usize,
}
impl Plan {
    fn prepare(s: &Session, p: &Value) -> Result<Self> {
        if bool_p(p, "includePreviews").unwrap_or(false) {
            return Err(bad("file.projectManager", "CAPABILITY_UNAVAILABLE: preview caches are rebuildable and are not collected"));
        }
        let all: Vec<_> = s.project.sequences().map(|i| i.id).collect();
        let selected = match p.get("sequences") {
            None => all.clone(),
            Some(Value::Array(a)) => a
                .iter()
                .map(|v| {
                    v.as_u64()
                        .map(ItemId)
                        .filter(|id| s.project.sequence(*id).is_some())
                        .ok_or_else(|| bad("file.projectManager", "sequences must contain valid sequence ids"))
                })
                .collect::<Result<Vec<_>>>()?,
            _ => return Err(bad("file.projectManager", "sequences must be an array")),
        };
        if selected.is_empty() {
            return Err(bad("file.projectManager", "select at least one sequence"));
        }
        let exclude = bool_p(p, "excludeUnused").unwrap_or(true);
        let full = !exclude && all.iter().all(|id| selected.contains(id));
        let history = bool_p(p, "includeHistory").unwrap_or(full);
        if history && !full {
            return Err(bad("file.projectManager", "includeHistory requires every sequence and excludeUnused:false"));
        }
        if !exclude && !full {
            return Err(bad("file.projectManager", "selecting sequences requires excludeUnused:true"));
        }
        let mut snapshot = s.document_snapshot();
        if !history {
            if exclude {
                let keep = crate::project_tools::closure(&s.project, &selected);
                let mut project = crate::project_tools::project_subset(&s.project, &keep);
                project.source_graphics.retain(|id, _| keep.contains(id));
                snapshot.project = Arc::new(project);
            }
            snapshot.project_id = DocumentId::default();
            snapshot.revision = 1;
            snapshot.history = History { limit: 200, ..Default::default() };
            snapshot.operations.clear();
        }
        let excluded = s.project.items.len() - snapshot.project.items.len();
        let retained = |id: &ItemId| snapshot.project.sequence(*id).is_some();
        snapshot.view.open_sequences.retain(retained);
        snapshot.view.sequences.retain(|id, _| retained(id));
        snapshot.view.playheads.retain(|id, _| retained(id));
        snapshot.view.active_sequence = snapshot.view.active_sequence.filter(retained);
        let mut plan = Self { snapshot, copies: Vec::new(), sequences: Vec::new(), excluded };
        let mut mapped = BTreeMap::new();
        let mut remaining = MAX_BYTES;
        let mut current = (*plan.snapshot.project).clone();
        plan.rewrite(&mut current, s, p, &mut mapped, &mut remaining, false)?;
        plan.snapshot.project = Arc::new(current);
        for redo in [false, true] {
            let entries = if redo { plan.snapshot.history.redo.clone() } else { plan.snapshot.history.undo.clone() };
            let mut rewritten = Vec::new();
            for (label, project) in entries.into_iter().rev().take(200).collect::<Vec<_>>().into_iter().rev() {
                let mut project = (*project).clone();
                plan.rewrite(&mut project, s, p, &mut mapped, &mut remaining, true)?;
                rewritten.push((label, Arc::new(project)));
            }
            if redo {
                plan.snapshot.history.redo = rewritten;
            } else {
                plan.snapshot.history.undo = rewritten;
            }
        }
        Ok(plan)
    }

    fn rewrite(
        &mut self,
        project: &mut Project,
        s: &Session,
        p: &Value,
        mapped: &mut BTreeMap<(ItemId, String, bool, bool), String>,
        remaining: &mut u64,
        historical: bool,
    ) -> Result<()> {
        if !project.extensions.is_empty() {
            return Err(EngineError::Other("CAPABILITY_UNAVAILABLE: collection cannot validate optional extension dependencies".into()));
        }
        project.settings.scratch = Default::default();
        project.settings.ingest.destination = None;
        project.settings.ingest.enabled = false;
        for lut in &mut project.luts {
            lut.source_path = None;
        }
        let generated: BTreeSet<_> = project.narrations.keys().copied().collect();
        for item in project.items.values_mut() {
            let ItemKind::Media(media) = &mut item.kind else { continue };
            let owned = s.media.owned_original(item.id, media).map(|(bytes, _)| bytes);
            if !bool_p(p, "includeProxies").unwrap_or(true) {
                media.proxy = None;
            }
            let image = media.info.kind == MediaKind::ImageSequence;
            for (reference, proxy) in std::iter::once((&mut media.media, false)).chain(media.proxy.as_mut().map(|p| (p, true))) {
                let MediaRef::File { path } = reference else { continue };
                let key = (item.id, path.clone(), proxy, image && !proxy);
                if let Some(relative) = mapped.get(&key) {
                    if !proxy
                        && let Some(expected) = media.identity
                        && let Some(copy) = self.copies.iter_mut().find(|c| c.asset.path == *relative)
                    {
                        if copy.identity.is_some_and(|i| i != expected) {
                            return Err(EngineError::Other("MEDIA_CHANGED: history contains different originals at the same path".into()));
                        }
                        copy.identity = Some(expected);
                    }
                    *path = relative.clone();
                    continue;
                }
                let owned = (!proxy).then(|| owned.clone()).flatten();
                let resolved = if owned.is_some() {
                    path.clone()
                } else {
                    // The pool registers current paths when opening a document. A history-only
                    // reference has no pool registration, so apply stored-document policy directly.
                    let resolved = if historical { crate::doc_path::resolve(path, s.path.as_deref().map(Path::new)) } else { s.media.follow(path) }
                        .map_err(|e| EngineError::Other(format!("MEDIA_CHANGED: source path refused: {e}")))?;
                    if Path::new(&resolved).is_relative() {
                        crate::doc_path::resolve(path, s.path.as_deref().map(Path::new)).map_err(|e| EngineError::Other(e.to_string()))?
                    } else {
                        resolved
                    }
                };
                let members = if image && !proxy {
                    if owned.is_some() {
                        return Err(EngineError::Other("CAPABILITY_UNAVAILABLE: byte-backed image sequences need file membership".into()));
                    }
                    let frames = crate::media_pool::image_sequence_frames(&resolved, &*s.services).map_err(|e| EngineError::Other(e.to_string()))?;
                    self.sequences.push(Sequence { first: resolved.clone(), frames: frames.clone() });
                    frames.into_iter().flatten().collect::<Vec<_>>()
                } else {
                    vec![resolved.clone()]
                };
                if self.copies.len().checked_add(members.len()).is_none_or(|n| n > MAX_FILES) {
                    return Err(EngineError::Other("RESOURCE_LIMIT: collection exceeds 100000 files".into()));
                }
                let folder = format!("{}/{}", if proxy { "Proxies" } else { "Media" }, self.copies.len());
                let mut first = None;
                for source in members {
                    let filename = if image && !proxy {
                        Path::new(&source)
                            .file_name()
                            .and_then(|s| s.to_str())
                            .filter(|s| component(s))
                            .ok_or_else(|| bad("file.projectManager", "image sequence filenames must be portable"))?
                            .to_string()
                    } else {
                        let extension = Path::new(path).extension().and_then(|s| s.to_str()).filter(|s| component(s) && s.len() <= 16).unwrap_or("bin");
                        format!("source.{extension}")
                    };
                    let relative = format!("{folder}/{filename}");
                    if source == resolved {
                        first = Some(relative.clone());
                    }
                    let bytes = match &owned {
                        Some(bytes) => bytes.len() as u64,
                        None => s
                            .services
                            .file_size(&source)
                            .map_err(|e| EngineError::Other(format!("MEDIA_CHANGED: missing/unreadable original or requested proxy: {e}")))?,
                    };
                    *remaining = remaining.checked_sub(bytes).ok_or_else(|| EngineError::Other("RESOURCE_LIMIT: collection exceeds 64 GiB".into()))?;
                    self.copies.push(Copy {
                        source,
                        owned: owned.clone(),
                        asset: Asset {
                            path: relative,
                            role: if proxy {
                                "proxy"
                            } else if owned.is_some() {
                                "ownedOriginal"
                            } else if generated.contains(&item.id) {
                                "generatedOriginal"
                            } else {
                                "original"
                            }
                            .into(),
                            source_id: item.id.0.to_string(),
                            essential: !proxy,
                            bytes,
                            sha256: String::new(),
                        },
                        identity: if proxy || image { None } else { media.identity },
                    });
                }
                let first = first.ok_or_else(|| EngineError::Other("MEDIA_CHANGED: missing first image sequence member".into()))?;
                mapped.insert(key, first.clone());
                *path = first;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum Point {
    Copied,
    Verified,
}
#[cfg(test)]
thread_local! { static FAULT: std::cell::RefCell<Option<Box<dyn FnMut(Point, &Progress) -> std::result::Result<(), String>>>> = const { std::cell::RefCell::new(None) }; }
#[cfg(test)]
fn fault(p: Point, progress: &Progress) -> std::result::Result<(), String> {
    FAULT.with(|f| f.borrow_mut().as_mut().map_or(Ok(()), |f| f(p, progress)))
}

fn publish(parent: &Dir, parent_path: &Path, from: &Path, to: &Path) -> std::result::Result<(), String> {
    #[cfg(windows)]
    {
        let _ = parent; // Keep the parent handle alive throughout publication.
        atomicwrites::move_atomic(&parent_path.join(from), &parent_path.join(to)).map_err(io)
    }
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    {
        let _ = parent_path;
        rustix::fs::renameat_with(parent, from, parent, to, rustix::fs::RenameFlags::NOREPLACE).map_err(io)
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "android", target_vendor = "apple")))]
    {
        let _ = (parent, parent_path, from, to);
        Err("CAPABILITY_UNAVAILABLE: exclusive directory publication is unavailable on this platform".into())
    }
}
fn write(path: &Path, bytes: &[u8]) -> std::result::Result<(), String> {
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(io)?;
    file.write_all(bytes).map_err(io)?;
    file.sync_all().map_err(io)
}
fn execute(mut plan: Plan, mut m: Manifest, dest: &Path, services: &dyn Services, progress: &Progress) -> std::result::Result<u64, String> {
    check_cancel(progress)?;
    let parent_path = dest.parent().ok_or("INVALID_ARGUMENT: collection needs a parent folder")?;
    let parent = root(parent_path)?;
    let stage = tempfile::Builder::new().prefix(".shotclutch-collect-").tempdir_in(parent_path).map_err(io)?;
    let mut bound = Vec::new();
    let mut total = 0;
    for copy in &mut plan.copies {
        check_cancel(progress)?;
        let raw: SharedReader = match &copy.owned {
            Some(bytes) => Arc::new(shotclutch_media::reader::MemReader(bytes.clone())),
            None => reader(services, &copy.source, copy.asset.bytes).map_err(io)?,
        };
        if raw.len() != copy.asset.bytes {
            return Err("MEDIA_CHANGED: original size changed before collection".into());
        }
        let source = BoundReader::bind(raw, progress)?;
        let path = stage.path().join(&copy.asset.path);
        fs::create_dir_all(path.parent().ok_or("invalid collection path")?).map_err(io)?;
        let mut file = fs::OpenOptions::new().create_new(true).write(true).open(&path).map_err(io)?;
        let mut offset = 0;
        let mut buf = vec![0; 256 * 1024];
        while offset < source.len() {
            check_cancel(progress)?;
            let n = (source.len() - offset).min(buf.len() as u64) as usize;
            source.read_at(offset, &mut buf[..n]).map_err(io)?;
            file.write_all(&buf[..n]).map_err(io)?;
            offset += n as u64;
        }
        file.sync_all().map_err(io)?;
        if let Some(identity) = copy.identity {
            let actual = crate::relink::identity_of(&crate::FsServices, &path.to_string_lossy()).map_err(io)?;
            if actual != identity {
                return Err("MEDIA_CHANGED: original differs from the project's imported identity".into());
            }
        }
        copy.asset.sha256 = source.digest.clone();
        total += offset;
        bound.push(source);
        progress.done.fetch_add(1, Ordering::Relaxed);
    }
    #[cfg(test)]
    fault(Point::Copied, progress)?;
    for (copy, before) in plan.copies.iter().zip(&bound) {
        if copy.owned.is_none() {
            let now = BoundReader::bind(reader(services, &copy.source, copy.asset.bytes).map_err(io)?, progress)?;
            if now.len() != before.len() || now.digest != before.digest {
                return Err("MEDIA_CHANGED: original changed during collection".into());
            }
        }
    }
    for sequence in &plan.sequences {
        if crate::media_pool::image_sequence_frames(&sequence.first, services).map_err(io)? != sequence.frames {
            return Err("MEDIA_CHANGED: image sequence membership changed during collection".into());
        }
    }
    let bytes = plan.snapshot.encode(None, false)?;
    if bytes.len() as u64 > MAX_DOCUMENT {
        return Err("RESOURCE_LIMIT: collected document exceeds 256 MiB".into());
    }
    m.project_sha256 = digest(&bytes);
    m.assets = plan.copies.into_iter().map(|c| c.asset).collect();
    write(&stage.path().join(&m.project), &bytes)?;
    let manifest = serde_json::to_vec_pretty(&m).map_err(io)?;
    if manifest.len() as u64 > MAX_MANIFEST {
        return Err("RESOURCE_LIMIT: collection manifest exceeds 32 MiB".into());
    }
    write(&stage.path().join(MANIFEST), &manifest)?;
    verify(stage.path(), progress)?;
    #[cfg(test)]
    fault(Point::Verified, progress)?;
    check_cancel(progress)?;
    publish(
        &parent,
        parent_path,
        Path::new(stage.path().file_name().ok_or("missing staging name")?),
        Path::new(dest.file_name().ok_or("missing destination name")?),
    )?;
    // Once renamed, success wins a late cancellation. No final path is deleted on failure.
    progress.done.fetch_add(1, Ordering::Relaxed);
    Ok(total + bytes.len() as u64 + manifest.len() as u64)
}

pub(crate) fn run(s: &mut Session, p: &Value) -> Result<Value> {
    for field in ["excludeUnused", "includeHistory", "includeProxies", "includePreviews", "wait", "dryRun", "overwrite"] {
        if p.get(field).is_some_and(|v| !v.is_boolean()) {
            return Err(bad("file.projectManager", format!("{field} must be a boolean")));
        }
    }
    if p.get("projectName").is_some_and(|v| !v.is_string()) {
        return Err(bad("file.projectManager", "projectName must be a string"));
    }
    let name = project_name(s, p)?;
    let project = format!("{name}.scproj");
    let dest = PathBuf::from(str_p(p, "destination").ok_or_else(|| bad("file.projectManager", "need destination"))?);
    let dest = std::path::absolute(dest).map_err(|e| EngineError::Other(io(e)))?;
    if !dest.file_name().and_then(|s| s.to_str()).is_some_and(component) {
        return Err(bad("file.projectManager", "destination must name a new portable collection folder"));
    }
    let parent = dest.parent().ok_or_else(|| bad("file.projectManager", "destination needs a parent"))?;
    let parent = fs::canonicalize(parent).map_err(|e| EngineError::Other(format!("collection parent must already exist: {e}")))?;
    let dest = parent.join(dest.file_name().ok_or_else(|| bad("file.projectManager", "need destination name"))?);
    let operation: DocumentId = match p.get("operationId") {
        Some(id) => serde_json::from_value(id.clone()).map_err(|e| bad("file.projectManager", format!("operationId must be a UUID: {e}")))?,
        None => DocumentId::default(),
    };
    let mut request = p.clone();
    if let Some(obj) = request.as_object_mut() {
        for field in ["wait", "dryRun", "overwrite", "operationId", "destination"] {
            obj.remove(field);
        }
    }
    let mut hash = Sha256::new();
    hash.update(s.document_snapshot().encode(None, false).map_err(EngineError::Other)?);
    hash.update(serde_json::to_vec(&request).map_err(|e| EngineError::Other(io(e)))?);
    let request = format!("{:x}", hash.finalize());
    if fs::symlink_metadata(&dest).is_ok() {
        let receipt = verify(&dest, &Progress::default())
            .map_err(|e| EngineError::Other(format!("DESTINATION_EXISTS: existing collection cannot be overwritten: {e}")))?;
        if receipt.operation_id != operation
            || receipt.request_sha256 != request
            || receipt.source_project_id != s.document_id
            || receipt.source_revision != s.revision
        {
            return Err(EngineError::Other("OPERATION_CONFLICT: destination belongs to another collection/request; choose a new folder".into()));
        }
        return Ok(
            json!({"project": dest.join(&receipt.project), "manifest": dest.join(MANIFEST), "operationId": operation, "state": "committed", "replayed": true}),
        );
    }
    let plan = Plan::prepare(s, p)?;
    let m = Manifest {
        version: 1,
        operation_id: operation,
        request_sha256: request,
        source_project_id: s.document_id,
        source_revision: s.revision,
        project_id: plan.snapshot.project_id,
        revision: plan.snapshot.revision,
        document_mode: if plan.snapshot.project_id == s.document_id { "fullDocument" } else { "derivedCopy" }.into(),
        undo_states: plan.snapshot.history.undo.len(),
        redo_states: plan.snapshot.history.redo.len(),
        operation_receipts: plan.snapshot.operations.len(),
        project,
        project_sha256: String::new(),
        assets: Vec::new(),
        embedded: ["editorial project", "LUT text", "graphics/captions", "transcripts", "narration metadata", "project notes"].map(str::to_string).to_vec(),
        rebuildable_caches: ["render previews", "waveforms", "thumbnails", "auto-save/recovery files"].map(str::to_string).to_vec(),
        requirements: vec!["Compatible ShotClutch build and decoders; system fonts are not bundled or qualified by byte verification".into()],
    };
    let total: u64 = plan.copies.iter().map(|c| c.asset.bytes).sum();
    let files: Vec<_> = plan
        .copies
        .iter()
        .map(|c| json!({"item": c.asset.source_id.parse::<u64>().ok(), "to": c.asset.path, "bytes": c.asset.bytes, "role": c.asset.role}))
        .collect();
    let mut out = json!({"project": dest.join(&m.project), "manifest": dest.join(MANIFEST), "operationId": operation, "documentMode": m.document_mode,
        "projectId": m.project_id, "sourceProjectId": m.source_project_id, "sourceRevision": m.source_revision.to_string(),
        "undoStates": m.undo_states, "redoStates": m.redo_states, "operationReceipts": m.operation_receipts,
        "mode": "collect", "originalBytes": total, "resultBytes": total, "files": files, "excluded": plan.excluded,
        "rebuildableCachesOmitted": m.rebuildable_caches, "requirements": m.requirements});
    if bool_p(p, "dryRun").unwrap_or(false) {
        out["state"] = json!("planned");
        return Ok(out);
    }
    let id = s.jobs.iter().map(|j| j.id).max().unwrap_or(0) + 1;
    let job = crate::Job { id, label: format!("Collect: {name}"), progress: Default::default(), result: Default::default() };
    job.progress.total.store(plan.copies.len() as u64 + 1, Ordering::Relaxed);
    let (progress, result) = (job.progress.clone(), job.result.clone());
    let services = s.services.clone();
    let path = dest.join(&m.project).to_string_lossy().into_owned();
    let run = move || {
        let start = web_time::Instant::now();
        let r = execute(plan, m, &dest, &*services, &progress).map(|bytes| shotclutch_export::Report {
            path,
            frames: 0,
            seconds: start.elapsed().as_secs_f64(),
            bytes,
            render_fps: 0.0,
            extra_files: Vec::new(),
        });
        if let Err(e) = &r {
            *progress.error.lock().unwrap_or_else(|e| e.into_inner()) = Some(e.clone());
        }
        *progress.status.lock().unwrap_or_else(|e| e.into_inner()) =
            if r.is_ok() { "Collection published and verified".into() } else { "Collection failed; destination preserved".into() };
        *result.lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
        progress.finished.store(true, Ordering::Release);
    };
    let run = crate::export_tools::guard_job(job.progress.clone(), job.result.clone(), run);
    s.jobs.push(job);
    if bool_p(p, "wait").unwrap_or(false) {
        run();
        let job = s.jobs.last().ok_or_else(|| EngineError::Other("collection job missing".into()))?;
        let result = job.result.lock().unwrap_or_else(|e| e.into_inner());
        match result.as_ref() {
            Some(Ok(_)) => out["state"] = json!("committed"),
            Some(Err(e)) => return Err(EngineError::Other(e.clone())),
            None => return Err(EngineError::Other("collection job result missing".into())),
        }
    } else {
        std::thread::Builder::new().name("shotclutch-collect".into()).spawn(run).map_err(|e| EngineError::Other(io(e)))?;
        out["state"] = json!("queued");
    }
    out["job"] = json!(id);
    Ok(out)
}

pub(crate) fn commands() -> Vec<CommandSpec> {
    vec![CommandSpec {
        id: "file.verifyCollection",
        label: "Verify Collected Project",
        menu: &[],
        shortcut: None,
        params: r#"{"path":str}"#,
        enabled: always,
        journal: false,
        run: |_, p| {
            let path = str_p(p, "path").ok_or_else(|| bad("file.verifyCollection", "need collection folder path"))?;
            let m = verify(Path::new(path), &Progress::default()).map_err(EngineError::Other)?;
            Ok(json!({"verified": true, "project": Path::new(path).join(m.project), "projectId": m.project_id, "revision": m.revision.to_string(),
                "operationId": m.operation_id, "documentMode": m.document_mode, "assets": m.assets.len(), "undoStates": m.undo_states, "redoStates": m.redo_states,
                "operationReceipts": m.operation_receipts, "requirements": m.requirements}))
        },
    }]
}

#[cfg(test)]
#[path = "m1_collection_tests.rs"]
mod tests;
