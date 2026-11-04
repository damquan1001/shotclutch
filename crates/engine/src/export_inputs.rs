//! Job-local originals. Every decoder read is checked against the pre-render block manifest;
//! a decoder cannot silently substitute bytes from a reopened file or the live preview cache.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use shotclutch_media::{ByteReader, SharedReader};
use shotclutch_project::{ItemId, MediaRef, Project};

use crate::{MediaPool, Services};

const BLOCK: usize = 256 * 1024;
const MAX_FILES: usize = 100_000;
const MAX_BYTES: u64 = 64 * 1024 * 1024 * 1024;

fn changed() -> io::Error {
    io::Error::other("MEDIA_CHANGED: an original changed during export")
}
pub(crate) fn check_cancel(progress: &shotclutch_export::Progress) -> Result<(), String> {
    if progress.cancel.load(Ordering::Acquire) { Err("CANCELED: export stopped before publication".into()) } else { Ok(()) }
}

pub(crate) struct BoundReader {
    raw: SharedReader,
    hashes: Vec<[u8; 32]>,
    pub(crate) digest: String,
    invalid: AtomicBool,
}
impl BoundReader {
    pub(crate) fn bind(raw: SharedReader, progress: &shotclutch_export::Progress) -> Result<Self, String> {
        let mut hashes = Vec::new();
        let mut whole = Sha256::new();
        let mut buffer = vec![0; BLOCK];
        let mut offset = 0;
        while offset < raw.len() {
            check_cancel(progress)?;
            let n = (raw.len() - offset).min(BLOCK as u64) as usize;
            raw.read_at(offset, &mut buffer[..n]).map_err(|_| "MEDIA_CHANGED: original could not be fingerprinted")?;
            whole.update(&buffer[..n]);
            hashes.push(Sha256::digest(&buffer[..n]).into());
            offset += n as u64;
        }
        Ok(Self { raw, hashes, digest: format!("{:x}", whole.finalize()), invalid: AtomicBool::new(false) })
    }
}
impl ByteReader for BoundReader {
    fn len(&self) -> u64 {
        self.raw.len()
    }
    fn read_at(&self, offset: u64, out: &mut [u8]) -> io::Result<()> {
        if self.invalid.load(Ordering::Acquire) {
            return Err(changed());
        }
        let end = offset.checked_add(out.len() as u64).filter(|end| *end <= self.len()).ok_or_else(|| io::Error::from(io::ErrorKind::UnexpectedEof))?;
        let mut at = offset;
        let mut buffer = vec![0; BLOCK];
        while at < end {
            let block = at / BLOCK as u64;
            let start = block * BLOCK as u64;
            let n = (self.len() - start).min(BLOCK as u64) as usize;
            if self.raw.read_at(start, &mut buffer[..n]).is_err() || self.hashes.get(block as usize) != Some(&<[u8; 32]>::from(Sha256::digest(&buffer[..n]))) {
                self.invalid.store(true, Ordering::Release);
                return Err(changed());
            }
            let from = (at - start) as usize;
            let count = (end - at).min((n - from) as u64) as usize;
            let dest = (at - offset) as usize;
            out[dest..dest + count].copy_from_slice(&buffer[from..from + count]);
            at += count as u64;
        }
        Ok(())
    }
}

pub(crate) fn reader(services: &dyn Services, path: &str, remaining: u64) -> io::Result<SharedReader> {
    let size = services.file_size(path)?;
    if size > remaining {
        return Err(io::Error::other("RESOURCE_LIMIT: export originals exceed 64 GiB"));
    }
    let raw = match services.reader(path) {
        Some(raw) => raw?,
        None => Arc::new(shotclutch_media::reader::MemReader(services.read_file(path)?.into())) as SharedReader,
    };
    if raw.len() != size {
        return Err(changed());
    }
    Ok(raw)
}

/// Only sources present when the job planned its originals can be read. Missing files stay
/// missing for this job; image sequence membership is also frozen, including its holes.
struct PinnedServices {
    readers: Arc<BTreeMap<String, Arc<BoundReader>>>,
}
impl PinnedServices {
    fn get(&self, path: &str) -> io::Result<SharedReader> {
        self.readers.get(path).cloned().map(|r| r as SharedReader).ok_or_else(|| io::ErrorKind::NotFound.into())
    }
}
impl Services for PinnedServices {
    fn read_file(&self, path: &str) -> io::Result<Vec<u8>> {
        let r = self.get(path)?;
        let len = usize::try_from(r.len()).map_err(|_| io::Error::other("RESOURCE_LIMIT: source does not fit in memory"))?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(len).map_err(|_| io::Error::other("RESOURCE_LIMIT: source does not fit in memory"))?;
        bytes.resize(len, 0);
        r.read_at(0, &mut bytes)?;
        Ok(bytes)
    }
    fn write_file(&self, _: &str, _: &[u8]) -> io::Result<()> {
        Err(io::ErrorKind::PermissionDenied.into())
    }
    fn reader(&self, path: &str) -> Option<io::Result<SharedReader>> {
        Some(self.get(path))
    }
    fn file_size(&self, path: &str) -> io::Result<u64> {
        self.get(path).map(|r| r.len())
    }
    fn list_dir(&self, dir: &str) -> Option<io::Result<Vec<String>>> {
        let dir = std::path::Path::new(dir);
        Some(Ok(self
            .readers
            .keys()
            .filter_map(|p| {
                let p = std::path::Path::new(p);
                (p.parent().unwrap_or_else(|| std::path::Path::new("")) == dir).then(|| p.file_name().map(|p| p.to_string_lossy().into_owned())).flatten()
            })
            .collect()))
    }
    fn file_loader(&self) -> Option<shotclutch_media::sequence::FrameLoader> {
        let readers = self.readers.clone();
        Some(Arc::new(move |path| PinnedServices { readers: readers.clone() }.read_file(path)))
    }
}

pub(crate) struct Inputs {
    pub pool: Arc<MediaPool>,
    pub services: Arc<dyn Services>,
    pub manifest: Vec<Value>,
    pub paths: Vec<PathBuf>,
    pub sequences: Vec<shotclutch_media::sequence::Numbered>,
    readers: BTreeMap<String, Arc<BoundReader>>,
}
impl Inputs {
    pub fn prepare(
        pool: Arc<MediaPool>,
        project: &Project,
        sequence: ItemId,
        services: &dyn Services,
        progress: &shotclutch_export::Progress,
    ) -> Result<Self, String> {
        if project.nest_cycle().is_some() {
            return Err("VERIFICATION_FAILED: cyclic nesting must be repaired before export".into());
        }
        let mut sequences = vec![sequence];
        let mut visited = BTreeSet::new();
        let mut sources = BTreeMap::new();
        while let Some(id) = sequences.pop() {
            check_cancel(progress)?;
            if !visited.insert(id) {
                continue;
            }
            if visited.len() > 512 {
                return Err("RESOURCE_LIMIT: export nesting limit exceeded".into());
            }
            let seq = project.sequence(id).ok_or("NOT_FOUND: export sequence is unavailable")?;
            for clip in seq.all_tracks().flat_map(|t| &t.items) {
                if project.sequence(clip.item).is_some() {
                    sequences.push(clip.item);
                } else if let Some((id, media, _)) = project.resolve_media(clip.item) {
                    sources.insert(id, media);
                } else if project.item(clip.item).is_none_or(|i| matches!(i.kind, shotclutch_project::ItemKind::Subclip { .. })) {
                    return Err("MEDIA_CHANGED: export source item is unavailable".into());
                }
            }
        }
        if sources.len() > 512 {
            return Err("RESOURCE_LIMIT: export exceeds 512 original media items".into());
        }
        let mut manifest = Vec::new();
        let mut readers = BTreeMap::new();
        let mut remaining = MAX_BYTES;
        for (id, media) in sources {
            check_cancel(progress)?;
            if let Some((bytes, digest)) = pool.owned_binding(id, media) {
                remaining = remaining.checked_sub(bytes.len() as u64).ok_or("RESOURCE_LIMIT: export originals exceed 64 GiB")?;
                let MediaRef::File { path } = &media.media else { return Err("INTERNAL_ERROR: owned original has no reference".into()) };
                let name = std::path::Path::new(path).file_name().unwrap_or_default().to_string_lossy();
                let size = bytes.len();
                let source = pool.open_bytes(&name, bytes).map_err(|e| e.to_string())?;
                // A new decoder, backed by the retained immutable original bytes. Do not copy
                // either preview decoder state or its offline-status wrapper into this job.
                pool.insert_file(id, path, source);
                manifest.push(json!({"sourceId": id.0.to_string(), "state": "ownedOriginal", "sha256": digest, "bytes": size.to_string()}));
                continue;
            }
            if pool.host_supplied(id) {
                manifest.push(json!({"sourceId": id.0.to_string(), "state": "hostProvided"}));
                continue;
            }
            let MediaRef::File { path } = &media.media else { continue };
            let path = match pool.follow(path) {
                Ok(path) if !media.offline => path,
                _ => {
                    manifest.push(json!({"sourceId": id.0.to_string(), "state": "offline"}));
                    continue;
                }
            };
            let paths = if media.info.kind == shotclutch_media::MediaKind::ImageSequence {
                match crate::media_pool::image_sequence_frames(&path, services) {
                    Ok(frames) => {
                        if frames.len() > 100_000 {
                            return Err("RESOURCE_LIMIT: image sequence is too long".into());
                        }
                        frames.into_iter().flatten().collect::<Vec<_>>()
                    }
                    Err(_) => {
                        manifest.push(json!({"sourceId": id.0.to_string(), "state": "offline"}));
                        continue;
                    }
                }
            } else {
                vec![path.clone()]
            };
            for path in paths {
                if !readers.contains_key(&path) {
                    if readers.len() >= MAX_FILES {
                        return Err("RESOURCE_LIMIT: export exceeds 100000 original files".into());
                    }
                    let raw = match reader(services, &path, remaining) {
                        Ok(raw) => raw,
                        Err(e) if e.kind() == io::ErrorKind::NotFound => {
                            manifest.push(json!({"sourceId": id.0.to_string(), "state": "offline"}));
                            continue;
                        }
                        Err(e) => return Err(e.to_string()),
                    };
                    remaining -= raw.len();
                    let bound = Arc::new(BoundReader::bind(raw, progress)?);
                    readers.insert(path.clone(), bound);
                }
                let bound = &readers[&path];
                // Imported fast identity is an additional guard, never a substitute for SHA-256.
                if media.info.kind != shotclutch_media::MediaKind::ImageSequence
                    && let Some(identity) = media.identity
                {
                    let n = bound.len();
                    let span = crate::relink::FINGERPRINT_SPAN as u64;
                    let mut head = vec![0; n.min(span) as usize];
                    let mut tail = vec![0; n.saturating_sub(head.len() as u64).min(span) as usize];
                    bound.read_at(0, &mut head).and_then(|_| bound.read_at(n - tail.len() as u64, &mut tail)).map_err(|e| e.to_string())?;
                    if identity.size != n || identity.fingerprint != crate::relink::fingerprint(n, &head, &tail) {
                        return Err(changed().to_string());
                    }
                }
                manifest.push(json!({"sourceId": id.0.to_string(), "state": "original", "sha256": bound.digest, "bytes": bound.len().to_string(), "frameName": if media.info.kind == shotclutch_media::MediaKind::ImageSequence { std::path::Path::new(&path).file_name().map(|s| s.to_string_lossy().into_owned()) } else { None }}));
            }
        }
        let mut paths: Vec<PathBuf> = readers.keys().map(PathBuf::from).collect();
        let mut sequences = Vec::new();
        // Destinations cannot clobber any project original/proxy, including unused/offline
        // sources. Protect numbered sibling names even when a frame is currently missing.
        for media in project.items.values().filter_map(|item| item.as_media()) {
            for reference in [Some(&media.media), media.proxy.as_ref()].into_iter().flatten() {
                if let MediaRef::File { path } = reference
                    && let Ok(path) = pool.follow(path)
                {
                    paths.push(PathBuf::from(path));
                }
            }
            if media.info.kind == shotclutch_media::MediaKind::ImageSequence
                && let MediaRef::File { path } = &media.media
                && let Ok(path) = pool.follow(path)
                && let Some(sequence) = shotclutch_media::sequence::Numbered::parse(&if cfg!(windows) { path.to_lowercase() } else { path })
            {
                sequences.push(sequence);
            }
        }
        let services = Arc::new(PinnedServices { readers: Arc::new(readers.clone()) });
        Ok(Self { pool, services, manifest, paths, sequences, readers })
    }
    pub fn verify(&self, services: &dyn Services, progress: &shotclutch_export::Progress) -> Result<(), String> {
        for (path, bound) in &self.readers {
            check_cancel(progress)?;
            if bound.invalid.load(Ordering::Acquire) {
                return Err(changed().to_string());
            }
            let raw = reader(services, path, bound.len()).map_err(|_| changed().to_string())?;
            if BoundReader::bind(raw, progress)?.digest != bound.digest {
                return Err(changed().to_string());
            }
        }
        check_cancel(progress)
    }
}
