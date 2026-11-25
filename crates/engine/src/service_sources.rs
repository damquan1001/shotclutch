//! Confined, bounded source reads for transaction verification and media observations. These capabilities
//! are issued by trusted host code, never inferred from paths in an imported project.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use web_time::Instant;

use crate::{
    Session,
    service::{Cancellation, ErrorCode, ServiceError},
};
#[cfg(not(target_arch = "wasm32"))]
use shotclutch_project::observations::SourceFingerprint;
use shotclutch_project::{ItemId, MediaIdentity, MediaRef};

fn error(code: ErrorCode, message: &'static str) -> ServiceError {
    ServiceError { code, message: message.into() }
}

pub(crate) struct ReadRoot {
    #[cfg(not(target_arch = "wasm32"))]
    path: PathBuf,
    #[cfg(not(target_arch = "wasm32"))]
    alias: PathBuf,
    #[cfg(not(target_arch = "wasm32"))]
    dir: cap_std::fs::Dir,
}
impl ReadRoot {
    pub fn open(path: &Path) -> Result<Self, ServiceError> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if network_path(path) {
                return Err(error(ErrorCode::PermissionDenied, "network media roots are not supported by the local editor service"));
            }
            let alias = std::path::absolute(path).map_err(|_| error(ErrorCode::NotFound, "authorized media folder not found"))?;
            let path = std::fs::canonicalize(path).map_err(|_| error(ErrorCode::NotFound, "authorized media folder not found"))?;
            if network_path(&path) {
                return Err(error(ErrorCode::PermissionDenied, "resolved media folder requires a separate capability"));
            }
            let dir = cap_std::fs::Dir::open_ambient_dir(&path, cap_std::ambient_authority())
                .map_err(|_| error(ErrorCode::PermissionDenied, "media folder could not be authorized"))?;
            Ok(Self { path, alias, dir })
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = path;
            Err(error(ErrorCode::CapabilityUnavailable, "native media roots are unavailable on this host"))
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn network_path(path: &Path) -> bool {
    path.components().any(
        |part| matches!(part, std::path::Component::Prefix(prefix) if !matches!(prefix.kind(), std::path::Prefix::Disk(..) | std::path::Prefix::VerbatimDisk(..))),
    )
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct SourceBinding {
    #[serde(rename = "sourceId", with = "crate::service::item_id")]
    item: ItemId,
    proxy: bool,
    #[serde(rename = "sha256")]
    digest: String,
    #[serde(with = "shotclutch_format::session::decimal_u64")]
    bytes: u64,
}

#[cfg(not(target_arch = "wasm32"))]
impl SourceBinding {
    pub(crate) fn matches_original(&self, item: ItemId, fingerprint: &SourceFingerprint) -> bool {
        !self.proxy && self.item == item && self.digest == fingerprint.sha256 && self.bytes == fingerprint.bytes
    }
}

fn files(session: &Session, sequence: ItemId) -> Result<Vec<(ItemId, bool, String, Option<MediaIdentity>)>, ServiceError> {
    let mut sequences = vec![sequence];
    let mut visited = BTreeSet::new();
    let mut media = BTreeMap::new();
    while let Some(sequence) = sequences.pop() {
        if !visited.insert(sequence) {
            continue;
        }
        if visited.len() > 512 {
            return Err(error(ErrorCode::ResourceLimit, "nested sequence verification budget exceeded"));
        }
        let sequence = session.project.sequence(sequence).ok_or_else(|| error(ErrorCode::NotFound, "nested sequence not found"))?;
        for clip in sequence.all_tracks().flat_map(|track| &track.items) {
            let item = session.project.item(clip.item).ok_or_else(|| error(ErrorCode::MediaChanged, "referenced source item is missing"))?;
            if session.project.sequence(clip.item).is_some() {
                sequences.push(clip.item);
                continue;
            }
            if let Some((id, source, _)) = session.project.resolve_media(clip.item) {
                if source.offline {
                    return Err(error(ErrorCode::MediaChanged, "a referenced source is offline"));
                }
                if source.info.kind == shotclutch_media::MediaKind::ImageSequence {
                    return Err(error(ErrorCode::CapabilityUnavailable, "numbered image sources require a complete sequence manifest"));
                }
                if let MediaRef::File { path } = &source.media {
                    media.insert((id, false), (path.clone(), source.identity));
                }
                if let Some(MediaRef::File { path }) = &source.proxy {
                    media.insert((id, true), (path.clone(), None));
                }
            } else if matches!(item.kind, shotclutch_project::ItemKind::Subclip { .. }) {
                return Err(error(ErrorCode::MediaChanged, "subclip source chain is unavailable"));
            }
        }
    }
    if media.len() > 512 {
        return Err(error(ErrorCode::ResourceLimit, "source verification budget exceeded"));
    }
    Ok(media.into_iter().map(|((id, proxy), (path, identity))| (id, proxy, path, identity)).collect())
}

pub(crate) const MAX_SNAPSHOT_BYTES: u64 = 64 * 1024 * 1024;

pub(crate) struct ReadContext<'a> {
    pub roots: &'a [Arc<ReadRoot>],
    pub budget: u64,
    pub cancellation: &'a Cancellation,
    pub revoked: &'a Arc<AtomicBool>,
    pub expires: Instant,
}
impl ReadContext<'_> {
    pub(crate) fn check(&self) -> Result<(), ServiceError> {
        self.cancellation.check()?;
        if self.revoked.load(Ordering::Acquire) || Instant::now() >= self.expires {
            return Err(error(ErrorCode::PermissionDenied, "authorization was revoked or expired"));
        }
        Ok(())
    }
}

pub(crate) fn capture(session: &Session, sequence: ItemId, context: &ReadContext<'_>) -> Result<Vec<SourceBinding>, ServiceError> {
    context.check()?;
    if session.project.nest_cycle().is_some() {
        return Err(error(ErrorCode::VerificationFailed, "cyclic sequence nesting must be repaired before a structural transaction"));
    }
    let mut bindings = Vec::new();
    let mut remaining = context.budget;
    for (item, proxy, path, identity) in files(session, sequence)? {
        context.check()?;
        let (digest, bytes) = hash_file(session, &path, identity, remaining, context)?;
        remaining = remaining.checked_sub(bytes).ok_or_else(|| error(ErrorCode::ResourceLimit, "source read budget exceeded"))?;
        bindings.push(SourceBinding { item, proxy, digest, bytes });
    }
    context.check()?;
    Ok(bindings)
}

pub(crate) fn verify(session: &Session, sequence: ItemId, expected: &[SourceBinding], context: &ReadContext<'_>) -> Result<(), ServiceError> {
    if capture(session, sequence, context)? != expected {
        return Err(error(ErrorCode::MediaChanged, "source content changed since preview; relink or prepare again"));
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn hash_file(
    session: &Session,
    stored: &str,
    expected: Option<MediaIdentity>,
    mut budget: u64,
    context: &ReadContext<'_>,
) -> Result<(String, u64), ServiceError> {
    let (root, relative) = confined_source(session, stored, context)?;
    let fingerprint = hash_confined_file(root, &relative, expected, &mut budget, context, false, None)?;
    Ok((fingerprint.sha256, fingerprint.bytes))
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn observation_file(
    session: &Session,
    stored: &str,
    expected: Option<MediaIdentity>,
    offline: bool,
    context: &ReadContext<'_>,
    snapshot: Option<&mut Vec<u8>>,
) -> Result<SourceFingerprint, ServiceError> {
    let (root, relative) = confined_source(session, stored, context)?;
    let mut budget = if snapshot.is_some() { context.budget.min(MAX_SNAPSHOT_BYTES) } else { context.budget };
    hash_confined_file(root, &relative, expected, &mut budget, context, offline, snapshot)
}

/// Validate the locator before returning even derived coverage for the asset.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn authorize_search_source(session: &Session, stored: &str, context: &ReadContext<'_>) -> Result<(), ServiceError> {
    let (root, relative) = confined_source(session, stored, context)?;
    match root.dir.metadata(relative) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(error(ErrorCode::PermissionDenied, "source metadata is outside the read capability")),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn search_fingerprint(
    session: &Session,
    stored: &str,
    offline: bool,
    context: &ReadContext<'_>,
    remaining: &mut u64,
) -> Result<SourceFingerprint, ServiceError> {
    let (root, relative) = confined_source(session, stored, context)?;
    hash_confined_file(root, &relative, None, remaining, context, offline, None)
}

#[cfg(not(target_arch = "wasm32"))]
fn confined_source<'a>(session: &Session, stored: &str, context: &ReadContext<'a>) -> Result<(&'a ReadRoot, PathBuf), ServiceError> {
    context.check()?;
    let followed = session.media.follow(stored).map_err(|_| error(ErrorCode::PermissionDenied, "stored media location is not authorized"))?;
    if network_path(Path::new(&followed)) {
        return Err(error(ErrorCode::PermissionDenied, "network sources require a separate capability"));
    }
    let followed = Path::new(&followed);
    context
        .roots
        .iter()
        .find_map(|root| followed.strip_prefix(&root.path).or_else(|_| followed.strip_prefix(&root.alias)).ok().map(|p| (root.as_ref(), p.to_path_buf())))
        .ok_or_else(|| error(ErrorCode::PermissionRequired, "source folder requires an explicit host read grant"))
}

#[cfg(not(target_arch = "wasm32"))]
fn hash_confined_file(
    root: &ReadRoot,
    relative: &Path,
    expected: Option<MediaIdentity>,
    remaining: &mut u64,
    context: &ReadContext<'_>,
    offline: bool,
    mut snapshot: Option<&mut Vec<u8>>,
) -> Result<SourceFingerprint, ServiceError> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    // Resolve even metadata through the capability: ambient canonicalization here would
    // disclose existence outside the grant and would race a junction replacement.
    let metadata = root.dir.metadata(relative).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            error(ErrorCode::PermissionDenied, "source resolves outside the granted folder")
        } else {
            error(ErrorCode::MediaChanged, "source is missing or unavailable")
        }
    })?;
    if !metadata.is_file() {
        return Err(error(ErrorCode::MediaChanged, "source is not a regular file"));
    }
    if offline {
        return Err(error(ErrorCode::MediaChanged, "original is offline"));
    }
    // cap-std resolves paths relative to the retained directory handle and refuses escapes,
    // including symlink/junction races. No ambient open occurs after granting the root.
    let mut file = root.dir.open(relative).map_err(|_| error(ErrorCode::PermissionDenied, "source could not be opened inside the granted folder"))?;
    let before = file.metadata().map_err(|_| error(ErrorCode::MediaChanged, "source metadata is unavailable"))?;
    if !before.is_file() {
        return Err(error(ErrorCode::MediaChanged, "source is not a regular file"));
    }
    if before.len() > *remaining {
        return Err(error(ErrorCode::ResourceLimit, "source read budget exceeded"));
    }
    if let Some(contents) = snapshot.as_mut() {
        contents.try_reserve_exact(before.len() as usize).map_err(|_| error(ErrorCode::ResourceLimit, "could not allocate an original snapshot"))?;
    }
    let mut hash = Sha256::new();
    let mut buffer = vec![0u8; crate::relink::FINGERPRINT_SPAN];
    let mut head = Vec::new();
    let mut tail = Vec::new();
    let mut bytes = 0u64;
    loop {
        context.check()?;
        // Read at most the initial length. The final metadata check detects growth.
        if bytes == before.len() {
            break;
        }
        let wanted = (before.len() - bytes).min(buffer.len() as u64) as usize;
        context.cancellation.charge_run_work(crate::service::runs::Work::SourceBytes, wanted as u64)?;
        let count = file.read(&mut buffer[..wanted]).map_err(|_| error(ErrorCode::MediaChanged, "source read failed"))?;
        if count == 0 {
            break;
        }
        *remaining -= count as u64;
        bytes += count as u64;
        if bytes > before.len() {
            return Err(error(ErrorCode::MediaChanged, "source changed during verification"));
        }
        hash.update(&buffer[..count]);
        if let Some(contents) = snapshot.as_mut() {
            contents.extend_from_slice(&buffer[..count]);
        }
        if expected.is_some() {
            let head_count = count.min(crate::relink::FINGERPRINT_SPAN - head.len());
            head.extend_from_slice(&buffer[..head_count]);
            tail.extend_from_slice(&buffer[..count]);
            if tail.len() > crate::relink::FINGERPRINT_SPAN {
                tail.drain(..tail.len() - crate::relink::FINGERPRINT_SPAN);
            }
        }
    }
    let after = file.metadata().map_err(|_| error(ErrorCode::MediaChanged, "source metadata is unavailable"))?;
    if bytes != before.len() || after.len() != before.len() || after.modified().ok() != before.modified().ok() {
        return Err(error(ErrorCode::MediaChanged, "source changed during verification"));
    }
    if let Some(expected) = expected {
        let import_tail = if bytes > 2 * crate::relink::FINGERPRINT_SPAN as u64 { &tail[..] } else { &tail[tail.len() - (bytes as usize - head.len())..] };
        if expected.size != bytes || expected.fingerprint != crate::relink::fingerprint(bytes, &head, import_tail) {
            return Err(error(ErrorCode::MediaChanged, "source no longer matches its imported identity"));
        }
    }
    context.check()?;
    Ok(SourceFingerprint { sha256: format!("{:x}", hash.finalize()), bytes })
}

#[cfg(target_arch = "wasm32")]
fn hash_file(_: &Session, _: &str, _: Option<MediaIdentity>, _: u64, _: &ReadContext<'_>) -> Result<(String, u64), ServiceError> {
    Err(error(ErrorCode::CapabilityUnavailable, "source verification requires a native read capability"))
}
