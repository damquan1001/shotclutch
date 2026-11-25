//! Native, host-driven runs over the scoped editor service.

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use shotclutch_project::ClipId;
use shotclutch_project::observations::{ObservationId, SourceFingerprint};
use shotclutch_time::{Tick, TimeRange};

use super::*;
use crate::agent_policy::ReviewRequest;
use crate::edit_verification::{Criterion, Outcome, Report};
use crate::media_observations::ActiveModels;

mod budget;
mod phrase;
#[cfg(test)]
mod tests;

pub(super) use budget::Meter;
pub(crate) use budget::Work;
pub use budget::{Limits, Usage};
pub use phrase::{OfflinePhrasePlanner, PlannedAction, Planner};

const MAX_CHECKPOINT: usize = 512 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhraseSplit {
    pub project: DocumentId,
    pub sequence: ItemId,
    pub expected_revision: u64,
    pub phrase: String,
    pub asset: Option<ItemId>,
    pub speaker: Option<String>,
    pub placement: Option<ClipId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Occurrence {
    pub asset: ItemId,
    pub fingerprint: SourceFingerprint,
    pub range: TimeRange,
    pub audio_stream: u16,
    pub observations: Vec<ObservationId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    pub clips: Vec<ClipId>,
    pub onset: Tick,
    pub cut: Tick,
    pub rounding: Tick,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Excerpt {
    pub range: TimeRange,
    pub frame_pts: Vec<Tick>,
    pub first_sample: i64,
    pub sample_count: usize,
    pub sample_rate: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Search,
    Inspect,
    Plan,
    Stage,
    Verify,
    AwaitingPermission,
    Commit,
    Completed,
    Failed,
    Canceled,
}

impl Phase {
    fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Canceled)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Progress {
    pub number: u64,
    pub phase: Phase,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunStatus {
    pub id: DocumentId,
    pub project: DocumentId,
    pub sequence: ItemId,
    pub base_revision: u64,
    pub operation_id: String,
    pub phase: Phase,
    pub last_completed: Option<Phase>,
    /// Most recent transitions, with monotonically increasing event numbers.
    pub events: Vec<Progress>,
    pub usage: Usage,
    pub elapsed_ms: u64,
    pub candidates: Vec<Occurrence>,
    pub selected: Option<Occurrence>,
    pub placements: Vec<Placement>,
    pub excerpt: Option<Excerpt>,
    pub cut_verified: bool,
    pub verification: Option<Report>,
    pub transaction: Option<DocumentId>,
    pub receipt: Option<Value>,
    pub message: Option<String>,
    #[serde(skip)]
    pub review: Option<ReviewRequest>,
}

/// Independently pollable by the host while an advance call holds the session.
#[derive(Clone)]
pub struct RunHandle {
    status: Arc<Mutex<RunStatus>>,
    cancellation: Cancellation,
}

impl RunHandle {
    pub fn inspect(&self) -> RunStatus {
        self.status.lock().unwrap_or_else(|err| err.into_inner()).clone()
    }

    pub fn cancel(&self) {
        self.cancellation.cancel();
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Checkpoint {
    version: u32,
    actor: String,
    authorization_digest: String,
    request: PhraseSplit,
    limits: Limits,
    started_ms: u64,
    #[serde(default)]
    commit_journal: Option<String>,
    status: RunStatus,
}

/// Exclusive run ownership. The checkpoint survives release of this handle.
pub struct AgentRun {
    checkpoint: Checkpoint,
    path: PathBuf,
    _lock: File,
    client: EditorClient,
    handle: RunHandle,
    deadline: Instant,
}

impl AgentRun {
    pub fn handle(&self) -> RunHandle {
        self.handle.clone()
    }

    fn status(&mut self) -> &mut RunStatus {
        &mut self.checkpoint.status
    }

    fn enter(&mut self, phase: Phase) {
        let status = self.status();
        status.phase = phase;
        let number = status.events.last().map_or(1, |event| event.number + 1);
        if status.events.len() == 64 {
            status.events.remove(0);
        }
        status.events.push(Progress { number, phase });
    }

    fn publish(&self) {
        *self.handle.status.lock().unwrap_or_else(|err| err.into_inner()) = self.checkpoint.status.clone();
    }

    fn save(&mut self) -> Result<(), ServiceError> {
        self.checkpoint.status.elapsed_ms = self.checkpoint.status.elapsed_ms.max(now_ms().saturating_sub(self.checkpoint.started_ms));
        write_checkpoint(&self.path, &self.checkpoint)?;
        self.publish();
        Ok(())
    }

    fn complete(&mut self, receipt: Value) {
        self.status().message = None;
        self.status().review = None;
        self.status().last_completed = Some(Phase::Commit);
        self.status().transaction = serde_json::from_value(receipt["transactionId"].clone()).ok();
        self.status().verification = serde_json::from_value(receipt["verification"]["report"].clone()).ok();
        self.status().cut_verified = true;
        self.status().receipt = Some(receipt);
        self.enter(Phase::Completed);
    }

    fn finish_error(&mut self, session: &mut Session, err: ServiceError) {
        if let Some(id) = self.status().transaction {
            let run = self.checkpoint.status.id;
            session.service.drafts.retain(|key, draft| *key != id || draft.client != self.client.id || draft.run != Some(run));
        }
        self.status().review = None;
        self.status().message = Some(err.to_string());
        self.enter(if err.code == ErrorCode::Canceled { Phase::Canceled } else { Phase::Failed });
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

fn write_checkpoint(path: &Path, checkpoint: &Checkpoint) -> Result<(), ServiceError> {
    let bytes = serde_json::to_vec(checkpoint).map_err(|_| error(ErrorCode::InternalError, "agent checkpoint could not be encoded"))?;
    if bytes.len() > MAX_CHECKPOINT {
        return Err(error(ErrorCode::ResourceLimit, "agent checkpoint exceeds its size limit"));
    }
    shotclutch_format::atomic_write(path, &bytes).map_err(|_| error(ErrorCode::InternalError, "agent checkpoint could not be written"))
}

fn lock_checkpoint(session: &Session, project: DocumentId, id: DocumentId) -> Result<(PathBuf, File), ServiceError> {
    let storage = session
        .persistence
        .as_ref()
        .filter(|_| session.prefs.auto_save.recovery_journal)
        .ok_or_else(|| error(ErrorCode::CapabilityUnavailable, "agent runs require the host recovery journal"))?;
    let directory = storage.data_dir.join("AgentRuns").join(project.to_string());
    std::fs::create_dir_all(&directory).map_err(|_| error(ErrorCode::InternalError, "agent recovery storage is unavailable"))?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join(format!("{id}.lock")))
        .map_err(|_| error(ErrorCode::InternalError, "agent checkpoint lock is unavailable"))?;
    lock.try_lock().map_err(|_| error(ErrorCode::PermissionDenied, "agent run is already open or cannot be locked"))?;
    Ok((directory.join(format!("{id}.json")), lock))
}

fn get_run_grant(session: &Session, client: &EditorClient, project: DocumentId, sequence: ItemId) -> Result<Grant, ServiceError> {
    let grant = session.service.grant(session, client)?;
    grant.check(session, project, Some(sequence), false)?;
    if grant.policy.as_ref().is_none_or(|policy| !policy.ranges.contains_key(&sequence)) {
        return Err(error(ErrorCode::PermissionDenied, "agent run requires an editor policy for its target sequence"));
    }
    Ok(grant)
}

fn operation_id(id: DocumentId) -> String {
    format!("phrase-split-{id}")
}

fn authorization_digest(client: &EditorClient) -> Result<String, ServiceError> {
    digest(&("agent-run-authorization-v1", client.id))
}

fn retained_receipt(records: &[OperationRecord], checkpoint: &Checkpoint) -> Option<Value> {
    let id = operation_id(checkpoint.status.id);
    records
        .iter()
        .find(|r| {
            r.actor == checkpoint.actor
                && r.operation_id == id
                && r.idempotency_key == id
                && r.sequence == checkpoint.request.sequence.0
                && r.result["projectId"] == json!(checkpoint.request.project)
                && r.result["priorRevision"] == checkpoint.request.expected_revision.to_string()
                && r.result["policy"].is_object()
        })
        .map(|r| r.result.clone())
}

fn require_current_commit_journal(session: &Session, checkpoint: &Checkpoint) -> Result<(), ServiceError> {
    let Some(id) = &checkpoint.commit_journal else { return Ok(()) };
    if id.is_empty() || id.len() > 64 || !id.bytes().all(|byte| byte.is_ascii_digit() || byte == b'-') {
        return Err(error(ErrorCode::InvalidArgument, "invalid agent commit journal reference"));
    }
    let storage = session.persistence.as_ref().ok_or_else(|| error(ErrorCode::CapabilityUnavailable, "agent recovery storage is unavailable"))?;
    let dir = crate::autosave::recovery_root(&storage.data_dir).join(id);
    let exists = dir.join("snapshot.scproj").try_exists().map_err(|_| error(ErrorCode::InternalError, "agent commit journal could not be inspected"))?;
    if exists {
        let candidate = crate::autosave::RecoveryCandidate { id: id.clone(), dir, meta: Default::default() };
        let loaded =
            crate::autosave::load_candidate(&candidate).map_err(|_| error(ErrorCode::InternalError, "recover the agent commit journal before resuming"))?;
        if loaded.edit_state.as_ref().is_some_and(|state| retained_receipt(&state.operations, checkpoint).is_some()) {
            return Err(error(ErrorCode::RevisionConflict, "recover the project journal containing the committed agent edit before resuming"));
        }
    }
    Ok(())
}

impl Session {
    pub fn start_agent_run(&self, client: &EditorClient, request: PhraseSplit, limits: Limits) -> Result<AgentRun, ServiceError> {
        let grant = get_run_grant(self, client, request.project, request.sequence)?;
        if request.expected_revision != self.revision {
            return Err(error(ErrorCode::RevisionConflict, "project changed before the run started"));
        }
        if request.phrase.trim().is_empty() || request.phrase.len() > 512 || request.speaker.as_ref().is_some_and(|value| value.len() > 128) {
            return Err(error(ErrorCode::InvalidArgument, "provide a bounded phrase and speaker filter"));
        }
        let id = DocumentId::default();
        let limits = limits.validate()?;
        let (path, lock) = lock_checkpoint(self, request.project, id)?;
        let checkpoint = Checkpoint {
            version: 1,
            actor: grant.actor,
            authorization_digest: authorization_digest(client)?,
            request: request.clone(),
            limits,
            started_ms: now_ms(),
            commit_journal: None,
            status: RunStatus {
                id,
                project: request.project,
                sequence: request.sequence,
                base_revision: request.expected_revision,
                operation_id: operation_id(id),
                phase: Phase::Search,
                last_completed: None,
                events: vec![Progress { number: 1, phase: Phase::Search }],
                usage: Usage::default(),
                elapsed_ms: 0,
                candidates: vec![],
                selected: None,
                placements: vec![],
                excerpt: None,
                cut_verified: false,
                verification: None,
                transaction: None,
                receipt: None,
                message: None,
                review: None,
            },
        };
        let mut run = new_run(checkpoint, path, lock, client.clone());
        run.save()?;
        Ok(run)
    }

    /// Call after opening or recovering the project snapshot, so durable operations are available.
    pub fn resume_agent_run(&mut self, client: &EditorClient, id: DocumentId) -> Result<AgentRun, ServiceError> {
        self.service.grant(self, client)?;
        let (path, lock) = lock_checkpoint(self, self.document_id, id)?;
        let mut bytes = Vec::new();
        File::open(&path)
            .and_then(|file| file.take(MAX_CHECKPOINT as u64 + 1).read_to_end(&mut bytes))
            .map_err(|_| error(ErrorCode::NotFound, "agent checkpoint is unavailable"))?;
        if bytes.len() > MAX_CHECKPOINT {
            return Err(error(ErrorCode::ResourceLimit, "agent checkpoint exceeds its size limit"));
        }
        let checkpoint: Checkpoint = serde_json::from_slice(&bytes).map_err(|_| error(ErrorCode::InvalidArgument, "invalid agent checkpoint"))?;
        let grant = get_run_grant(self, client, checkpoint.request.project, checkpoint.request.sequence)?;
        if checkpoint.version != 1
            || checkpoint.status.id != id
            || checkpoint.actor != grant.actor
            || checkpoint.status.project != checkpoint.request.project
            || checkpoint.status.sequence != checkpoint.request.sequence
            || checkpoint.status.base_revision != checkpoint.request.expected_revision
            || checkpoint.status.operation_id != operation_id(id)
        {
            return Err(error(ErrorCode::PermissionDenied, "agent checkpoint identity does not match this authorization"));
        }
        let mut run = new_run(checkpoint, path, lock, client.clone());
        if let Some(receipt) = retained_receipt(&self.operation_records, &run.checkpoint) {
            run.complete(receipt);
        } else if !run.checkpoint.status.phase.terminal() {
            require_current_commit_journal(self, &run.checkpoint)?;
            let authorization = authorization_digest(client)?;
            if run.checkpoint.authorization_digest == authorization {
                return Err(error(ErrorCode::PermissionDenied, "resuming unpublished work requires a fresh editor grant"));
            }
            run.checkpoint.authorization_digest = authorization;
            run.checkpoint.commit_journal = None;
            let operation = operation_id(id);
            self.service.drafts.retain(|_, draft| draft.actor != grant.actor || draft.request.operation_id != operation || draft.run != Some(id));
            if run.checkpoint.request.expected_revision != self.revision {
                run.finish_error(self, error(ErrorCode::RevisionConflict, "project changed while the run was stopped"));
            } else {
                let status = run.status();
                status.transaction = None;
                status.cut_verified = false;
                status.verification = None;
                status.last_completed = None;
                status.candidates.clear();
                status.selected = None;
                status.placements.clear();
                status.excerpt = None;
                status.message = None;
                run.enter(Phase::Search);
            }
        }
        run.save()?;
        Ok(run)
    }

    /// Advance one phase. Hosts can poll or cancel through a cloned RunHandle during dispatch.
    pub fn advance_agent_run(&mut self, run: &mut AgentRun, models: &ActiveModels, planner: &dyn Planner) -> Result<RunStatus, ServiceError> {
        if run.checkpoint.status.phase.terminal() {
            return Ok(run.handle.inspect());
        }
        if self.document_id == run.checkpoint.request.project
            && get_run_grant(self, &run.client, run.checkpoint.request.project, run.checkpoint.request.sequence).is_ok()
            && let Some(receipt) = retained_receipt(&self.operation_records, &run.checkpoint)
        {
            run.complete(receipt);
        } else {
            if run.checkpoint.status.phase == Phase::Commit {
                run.checkpoint.commit_journal = self.persistence.as_ref().and_then(|storage| storage.session_dir.file_name()?.to_str().map(str::to_owned));
            }
            let meter = Arc::new(Meter::new(run));
            let cancellation = Cancellation(run.handle.cancellation.0.clone(), Some(meter.clone()));
            let result = run.step(self, models, planner, &cancellation);
            let retries = run.checkpoint.status.usage.retries;
            run.status().usage = meter.usage();
            run.status().usage.retries = retries;
            if let Err(err) = result {
                let err = cancellation.check().err().unwrap_or(err);
                run.finish_error(self, err);
            }
        }
        if let Err(err) = run.save() {
            if run.checkpoint.status.phase != Phase::Completed {
                run.finish_error(self, err);
            } else {
                run.status().message = Some("Edit committed. The run checkpoint could not be updated. Recovery can read the durable operation receipt.".into());
            }
            run.publish();
        }
        Ok(run.handle.inspect())
    }
}

impl AgentRun {
    fn step(&mut self, session: &mut Session, models: &ActiveModels, planner: &dyn Planner, cancellation: &Cancellation) -> Result<(), ServiceError> {
        cancellation.check()?;
        let request = self.checkpoint.request.clone();
        let phase = self.checkpoint.status.phase;
        let grant = get_run_grant(session, &self.client, request.project, request.sequence)?;
        if grant.actor != self.checkpoint.actor {
            return Err(error(ErrorCode::PermissionDenied, "agent run belongs to another actor"));
        }
        if session.revision != request.expected_revision {
            return Err(error(ErrorCode::RevisionConflict, "project changed during the run"));
        }
        let next = match phase {
            Phase::Search => {
                let query = phrase::query(session, &request)?;
                let allowed = sequence_assets(session, request.sequence, cancellation)?;
                cancellation.charge_run_work(Work::Tool, 1)?;
                let results = crate::media_search::search_scoped(session, &query, models, &grant.observation_context(cancellation), Some(&allowed))?;
                self.status().candidates = phrase::candidates(&results, cancellation)?;
                self.status().last_completed = Some(Phase::Search);
                if results.truncated || !results.coverage.budget_limited.is_empty() {
                    return Err(error(ErrorCode::ResourceLimit, "search coverage is incomplete; narrow the target or provide more source budget"));
                }
                if !results.coverage.missing_originals.is_empty() || phrase::uncertain(&results, &self.checkpoint.status.candidates, cancellation)? {
                    return Err(error(ErrorCode::MediaChanged, "phrase search encountered missing, stale, or unverified evidence"));
                }
                if self.status().candidates.is_empty() {
                    return Err(error(ErrorCode::NotFound, "no current exact word-timed transcript occurrence was found"));
                }
                Phase::Inspect
            }
            Phase::Inspect => {
                self.resolve_placement(session, &request, cancellation)?;
                cancellation.charge_run_work(Work::Tool, 1)?;
                let occurrence = self.selected()?.0;
                let excerpt = phrase::excerpt(session, &grant, occurrence, models, cancellation)?;
                self.status().excerpt = Some(excerpt);
                Phase::Plan
            }
            Phase::Plan => {
                let (occurrence, placement) = self.selected()?;
                let excerpt = self.checkpoint.status.excerpt.as_ref().ok_or_else(|| error(ErrorCode::InternalError, "agent excerpt is missing"))?;
                let action = planner.plan(occurrence, excerpt, placement)?;
                cancellation.check()?;
                if action != (PlannedAction::Split { clips: placement.clips.clone(), cut: placement.cut }) {
                    return Err(error(ErrorCode::VerificationFailed, "planner proposed a split outside the qualified evidence"));
                }
                if placement.clips.len() > self.checkpoint.limits.transaction_clips {
                    return Err(error(ErrorCode::ResourceLimit, "split exceeds the agent transaction target limit"));
                }
                Phase::Stage
            }
            Phase::Stage => {
                let (occurrence, placement) = self.selected()?;
                let occurrence = occurrence.clone();
                let operation = operation_id(self.checkpoint.status.id);
                let edit = EditRequest {
                    api_version: "1".into(),
                    operation_id: operation.clone(),
                    idempotency_key: operation,
                    project_id: request.project,
                    expected_revision: request.expected_revision,
                    sequence_id: request.sequence,
                    label: "Split at phrase".into(),
                    edits: vec![TimelineEdit::Split { clips: placement.clips.clone(), time: placement.cut.0.to_string() }],
                };
                cancellation.charge_run_work(Work::Tool, 1)?;
                let preview = session.preview_verified_edit(&self.client, edit, vec![Criterion::PreserveSplit { audio: true }], cancellation)?;
                if preview["state"] == "committed" {
                    self.complete(preview);
                    return Ok(());
                }
                let id = parse(preview["transactionId"].clone())?;
                self.status().transaction = Some(id);
                let draft = session.service.drafts.get(&id).ok_or_else(|| error(ErrorCode::NotFound, "staged edit is unavailable"))?;
                if draft.run != Some(self.checkpoint.status.id) {
                    return Err(error(ErrorCode::PermissionDenied, "draft belongs to another operation"));
                }
                if !draft.sources.iter().any(|source| source.matches_original(occurrence.asset, &occurrence.fingerprint)) {
                    return Err(error(ErrorCode::MediaChanged, "the staged edit references different original bytes from the phrase evidence"));
                }
                Phase::Verify
            }
            Phase::Verify => {
                let id = self.transaction()?;
                let (occurrence, placement) = self.selected()?;
                let draft = session.service.drafts.get(&id).ok_or_else(|| error(ErrorCode::NotFound, "staged edit is unavailable"))?;
                phrase::verify_cut(session, draft, placement, occurrence)?;
                self.status().cut_verified = true;
                cancellation.charge_run_work(Work::Tool, 1)?;
                let report = session.verify_staged_edit(&self.client, id, cancellation)?;
                let passed = report.outcome == Outcome::Passed;
                self.status().verification = Some(report);
                if !passed {
                    return Err(error(ErrorCode::VerificationFailed, "staged picture or audio preservation did not pass"));
                }
                self.check_permission(session, &grant, id)?
            }
            Phase::AwaitingPermission => {
                let next = self.check_permission(session, &grant, self.transaction()?)?;
                if next == Phase::AwaitingPermission {
                    return Ok(());
                }
                next
            }
            Phase::Commit => {
                cancellation.charge_run_work(Work::Tool, 1)?;
                match session.commit_agent_edit(&self.client, self.transaction()?, cancellation) {
                    Ok(receipt) => self.complete(receipt),
                    Err(err) if err.code == ErrorCode::PermissionRequired => {
                        let next = self.check_permission(session, &grant, self.transaction()?)?;
                        self.enter(next);
                    }
                    Err(err) if err.message == runs_storage_failure() && self.checkpoint.status.usage.retries < self.checkpoint.limits.retries => {
                        self.status().usage.retries += 1;
                        self.status().message = Some(err.to_string());
                        self.enter(Phase::Commit);
                    }
                    Err(err) => return Err(err),
                }
                // Publication can win a race with cancellation after the durable marker.
                return Ok(());
            }
            _ => return Ok(()),
        };
        self.status().last_completed = Some(phase);
        cancellation.check()?;
        self.enter(next);
        Ok(())
    }

    fn selected(&self) -> Result<(&Occurrence, &Placement), ServiceError> {
        let status = &self.checkpoint.status;
        status.selected.as_ref().zip(status.placements.first()).ok_or_else(|| error(ErrorCode::InternalError, "agent phrase placement is missing"))
    }

    fn transaction(&self) -> Result<DocumentId, ServiceError> {
        self.checkpoint.status.transaction.ok_or_else(|| error(ErrorCode::InternalError, "agent draft is missing"))
    }

    fn resolve_placement(&mut self, session: &Session, request: &PhraseSplit, cancellation: &Cancellation) -> Result<(), ServiceError> {
        let sequence = session.project.sequence(request.sequence).ok_or_else(|| error(ErrorCode::NotFound, "sequence is unavailable"))?;
        let mut possibilities = Vec::new();
        let mut unsupported = false;
        for occurrence in &self.checkpoint.status.candidates {
            let (placements, unresolved) = phrase::placements(&session.project, sequence, request, occurrence, cancellation)?;
            unsupported |= unresolved;
            possibilities.extend(placements.into_iter().map(|placement| (occurrence.clone(), placement)));
            if possibilities.len() > 64 {
                break;
            }
        }
        self.status().placements = possibilities.iter().take(64).map(|(_, placement)| placement.clone()).collect();
        if possibilities.len() > 1 {
            return Err(error(ErrorCode::InvalidArgument, "multiple phrase occurrences or placements qualify; narrow the target"));
        }
        if unsupported {
            return Err(error(
                ErrorCode::CapabilityUnavailable,
                "the phrase has a nested, retimed, or otherwise unsupported placement; select a supported linked clip",
            ));
        }
        let (occurrence, _) = possibilities.pop().ok_or_else(|| error(ErrorCode::NotFound, "the phrase has no eligible linked A/V placement"))?;
        self.status().selected = Some(occurrence);
        Ok(())
    }

    fn check_permission(&mut self, session: &Session, grant: &Grant, id: DocumentId) -> Result<Phase, ServiceError> {
        let draft = session.service.drafts.get(&id).ok_or_else(|| error(ErrorCode::NotFound, "staged edit is unavailable"))?;
        session.service.check_draft(session, grant, draft, true)?;
        match draft.policy_decision(grant) {
            Ok(_) => {
                self.status().review = None;
                Ok(Phase::Commit)
            }
            Err(err) if err.code == ErrorCode::PermissionRequired => {
                self.status().review = Some(session.review_agent_edit(&self.client, id)?);
                Ok(Phase::AwaitingPermission)
            }
            Err(err) => Err(err),
        }
    }
}

fn sequence_assets(session: &Session, sequence: ItemId, cancellation: &Cancellation) -> Result<BTreeSet<ItemId>, ServiceError> {
    let mut pending = BTreeSet::from([sequence]);
    let mut visited = BTreeSet::new();
    let mut assets = BTreeSet::new();
    while let Some(id) = pending.pop_first() {
        cancellation.check()?;
        if !visited.insert(id) {
            continue;
        }
        if visited.len() > 512 {
            return Err(error(ErrorCode::ResourceLimit, "agent target sequence references too many sources"));
        }
        if let Some(sequence) = session.project.sequence(id) {
            for clip in sequence.all_tracks().flat_map(|track| &track.items).filter(|clip| clip.enabled) {
                cancellation.check()?;
                pending.insert(clip.item);
            }
        } else if let Some((asset, _, _)) = session.project.resolve_media(id) {
            assets.insert(asset);
        }
    }
    Ok(assets)
}

fn new_run(checkpoint: Checkpoint, path: PathBuf, lock: File, client: EditorClient) -> AgentRun {
    let handle = RunHandle { status: Arc::new(Mutex::new(checkpoint.status.clone())), cancellation: Cancellation::default() };
    let elapsed = now_ms().saturating_sub(checkpoint.started_ms).max(checkpoint.status.elapsed_ms);
    let deadline = Instant::now() + Duration::from_millis(checkpoint.limits.elapsed_ms.saturating_sub(elapsed).min(3_600_000));
    AgentRun { checkpoint, path, _lock: lock, client, handle, deadline }
}
