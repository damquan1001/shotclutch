//! Versioned structural transactions. Only trusted host code issues access; request JSON cannot
//! choose an actor, grant itself rights, select a destination, or execute arbitrary commands.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use web_time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use shotclutch_format::session::{DocumentId, OperationRecord};
use shotclutch_project::{ItemId, Project};

use crate::Session;
use crate::transactions::{PreparedEdit, TimelineEdit, TransactionError};

const RECEIPT_LIFETIME: i64 = 7 * 24 * 60 * 60;
const METHODS: &[&str] = &[
    "project.inspect",
    "sequence.inspect",
    "command.list",
    "transaction.preview",
    "transaction.inspect",
    "transaction.commit",
    "transaction.abort",
    "operation.inspect",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidArgument,
    NotFound,
    RevisionConflict,
    PermissionRequired,
    PermissionDenied,
    CapabilityUnavailable,
    MediaChanged,
    VerificationFailed,
    ResourceLimit,
    Canceled,
    InternalError,
}

#[derive(Clone, Debug, Serialize)]
pub struct ServiceError {
    pub code: ErrorCode,
    pub message: String,
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = serde_json::to_value(self.code).unwrap_or(Value::Null);
        write!(f, "{}: {}", code.as_str().unwrap_or("INTERNAL_ERROR"), self.message)
    }
}
impl std::error::Error for ServiceError {}
fn error(code: ErrorCode, message: impl Into<String>) -> ServiceError {
    ServiceError { code, message: message.into() }
}
type Reply = Result<Value, ServiceError>;

/// Trusted host configuration, deliberately not deserializable. None means all sequences;
/// explicit roots are opened as confined directory capabilities when issuing the grant.
#[derive(Clone)]
pub struct EditorScope {
    pub sequences: Option<BTreeSet<ItemId>>,
    pub allow_edits: bool,
    pub media_roots: Vec<PathBuf>,
    pub lifetime: Duration,
    pub max_source_bytes: u64,
}
impl EditorScope {
    pub fn sequence(sequence: ItemId) -> Self {
        Self {
            sequences: Some([sequence].into()),
            allow_edits: false,
            media_roots: Vec::new(),
            lifetime: Duration::from_secs(15 * 60),
            max_source_bytes: 512 * 1024 * 1024,
        }
    }
}

/// An in-process authorization handle. It is never serialized into a project or accepted from
/// JSON. Revocation is atomic, including while another thread is hashing source media.
#[derive(Clone)]
pub struct EditorClient {
    id: DocumentId,
    revoked: Arc<AtomicBool>,
}
impl EditorClient {
    pub fn revoke(&self) {
        self.revoked.store(true, Ordering::Release);
    }
}
impl std::fmt::Debug for EditorClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EditorClient(<private>)")
    }
}

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub(crate) fn check(&self) -> Result<(), ServiceError> {
        if self.0.load(Ordering::Acquire) { Err(error(ErrorCode::Canceled, "operation canceled before commit")) } else { Ok(()) }
    }
}

#[derive(Clone)]
struct Grant {
    client: EditorClient,
    actor: String,
    project: DocumentId,
    scope: EditorScope,
    expires: Instant,
    roots: Vec<Arc<crate::service_sources::ReadRoot>>,
}
impl Grant {
    fn check(&self, session: &Session, project: DocumentId, sequence: Option<ItemId>, edit: bool) -> Result<(), ServiceError> {
        if self.client.revoked.load(Ordering::Acquire) || Instant::now() >= self.expires {
            return Err(error(ErrorCode::PermissionDenied, "authorization was revoked or expired"));
        }
        if self.project != project || session.document_id != project {
            return Err(error(ErrorCode::PermissionDenied, "authorization belongs to another project"));
        }
        if edit && !self.scope.allow_edits {
            return Err(error(ErrorCode::PermissionDenied, "read-only authorization cannot edit"));
        }
        if let Some(sequence) = sequence {
            if self.scope.sequences.as_ref().is_some_and(|allowed| !allowed.contains(&sequence)) {
                return Err(error(ErrorCode::PermissionDenied, "sequence is outside the authorized scope"));
            }
            if session.project.sequence(sequence).is_none() {
                return Err(error(ErrorCode::NotFound, "sequence not found"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditRequest {
    pub api_version: String,
    pub operation_id: String,
    pub idempotency_key: String,
    pub project_id: DocumentId,
    #[serde(with = "shotclutch_format::session::decimal_u64")]
    pub expected_revision: u64,
    #[serde(with = "item_id")]
    pub sequence_id: ItemId,
    pub label: String,
    pub edits: Vec<TimelineEdit>,
}

pub(crate) mod item_id {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};
    use shotclutch_project::ItemId;
    pub fn serialize<S: Serializer>(id: &ItemId, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&id.0.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<ItemId, D::Error> {
        let text = String::deserialize(d)?;
        let id: u64 = text.parse().map_err(D::Error::custom)?;
        if id.to_string() != text {
            return Err(D::Error::custom("expected a canonical decimal ID string"));
        }
        Ok(ItemId(id))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TransactionRef {
    api_version: String,
    project_id: DocumentId,
    transaction_id: DocumentId,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OperationRef {
    api_version: String,
    project_id: DocumentId,
    operation_id: String,
}

struct Draft {
    client: DocumentId,
    actor: String,
    request: EditRequest,
    digest: String,
    prepared: PreparedEdit,
    sources: Vec<crate::service_sources::SourceBinding>,
    expires: Instant,
    preview: Value,
}

#[derive(Default)]
pub(crate) struct EditorService {
    clients: BTreeMap<DocumentId, Grant>,
    drafts: BTreeMap<DocumentId, Draft>,
    owner: Option<EditorClient>,
}

fn identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && value.bytes().all(|b| b.is_ascii_alphanumeric() || b"_.:-".contains(&b))
}
fn version(v: &str) -> Result<(), ServiceError> {
    if v == "1" { Ok(()) } else { Err(error(ErrorCode::CapabilityUnavailable, "unsupported editor API version")) }
}
fn parse<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, ServiceError> {
    serde_json::from_value(value).map_err(|e| error(ErrorCode::InvalidArgument, format!("invalid request: {e}")))
}
fn digest<T: Serialize>(value: &T) -> Result<String, ServiceError> {
    let mut value = serde_json::to_value(value).map_err(|_| error(ErrorCode::InvalidArgument, "request cannot be serialized"))?;
    value.sort_all_objects();
    let bytes = serde_json::to_vec(&value).map_err(|_| error(ErrorCode::InvalidArgument, "request cannot be serialized"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn transaction_error(e: TransactionError) -> ServiceError {
    match e {
        TransactionError::RevisionConflict => error(ErrorCode::RevisionConflict, "project changed; prepare a new preview"),
        TransactionError::InvalidArgument(m) => error(ErrorCode::InvalidArgument, m),
        TransactionError::VerificationFailed(m) => error(ErrorCode::VerificationFailed, m),
        TransactionError::CapabilityUnavailable(m) => error(ErrorCode::CapabilityUnavailable, m),
        TransactionError::Canceled => error(ErrorCode::Canceled, "transaction canceled before commit"),
        TransactionError::PermissionDenied => error(ErrorCode::PermissionDenied, "authorization was revoked or expired"),
        TransactionError::PersistenceFailed(_) => {
            error(ErrorCode::InternalError, "durable storage failed; no edit was published; inspect the operation before retrying")
        }
    }
}

impl EditorService {
    fn issue(&mut self, session: &Session, actor: &str, scope: EditorScope) -> Result<EditorClient, ServiceError> {
        if !identifier(actor)
            || scope.lifetime.is_zero()
            || scope.lifetime > Duration::from_secs(3600)
            || scope.max_source_bytes == 0
            || scope.media_roots.len() > 32
        {
            return Err(error(ErrorCode::InvalidArgument, "invalid host authorization configuration"));
        }
        self.clients.retain(|_, grant| !grant.client.revoked.load(Ordering::Acquire) && Instant::now() < grant.expires);
        if self.clients.len() >= 16 {
            return Err(error(ErrorCode::ResourceLimit, "too many active editor authorizations"));
        }
        if scope.sequences.as_ref().is_some_and(|ids| ids.is_empty() || ids.iter().any(|id| session.project.sequence(*id).is_none())) {
            return Err(error(ErrorCode::NotFound, "authorized sequence not found"));
        }
        let roots = scope.media_roots.iter().map(|path| crate::service_sources::ReadRoot::open(path).map(Arc::new)).collect::<Result<Vec<_>, _>>()?;
        let client = EditorClient { id: DocumentId::default(), revoked: Arc::new(AtomicBool::new(false)) };
        self.clients.insert(
            client.id,
            Grant { client: client.clone(), actor: actor.into(), project: session.document_id, expires: Instant::now() + scope.lifetime, scope, roots },
        );
        Ok(client)
    }

    fn owner(&mut self, session: &Session) -> Result<EditorClient, ServiceError> {
        if let Some(client) = &self.owner
            && let Some(grant) = self.clients.get(&client.id)
            && grant.check(session, session.document_id, None, true).is_ok()
        {
            return Ok(client.clone());
        }
        if let Some(old) = self.owner.take() {
            old.revoke();
            self.clients.remove(&old.id);
            self.drafts.retain(|_, draft| draft.client != old.id);
        }
        let roots = session.path.as_ref().and_then(|p| std::path::absolute(p).ok()).and_then(|p| p.parent().map(PathBuf::from)).into_iter().collect();
        let scope = session.owner_editor_scope.clone().unwrap_or(EditorScope {
            sequences: None,
            allow_edits: true,
            media_roots: roots,
            lifetime: Duration::from_secs(3600),
            max_source_bytes: 512 * 1024 * 1024,
        });
        let client = self.issue(session, "local-owner", scope)?;
        self.owner = Some(client.clone());
        Ok(client)
    }

    fn grant(&self, session: &Session, client: &EditorClient) -> Result<Grant, ServiceError> {
        let grant = self.clients.get(&client.id).ok_or_else(|| error(ErrorCode::PermissionDenied, "unknown editor authorization"))?;
        grant.check(session, session.document_id, None, false)?;
        Ok(grant.clone())
    }

    fn dispatch(&mut self, session: &mut Session, client: &EditorClient, method: &str, params: Value, cancellation: &Cancellation) -> Reply {
        let grant = self.grant(session, client)?;
        cancellation.check()?;
        if serde_json::to_vec(&params).map_err(|_| error(ErrorCode::InvalidArgument, "invalid request"))?.len() > 256 * 1024 {
            return Err(error(ErrorCode::ResourceLimit, "editor request exceeds 256 KiB"));
        }
        match method {
            "project.inspect" => {
                let sequences: Vec<_> = session
                    .project
                    .sequences()
                    .filter(|s| grant.scope.sequences.as_ref().is_none_or(|ids| ids.contains(&s.id)))
                    .map(|s| s.id.0.to_string())
                    .collect();
                Ok(
                    json!({"projectId": session.document_id, "revision": session.revision.to_string(), "activeSequence": session.state.active_sequence.filter(|s| grant.scope.sequences.as_ref().is_none_or(|ids| ids.contains(s))).map(|s| s.0.to_string()), "sequences": sequences}),
                )
            }
            "sequence.inspect" => {
                let sequence = match params.get("item") {
                    Some(v) => {
                        let text = v.as_str().ok_or_else(|| error(ErrorCode::InvalidArgument, "sequence ID must be a decimal string"))?;
                        let id = text.parse::<u64>().map_err(|_| error(ErrorCode::InvalidArgument, "invalid sequence ID"))?;
                        if id.to_string() != text {
                            return Err(error(ErrorCode::InvalidArgument, "invalid sequence ID"));
                        }
                        ItemId(id)
                    }
                    None => session.state.active_sequence.ok_or_else(|| error(ErrorCode::NotFound, "no active sequence"))?,
                };
                grant.check(session, session.document_id, Some(sequence), false)?;
                let q = session.project.sequence(sequence).ok_or_else(|| error(ErrorCode::NotFound, "sequence not found"))?;
                let track = |t: &shotclutch_project::Track| json!({"id": t.id.0.to_string(), "name": t.name, "locked": t.locked, "items": t.items.iter().map(|i| json!({"clip": i.id.0.to_string(), "item": i.item.0.to_string(), "name": i.name, "start": i.start.0.to_string(), "duration": i.duration.0.to_string(), "end": i.end().0.to_string(), "sourceIn": i.source_in.0.to_string(), "sourceOut": i.source_out().0.to_string(), "speed": i.speed, "reverse": i.reverse, "enabled": i.enabled})).collect::<Vec<_>>()});
                Ok(json!({"apiVersion": "1", "projectId": session.document_id, "observedRevision": session.revision.to_string(), "id": sequence.0.to_string(), "duration": q.duration().0.to_string(), "settings": q.settings, "video": q.video_tracks.iter().map(track).collect::<Vec<_>>(), "audio": q.audio_tracks.iter().map(track).collect::<Vec<_>>()}))
            }
            "command.list" => Ok(Value::Array(METHODS.iter().map(|id| {
                let edit = matches!(*id, "transaction.preview" | "transaction.commit");
                let params = match *id {
                    "transaction.preview" => "{apiVersion,projectId,operationId,idempotencyKey,expectedRevision,sequenceId,label,edits}",
                    "transaction.inspect" | "transaction.commit" | "transaction.abort" => "{apiVersion,projectId,transactionId}",
                    "operation.inspect" => "{apiVersion,projectId,operationId}",
                    "sequence.inspect" => "{item?: decimal-string}",
                    _ => "{}",
                };
                json!({"id": id, "label": id, "apiVersion": "1", "params": params, "enabled": !edit || grant.scope.allow_edits, "capability": if edit { "sequence.edit" } else { "sequence.read" }, "preview": *id == "transaction.preview", "reversible": *id == "transaction.commit", "job": false})
            }).collect())),
            "transaction.preview" => self.preview(session, &grant, parse(params)?, cancellation),
            "transaction.commit" | "transaction.abort" | "transaction.inspect" => {
                let reference: TransactionRef = parse(params)?;
                version(&reference.api_version)?;
                grant.check(session, reference.project_id, None, method == "transaction.commit")?;
                if let Some(record) = session.operation_records.iter().find(|r| {
                    r.actor == grant.actor && r.retained_until > crate::autosave::unix_now() && r.result["transactionId"] == json!(reference.transaction_id)
                }) {
                    grant.check(session, reference.project_id, Some(ItemId(record.sequence)), method == "transaction.commit")?;
                    return if method == "transaction.abort" {
                        Err(error(ErrorCode::InvalidArgument, "committed edits require undo, not abort"))
                    } else {
                        Ok(record.result.clone())
                    };
                }
                let draft = self.drafts.get(&reference.transaction_id).ok_or_else(|| error(ErrorCode::NotFound, "preview not found"))?;
                self.check_draft(session, &grant, draft, method == "transaction.commit")?;
                if method == "transaction.inspect" {
                    return Ok(draft.preview.clone());
                }
                if method == "transaction.abort" {
                    self.drafts.remove(&reference.transaction_id);
                    return Ok(json!({"transactionId": reference.transaction_id, "state": "aborted"}));
                }
                self.commit(session, &grant, reference.transaction_id, cancellation)
            }
            "operation.inspect" => {
                let reference: OperationRef = parse(params)?;
                version(&reference.api_version)?;
                grant.check(session, reference.project_id, None, false)?;
                let record = session
                    .operation_records
                    .iter()
                    .find(|r| r.actor == grant.actor && r.operation_id == reference.operation_id && r.retained_until > crate::autosave::unix_now())
                    .ok_or_else(|| error(ErrorCode::NotFound, "retained operation not found"))?;
                grant.check(session, reference.project_id, Some(ItemId(record.sequence)), false)?;
                Ok(json!({"observedRevision": session.revision.to_string(), "result": record.result}))
            }
            _ => Err(error(ErrorCode::PermissionDenied, "command is outside the scoped editor service")),
        }
    }

    fn preview(&mut self, session: &mut Session, grant: &Grant, request: EditRequest, cancellation: &Cancellation) -> Reply {
        version(&request.api_version)?;
        grant.check(session, request.project_id, Some(request.sequence_id), true)?;
        if !identifier(&request.operation_id) || !identifier(&request.idempotency_key) {
            return Err(error(ErrorCode::InvalidArgument, "operation IDs and idempotency keys must be bounded identifiers"));
        }
        let digest = digest(&request)?;
        if let Some(record) = session.operation_records.iter().find(|r| {
            r.actor == grant.actor
                && (r.idempotency_key == request.idempotency_key || r.operation_id == request.operation_id)
                && r.retained_until > crate::autosave::unix_now()
        }) {
            return if record.request == digest {
                Ok(record.result.clone())
            } else {
                Err(error(ErrorCode::InvalidArgument, "operation ID or idempotency key was used with a different request"))
            };
        }
        self.drafts.retain(|_, draft| Instant::now() < draft.expires);
        if let Some(draft) = self.drafts.values().find(|d| {
            d.actor == grant.actor
                && d.request.project_id == request.project_id
                && (d.request.idempotency_key == request.idempotency_key || d.request.operation_id == request.operation_id)
        }) {
            if draft.digest != digest {
                return Err(error(ErrorCode::InvalidArgument, "idempotency key was used with a different request"));
            }
            self.check_draft(session, grant, draft, true)?;
            return Ok(draft.preview.clone());
        }
        if self.drafts.len() >= 32 || self.drafts.values().filter(|d| d.client == grant.client.id).count() >= 8 {
            return Err(error(ErrorCode::ResourceLimit, "too many pending previews; abort an unused preview"));
        }
        let prepared = session
            .prepare_edits_guarded(request.expected_revision, request.sequence_id, &request.label, &request.edits, || {
                cancellation.check().map_err(|_| TransactionError::Canceled)?;
                if grant.client.revoked.load(Ordering::Acquire) || Instant::now() >= grant.expires {
                    return Err(TransactionError::PermissionDenied);
                }
                Ok(())
            })
            .map_err(transaction_error)?;
        let context = crate::service_sources::ReadContext {
            roots: &grant.roots,
            budget: grant.scope.max_source_bytes,
            cancellation,
            revoked: &grant.client.revoked,
            expires: grant.expires,
        };
        let sources = crate::service_sources::capture(session, request.sequence_id, &context)?;
        grant.check(session, request.project_id, Some(request.sequence_id), true)?;
        cancellation.check()?;
        let id = DocumentId::default();
        let expires = (Instant::now() + Duration::from_secs(5 * 60)).min(grant.expires);
        let changes = changes(&session.project, prepared.project(), request.sequence_id)?;
        let preview = json!({"apiVersion": "1", "state": "preview_ready", "transactionId": id, "projectId": request.project_id, "operationId": request.operation_id, "baseRevision": request.expected_revision.to_string(), "sequenceId": request.sequence_id.0.to_string(), "expiresAt": crate::autosave::unix_now() + expires.saturating_duration_since(Instant::now()).as_secs() as i64, "verification": {"kind": "structural", "passed": true}, "changes": changes, "sourceCount": sources.len()});
        self.drafts
            .insert(id, Draft { client: grant.client.id, actor: grant.actor.clone(), request, digest, prepared, sources, expires, preview: preview.clone() });
        Ok(preview)
    }

    fn check_draft(&self, session: &Session, grant: &Grant, draft: &Draft, edit: bool) -> Result<(), ServiceError> {
        grant.check(session, draft.request.project_id, Some(draft.request.sequence_id), edit)?;
        if draft.client != grant.client.id {
            return Err(error(ErrorCode::PermissionDenied, "preview belongs to another authorization session"));
        }
        if Instant::now() >= draft.expires {
            return Err(error(ErrorCode::Canceled, "preview expired; prepare a new request"));
        }
        if !draft.prepared.is_current(session) {
            return Err(error(ErrorCode::RevisionConflict, "project changed; prepare a new preview"));
        }
        Ok(())
    }

    fn commit(&mut self, session: &mut Session, grant: &Grant, id: DocumentId, cancellation: &Cancellation) -> Reply {
        let draft = self.drafts.get(&id).ok_or_else(|| error(ErrorCode::NotFound, "preview not found"))?;
        self.check_draft(session, grant, draft, true)?;
        crate::service_sources::verify(
            session,
            draft.request.sequence_id,
            &draft.sources,
            &crate::service_sources::ReadContext {
                roots: &grant.roots,
                budget: grant.scope.max_source_bytes,
                cancellation,
                revoked: &grant.client.revoked,
                expires: draft.expires.min(grant.expires),
            },
        )?;
        grant.check(session, draft.request.project_id, Some(draft.request.sequence_id), true)?;
        cancellation.check()?;
        if session.operation_records.iter().filter(|r| r.retained_until > crate::autosave::unix_now()).count() >= 128 {
            return Err(error(ErrorCode::ResourceLimit, "operation receipt retention is full"));
        }
        if session.persistence.is_none() || !session.prefs.auto_save.recovery_journal {
            return Err(error(ErrorCode::CapabilityUnavailable, "durable recovery must be enabled by the host"));
        }
        session.ensure_revision_available().map_err(|_| error(ErrorCode::ResourceLimit, "revision counter exhausted"))?;
        let mut draft = self.drafts.remove(&id).ok_or_else(|| error(ErrorCode::NotFound, "preview not found"))?;
        let retained_until = crate::autosave::unix_now() + RECEIPT_LIFETIME;
        let result = json!({"apiVersion": "1", "state": "committed", "transactionId": id, "projectId": session.document_id, "operationId": draft.request.operation_id, "priorRevision": session.revision.to_string(), "revision": (session.revision + 1).to_string(), "sequenceId": draft.request.sequence_id.0.to_string(), "changes": draft.preview["changes"], "label": draft.request.label, "commands": draft.request.edits, "sources": draft.sources, "verification": {"kind": "structural", "passed": true}, "warnings": [], "undoAvailable": session.history.limit > 0, "retainedUntil": retained_until});
        draft.prepared.operation = Some(OperationRecord {
            actor: grant.actor.clone(),
            operation_id: draft.request.operation_id,
            idempotency_key: draft.request.idempotency_key,
            request: draft.digest,
            sequence: draft.request.sequence_id.0,
            retained_until,
            result: result.clone(),
        });
        session.commit_edits_durable(draft.prepared).map_err(transaction_error)?;
        Ok(result)
    }
}

fn changes(before: &Project, after: &Project, sequence: ItemId) -> Result<Value, ServiceError> {
    let clips = |p: &Project| -> BTreeMap<_, _> {
        p.sequence(sequence).into_iter().flat_map(|s| s.all_tracks()).flat_map(|t| t.items.iter().map(move |i| (i.id, (t.id, i.clone())))).collect()
    };
    let (old, new) = (clips(before), clips(after));
    let ids: BTreeSet<_> = old.keys().chain(new.keys()).copied().collect();
    if ids.len() > 100_000 {
        return Err(error(ErrorCode::ResourceLimit, "sequence exceeds structural preview budget"));
    }
    let range = |entry: Option<&(shotclutch_project::TrackId, shotclutch_project::TrackItem)>| {
        entry.map(|(track, clip)| json!({"trackId": track.0.to_string(), "sourceId": clip.item.0.to_string(), "start": clip.start.0.to_string(), "end": clip.range().end().0.to_string(), "sourceIn": clip.source_in.0.to_string()}))
    };
    Ok(Value::Array(
        ids.into_iter()
            .filter(|id| old.get(id) != new.get(id))
            .map(|id| json!({"clipId": id.0.to_string(), "before": range(old.get(&id)), "after": range(new.get(&id))}))
            .collect(),
    ))
}

pub(crate) fn validate_records(records: &[OperationRecord], project: DocumentId) -> Result<(), String> {
    let mut keys = BTreeSet::new();
    let mut operations = BTreeSet::new();
    let mut transactions = BTreeSet::new();
    for record in records {
        let transaction = serde_json::from_value::<DocumentId>(record.result["transactionId"].clone()).ok();
        let decimal = |key: &str| record.result[key].as_str().and_then(|s| s.parse::<u64>().ok().filter(|n| n.to_string() == s));
        let revisions_valid = decimal("priorRevision").and_then(|n| n.checked_add(1)) == decimal("revision") && decimal("revision").is_some();
        if !identifier(&record.actor)
            || !identifier(&record.operation_id)
            || !identifier(&record.idempotency_key)
            || record.request.len() != 64
            || !record.request.bytes().all(|b| b.is_ascii_hexdigit())
            || record.result["projectId"] != json!(project)
            || record.result["operationId"] != record.operation_id
            || record.result["state"] != "committed"
            || record.result["apiVersion"] != "1"
            || decimal("sequenceId") != Some(record.sequence)
            || record.result["retainedUntil"].as_i64() != Some(record.retained_until)
            || !revisions_valid
            || transaction.is_none()
            || !transactions.insert(transaction)
            || !keys.insert((&record.actor, &record.idempotency_key))
            || !operations.insert((&record.actor, &record.operation_id))
        {
            return Err("invalid or duplicate retained operation receipt".into());
        }
    }
    Ok(())
}

impl Session {
    pub(crate) fn close_editor_service(&mut self) {
        for grant in self.service.clients.values() {
            grant.client.revoke();
        }
        self.service = EditorService::default();
    }
    pub fn configure_owner_editor(&mut self, scope: EditorScope) {
        if let Some(owner) = self.service.owner.take() {
            self.revoke_editor(&owner);
        }
        self.owner_editor_scope = Some(scope);
    }
    pub fn revoke_editor(&mut self, client: &EditorClient) {
        client.revoke();
        self.service.clients.remove(&client.id);
        self.service.drafts.retain(|_, draft| draft.client != client.id);
    }
    fn with_editor_service<T>(&mut self, f: impl FnOnce(&mut EditorService, &mut Session) -> T) -> T {
        let mut service = std::mem::take(&mut self.service);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&mut service, self)));
        self.service = service;
        match result {
            Ok(value) => value,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }
    pub fn authorize_editor(&mut self, actor: &str, scope: EditorScope) -> Result<EditorClient, ServiceError> {
        self.with_editor_service(|service, session| service.issue(session, actor, scope))
    }
    pub fn execute_as(&mut self, client: &EditorClient, id: &str, params: Value) -> Reply {
        self.execute_as_cancellable(client, id, params, &Cancellation::default())
    }
    pub fn execute_as_cancellable(&mut self, client: &EditorClient, id: &str, params: Value, cancellation: &Cancellation) -> Reply {
        self.with_editor_service(|service, session| service.dispatch(session, client, id, params, cancellation))
    }
    /// Trusted local-owner entry point. Transport implementations must authenticate owner
    /// authority before calling this; scoped clients must use execute_as instead.
    pub fn execute_transaction_owner(&mut self, id: &str, params: Value, cancellation: &Cancellation) -> Reply {
        self.with_editor_service(|service, session| {
            let owner = service.owner(session)?;
            service.dispatch(session, &owner, id, params, cancellation)
        })
    }
    pub fn require_editor_read(&self, client: &EditorClient, sequence: ItemId) -> Result<(), ServiceError> {
        self.service.grant(self, client)?.check(self, self.document_id, Some(sequence), false)
    }
}

pub(crate) fn commands() -> Vec<crate::commands::CommandSpec> {
    use crate::commands::{CommandSpec, always};
    fn call(s: &mut Session, p: &Value, id: &str) -> crate::Result<Value> {
        s.execute_transaction_owner(id, p.clone(), &Cancellation::default()).map_err(|e| crate::EngineError::Other(e.to_string()))
    }
    vec![
        CommandSpec {
            id: "transaction.preview",
            label: "Preview Transaction",
            menu: &[],
            shortcut: None,
            params: "{apiVersion,projectId,operationId,idempotencyKey,expectedRevision,sequenceId,label,edits}",
            enabled: always,
            run: |s, p| call(s, p, "transaction.preview"),
            journal: false,
        },
        CommandSpec {
            id: "transaction.commit",
            label: "Commit Transaction",
            menu: &[],
            shortcut: None,
            params: "{apiVersion,projectId,transactionId}",
            enabled: always,
            run: |s, p| call(s, p, "transaction.commit"),
            journal: false,
        },
        CommandSpec {
            id: "transaction.abort",
            label: "Abort Transaction",
            menu: &[],
            shortcut: None,
            params: "{apiVersion,projectId,transactionId}",
            enabled: always,
            run: |s, p| call(s, p, "transaction.abort"),
            journal: false,
        },
        CommandSpec {
            id: "transaction.inspect",
            label: "Inspect Transaction",
            menu: &[],
            shortcut: None,
            params: "{apiVersion,projectId,transactionId}",
            enabled: always,
            run: |s, p| call(s, p, "transaction.inspect"),
            journal: false,
        },
        CommandSpec {
            id: "operation.inspect",
            label: "Inspect Operation",
            menu: &[],
            shortcut: None,
            params: "{apiVersion,projectId,operationId}",
            enabled: always,
            run: |s, p| call(s, p, "operation.inspect"),
            journal: false,
        },
    ]
}

#[cfg(test)]
#[path = "m1_service_tests.rs"]
mod m1_service_tests;
