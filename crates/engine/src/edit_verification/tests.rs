use std::path::Path;

use shotclutch_media::{DemoScene, Generator, MediaSource, generators::GeneratorSource};
use shotclutch_project::{ClipId, ItemKind, Label, MediaClip, MediaRef, Track, TrackId, TrackKind};
use shotclutch_time::{FrameRate, Tick, TimeRange};

use super::*;
use crate::autosave::AutosaveConfig;
use crate::edit_verification::{Assessment, Criterion, Outcome, SemanticCheck, Snapshot};
use crate::media_test_util::{make_movie, session_with};
use crate::transactions::{Edge, TrimMode};

#[path = "../service/agent_tests.rs"]
mod agent_policy_tests;

fn tick(frame: i64) -> Tick {
    FrameRate::FPS_24.tick_of(frame)
}

struct Fixture {
    dir: tempfile::TempDir,
    session: Session,
    client: EditorClient,
    sequence: ItemId,
    video: ClipId,
    audio: ClipId,
    asset: ItemId,
}

impl Fixture {
    fn new(frames: i64) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("original.mov");
        make_movie(&path, DemoScene::Aurora, 32, 18, frames);
        // Legal unused bytes large enough that a quick identity check cannot see a middle write.
        let mut data = std::fs::read(&path).unwrap();
        let padding = 4 * crate::relink::FINGERPRINT_SPAN;
        data.extend_from_slice(&(padding as u32).to_be_bytes());
        data.extend_from_slice(b"free");
        data.resize(data.len() + padding - 8, 0);
        std::fs::write(&path, data).unwrap();
        let (mut session, assets, sequence) = session_with(&[&path]);
        let project = Arc::make_mut(&mut session.project);
        let link = project.alloc_id();
        let seq = project.sequence_mut(sequence).unwrap();
        seq.video_tracks[0].items[0].link = Some(link);
        seq.audio_tracks[0].items[0].link = Some(link);
        let (video, audio) = (seq.video_tracks[0].items[0].id, seq.audio_tracks[0].items[0].id);
        session.start_autosave(AutosaveConfig::new(&dir.path().join("recovery"))).unwrap();
        let client = authorize(&mut session, sequence, dir.path(), "agent");
        Self { dir, session, client, sequence, video, audio, asset: assets[0] }
    }

    fn request(&self, name: &str, edits: Vec<TimelineEdit>) -> EditRequest {
        EditRequest {
            api_version: "1".into(),
            project_id: self.session.document_id,
            operation_id: name.into(),
            idempotency_key: name.into(),
            expected_revision: self.session.revision,
            sequence_id: self.sequence,
            label: "Verified edit".into(),
            edits,
        }
    }

    fn split(&self, name: &str) -> EditRequest {
        self.request(name, vec![TimelineEdit::Split { clips: vec![self.video, self.audio], time: tick(5).0.to_string() }])
    }

    fn stage(&mut self, request: EditRequest, criteria: Vec<Criterion>) -> DocumentId {
        let value = self.session.preview_verified_edit(&self.client, request, criteria, &Default::default()).unwrap();
        assert_eq!(value["verification"]["state"], "pending");
        serde_json::from_value(value["transactionId"].clone()).unwrap()
    }

    fn verify(&mut self, id: DocumentId) -> crate::edit_verification::Report {
        self.session.verify_staged_edit(&self.client, id, &Default::default()).unwrap()
    }

    fn execute(&mut self, method: &str, id: DocumentId) -> Reply {
        let reference = reference(&self.session, id);
        self.session.execute_as(&self.client, method, reference)
    }

    fn append_video(&mut self, item: ItemId, start: i64, end: i64, top: bool) {
        let p = Arc::make_mut(&mut self.session.project);
        let mut clip = p.make_track_item(item, TrackKind::Video, tick(start), TimeRange::new(Tick::ZERO, tick(end - start)), FrameRate::FPS_24).unwrap();
        for effect in &mut clip.effects {
            shotclutch_project::resolve_auto_points(effect, (32, 18), (32, 18));
        }
        let track = top.then(|| Track::new(TrackId(p.alloc_id()), TrackKind::Video, "Overlay".into()));
        let seq = p.sequence_mut(self.sequence).unwrap();
        if let Some(track) = track {
            seq.video_tracks.push(track);
        }
        seq.video_tracks.last_mut().unwrap().items.push(clip);
    }
}

fn authorize(s: &mut Session, sequence: ItemId, root: &Path, actor: &str) -> EditorClient {
    let mut scope = EditorScope::sequence(sequence);
    scope.allow_edits = true;
    scope.media_roots.push(root.to_owned());
    s.authorize_editor(actor, scope).unwrap()
}

fn reference(s: &Session, id: DocumentId) -> Value {
    json!({"apiVersion":"1","projectId":s.document_id,"transactionId":id})
}

#[test]
fn verified_split_recovers_receipt_and_one_undo_restores_base() {
    let mut f = Fixture::new(12);
    let mut scope = EditorScope::sequence(f.sequence);
    scope.allow_edits = true;
    scope.media_roots.push(f.dir.path().to_owned());
    scope.max_source_bytes = std::fs::metadata(f.dir.path().join("original.mov")).unwrap().len();
    f.client = f.session.authorize_editor("agent", scope).unwrap();
    let before = f.session.project.clone();
    let request = f.split("split");
    let criteria = vec![Criterion::PreserveSplit { audio: true }];
    let id = f.stage(request.clone(), criteria.clone());
    assert_eq!(f.execute("transaction.commit", id).unwrap_err().code, ErrorCode::VerificationFailed);
    assert_eq!(
        f.session.preview_verified_edit(&f.client, request.clone(), vec![Criterion::PreserveSplit { audio: false }], &Default::default()).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(f.session.execute_as(&f.client, "transaction.preview", serde_json::to_value(&request).unwrap()).unwrap_err().code, ErrorCode::InvalidArgument);
    let report = f.verify(id);
    assert_eq!(report.outcome, Outcome::Passed, "{report:?}");
    assert_eq!(report.audio.len(), 2);
    assert!(report.audio.iter().all(|a| a.rms > 0.0));
    for side in [Snapshot::Base, Snapshot::Staged] {
        assert!(report.frames.iter().any(|entry| entry.snapshot == side && entry.time == tick(4)));
        assert!(report.frames.iter().any(|entry| entry.snapshot == side && entry.time == tick(5)));
    }
    assert!(Arc::ptr_eq(&f.session.project, &before));
    let receipt = f.execute("transaction.commit", id).unwrap();
    assert_eq!(receipt["verification"]["report"], serde_json::to_value(&report).unwrap());
    assert_eq!(f.execute("transaction.commit", id).unwrap(), receipt);
    assert_eq!(f.session.preview_verified_edit(&f.client, request, criteria, &Default::default()).unwrap(), receipt);
    let revision = f.session.revision;
    let Fixture { dir, session, sequence, .. } = f;
    drop(session);
    let mut restored = Session::default();
    restored.start_autosave(AutosaveConfig::new(&dir.path().join("recovery"))).unwrap();
    restored.execute("file.recover", json!({})).unwrap();
    assert_eq!(restored.revision, revision);
    let client = authorize(&mut restored, sequence, dir.path(), "agent");
    let params = reference(&restored, id);
    assert_eq!(restored.execute_as(&client, "transaction.commit", params).unwrap(), receipt);
    assert_eq!(restored.undo().as_deref(), Some("Verified edit"));
    assert_eq!(*restored.project, *before);
    restored.shutdown();
}

#[test]
fn trims_distinguish_expected_mapping_black_picture_and_silence() {
    let mut f = Fixture::new(12);
    f.append_video(f.asset, 12, 24, false);
    let request = |f: &Fixture, name: &str| {
        f.request(name, vec![TimelineEdit::Trim { clip: f.video, edge: Edge::Out, mode: TrimMode::Regular, delta: (-tick(3).0).to_string(), linked: true }])
    };
    let audio = TimeRange::from_bounds(tick(10), tick(11));
    let bad = f.stage(
        request(&f, "unwanted-gap"),
        vec![
            Criterion::Luminance { time: tick(10), region: None, minimum: 0.01 },
            Criterion::AudioLevel { range: audio, minimum_rms: 0.01, maximum_peak: 1.0 },
        ],
    );
    let report = f.verify(bad);
    assert_eq!(report.outcome, Outcome::Failed, "{report:?}");
    assert!(report.criteria.iter().all(|entry| entry.outcome == Outcome::Failed));
    assert_eq!(f.execute("transaction.commit", bad).unwrap_err().code, ErrorCode::VerificationFailed);
    assert!(f.execute("transaction.inspect", bad).is_ok());
    f.execute("transaction.abort", bad).unwrap();

    let audio_only = f.stage(
        request(&f, "audio-only"),
        vec![Criterion::FrameMatches { base: tick(2), staged: tick(2) }, Criterion::AudioLevel { range: audio, minimum_rms: 0.01, maximum_peak: 1.0 }],
    );
    let report = f.verify(audio_only);
    assert_eq!(report.outcome, Outcome::Failed);
    assert_eq!(report.criteria[0].outcome, Outcome::Passed);
    assert_eq!(report.criteria[1].outcome, Outcome::Failed);
    assert_eq!(f.execute("transaction.commit", audio_only).unwrap_err().code, ErrorCode::VerificationFailed);

    let good = f.stage(
        request(&f, "intentional-gap"),
        vec![
            Criterion::FrameMatches { base: tick(2), staged: tick(2) },
            Criterion::AudioMatches { base_start: tick(2), staged_start: tick(2), duration: tick(1) },
            Criterion::AudioLevel { range: audio, minimum_rms: 0.0, maximum_peak: 0.0 },
        ],
    );
    let report = f.verify(good);
    assert_eq!(report.outcome, Outcome::Passed, "{report:?}");
    assert!(matches!(report.criteria[2].observed, Assessment::AudioLevel { rms: 0.0, peak: 0.0 }));
    f.execute("transaction.commit", good).unwrap();
    f.session.shutdown();
}

#[test]
fn opaque_overlay_fails_composed_luminance_and_semantic_is_inconclusive() {
    let mut f = Fixture::new(12);
    let base = f.session.try_render_program_at(1.0, tick(5)).unwrap().to_rgba8();
    assert!(base.chunks_exact(4).map(|pixel| u64::from(pixel[0])).sum::<u64>() > 32 * 18);
    let p = Arc::make_mut(&mut f.session.project);
    let generator = Generator::BlackVideo;
    let info = GeneratorSource::new(generator.clone(), 32, 18, FrameRate::FPS_24, tick(12)).info().clone();
    let black = p.add_item(
        "Black",
        Label::Iris,
        ItemKind::Media(MediaClip {
            media: MediaRef::Generator(generator),
            info,
            interpret: Default::default(),
            mark_in: None,
            mark_out: None,
            markers: Vec::new(),
            offline: false,
            proxy: None,
            identity: None,
        }),
        None,
    );
    f.append_video(black, 3, 9, true);
    let project = Arc::make_mut(&mut f.session.project);
    let transition_id = shotclutch_project::TransitionId(project.alloc_id());
    let upper = project.sequence_mut(f.sequence).unwrap().video_tracks.last_mut().unwrap();
    let overlay = &mut upper.items[0];
    let opacity = overlay.effect_mut("opacity").unwrap().param_mut("opacity").unwrap();
    opacity.keyframes.push(shotclutch_project::Keyframe::new(tick(5), opacity.value.clone()));
    upper.transitions.push(shotclutch_project::Transition {
        id: transition_id,
        effect: shotclutch_project::effect::find_effect("cross_dissolve").unwrap().instance(),
        start: tick(6),
        duration: tick(3),
        from: Some(overlay.id),
        to: None,
        align: shotclutch_project::TransitionAlign::EndAtCut,
        reverse: false,
    });
    let id = f.stage(f.split("overlay"), vec![Criterion::Luminance { time: tick(5), region: None, minimum: 0.01 }]);
    let report = f.verify(id);
    assert_eq!(report.outcome, Outcome::Failed, "{report:?}");
    assert!(matches!(report.criteria[0].observed, Assessment::Luminance { mean: 0.0 }));
    assert!(report.frames.iter().any(|entry| entry.snapshot == Snapshot::Staged && entry.time == tick(7)));
    assert!(report.frames.iter().any(|entry| entry.snapshot == Snapshot::Staged && entry.time == tick(8)));
    let id = f.stage(f.split("semantic"), vec![Criterion::Semantic { check: SemanticCheck::SubjectVisible }]);
    assert_eq!(f.verify(id).outcome, Outcome::Inconclusive);
    assert_eq!(f.execute("transaction.commit", id).unwrap_err().code, ErrorCode::VerificationFailed);
    f.session.shutdown();
}

#[test]
fn verification_is_bound_to_grant_revision_expiry_cancellation_and_original_bytes() {
    let mut f = Fixture::new(12);
    let id = f.stage(f.split("bound"), vec![Criterion::PreserveSplit { audio: false }]);
    let other = authorize(&mut f.session, f.sequence, f.dir.path(), "agent");
    assert_eq!(f.session.verify_staged_edit(&other, id, &Default::default()).unwrap_err().code, ErrorCode::PermissionDenied);
    assert_eq!(f.verify(id).outcome, Outcome::Passed);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert_eq!(f.session.verify_staged_edit(&f.client, id, &cancellation).unwrap_err().code, ErrorCode::Canceled);
    assert_eq!(f.execute("transaction.commit", id).unwrap_err().code, ErrorCode::VerificationFailed);
    assert_eq!(f.verify(id).outcome, Outcome::Passed);
    let path = f.dir.path().join("original.mov");
    let original = std::fs::read(&path).unwrap();
    let mut changed = original.clone();
    let index = changed.len() - 2 * crate::relink::FINGERPRINT_SPAN;
    changed[index] ^= 1;
    std::fs::write(&path, changed).unwrap();
    assert_eq!(f.execute("transaction.commit", id).unwrap_err().code, ErrorCode::MediaChanged);
    std::fs::write(&path, original).unwrap();
    f.session.service.drafts.get_mut(&id).unwrap().expires = Instant::now();
    assert_eq!(f.session.verify_staged_edit(&f.client, id, &Default::default()).unwrap_err().code, ErrorCode::Canceled);
    let id = f.stage(f.split("stale"), vec![Criterion::PreserveSplit { audio: false }]);
    assert_eq!(f.verify(id).outcome, Outcome::Passed);
    f.session.execute("file.newBin", json!({"name":"Human edit"})).unwrap();
    assert_eq!(f.session.verify_staged_edit(&f.client, id, &Default::default()).unwrap_err().code, ErrorCode::RevisionConflict);
    assert_eq!(f.execute("transaction.commit", id).unwrap_err().code, ErrorCode::RevisionConflict);
    f.session.shutdown();
}

#[test]
fn ripple_records_displaced_ranges_and_failed_durable_write_keeps_the_verified_draft() {
    let mut f = Fixture::new(12);
    f.append_video(f.asset, 12, 24, false);
    let request =
        f.request("ripple", vec![TimelineEdit::Trim { clip: f.video, edge: Edge::Out, mode: TrimMode::Ripple, delta: (-tick(3).0).to_string(), linked: true }]);
    let id = f.stage(request, vec![Criterion::FrameMatches { base: tick(13), staged: tick(10) }]);
    let report = f.verify(id);
    assert_eq!(report.outcome, Outcome::Passed, "{report:?}");
    assert!(report.affected.iter().any(|a| a.snapshot == Snapshot::Base && a.range.end() == tick(24)));
    assert!(report.affected.iter().any(|a| a.snapshot == Snapshot::Staged && a.range.end() == tick(21)));
    let target = f.session.persistence.as_ref().unwrap().session_dir.join("snapshot.scproj");
    std::fs::create_dir(&target).unwrap();
    let before = f.session.project.clone();
    let revision = f.session.revision;
    assert_eq!(f.execute("transaction.commit", id).unwrap_err().code, ErrorCode::InternalError);
    assert!(Arc::ptr_eq(&f.session.project, &before));
    assert_eq!(f.session.revision, revision);
    assert!(f.session.operation_records.is_empty());
    assert_eq!(f.execute("transaction.inspect", id).unwrap()["verification"]["outcome"], "passed");
    std::fs::remove_dir(target).unwrap();
    f.execute("transaction.commit", id).unwrap();
    f.session.shutdown();
}

#[test]
fn missing_original_and_sampling_budget_are_inconclusive() {
    let mut f = Fixture::new(72);
    let edits = (1..71).rev().map(|frame| TimelineEdit::Split { clips: vec![f.video, f.audio], time: tick(frame).0.to_string() }).collect();
    let id = f.stage(f.request("large", edits), vec![Criterion::PreserveSplit { audio: false }]);
    let report = f.verify(id);
    assert_eq!(report.outcome, Outcome::Inconclusive);
    assert!(report.gaps.iter().any(|gap| gap.contains("budget")), "{report:?}");
    let id = f.stage(f.split("missing"), vec![Criterion::PreserveSplit { audio: false }]);
    std::fs::remove_file(f.dir.path().join("original.mov")).unwrap();
    assert_eq!(f.verify(id).outcome, Outcome::Inconclusive);
    assert_eq!(f.execute("transaction.commit", id).unwrap_err().code, ErrorCode::VerificationFailed);
    f.session.shutdown();
}
