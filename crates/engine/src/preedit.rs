//! Owner-side Smart Rough Cut commands. Scoped clients do not dispatch these commands.

mod analysis;
mod assembly;
#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::{Arc, mpsc};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use shotclutch_format::session::{DocumentId, OperationRecord, decimal_u64};
use shotclutch_project::preedit::*;
use shotclutch_project::{ItemId, MediaClip, MediaRef};
use shotclutch_time::{TICKS_PER_SECOND, Tick, TimeRange};

use crate::commands::CommandSpec;
use crate::service::Cancellation;
use crate::{EngineError, Result, Session};

pub struct PendingAnalysis {
    document: DocumentId,
    item: ItemId,
    original_path: PathBuf,
    analysis: Option<u64>,
    previous_map: Option<u64>,
    receiver: mpsc::Receiver<Message>,
    cancel: Cancellation,
}

pub struct PendingApplication {
    document: DocumentId,
    request: Apply,
    receiver: mpsc::Receiver<std::result::Result<VerifiedOriginal, String>>,
    cancel: Cancellation,
}

struct VerifiedOriginal {
    path: PathBuf,
    modified: std::time::SystemTime,
    fingerprint: (String, u64),
}

impl Drop for PendingApplication {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

pub(crate) fn clear_jobs(s: &mut Session) {
    s.preedit_job = None;
    s.preedit_apply = None;
    s.preedit_apply_result = None;
}

impl Drop for PendingAnalysis {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

enum Message {
    Started(PreEditAnalysis),
    Chunk(Arc<AnalysisChunk>),
    Finished(std::result::Result<(), String>),
}

fn fail(text: impl Into<String>) -> EngineError {
    EngineError::Other(text.into())
}

fn stopped(cancel: &Cancellation) -> std::result::Result<(), String> {
    cancel.check().map_err(|e| e.to_string())
}

fn typed<T: serde::de::DeserializeOwned>(value: &Value) -> Result<T> {
    serde_json::from_value(value.clone()).map_err(|e| fail(format!("INVALID_ARGUMENT: {e}")))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Start {
    #[serde(with = "crate::service::item_id")]
    item: ItemId,
    audio_stream: usize,
    channel: Option<u16>,
    model: Option<String>,
    language: Option<String>,
    range: Option<ExactRange>,
    #[serde(default, with = "optional_decimal")]
    previous_map: Option<u64>,
    #[serde(default, with = "optional_decimal")]
    resume: Option<u64>,
    min_pause_ms: Option<u32>,
    handle_ms: Option<u32>,
}

mod optional_decimal {
    use serde::{Deserialize, Deserializer, de::Error};
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
        Option::<String>::deserialize(d)?
            .map(|s| {
                let n = s.parse::<u64>().map_err(D::Error::custom)?;
                if n.to_string() != s {
                    return Err(D::Error::custom("expected a canonical decimal ID"));
                }
                Ok(n)
            })
            .transpose()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExactRange {
    start: String,
    end: String,
}

impl ExactRange {
    fn parse(&self) -> Result<TimeRange> {
        let parse = |s: &str| {
            s.parse::<i64>()
                .ok()
                .filter(|n| n.to_string() == s && *n >= 0)
                .map(Tick)
                .ok_or_else(|| fail("INVALID_ARGUMENT: source ticks must be nonnegative decimal strings"))
        };
        let (start, end) = (parse(&self.start)?, parse(&self.end)?);
        if end <= start {
            return Err(fail("INVALID_ARGUMENT: source range is empty"));
        }
        Ok(TimeRange::from_bounds(start, end))
    }
}

fn original(s: &Session, id: ItemId) -> Result<(&MediaClip, PathBuf)> {
    let (_, media, subclip) = s.project.resolve_media(id).ok_or_else(|| fail("NOT_FOUND: select original media"))?;
    if media.offline {
        return Err(fail("MEDIA_CHANGED: original is offline"));
    }
    let MediaRef::File { path } = &media.media else { return Err(fail("CAPABILITY_UNAVAILABLE: rough cut requires a local original file")) };
    if subclip.is_some() || !matches!(media.info.kind, shotclutch_media::MediaKind::Movie | shotclutch_media::MediaKind::AudioOnly) {
        return Err(fail("CAPABILITY_UNAVAILABLE: select an original audio or movie asset"));
    }
    let path = s.media.follow(path).map_err(|_| fail("PERMISSION_REQUIRED: original location is unavailable"))?;
    let path = PathBuf::from(path);
    #[cfg(not(target_arch = "wasm32"))]
    if crate::service_sources::network_path(&path) {
        return Err(fail("PERMISSION_DENIED: rough cut requires a local file"));
    }
    Ok((media, path))
}

fn start(s: &mut Session, value: &Value) -> Result<Value> {
    if cfg!(target_arch = "wasm32") {
        return Err(fail("CAPABILITY_UNAVAILABLE: rough cut analysis requires a native host"));
    }
    if s.preedit_job.is_some() {
        return Err(fail("RESOURCE_LIMIT: a rough-cut analysis is already running"));
    }
    let req: Start = typed(value)?;
    let (media, path) = original(s, req.item)?;
    let audio = media.info.audio_streams.get(req.audio_stream).ok_or_else(|| fail("INVALID_ARGUMENT: select an available audio stream"))?;
    if req.channel.is_some_and(|i| u32::from(i) >= audio.channels) || audio.channels > 32 {
        return Err(fail("INVALID_ARGUMENT: unsupported analysis channel selection"));
    }
    let available = TimeRange::from_bounds(Tick::ZERO, media.duration());
    let requested = req.range.as_ref().map(ExactRange::parse).transpose()?.unwrap_or(available);
    let range = if requested == available {
        requested
    } else {
        let original = indexed_video(s, req.item, media.info.has_video())?;
        assembly::snap_decision(original.as_deref(), audio.sample_rate, available, requested, true)?
    };
    if !encloses(available, range) || range.duration.0 > 4 * 3600 * TICKS_PER_SECOND {
        return Err(fail("INVALID_ARGUMENT: select a source range within four hours"));
    }
    let pause = req.min_pause_ms.unwrap_or(1000);
    let handle = req.handle_ms.unwrap_or(150);
    if !(200..=10_000).contains(&pause) || handle > 1000 {
        return Err(fail("INVALID_ARGUMENT: pause threshold or retained handle is out of bounds"));
    }
    let language = req.language.unwrap_or_else(|| "en".into());
    if language.len() != 2 || !language.bytes().all(|byte| byte.is_ascii_lowercase()) {
        return Err(fail("INVALID_ARGUMENT: enter a two-letter language code"));
    }
    let settings =
        AnalysisSettings { model: req.model, language, min_pause: Tick::from_units(i64::from(pause), 1000), handle: Tick::from_units(i64::from(handle), 1000) };
    let model_digest = analysis::model_digest(&settings).map_err(fail)?;
    let resume = req.resume.map(|id| s.project.preedit.analyses.get(&id).cloned().ok_or_else(|| fail("NOT_FOUND: analysis to resume"))).transpose()?;
    if let Some(saved) = &resume
        && (saved.status == AnalysisStatus::Complete
            || saved.settings != settings
            || saved.analyzer_version != analysis::ANALYZER_VERSION
            || saved.model_digest != model_digest
            || saved.source.item != req.item
            || saved.source.range != range
            || saved.source.audio_stream != req.audio_stream
            || saved.source.channel != req.channel)
    {
        return Err(fail("INVALID_ARGUMENT: resume requires the same source range, stream and settings"));
    }
    if req.previous_map.is_some_and(|id| !s.project.preedit.maps.contains_key(&id)) {
        return Err(fail("NOT_FOUND: previous rough-cut map"));
    }
    let (send, receiver) = mpsc::sync_channel(2);
    let cancel = Cancellation::default();
    let signal = cancel.clone();
    let identity = media.identity;
    let media_pool = s.media.clone();
    let services = s.services.clone();
    let worker_path = path.clone();
    std::thread::Builder::new()
        .name("smart-rough-cut".into())
        .spawn(move || {
            let result = (|| {
                let verified = verify_original(worker_path.clone(), identity, &signal)?;
                let (sha256, bytes) = verified.fingerprint;
                let source = SourceBinding { item: req.item, range, audio_stream: req.audio_stream, channel: req.channel, sha256, bytes };
                if resume.as_ref().is_some_and(|saved| saved.source != source) {
                    return Err("MEDIA_CHANGED: resumed source no longer matches the original".into());
                }
                let path = worker_path.to_str().ok_or("CAPABILITY_UNAVAILABLE: original path is not valid UTF-8")?;
                let decoded = media_pool.probe_file(path, &*services).map_err(|_| "CAPABILITY_UNAVAILABLE: could not open the original audio reader")?;
                if decoded.info().audio_streams.get(req.audio_stream).is_none() {
                    return Err("CAPABILITY_UNAVAILABLE: original audio stream is unavailable".into());
                }
                let analysis = PreEditAnalysis {
                    source,
                    settings,
                    model_digest,
                    analyzer_version: analysis::ANALYZER_VERSION,
                    status: AnalysisStatus::Running,
                    chunks: resume.map_or_else(Vec::new, |a| a.chunks.clone()),
                };
                send.send(Message::Started(analysis.clone())).map_err(|_| "CANCELED: project closed")?;
                analysis::run(decoded, &analysis, &signal, &send)?;
                let after = verify_original(worker_path, identity, &signal)?;
                if after.fingerprint.0 != analysis.source.sha256 || after.modified != verified.modified {
                    return Err("MEDIA_CHANGED: original changed during rough-cut analysis".into());
                }
                Ok(())
            })();
            let _ = send.send(Message::Finished(result));
        })
        .map_err(|_| fail("CAPABILITY_UNAVAILABLE: could not start a native analysis worker"))?;
    s.preedit_job = Some(PendingAnalysis {
        document: s.document_id,
        item: req.item,
        original_path: path,
        analysis: req.resume,
        previous_map: req.previous_map,
        receiver,
        cancel,
    });
    Ok(json!({"state": "running"}))
}

pub(crate) fn poll(s: &mut Session) {
    poll_application(s);
    let Some(mut job) = s.preedit_job.take() else { return };
    if job.document != s.document_id || !matches!(original(s, job.item), Ok((_, path)) if path == job.original_path) {
        return;
    }
    let message = match job.receiver.try_recv() {
        Ok(message) => message,
        Err(mpsc::TryRecvError::Empty) => {
            s.preedit_job = Some(job);
            return;
        }
        Err(mpsc::TryRecvError::Disconnected) => Message::Finished(Err("INTERNAL_ERROR: rough-cut worker stopped".into())),
    };
    match publish(s, &mut job, message) {
        Ok(true) => s.preedit_job = Some(job),
        Ok(false) => {}
        Err(error) => s.error_toast("Smart Rough Cut", error.to_string()),
    }
}

fn publish(s: &mut Session, job: &mut PendingAnalysis, message: Message) -> Result<bool> {
    let mut project = (*s.project).clone();
    project.next_id = project.next_id.max(s.id_watermark);
    let mut terminal = None;
    match message {
        Message::Started(analysis) => {
            let id = job.analysis.unwrap_or_else(|| project.alloc_id());
            job.analysis = Some(id);
            project.preedit.analyses.insert(id, Arc::new(analysis));
        }
        Message::Chunk(chunk) => {
            let saved = job.analysis.and_then(|id| project.preedit.analyses.get_mut(&id)).ok_or_else(|| fail("REVISION_CONFLICT: analysis was removed"))?;
            Arc::make_mut(saved).chunks.push(chunk);
        }
        Message::Finished(result) => {
            let was_canceled = stopped(&job.cancel).is_err();
            let changed = result.as_ref().err().is_some_and(|error| error.starts_with("MEDIA_CHANGED:"));
            if let Some(id) = job.analysis
                && let Some(saved) = project.preedit.analyses.get_mut(&id)
            {
                let saved = Arc::make_mut(saved);
                saved.status = if was_canceled {
                    AnalysisStatus::Canceled
                } else if result.is_ok() {
                    AnalysisStatus::Complete
                } else {
                    AnalysisStatus::Failed
                };
                if !changed && !saved.chunks.is_empty() {
                    let mut map = analysis::propose(id, saved);
                    let source = saved.source.clone();
                    if let Some(old_id) = job.previous_map.filter(|id| project.preedit.maps.contains_key(id)) {
                        // A review action during analysis may have produced a newer revision.
                        let current = current_revision(&project.preedit, old_id);
                        if let Some(old) = project.preedit.maps.get(&current) {
                            map.previous = Some(current);
                            let compatible = project.preedit.analyses.get(&old.analysis).is_some_and(|a| a.source == source);
                            if compatible {
                                map.overrides.clone_from(&old.overrides);
                            } else {
                                map.conflicts.extend(old.overrides.iter().cloned());
                            }
                            map.conflicts.extend(old.conflicts.iter().cloned());
                        }
                    }
                    let map_id = project.alloc_id();
                    project.preedit.maps.insert(map_id, Arc::new(map));
                }
            }
            terminal = Some(if was_canceled { Ok(()) } else { result });
        }
    }
    let prepared = s.prepare_project("Analyze Smart Rough Cut", project, Vec::new())?;
    s.commit_analysis(prepared).map_err(|e| fail(e.to_string()))?;
    if let Some(result) = terminal {
        result.map_err(fail)?;
        Ok(false)
    } else {
        Ok(true)
    }
}

fn current_revision(state: &PreEdit, mut id: u64) -> u64 {
    let from = id;
    for (&next, map) in state.maps.range(from..) {
        if map.previous == Some(id) {
            id = next;
        }
    }
    id
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Review {
    #[serde(with = "decimal_u64")]
    map: u64,
    #[serde(with = "decimal_u64")]
    expected_revision: u64,
    action: Action,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum Action {
    Range { range: ExactRange, decision: Decision },
    Candidate { index: usize, decision: Decision },
    ChooseTake { group: usize, retain: Vec<usize> },
    RejectGroup { group: usize },
    SplitGroup { group: usize, at: usize },
    ResetSuggestions,
    ResetDecisions,
    DismissConflict { index: usize },
    MarkReviewed,
}

fn review(s: &mut Session, value: &Value) -> Result<Value> {
    let req: Review = typed(value)?;
    if s.revision != req.expected_revision || current_revision(&s.project.preedit, req.map) != req.map {
        return Err(fail("REVISION_CONFLICT: rough-cut review changed; refresh the workspace"));
    }
    let mut map = s.project.preedit.maps.get(&req.map).map(|m| (**m).clone()).ok_or_else(|| fail("NOT_FOUND: rough-cut map"))?;
    let analysis = &s.project.preedit.analyses[&map.analysis];
    let (media, _) = original(s, analysis.source.item)?;
    let video = indexed_video(s, analysis.source.item, media.info.has_video())?;
    let sample_rate =
        media.info.audio_streams.get(analysis.source.audio_stream).ok_or_else(|| fail("MEDIA_CHANGED: selected audio stream is missing"))?.sample_rate;
    let snap = |range, inward| assembly::snap_decision(video.as_deref(), sample_rate, analysis.source.range, range, inward);
    map.previous = Some(req.map);
    map.reviewed = false;
    match req.action {
        Action::Range { range, decision } => map.overrides.push(Override { range: snap(range.parse()?, false)?, decision }),
        Action::Candidate { index, decision } => {
            let candidate = map.candidates.get(index).ok_or_else(|| fail("NOT_FOUND: rough-cut candidate"))?;
            let range = snap(candidate.range, candidate.kind == CandidateKind::Pause && decision == Decision::Remove)?;
            if decision == Decision::Remove && crosses_word(analysis, range) {
                return Err(fail("INVALID_ARGUMENT: snapped suggestion crosses another word; adjust and review the range manually"));
            }
            map.overrides.push(Override { range, decision });
        }
        Action::ChooseTake { group, retain } => {
            let group = map.takes.get(group).ok_or_else(|| fail("NOT_FOUND: take group"))?;
            if retain.is_empty() || retain.iter().any(|i| !group.alternatives.contains(i)) {
                return Err(fail("INVALID_ARGUMENT: select at least one take from the group"));
            }
            for index in &group.alternatives {
                let decision = if retain.contains(index) { Decision::Keep } else { Decision::Remove };
                let range = snap(map.candidates[*index].range, false)?;
                if decision == Decision::Remove && crosses_word(analysis, range) {
                    return Err(fail("INVALID_ARGUMENT: take boundary crosses another word; adjust the range manually"));
                }
                map.overrides.push(Override { range, decision });
            }
        }
        Action::RejectGroup { group } => {
            if group >= map.takes.len() {
                return Err(fail("NOT_FOUND: take group"));
            }
            Arc::make_mut(&mut map.takes).remove(group);
        }
        Action::SplitGroup { group, at } => {
            let groups = Arc::make_mut(&mut map.takes);
            let original = groups.get(group).ok_or_else(|| fail("NOT_FOUND: take group"))?;
            if at == 0 || at >= original.alternatives.len() {
                return Err(fail("INVALID_ARGUMENT: split must be inside the group"));
            }
            let original = groups.remove(group);
            for part in [&original.alternatives[..at], &original.alternatives[at..]].into_iter().rev().filter(|part| part.len() > 1) {
                groups.insert(group, TakeGroup { alternatives: part.to_vec(), explanation: original.explanation.clone() });
            }
        }
        Action::ResetSuggestions => {
            let proposed = analysis::propose(map.analysis, analysis);
            map.candidates = proposed.candidates;
            map.takes = proposed.takes;
        }
        Action::ResetDecisions => {
            map.overrides.clear();
            map.conflicts.clear();
        }
        Action::DismissConflict { index } => {
            if index >= map.conflicts.len() {
                return Err(fail("NOT_FOUND: rough-cut decision conflict"));
            }
            map.conflicts.remove(index);
        }
        Action::MarkReviewed => {
            if !map.conflicts.is_empty() {
                return Err(fail("REVISION_CONFLICT: resolve the carried decisions before marking reviewed"));
            }
            if map.segments(analysis.source.range).map_err(fail)?.is_empty() {
                return Err(fail("INVALID_ARGUMENT: retain some source material"));
            }
            map.reviewed = true;
        }
    }
    if map.overrides.len() > 4096 {
        return Err(fail("RESOURCE_LIMIT: too many rough-cut decisions"));
    }
    let mut project = (*s.project).clone();
    project.next_id = project.next_id.max(s.id_watermark);
    let id = project.alloc_id();
    project.preedit.maps.insert(id, Arc::new(map));
    let prepared = s.prepare_project("Review Smart Rough Cut", project, vec![json!({"map": id.to_string()})])?;
    s.commit_edits(prepared).map_err(|e| fail(e.to_string()))?.pop().ok_or_else(|| fail("INTERNAL_ERROR: missing review receipt"))
}

fn crosses_word(analysis: &PreEditAnalysis, range: TimeRange) -> bool {
    analysis
        .chunks
        .iter()
        .filter_map(|chunk| chunk.transcript.as_ref())
        .flat_map(|t| &t.words)
        .any(|word| [range.start, range.end()].into_iter().any(|edge| word.start < edge && edge < word.end))
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Apply {
    #[serde(with = "decimal_u64")]
    map: u64,
    #[serde(with = "decimal_u64")]
    expected_revision: u64,
    #[serde(default, deserialize_with = "optional_decimal::deserialize")]
    replace: Option<u64>,
    operation_id: String,
}

fn apply(s: &mut Session, value: &Value) -> Result<Value> {
    let req: Apply = typed(value)?;
    commit_application(s, req, None)
}

fn start_apply(s: &mut Session, value: &Value) -> Result<Value> {
    if cfg!(target_arch = "wasm32") {
        return Err(fail("CAPABILITY_UNAVAILABLE: application requires a native recovery worker"));
    }
    if s.preedit_apply.is_some() {
        return Err(fail("RESOURCE_LIMIT: a rough cut is already being verified"));
    }
    let req: Apply = typed(value)?;
    if let Some(result) = existing_application(s, &req)? {
        return Ok(result);
    }
    let (_, source) = validate_application(s, &req)?;
    let (media, path) = original(s, source.source.item)?;
    let expected = media.identity;
    let cancel = Cancellation::default();
    let signal = cancel.clone();
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name("rough-cut-apply".into())
        .spawn(move || {
            let _ = sender.send(verify_original(path, expected, &signal));
        })
        .map_err(|_| fail("CAPABILITY_UNAVAILABLE: could not start original verification"))?;
    s.preedit_apply_result = None;
    s.preedit_apply = Some(PendingApplication { document: s.document_id, request: req, receiver, cancel });
    Ok(json!({"state":"verifying"}))
}

fn poll_application(s: &mut Session) {
    let Some(job) = s.preedit_apply.take() else { return };
    if job.document != s.document_id {
        return;
    }
    let original = match job.receiver.try_recv() {
        Ok(result) => result,
        Err(mpsc::TryRecvError::Disconnected) => Err("INTERNAL_ERROR: original verification worker stopped".into()),
        Err(mpsc::TryRecvError::Empty) => {
            s.preedit_apply = Some(job);
            return;
        }
    };
    s.preedit_apply_result =
        Some(stopped(&job.cancel).and(original).and_then(|original| commit_application(s, job.request.clone(), Some(original)).map_err(|e| e.to_string())));
}

fn verify_original(path: PathBuf, identity: Option<shotclutch_project::MediaIdentity>, cancel: &Cancellation) -> std::result::Result<VerifiedOriginal, String> {
    #[cfg(not(target_arch = "wasm32"))]
    if std::fs::canonicalize(&path).is_ok_and(|resolved| crate::service_sources::network_path(&resolved)) {
        return Err("PERMISSION_DENIED: original resolves to a network or device path".into());
    }
    let stamp = || std::fs::metadata(&path).and_then(|m| m.modified()).map_err(|_| "MEDIA_CHANGED: original timestamp unavailable");
    let modified = stamp()?;
    let fingerprint = analysis::fingerprint(&path, identity, cancel)?;
    if stamp()? != modified {
        return Err("MEDIA_CHANGED: original changed during verification".into());
    }
    Ok(VerifiedOriginal { path, modified, fingerprint })
}

fn existing_application(s: &Session, req: &Apply) -> Result<Option<Value>> {
    if req.operation_id.is_empty() || req.operation_id.len() > 128 || !req.operation_id.bytes().all(|b| b.is_ascii_alphanumeric() || b"_.:-".contains(&b)) {
        return Err(fail("INVALID_ARGUMENT: invalid application operation ID"));
    }
    if let Some(old) = s.operation_records.iter().find(|old| old.actor == "shotclutch:preedit" && old.operation_id == req.operation_id) {
        return if old.request == digest(req)? { Ok(Some(old.result.clone())) } else { Err(fail("INVALID_ARGUMENT: operation ID was already used")) };
    }
    Ok(None)
}

fn validate_application<'a>(s: &'a Session, req: &Apply) -> Result<(&'a PreEditMap, &'a PreEditAnalysis)> {
    if req.expected_revision != s.revision || current_revision(&s.project.preedit, req.map) != req.map {
        return Err(fail("REVISION_CONFLICT: refresh the rough-cut preview"));
    }
    let (map, saved) = records(s, req.map)?;
    if !map.reviewed {
        return Err(fail("INVALID_ARGUMENT: mark this map reviewed before applying"));
    }
    Ok((map, saved))
}

fn commit_application(s: &mut Session, req: Apply, verified: Option<VerifiedOriginal>) -> Result<Value> {
    if let Some(result) = existing_application(s, &req)? {
        return Ok(result);
    }
    let (map, saved) = validate_application(s, &req)?;
    let (media, path) = original(s, saved.source.item)?;
    let verified = match verified {
        Some(verified) => verified,
        None => verify_original(path.clone(), media.identity, &Cancellation::default()).map_err(fail)?,
    };
    if path != verified.path || !matches!(std::fs::metadata(&path), Ok(m) if m.len() == verified.fingerprint.1 && m.modified().ok() == Some(verified.modified))
    {
        return Err(fail("MEDIA_CHANGED: original changed since verification"));
    }
    let (sha256, bytes) = verified.fingerprint;
    if (sha256, bytes) != (saved.source.sha256.clone(), saved.source.bytes) {
        return Err(fail("MEDIA_CHANGED: source no longer matches the reviewed map"));
    }
    verify_edit_boundaries(s, map, saved)?;
    let mut project = (*s.project).clone();
    project.next_id = project.next_id.max(s.id_watermark);
    let mut application = assembly::build(&mut project, req.map, map, saved)?;
    if let Some(replace) = req.replace {
        let previous = s.project.preedit.applications.get(&replace).ok_or_else(|| fail("NOT_FOUND: generated application to replace"))?;
        let (_, previous_source) = records(s, previous.map)?;
        if previous_source.source.item != saved.source.item {
            return Err(fail("INVALID_ARGUMENT: the generated sequence belongs to another source"));
        }
        let old = s.project.item(previous.sequence).ok_or_else(|| fail("NOT_FOUND: generated sequence"))?;
        let old_seq = old.as_sequence().ok_or_else(|| fail("NOT_FOUND: generated sequence"))?;
        if old_seq.all_tracks().any(|track| track.locked) || digest(old_seq)? != previous.sequence_digest {
            return Err(fail("REVISION_CONFLICT: generated sequence changed; create a fresh sequence"));
        }
        let built = project.items.remove(&application.sequence).ok_or_else(|| fail("INTERNAL_ERROR: assembled sequence missing"))?;
        project.root.remove_item(application.sequence);
        project.item_mut(previous.sequence).ok_or_else(|| fail("NOT_FOUND: generated sequence"))?.kind = built.kind;
        application.sequence = previous.sequence;
    }
    let application_id = project.alloc_id();
    let sequence = application.sequence;
    project.preedit.applications.insert(application_id, Arc::new(application));
    let transaction_id = DocumentId::default();
    let retained_until = crate::autosave::unix_now() + 7 * 24 * 60 * 60;
    let revision = s.revision.checked_add(1).ok_or_else(|| fail("RESOURCE_LIMIT: revision exhausted"))?;
    let reply = json!({
        "apiVersion":"1","projectId":s.document_id,"operationId":req.operation_id,"transactionId":transaction_id,
        "sequenceId":sequence.0.to_string(),"priorRevision":s.revision.to_string(),"revision":revision.to_string(),
        "state":"committed","retainedUntil":retained_until,"application":application_id.to_string(),
    });
    let mut prepared = s.prepare_project("Apply Smart Rough Cut", project, vec![reply.clone()])?;
    prepared.operation = Some(OperationRecord {
        actor: "shotclutch:preedit".into(),
        operation_id: req.operation_id.clone(),
        idempotency_key: req.operation_id.clone(),
        request: digest(&req)?,
        sequence: sequence.0,
        retained_until,
        result: reply.clone(),
    });
    s.commit_edits_durable(prepared).map_err(|e| fail(e.to_string()))?;
    Ok(reply)
}

fn verify_edit_boundaries(s: &Session, map: &PreEditMap, saved: &PreEditAnalysis) -> Result<()> {
    let (media, _) = original(s, saved.source.item)?;
    let Some(source) = indexed_video(s, saved.source.item, media.info.has_video())? else {
        return Ok(());
    };
    let end = media.duration();
    for segment in map.segments(saved.source.range).map_err(fail)? {
        for boundary in [segment.source.start, segment.source.end()] {
            if boundary != Tick::ZERO && boundary != end && source.video_edit_boundary(boundary, false) != Some(boundary) {
                return Err(fail("CAPABILITY_UNAVAILABLE: a video cut does not match an indexed source frame boundary"));
            }
        }
    }
    Ok(())
}

fn indexed_video(s: &Session, item: ItemId, has_video: bool) -> Result<Option<shotclutch_media::SharedSource>> {
    if has_video {
        s.media.full_res_source(&s.project, item, &*s.services).map(Some).ok_or_else(|| fail("MEDIA_CHANGED: original video is unavailable"))
    } else {
        Ok(None)
    }
}

fn digest(value: &impl Serialize) -> Result<String> {
    let bytes = serde_json::to_vec(value).map_err(|e| fail(e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn records(s: &Session, id: u64) -> Result<(&PreEditMap, &PreEditAnalysis)> {
    let map = s.project.preedit.maps.get(&id).ok_or_else(|| fail("NOT_FOUND: rough-cut map"))?;
    let analysis = s.project.preedit.analyses.get(&map.analysis).ok_or_else(|| fail("NOT_FOUND: rough-cut analysis"))?;
    Ok((map, analysis))
}

/// Build a disposable sequence with the same mapping, audio and captions as Apply.
pub fn preview(s: &Session, map: u64) -> Result<(Arc<shotclutch_project::Project>, ItemId)> {
    let (record, analysis) = records(s, map)?;
    verify_edit_boundaries(s, record, analysis)?;
    let mut project = (*s.project).clone();
    let result = assembly::build(&mut project, map, record, analysis)?;
    Ok((Arc::new(project), result.sequence))
}

pub(crate) fn commands() -> Vec<CommandSpec> {
    let command = |id, label, params, run| CommandSpec { id, label, params, run, menu: &[], shortcut: None, enabled: |_| Ok(()), journal: false };
    vec![
        command(
            "preedit.analyze",
            "Analyze Smart Rough Cut",
            r#"{"item":string,"audioStream":integer,"model":string?,"language":string?,"channel":integer?,"range":object?,"resume":string?,"previousMap":string?}"#,
            start,
        ),
        command("preedit.cancel", "Cancel Rough Cut Analysis", "{}", |s, _| {
            let job = s.preedit_job.as_ref().ok_or_else(|| fail("NOT_FOUND: no rough-cut analysis is running"))?;
            job.cancel.cancel();
            Ok(json!({"state":"cancel_requested"}))
        }),
        command("preedit.inspect", "Inspect Smart Rough Cut", "{}", |s, _| {
            Ok(json!({"running":s.preedit_job.is_some(),"revision":s.revision.to_string(),"preedit":s.project.preedit}))
        }),
        command("preedit.review", "Review Smart Rough Cut", r#"{"map":string,"expectedRevision":string,"action":object}"#, review),
        command("preedit.preview", "Preview Smart Rough Cut", r#"{"map":string}"#, |s, p| {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Preview {
                #[serde(with = "decimal_u64")]
                map: u64,
            }
            let req: Preview = typed(p)?;
            let (map, analysis) = records(s, req.map)?;
            let segments = map.segments(analysis.source.range).map_err(fail)?;
            Ok(json!({
                "map":req.map.to_string(), "revision":s.revision.to_string(), "reviewed":map.reviewed,
                "segments": segments.iter().map(|segment| json!({
                    "source":{"start":segment.source.start.0.to_string(),"end":segment.source.end().0.to_string()},
                    "output":{"start":segment.output.start.0.to_string(),"end":segment.output.end().0.to_string()}
                })).collect::<Vec<_>>()
            }))
        }),
        command("preedit.apply", "Apply Smart Rough Cut", r#"{"map":string,"expectedRevision":string,"operationId":string,"replace":string?}"#, apply),
        command(
            "preedit.applyBackground",
            "Verify and Apply Smart Rough Cut",
            r#"{"map":string,"expectedRevision":string,"operationId":string,"replace":string?}"#,
            start_apply,
        ),
        command("preedit.cancelApply", "Cancel Rough Cut Application", "{}", |s, _| {
            let job = s.preedit_apply.as_ref().ok_or_else(|| fail("NOT_FOUND: no pending rough-cut application"))?;
            job.cancel.cancel();
            Ok(json!({"state":"cancel_requested"}))
        }),
    ]
}
