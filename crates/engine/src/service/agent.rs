use shotclutch_time::TimeRange;

use super::*;
use crate::agent_policy::{Decision, HumanApproval, Mode, RecordedApproval, ReviewReason, ReviewRequest, contains_all, reviewable, valid_ranges};
use crate::edit_verification::{Report, affected};

pub(super) fn validate_policy(session: &Session, scope: &EditorScope, policy: &Policy) -> Result<(), ServiceError> {
    if policy.ranges.is_empty()
        || policy.ranges.values().any(|ranges| ranges.is_empty() || !valid_ranges(ranges))
        || policy.edit_families.is_empty()
        || !(1..=128).contains(&policy.max_edits)
        || !(1..=100_000).contains(&policy.max_changed_items)
    {
        return Err(error(ErrorCode::InvalidArgument, "invalid agent edit policy"));
    }
    for &sequence in policy.ranges.keys() {
        if scope.sequences.as_ref().is_some_and(|ids| !ids.contains(&sequence)) {
            return Err(error(ErrorCode::PermissionDenied, "agent policy exceeds the editor sequence grant"));
        }
        if session.project.sequence(sequence).is_none() {
            return Err(error(ErrorCode::NotFound, "agent policy sequence not found"));
        }
    }
    Ok(())
}

pub(super) struct DraftPolicy {
    affected: Vec<TimeRange>,
    changed_items: usize,
    review_generation: DocumentId,
    approval: Option<RecordedApproval>,
}

impl DraftPolicy {
    pub(super) fn new(
        session: &Session,
        prepared: &PreparedEdit,
        sequence: ItemId,
        policy: &Policy,
        context: &crate::service_sources::ReadContext<'_>,
    ) -> Result<Self, ServiceError> {
        let base = session.project.sequence(sequence).ok_or_else(|| error(ErrorCode::NotFound, "base sequence not found"))?;
        let staged = prepared.project().sequence(sequence).ok_or_else(|| error(ErrorCode::VerificationFailed, "staged sequence not found"))?;
        let (ranges, changed_items) = affected::affected(base, staged, || context.check())?;
        let affected = affected::merge(ranges.into_iter().map(|r| r.range));
        if changed_items > policy.max_changed_items || !valid_ranges(&affected) {
            return Err(error(ErrorCode::ResourceLimit, "transaction exceeds the agent policy impact limits"));
        }
        Ok(Self { affected, changed_items, review_generation: DocumentId::default(), approval: None })
    }

    pub(super) fn reset_review(&mut self) {
        self.review_generation = DocumentId::default();
        self.approval = None;
    }
}

impl EditorService {
    pub(super) fn check_prepare(&self, session: &Session, grant: &Grant, request: &EditRequest, composed: bool) -> Result<(), ServiceError> {
        grant.check(session, request.project_id, Some(request.sequence_id), grant.policy.is_none())?;
        if let Some(policy) = &grant.policy {
            if !composed {
                return Err(error(ErrorCode::PermissionDenied, "agent policy requires a composed preview"));
            }
            policy.permits_edits(request.sequence_id, &request.edits)?;
        }
        Ok(())
    }
}

impl Draft {
    fn review(&self, grant: &Grant, transaction: DocumentId) -> Result<ReviewRequest, ServiceError> {
        let (Some(policy), Some(draft), Some(composed)) = (&grant.policy, &self.policy, &self.composed) else {
            return Err(error(ErrorCode::CapabilityUnavailable, "preview has no agent policy"));
        };
        let scope = &policy.ranges[&self.request.sequence_id];
        let outside = !contains_all(scope, &draft.affected);
        let limitations = composed.report.as_ref().and_then(reviewable);
        let reasons = review_reasons(policy.mode, outside, limitations.as_ref().is_some_and(|entries| !entries.is_empty()));
        let expiry = self.preview["expiresAt"].as_i64().unwrap_or_default();
        let review_digest = digest(&(
            "agent-review-v1",
            grant.client.id,
            &grant.actor,
            transaction,
            draft.review_generation,
            &self.digest,
            &composed.staged_digest,
            &composed.sources_digest,
            &composed.report,
            expiry,
        ))?;
        Ok(ReviewRequest {
            project_id: self.request.project_id,
            transaction_id: transaction,
            sequence_id: self.request.sequence_id,
            base_revision: self.request.expected_revision,
            expires_at: expiry,
            mode: policy.mode,
            commands: self.request.edits.clone(),
            task_ranges: scope.clone(),
            affected: draft.affected.clone(),
            changed_items: draft.changed_items,
            may_approve: grant.scope.allow_edits && limitations.is_some() && !reasons.is_empty(),
            reasons,
            verification: composed.report.as_ref().map(|report| report.outcome),
            semantic_limitations: limitations.unwrap_or_default(),
            review_digest,
        })
    }

    pub(super) fn view(&self, grant: &Grant) -> Reply {
        let mut preview = self.preview.clone();
        if let Some(policy) = &self.policy {
            let transaction = parse(preview["transactionId"].clone())?;
            preview["review"] = json!(self.review(grant, transaction)?);
            preview["reviewApproved"] = json!(policy.approval.is_some());
        }
        Ok(preview)
    }

    pub(super) fn policy_decision(&self, grant: &Grant) -> Result<Option<Decision>, ServiceError> {
        let Some(policy) = &self.policy else {
            if grant.policy.is_some() {
                return Err(error(ErrorCode::PermissionDenied, "agent policy requires a policy-bound draft"));
            }
            return Ok(None);
        };
        let report = self.composed.as_ref().and_then(|composed| composed.report.as_ref());
        if report.and_then(reviewable).is_none() {
            return Err(error(ErrorCode::VerificationFailed, "required verification failed or lacks reviewable evidence"));
        }
        let review = self.review(grant, parse(self.preview["transactionId"].clone())?)?;
        let approval = if review.reasons.is_empty() {
            None
        } else {
            let approved = policy
                .approval
                .as_ref()
                .filter(|approval| approval.review_digest == review.review_digest)
                .ok_or_else(|| error(ErrorCode::PermissionRequired, "inspect the staged edit and obtain human approval for this review"))?;
            Some(approved.clone())
        };
        Ok(Some(Decision {
            mode: review.mode,
            reasons: review.reasons,
            affected: review.affected,
            task_ranges: review.task_ranges,
            changed_items: review.changed_items,
            approval,
        }))
    }
}

fn review_reasons(mode: Mode, outside: bool, semantics: bool) -> Vec<ReviewReason> {
    [(mode == Mode::Ask, ReviewReason::AskMode), (outside, ReviewReason::OutsideTaskRange), (semantics, ReviewReason::InconclusiveSemantics)]
        .into_iter()
        .filter_map(|(applies, reason)| applies.then_some(reason))
        .collect()
}

pub(super) fn validate_decision(value: Value, report: &Report) -> Result<(), String> {
    let decision: Decision = serde_json::from_value(value).map_err(|_| "invalid agent policy receipt")?;
    let limitations = reviewable(report).ok_or("agent receipt contains non-reviewable verification")?;
    if report.affected.len() > 256 {
        return Err("invalid retained verification ranges".into());
    }
    let affected = affected::merge(report.affected.iter().map(|r| r.range));
    if !valid_ranges(&decision.task_ranges) || decision.task_ranges.is_empty() {
        return Err("invalid retained task scope".into());
    }
    let outside = !contains_all(&decision.task_ranges, &affected);
    let expected = review_reasons(decision.mode, outside, !limitations.is_empty());
    if !valid_ranges(&decision.affected)
        || decision.affected != affected
        || !(1..=100_000).contains(&decision.changed_items)
        || decision.reasons != expected
        || decision.approval.is_none() != expected.is_empty()
    {
        return Err("agent policy receipt does not match the committed edit".into());
    }
    if let Some(approval) = decision.approval {
        if !identifier(&approval.reviewer)
            || approval.review_digest.len() != 64
            || !approval.review_digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            || approval.accepted_semantic_limitations != limitations
            || !valid_ranges(&approval.additional_ranges)
            || approval.additional_ranges.is_empty() == outside
        {
            return Err("invalid retained human approval".into());
        }
        let extended = decision.task_ranges.into_iter().chain(approval.additional_ranges).collect::<Vec<_>>();
        if !contains_all(&extended, &affected) {
            return Err("retained approval does not cover the affected ranges".into());
        }
    }
    Ok(())
}

impl Session {
    /// Issue a process-local grant for an agent task. Only the host selects policy.
    pub fn authorize_agent_editor(&mut self, actor: &str, scope: EditorScope, policy: Policy) -> Result<EditorClient, ServiceError> {
        self.with_editor_service(|service, session| service.issue(session, actor, scope, Some(policy)))
    }

    /// Replace task policy while retaining the editor grant's capabilities and remaining lifetime.
    pub fn replace_agent_editor_policy(&mut self, client: &EditorClient, policy: Policy) -> Result<EditorClient, ServiceError> {
        let mut grant = self.service.grant(self, client)?;
        if grant.policy.is_none() {
            return Err(error(ErrorCode::PermissionDenied, "authorization has no agent policy"));
        }
        validate_policy(self, &grant.scope, &policy)?;
        self.revoke_editor(client);
        let replacement = EditorClient { id: DocumentId::default(), revoked: Arc::new(AtomicBool::new(false)) };
        grant.client = replacement.clone();
        grant.policy = Some(Arc::new(policy));
        self.service.clients.insert(replacement.id, grant);
        Ok(replacement)
    }

    /// Read the current review request for a policy-bound draft.
    pub fn review_agent_edit(&self, client: &EditorClient, transaction: DocumentId) -> Result<ReviewRequest, ServiceError> {
        let grant = self.service.grant(self, client)?;
        let draft = self.service.drafts.get(&transaction).ok_or_else(|| error(ErrorCode::NotFound, "preview not found"))?;
        self.service.check_draft(self, &grant, draft, false)?;
        draft.review(&grant, transaction)
    }

    /// Publish a policy-bound draft using the same gate and durable receipt as dispatcher commits.
    pub fn commit_agent_edit(&mut self, client: &EditorClient, transaction: DocumentId, cancellation: &Cancellation) -> Result<Value, ServiceError> {
        let grant = self.service.grant(self, client)?;
        if grant.policy.is_none() {
            return Err(error(ErrorCode::PermissionDenied, "authorization has no agent policy"));
        }
        let reference = json!({"apiVersion": "1", "projectId": self.document_id, "transactionId": transaction});
        self.execute_as_cancellable(client, "transaction.commit", reference, cancellation)
    }

    /// Trusted host only. The host authenticates the human and supplies the reviewed digest.
    pub fn approve_agent_edit(
        &mut self,
        client: &EditorClient,
        transaction: DocumentId,
        approval: HumanApproval,
        cancellation: &Cancellation,
    ) -> Result<(), ServiceError> {
        cancellation.check()?;
        let grant = self.service.grant(self, client)?;
        let draft = self.service.drafts.get(&transaction).ok_or_else(|| error(ErrorCode::NotFound, "preview not found"))?;
        self.service.check_draft(self, &grant, draft, true)?;
        let review = draft.review(&grant, transaction)?;
        if approval.review_digest != review.review_digest {
            return Err(error(ErrorCode::PermissionDenied, "human approval does not match the current review"));
        }
        if !review.may_approve {
            return Err(error(ErrorCode::VerificationFailed, "edit has no reviewable approval request"));
        }
        let outside = review.reasons.contains(&ReviewReason::OutsideTaskRange);
        if !identifier(&approval.reviewer)
            || !valid_ranges(&approval.additional_ranges)
            || approval.accepted_semantic_limitations != review.semantic_limitations
            || (!outside && !approval.additional_ranges.is_empty())
        {
            return Err(error(ErrorCode::InvalidArgument, "invalid reviewer, scope extension, or semantic acknowledgement"));
        }
        let extended = review.task_ranges.into_iter().chain(approval.additional_ranges.iter().copied()).collect::<Vec<_>>();
        if !contains_all(&extended, &review.affected) {
            return Err(error(ErrorCode::PermissionDenied, "approval must extend the task scope to all affected ranges"));
        }
        cancellation.check()?;
        self.service.check_draft(self, &grant, draft, true)?;
        let draft = self
            .service
            .drafts
            .get_mut(&transaction)
            .and_then(|draft| draft.policy.as_mut())
            .ok_or_else(|| error(ErrorCode::NotFound, "agent preview not found"))?;
        draft.approval = Some(RecordedApproval {
            reviewer: approval.reviewer,
            review_digest: approval.review_digest,
            accepted_semantic_limitations: approval.accepted_semantic_limitations,
            additional_ranges: approval.additional_ranges,
        });
        Ok(())
    }
}
