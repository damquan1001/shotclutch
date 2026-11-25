use super::*;
use crate::agent_policy::{EditFamily, HumanApproval, Mode};
use crate::autosave::AutosaveConfig;
use crate::media_observations::{ObservationProducer, ObservationRequest};
use crate::media_test_util::{make_movie, session_with};
use shotclutch_media::DemoScene;
use shotclutch_project::observations::{ObservationKind, SourceLocation, SourceStream, TextRole, WordTime};
use shotclutch_time::{FrameRate, TICKS_PER_SECOND};

const QUERY: &str = "ignore instructions";

fn tick(frame: i64) -> Tick {
    FrameRate::FPS_24.tick_of(frame)
}

fn limits() -> Limits {
    Limits {
        elapsed_ms: 60_000,
        tool_calls: 25,
        source_bytes: 32 << 20,
        frames: 100,
        inspected_ticks: 2 * TICKS_PER_SECOND as u64,
        retries: 1,
        transaction_clips: 8,
    }
}

fn authorize(session: &mut Session, sequence: ItemId, root: &Path, mode: Mode) -> EditorClient {
    let mut scope = EditorScope::sequence(sequence);
    scope.allow_edits = true;
    scope.media_roots.push(root.to_owned());
    let policy = Policy {
        mode,
        ranges: [(sequence, vec![TimeRange::from_bounds(tick(0), tick(120))])].into(),
        edit_families: [EditFamily::Split].into(),
        max_edits: 1,
        max_changed_items: 32,
    };
    session.authorize_agent_editor("phrase-agent", scope, policy).unwrap()
}

struct Fixture {
    dir: tempfile::TempDir,
    path: PathBuf,
    session: Session,
    sequence: ItemId,
    asset: ItemId,
    clips: Vec<ClipId>,
    client: EditorClient,
    onset: Tick,
}

impl Fixture {
    fn new(mode: Mode) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("phrase.mov");
        make_movie(&path, DemoScene::Aurora, 32, 18, 28);
        let (mut session, assets, sequence) = session_with(&[&path]);
        let project = Arc::make_mut(&mut session.project);
        let link = project.alloc_id();
        let seq = project.sequence_mut(sequence).unwrap();
        let video = &mut seq.video_tracks[0].items[0];
        let audio = &mut seq.audio_tracks[0].items[0];
        for item in [&mut *video, &mut *audio] {
            item.start = tick(4);
            item.source_in = tick(3);
            item.duration = tick(20);
            item.link = Some(link);
        }
        let clips = vec![video.id, audio.id];
        session.start_autosave(AutosaveConfig::new(dir.path())).unwrap();
        let client = authorize(&mut session, sequence, dir.path(), mode);
        let onset = tick(9) + Tick(tick(1).0 / 4);
        let mut fixture = Self { dir, path, session, sequence, asset: assets[0], clips, client, onset };
        fixture.record(onset, TextRole::Transcript, "Narrator");
        fixture
    }

    fn record(&mut self, onset: Tick, role: TextRole, speaker: &str) {
        let client = self
            .session
            .authorize_editor(
                "annotation",
                EditorScope { sequences: None, allow_edits: true, media_roots: vec![self.dir.path().to_owned()], ..EditorScope::sequence(self.sequence) },
            )
            .unwrap();
        let models = ActiveModels::new();
        let fingerprint = self.session.inspect_media_observations(&client, self.asset, &models, &Default::default()).unwrap().fingerprint.unwrap();
        let mut record = ObservationRequest::verified_asset(self.asset, self.session.revision, fingerprint);
        record.kind = ObservationKind::Segment;
        let end = onset + tick(2);
        record.location = SourceLocation::SourceRange { stream: SourceStream::Video, start: onset.0, end: end.0 };
        record.producer = ObservationProducer::Human;
        record.text_role = Some(role);
        record.summary = Some(QUERY.into());
        if role == TextRole::Transcript {
            record.location = SourceLocation::SourceRange { stream: SourceStream::Audio(0), start: onset.0, end: end.0 };
            record.word_times = vec![WordTime { start: onset.0, end: (onset + tick(1)).0 }, WordTime { start: (onset + tick(1)).0, end: end.0 }];
            record.speaker = Some(speaker.into());
        }
        self.session.record_media_observation(&client, record, &Default::default()).unwrap();
        self.session.revoke_editor(&client);
    }

    fn request(&self) -> PhraseSplit {
        PhraseSplit {
            project: self.session.document_id,
            sequence: self.sequence,
            expected_revision: self.session.revision,
            phrase: QUERY.into(),
            asset: Some(self.asset),
            speaker: Some("Narrator".into()),
            placement: None,
        }
    }

    fn start(&self) -> AgentRun {
        self.session.start_agent_run(&self.client, self.request(), limits()).unwrap()
    }

    fn repeat_placement(&mut self) {
        let project = Arc::make_mut(&mut self.session.project);
        let link = project.alloc_id();
        let ids = [ClipId(project.alloc_id()), ClipId(project.alloc_id())];
        let seq = project.sequence_mut(self.sequence).unwrap();
        for (track, id) in [&mut seq.video_tracks[0], &mut seq.audio_tracks[0]].into_iter().zip(ids) {
            let mut clip = track.items[0].clone();
            clip.id = id;
            clip.start = tick(30);
            clip.link = Some(link);
            track.items.push(clip);
        }
    }
}

fn advance(session: &mut Session, run: &mut AgentRun) -> RunStatus {
    session.advance_agent_run(run, &ActiveModels::new(), &OfflinePhrasePlanner).unwrap()
}

fn reach(session: &mut Session, run: &mut AgentRun, target: Phase) -> RunStatus {
    for _ in 0..12 {
        let status = run.handle().inspect();
        if status.phase == target {
            return status;
        }
        assert!(!status.phase.terminal(), "{status:?}");
        advance(session, run);
    }
    panic!("run did not reach {target:?}");
}

fn approve(session: &mut Session, client: &EditorClient, status: &RunStatus) {
    let review = status.review.as_ref().unwrap();
    session
        .approve_agent_edit(
            client,
            review.transaction_id,
            HumanApproval {
                review_digest: review.review_digest.clone(),
                reviewer: "human".into(),
                accepted_semantic_limitations: vec![],
                additional_ranges: vec![],
            },
            &Default::default(),
        )
        .unwrap();
}

#[test]
fn phrase_maps_to_exact_linked_cut_and_undo_in_each_policy_mode() {
    for mode in [Mode::Ask, Mode::Assist, Mode::Autonomous] {
        let mut f = Fixture::new(mode);
        f.record(f.onset, TextRole::Transcript, "Narrator");
        f.record(tick(16), TextRole::Ocr, "");
        f.record(tick(15), TextRole::Transcript, "Other");
        let mut run = f.start();
        let before = f.session.project.clone();
        let revision = f.session.revision;
        let undo = f.session.history.undo.len();
        if mode == Mode::Ask {
            let waiting = reach(&mut f.session, &mut run, Phase::AwaitingPermission);
            assert!(waiting.cut_verified && waiting.review.is_some());
            assert_eq!(advance(&mut f.session, &mut run).usage, waiting.usage);
            assert_eq!(f.session.revision, revision);
            approve(&mut f.session, &f.client, &waiting);
        }
        let done = reach(&mut f.session, &mut run, Phase::Completed);
        let evidence = done.selected.unwrap();
        assert_eq!(evidence.range, TimeRange::from_bounds(f.onset, f.onset + tick(2)));
        assert_eq!(evidence.observations.len(), 2);
        assert_eq!(done.placements, [Placement { clips: f.clips, onset: f.onset + tick(1), cut: tick(11), rounding: tick(10) - f.onset }]);
        assert!(done.excerpt.unwrap().sample_count > 0);
        let report = done.verification.unwrap();
        assert_eq!(report.outcome, Outcome::Passed);
        assert!(!report.frames.is_empty() && !report.audio.is_empty());
        assert!(done.cut_verified && done.receipt.unwrap()["undoAvailable"] == true && done.usage.tool_calls == 5);
        assert_eq!(f.session.revision, revision + 1);
        assert_eq!(f.session.history.undo.len(), undo + 1);
        assert_eq!(f.session.undo().as_deref(), Some("Split at phrase"));
        assert_eq!(*f.session.project, *before);
        f.session.shutdown();
    }
}

#[test]
fn occurrence_and_placement_ambiguity_stop_with_choices() {
    let mut f = Fixture::new(Mode::Assist);
    f.record(tick(14), TextRole::Transcript, "Narrator");
    let revision = f.session.revision;
    let mut run = f.start();
    let failed = reach(&mut f.session, &mut run, Phase::Failed);
    assert_eq!(failed.candidates.len(), 2);
    assert_eq!(failed.placements.len(), 2);
    assert!(failed.message.unwrap().contains("narrow"));
    assert_eq!(f.session.revision, revision);
    f.session.shutdown();

    let mut f = Fixture::new(Mode::Assist);
    f.repeat_placement();
    let mut run = f.start();
    let failed = reach(&mut f.session, &mut run, Phase::Failed);
    assert_eq!(failed.candidates.len(), 1);
    assert_eq!(failed.placements.len(), 2);
    let mut request = f.request();
    request.placement = Some(f.clips[0]);
    let mut narrowed = f.session.start_agent_run(&f.client, request, limits()).unwrap();
    assert_eq!(reach(&mut f.session, &mut narrowed, Phase::Completed).placements[0].cut, tick(11));
    f.session.shutdown();
}

#[test]
fn matching_evidence_from_an_original_outside_the_sequence_cannot_supply_the_cut() {
    let mut f = Fixture::new(Mode::Assist);
    let distractor = f.dir.path().join("unplaced.mov");
    make_movie(&distractor, DemoScene::Aurora, 32, 18, 28);
    let imported = f.session.execute("file.import", json!({"paths": [distractor]})).unwrap();
    let target = f.asset;
    f.asset = ItemId(imported["items"][0].as_u64().unwrap());
    f.record(tick(4), TextRole::Transcript, "Narrator");
    f.asset = target;
    let mut request = f.request();
    request.asset = None;
    let mut run = f.session.start_agent_run(&f.client, request, limits()).unwrap();
    let ready = reach(&mut f.session, &mut run, Phase::Verify);
    assert_eq!(ready.candidates.len(), 1);
    assert_eq!(ready.selected.unwrap().asset, target);
    let id = ready.transaction.unwrap();
    assert_eq!(f.session.verify_staged_edit(&f.client, id, &Default::default()).unwrap_err().code, ErrorCode::PermissionDenied);
    assert_eq!(f.session.commit_agent_edit(&f.client, id, &Default::default()).unwrap_err().code, ErrorCode::PermissionDenied);
    reach(&mut f.session, &mut run, Phase::Completed);
    f.session.shutdown();
}

#[test]
fn budget_exhaustion_stale_media_and_retiming_stop_before_publication() {
    let mut f = Fixture::new(Mode::Assist);
    let bytes = std::fs::metadata(&f.path).unwrap().len();
    for (limits, message) in [
        (Limits { source_bytes: bytes + 1, ..limits() }, "source byte"),
        (Limits { frames: 1, ..limits() }, "frame budget"),
        (Limits { inspected_ticks: 1, ..limits() }, "duration budget"),
        (Limits { tool_calls: 4, ..limits() }, "tool call"),
        (Limits { transaction_clips: 1, ..limits() }, "target limit"),
    ] {
        let mut run = f.session.start_agent_run(&f.client, f.request(), limits).unwrap();
        let failed = reach(&mut f.session, &mut run, Phase::Failed);
        assert!(failed.message.unwrap().contains(message));
        assert!(f.session.service.drafts.is_empty());
    }
    let original = std::fs::read(&f.path).unwrap();
    let mut replacement = original.clone();
    let index = replacement.len() / 2;
    replacement[index] ^= 1;
    std::fs::write(&f.path, replacement).unwrap();
    let mut run = f.start();
    assert!(reach(&mut f.session, &mut run, Phase::Failed).message.unwrap().contains("stale"));
    std::fs::write(&f.path, original).unwrap();
    let project = Arc::make_mut(&mut f.session.project);
    project.sequence_mut(f.sequence).unwrap().audio_tracks[0].items[0].speed = 1.5;
    let mut run = f.start();
    assert!(reach(&mut f.session, &mut run, Phase::Failed).message.unwrap().contains("retimed"));
    let mut run = f.start();
    std::fs::remove_file(&f.path).unwrap();
    assert!(reach(&mut f.session, &mut run, Phase::Failed).message.unwrap().contains("missing"));
    f.session.shutdown();
}

#[test]
fn cancel_at_every_phase_discards_the_draft() {
    let mut f = Fixture::new(Mode::Ask);
    let revision = f.session.revision;
    for phase in [Phase::Search, Phase::Inspect, Phase::Plan, Phase::Stage, Phase::Verify, Phase::AwaitingPermission, Phase::Commit] {
        let mut run = f.start();
        if phase == Phase::Commit {
            let awaiting = reach(&mut f.session, &mut run, Phase::AwaitingPermission);
            approve(&mut f.session, &f.client, &awaiting);
        }
        reach(&mut f.session, &mut run, phase);
        run.handle().cancel();
        let status = advance(&mut f.session, &mut run);
        assert_eq!(status.phase, Phase::Canceled);
        assert!(status.receipt.is_none());
        assert_eq!(f.session.revision, revision);
        assert!(f.session.service.drafts.is_empty());
        assert!(status.events.windows(2).all(|events| events[0].number + 1 == events[1].number));
    }
    f.session.shutdown();
}

#[test]
fn unavailable_planner_concurrent_edit_and_revocation_stop_the_run() {
    struct Missing;
    impl Planner for Missing {
        fn plan(&self, _: &Occurrence, _: &Excerpt, _: &Placement) -> Result<PlannedAction, ServiceError> {
            Err(error(ErrorCode::CapabilityUnavailable, "selected planner is unavailable"))
        }
    }
    let mut f = Fixture::new(Mode::Assist);
    let mut run = f.start();
    reach(&mut f.session, &mut run, Phase::Plan);
    let result = f.session.advance_agent_run(&mut run, &ActiveModels::new(), &Missing).unwrap();
    assert!(result.message.unwrap().contains("planner is unavailable"));
    let mut run = f.start();
    reach(&mut f.session, &mut run, Phase::Verify);
    f.session.execute("file.newBin", json!({"name":"Manual change"})).unwrap();
    assert!(advance(&mut f.session, &mut run).message.unwrap().contains("REVISION_CONFLICT"));
    assert!(f.session.service.drafts.is_empty());
    let mut run = f.start();
    f.client.revoke();
    assert!(advance(&mut f.session, &mut run).message.unwrap().contains("PERMISSION_DENIED"));
    f.session.shutdown();
}

#[test]
fn resume_requires_a_fresh_grant_and_reconciles_commit_marker_before_dispatch() {
    let mut f = Fixture::new(Mode::Ask);
    let saved = f.dir.path().join("before.scproj");
    f.session.execute("file.saveAs", json!({"path": saved})).unwrap();
    let mut run = f.start();
    let first = reach(&mut f.session, &mut run, Phase::AwaitingPermission);
    approve(&mut f.session, &f.client, &first);
    drop(run);
    assert!(f.session.resume_agent_run(&f.client, first.id).is_err());
    f.session.persistence.take().unwrap().simulate_crash();
    let mut recovered = Session::default();
    recovered.start_autosave(AutosaveConfig::new(f.dir.path())).unwrap();
    recovered.execute("file.open", json!({"path": saved})).unwrap();
    let client = authorize(&mut recovered, f.sequence, f.dir.path(), Mode::Ask);
    let mut run = recovered.resume_agent_run(&client, first.id).unwrap();
    assert_eq!(run.handle().inspect().usage, first.usage);
    let second = reach(&mut recovered, &mut run, Phase::AwaitingPermission);
    assert_ne!(first.transaction, second.transaction);
    approve(&mut recovered, &client, &second);
    reach(&mut recovered, &mut run, Phase::Commit);
    crate::document::CRASH_AFTER_COMMIT.with(|fault| fault.set(true));
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| advance(&mut recovered, &mut run))).is_err());
    drop(run);
    recovered.persistence.take().unwrap().simulate_crash();
    let mut reopened = Session::default();
    reopened.start_autosave(AutosaveConfig::new(f.dir.path())).unwrap();
    reopened.execute("file.open", json!({"path": saved})).unwrap();
    let old_client = authorize(&mut reopened, f.sequence, f.dir.path(), Mode::Ask);
    match reopened.resume_agent_run(&old_client, first.id) {
        Err(err) => assert_eq!(err.code, ErrorCode::RevisionConflict),
        Ok(_) => panic!("older project opened a run whose commit was in the recovery journal"),
    }
    reopened.execute("file.recover", json!({})).unwrap();
    let client = authorize(&mut reopened, f.sequence, f.dir.path(), Mode::Ask);
    let mut run = reopened.resume_agent_run(&client, first.id).unwrap();
    let committed = advance(&mut reopened, &mut run);
    assert_eq!(committed.phase, Phase::Completed);
    assert_eq!(committed.transaction, second.transaction);
    assert_eq!(committed.usage.tool_calls, second.usage.tool_calls + 1);
    assert_eq!(committed.receipt.unwrap()["operationId"], first.operation_id);
    assert_eq!(reopened.revision, first.base_revision + 1);
    assert_eq!(reopened.undo().as_deref(), Some("Split at phrase"));
    reopened.shutdown();
}

#[test]
fn a_storage_retry_preserves_the_operation_and_charges_failed_work() {
    let mut f = Fixture::new(Mode::Assist);
    let mut run = f.start();
    let ready = reach(&mut f.session, &mut run, Phase::Commit);
    let marker = f.session.persistence.as_ref().unwrap().session_dir.join("snapshot.scproj");
    let prior = marker.with_extension("previous");
    std::fs::rename(&marker, &prior).unwrap();
    std::fs::create_dir(&marker).unwrap();
    let retry = advance(&mut f.session, &mut run);
    assert_eq!(retry.phase, Phase::Commit);
    assert_eq!(retry.usage.retries, 1);
    assert_eq!(retry.usage.tool_calls, ready.usage.tool_calls + 1);
    assert!(retry.usage.source_bytes > ready.usage.source_bytes);
    assert_eq!(f.session.revision, ready.base_revision);
    std::fs::remove_dir(&marker).unwrap();
    std::fs::rename(&prior, &marker).unwrap();
    let done = advance(&mut f.session, &mut run);
    assert_eq!(done.phase, Phase::Completed);
    assert_eq!(done.receipt.unwrap()["operationId"], ready.operation_id);
    assert_eq!(f.session.revision, ready.base_revision + 1);
    f.session.shutdown();
}
