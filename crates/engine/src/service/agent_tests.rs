use super::*;
use crate::agent_policy::{EditFamily, HumanApproval, Mode, Policy, ReviewReason, ReviewRequest};
use crate::media_observations::{ActiveModels, ObservationRequest};

fn range(start: i64, end: i64) -> TimeRange {
    TimeRange::from_bounds(tick(start), tick(end))
}

fn policy(sequence: ItemId, mode: Mode, regions: Vec<TimeRange>) -> Policy {
    Policy {
        mode,
        ranges: [(sequence, regions)].into(),
        edit_families: [EditFamily::Split, EditFamily::RegularTrim, EditFamily::RippleTrim].into(),
        max_edits: 4,
        max_changed_items: 12,
    }
}

fn scope(f: &Fixture, edit: bool) -> EditorScope {
    let mut scope = EditorScope::sequence(f.sequence);
    scope.allow_edits = edit;
    scope.media_roots.push(f.dir.path().to_owned());
    scope
}

fn bind(f: &mut Fixture, policy: Policy, edit: bool) {
    let scope = scope(f, edit);
    f.session.revoke_editor(&f.client);
    f.client = f.session.authorize_agent_editor("agent", scope, policy).unwrap();
}

fn review(f: &Fixture, id: DocumentId) -> ReviewRequest {
    f.session.review_agent_edit(&f.client, id).unwrap()
}

fn approval(review: &ReviewRequest, additional_ranges: Vec<TimeRange>) -> HumanApproval {
    HumanApproval {
        review_digest: review.review_digest.clone(),
        reviewer: "editor-reviewer".into(),
        accepted_semantic_limitations: review.semantic_limitations.clone(),
        additional_ranges,
    }
}

fn approve(f: &mut Fixture, id: DocumentId, decision: HumanApproval) -> Result<(), ServiceError> {
    f.session.approve_agent_edit(&f.client, id, decision, &Default::default())
}

fn trim(f: &Fixture, name: &str, mode: TrimMode) -> EditRequest {
    f.request(name, vec![TimelineEdit::Trim { clip: f.video, edge: Edge::Out, mode, delta: (-tick(3).0).to_string(), linked: true }])
}

fn compare_frame() -> Criterion {
    Criterion::FrameMatches { base: tick(2), staged: tick(2) }
}

#[test]
fn every_mode_runs_split_regular_trim_and_ripple_through_the_same_gate() {
    for mode in [Mode::Ask, Mode::Assist, Mode::Autonomous] {
        let mut f = Fixture::new(12);
        f.append_video(f.asset, 12, 24, false);
        let limit = if mode == Mode::Autonomous { 24 } else { 12 };
        let task = policy(f.sequence, mode, vec![range(0, 5), range(5, limit)]);
        bind(&mut f, task, true);
        let before = f.session.project.clone();
        for variant in 0..3 {
            let (request, criterion) = match variant {
                0 => (f.split("split"), Criterion::PreserveSplit { audio: true }),
                1 => (trim(&f, "trim", TrimMode::Regular), compare_frame()),
                _ => (trim(&f, "ripple", TrimMode::Ripple), Criterion::FrameMatches { base: tick(13), staged: tick(10) }),
            };
            let revision = f.session.revision;
            assert_eq!(f.session.execute_as(&f.client, "transaction.preview", json!(request)).unwrap_err().code, ErrorCode::PermissionDenied);
            let id = f.stage(request, vec![criterion]);
            assert_eq!(f.execute("transaction.commit", id).unwrap_err().code, ErrorCode::VerificationFailed);
            assert_eq!(f.verify(id).outcome, Outcome::Passed);
            let review = review(&f, id);
            let inspect = f.execute("transaction.inspect", id).unwrap();
            assert_eq!(inspect["review"], json!(review));
            assert_eq!(review.affected, vec![range(0, if variant == 2 { 24 } else { 12 })]);
            assert_eq!(review.changed_items, [4, 2, 3][variant]);
            let needs_extension = variant == 2 && mode != Mode::Autonomous;
            let needs_review = mode == Mode::Ask || needs_extension;
            assert_eq!(review.reasons.contains(&ReviewReason::AskMode), mode == Mode::Ask);
            assert_eq!(review.reasons.contains(&ReviewReason::OutsideTaskRange), needs_extension);
            assert_eq!(review.may_approve, needs_review);
            if needs_review {
                assert_eq!(f.execute("transaction.commit", id).unwrap_err().code, ErrorCode::PermissionRequired);
                assert_eq!(f.session.revision, revision);
                assert_eq!(*f.session.project, *before);
                let mut decision = approval(&review, Vec::new());
                if needs_extension {
                    assert_eq!(approve(&mut f, id, decision.clone()).unwrap_err().code, ErrorCode::PermissionDenied);
                    decision.additional_ranges = vec![range(12, 24)];
                }
                approve(&mut f, id, decision).unwrap();
                assert_eq!(f.execute("transaction.inspect", id).unwrap()["reviewApproved"], true);
            }
            let receipt = f.session.commit_agent_edit(&f.client, id, &Default::default()).unwrap();
            assert_eq!(receipt["policy"]["mode"], json!(mode));
            assert_eq!(receipt["policy"]["approval"].is_object(), needs_review);
            assert_eq!(f.execute("transaction.commit", id).unwrap(), receipt);
            assert_eq!(f.session.revision, revision + 1);
            assert_eq!(f.session.undo().as_deref(), Some("Verified edit"));
            assert_eq!(*f.session.project, *before);
        }
        f.session.shutdown();
    }
}

#[test]
fn proposal_grants_and_hard_limits_stay_separate_from_approval() {
    let mut f = Fixture::new(12);
    let mut task = policy(f.sequence, Mode::Ask, vec![range(0, 12)]);
    bind(&mut f, task.clone(), false);
    let id = f.stage(f.split("read-only"), vec![Criterion::PreserveSplit { audio: true }]);
    assert_eq!(f.verify(id).outcome, Outcome::Passed);
    let current = review(&f, id);
    assert!(!current.may_approve);
    assert_eq!(approve(&mut f, id, approval(&current, vec![])).unwrap_err().code, ErrorCode::PermissionDenied);
    assert_eq!(f.execute("transaction.commit", id).unwrap_err().code, ErrorCode::PermissionDenied);
    let revision = f.session.revision;

    task.edit_families.remove(&EditFamily::RippleTrim);
    task.max_edits = 1;
    task.max_changed_items = 1;
    let mut project_scope = scope(&f, true);
    project_scope.sequences = None;
    f.client = f.session.authorize_agent_editor("write-enabled", project_scope, task).unwrap();
    let observation = f.session.inspect_media_observations(&f.client, f.asset, &ActiveModels::default(), &Default::default()).unwrap();
    let request = ObservationRequest::verified_asset(f.asset, revision, observation.fingerprint.unwrap());
    assert_eq!(f.session.record_media_observation(&f.client, request, &Default::default()).unwrap_err().code, ErrorCode::PermissionDenied);
    let request = trim(&f, "forbidden", TrimMode::Ripple);
    assert_eq!(f.session.preview_verified_edit(&f.client, request, vec![compare_frame()], &Default::default()).unwrap_err().code, ErrorCode::PermissionDenied);
    let mut batch = f.split("batch");
    batch.edits.extend(batch.edits.clone());
    assert_eq!(f.session.preview_verified_edit(&f.client, batch, vec![compare_frame()], &Default::default()).unwrap_err().code, ErrorCode::ResourceLimit);
    let split = f.split("impact-limit");
    assert_eq!(f.session.preview_verified_edit(&f.client, split, vec![compare_frame()], &Default::default()).unwrap_err().code, ErrorCode::ResourceLimit);
    assert_eq!(f.execute("transaction.approve", id).unwrap_err().code, ErrorCode::PermissionDenied);
    assert_eq!(f.session.revision, revision);
    f.session.shutdown();
}

#[test]
fn semantic_approval_survives_storage_retry_and_recovery_with_one_undo() {
    let mut f = Fixture::new(12);
    let task = policy(f.sequence, Mode::Assist, vec![range(0, 12)]);
    bind(&mut f, task.clone(), true);
    let before = f.session.project.clone();
    let undo_count = f.session.history.undo.len();
    let criteria = vec![compare_frame(), Criterion::Semantic { check: SemanticCheck::TitleReadable }];
    let id = f.stage(f.split("semantic"), criteria);
    let uncommitted = f.stage(f.split("uncommitted"), vec![compare_frame()]);
    let report = f.verify(id);
    assert_eq!(report.outcome, Outcome::Inconclusive);
    let current = review(&f, id);
    assert_eq!(current.reasons, [ReviewReason::InconclusiveSemantics]);
    assert_eq!(current.semantic_limitations, [1]);
    assert_eq!(f.execute("transaction.commit", id).unwrap_err().code, ErrorCode::PermissionRequired);
    let accepted = approval(&current, vec![]);
    let mut incomplete = accepted.clone();
    incomplete.accepted_semantic_limitations.clear();
    assert_eq!(approve(&mut f, id, incomplete).unwrap_err().code, ErrorCode::InvalidArgument);
    approve(&mut f, id, accepted).unwrap();
    let target = f.session.persistence.as_ref().unwrap().session_dir.join("snapshot.scproj");
    std::fs::create_dir(&target).unwrap();
    let revision = f.session.revision;
    assert_eq!(f.execute("transaction.commit", id).unwrap_err().code, ErrorCode::InternalError);
    assert_eq!(f.session.revision, revision);
    assert!(Arc::ptr_eq(&before, &f.session.project));
    std::fs::remove_dir(&target).unwrap();
    let receipt = f.execute("transaction.commit", id).unwrap();
    assert_eq!(receipt["verification"]["outcome"], "inconclusive");
    assert_eq!(receipt["policy"]["approval"]["reviewer"], "editor-reviewer");
    assert_eq!(receipt["policy"]["approval"]["acceptedSemanticLimitations"], json!([1]));
    let mut tampered = f.session.operation_records.clone();
    tampered[0].result["policy"]["approval"] = Value::Null;
    assert!(validate_records(&tampered, f.session.document_id).is_err());
    let Fixture { dir, session, sequence, .. } = f;
    drop(session);
    let mut restored = Session::default();
    restored.start_autosave(AutosaveConfig::new(&dir.path().join("recovery"))).unwrap();
    restored.execute("file.recover", json!({})).unwrap();
    let mut scope = EditorScope::sequence(sequence);
    scope.allow_edits = true;
    scope.media_roots.push(dir.path().to_owned());
    let client = restored.authorize_agent_editor("agent", scope, task).unwrap();
    assert_eq!(restored.commit_agent_edit(&client, uncommitted, &Default::default()).unwrap_err().code, ErrorCode::NotFound);
    assert_eq!(restored.commit_agent_edit(&client, id, &Default::default()).unwrap(), receipt);
    assert_eq!(restored.revision, revision + 1);
    assert_eq!(restored.history.undo.len(), undo_count + 1);
    assert_eq!(restored.undo().as_deref(), Some("Verified edit"));
    assert_eq!(*restored.project, *before);
    assert_eq!(restored.history.undo.len(), undo_count);
    restored.shutdown();
}

#[test]
fn failed_checks_and_missing_media_evidence_cannot_be_approved() {
    let mut f = Fixture::new(12);
    let task = policy(f.sequence, Mode::Autonomous, vec![range(0, 12)]);
    bind(&mut f, task, true);
    let cases = [
        (Criterion::Luminance { time: tick(2), region: None, minimum: 1.0 }, SemanticCheck::TitleReadable, Outcome::Failed),
        (compare_frame(), SemanticCheck::SpeechIntelligible, Outcome::Inconclusive),
    ];
    let revision = f.session.revision;
    for (index, (technical, semantic, outcome)) in cases.into_iter().enumerate() {
        let id = f.stage(f.split(&format!("unreviewable-{index}")), vec![technical, Criterion::Semantic { check: semantic }]);
        assert_eq!(f.verify(id).outcome, outcome);
        let current = review(&f, id);
        assert!(!current.may_approve);
        assert_eq!(approve(&mut f, id, approval(&current, vec![])).unwrap_err().code, ErrorCode::VerificationFailed);
        assert_eq!(f.execute("transaction.commit", id).unwrap_err().code, ErrorCode::VerificationFailed);
    }
    let id = f.stage(f.split("missing-media"), vec![compare_frame(), Criterion::Semantic { check: SemanticCheck::TitleReadable }]);
    std::fs::remove_file(f.dir.path().join("original.mov")).unwrap();
    assert!(!f.verify(id).gaps.is_empty());
    let current = review(&f, id);
    assert!(!current.may_approve);
    assert_eq!(approve(&mut f, id, approval(&current, vec![])).unwrap_err().code, ErrorCode::VerificationFailed);
    assert_eq!(f.session.revision, revision);
    f.session.shutdown();
}

#[test]
fn review_is_bound_to_draft_grant_verification_attempt_and_current_inputs() {
    let mut f = Fixture::new(12);
    let task = policy(f.sequence, Mode::Ask, vec![range(0, 12)]);
    bind(&mut f, task.clone(), true);
    let first = f.stage(f.split("first"), vec![compare_frame()]);
    let second = f.stage(f.split("second"), vec![compare_frame()]);
    f.verify(first);
    f.verify(second);
    let accepted = approval(&review(&f, first), vec![]);
    assert_eq!(approve(&mut f, second, accepted.clone()).unwrap_err().code, ErrorCode::PermissionDenied);
    let other_scope = scope(&f, true);
    let other = f.session.authorize_agent_editor("other-agent", other_scope, task.clone()).unwrap();
    assert_eq!(f.session.approve_agent_edit(&other, first, accepted.clone(), &Default::default()).unwrap_err().code, ErrorCode::PermissionDenied);
    approve(&mut f, first, accepted.clone()).unwrap();
    f.verify(first);
    assert_eq!(f.execute("transaction.commit", first).unwrap_err().code, ErrorCode::PermissionRequired);
    assert_eq!(approve(&mut f, first, accepted).unwrap_err().code, ErrorCode::PermissionDenied);
    let accepted = approval(&review(&f, first), vec![]);
    let cancel = Cancellation::default();
    cancel.cancel();
    assert_eq!(f.session.approve_agent_edit(&f.client, first, accepted.clone(), &cancel).unwrap_err().code, ErrorCode::Canceled);
    approve(&mut f, first, accepted).unwrap();
    assert_eq!(f.session.commit_agent_edit(&f.client, first, &cancel).unwrap_err().code, ErrorCode::Canceled);
    let revision = f.session.revision;
    let path = f.dir.path().join("original.mov");
    let original = std::fs::read(&path).unwrap();
    let mut changed = original.clone();
    let index = changed.len() - 2 * crate::relink::FINGERPRINT_SPAN;
    changed[index] ^= 1;
    std::fs::write(&path, changed).unwrap();
    assert_eq!(f.execute("transaction.commit", first).unwrap_err().code, ErrorCode::MediaChanged);
    std::fs::write(&path, original).unwrap();
    f.session.service.drafts.get_mut(&first).unwrap().expires = Instant::now();
    assert_eq!(f.execute("transaction.commit", first).unwrap_err().code, ErrorCode::Canceled);
    assert_eq!(f.session.revision, revision);

    let accepted = approval(&review(&f, second), vec![]);
    approve(&mut f, second, accepted).unwrap();
    let mut new_mode = task;
    new_mode.mode = Mode::Autonomous;
    let replacement = f.session.replace_agent_editor_policy(&f.client, new_mode).unwrap();
    assert_eq!(f.execute("transaction.commit", second).unwrap_err().code, ErrorCode::PermissionDenied);
    f.client = replacement;
    assert_eq!(f.execute("transaction.commit", second).unwrap_err().code, ErrorCode::NotFound);

    let pending = f.stage(f.split("pending"), vec![compare_frame()]);
    f.verify(pending);
    f.session.execute("file.newBin", json!({"name": "Human edit"})).unwrap();
    assert_eq!(f.execute("transaction.commit", pending).unwrap_err().code, ErrorCode::RevisionConflict);
    f.client.revoke();
    assert_eq!(f.execute("transaction.commit", pending).unwrap_err().code, ErrorCode::PermissionDenied);
    f.session.shutdown();
}
