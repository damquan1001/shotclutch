use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Duration;

use serde_json::json;
use shotclutch_media::DemoScene;
use shotclutch_project::observations::{ConfidenceMeaning, SourceStream};
use shotclutch_time::FrameRate;

use super::*;
use crate::autosave::AutosaveConfig;
use crate::media_test_util::{make_movie, session_with};
use crate::service::{Cancellation, EditorClient, EditorScope};

fn authorize(s: &mut Session, root: &Path, allow_edits: bool) -> EditorClient {
    s.authorize_editor("native-reviewer", scope(root, allow_edits)).unwrap()
}

fn scope(root: &Path, allow_edits: bool) -> EditorScope {
    EditorScope { sequences: None, allow_edits, media_roots: vec![root.to_path_buf()], lifetime: Duration::from_secs(120), max_source_bytes: 64 * 1024 * 1024 }
}

fn enable_recovery(s: &mut Session, root: &Path) {
    s.start_autosave(AutosaveConfig::new(root)).unwrap();
    s.persistence.as_ref().unwrap().flush_checked().unwrap();
}

fn frame(index: i64) -> i64 {
    FrameRate::FPS_24.tick_of(index).0
}

fn span(stream: SourceStream, start: i64, end: i64) -> SourceLocation {
    SourceLocation::SourceRange { stream, start: frame(start), end: frame(end) }
}

fn model() -> ModelVersion {
    ModelVersion { producer_version: "1".into(), model: "fixture-model".into(), model_version: "1".into(), configuration_sha256: "a".repeat(64) }
}

fn model_producer() -> ObservationProducer {
    ObservationProducer::Model {
        producer: "local-visual".into(),
        version: model(),
        confidence: Confidence { score: 0.75, meaning: ConfidenceMeaning::ModelScore },
    }
}

struct Fixture {
    root: tempfile::TempDir,
    session: Session,
    asset: ItemId,
    client: EditorClient,
    baseline: SourceFingerprint,
}

impl Fixture {
    fn new(saved: bool, recovery: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let media = root.path().join("original.mov");
        make_movie(&media, DemoScene::Aurora, 32, 18, 8);
        // A legal unreferenced MOV atom leaves room for a change the quick relink hash misses.
        let padding = 3 * crate::relink::FINGERPRINT_SPAN;
        let mut file = std::fs::OpenOptions::new().append(true).open(&media).unwrap();
        file.write_all(&(padding as u32).to_be_bytes()).unwrap();
        file.write_all(b"free").unwrap();
        file.write_all(&vec![0; padding - 8]).unwrap();
        drop(file);
        let (mut session, assets, _) = session_with(&[&media]);
        let asset = assets[0];
        if saved {
            session.execute("file.saveAs", json!({"path": root.path().join("editing.scproj")})).unwrap();
        }
        if recovery {
            enable_recovery(&mut session, &root.path().join("recovery"));
        }
        let client = authorize(&mut session, root.path(), true);
        let baseline = session.inspect_media_observations(&client, asset, &ActiveModels::new(), &Cancellation::default()).unwrap().fingerprint.unwrap();
        Self { root, session, asset, client, baseline }
    }

    fn request(&self) -> ObservationRequest {
        ObservationRequest::verified_asset(self.asset, self.session.revision, self.baseline.clone())
    }

    fn record(&mut self, request: ObservationRequest) -> Result<ObservationId, ServiceError> {
        self.session.record_media_observation(&self.client, request, &Cancellation::default())
    }

    fn read(&self, active_models: &ActiveModels) -> ObservationInspection {
        self.session.inspect_media_observations(&self.client, self.asset, active_models, &Cancellation::default()).unwrap()
    }

    fn relate(&mut self, kind: ObservationKind, location: SourceLocation, related: Vec<ObservationId>, producer: ObservationProducer) -> ObservationId {
        let request = ObservationRequest { kind, location, related, producer, summary: Some("Local evidence".into()), ..self.request() };
        self.record(request).unwrap()
    }

    fn relink(&mut self, path: &Path, force: bool) {
        self.session.execute("media.relink", json!({"item": self.asset.0, "path": path, "force": force, "relinkOthers": false})).unwrap();
    }
}

#[test]
fn all_levels_round_trip_with_exact_frame_evidence_and_subset_filtering() {
    let mut f = Fixture::new(false, true);
    let asset = f.record(f.request()).unwrap();
    let scene = f.relate(ObservationKind::Scene, span(SourceStream::All, 0, 7), vec![asset], model_producer());
    let first = f.relate(ObservationKind::Shot, span(SourceStream::Video, 0, 4), vec![scene], model_producer());
    let second = f.relate(ObservationKind::Shot, span(SourceStream::Video, 3, 7), vec![scene], model_producer());
    let segment = f.relate(ObservationKind::Segment, span(SourceStream::Audio(0), 2, 6), vec![first, second], ObservationProducer::Human);
    let request = ObservationRequest {
        kind: ObservationKind::Frame,
        location: SourceLocation::VideoPresentation { pts: frame(4) + 1 },
        related: vec![segment],
        producer: model_producer(),
        ..f.request()
    };
    assert_eq!(f.record(request.clone()).unwrap_err().code, ErrorCode::VerificationFailed);
    let still = f.record(ObservationRequest { location: SourceLocation::VideoPresentation { pts: frame(4) }, ..request }).unwrap();
    let path = f.root.path().join("saved.scproj");
    f.session.execute("file.saveAs", json!({"path": path})).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let records = &wire["project"]["observations"];
    assert_eq!(wire["schema_version"], shotclutch_format::SCHEMA_VERSION);
    assert_eq!(records[still.0.to_string()]["evidence"]["location"]["pts"], frame(4).to_string());
    assert_eq!(records[segment.0.to_string()]["evidence"]["location"]["end"], frame(6).to_string());
    assert!(wire.to_string().contains("native-reviewer"));
    assert!(records[asset.0.to_string()].get("summary").is_none());
    let mut reopened = Session::default();
    reopened.execute("file.open", json!({"path": path})).unwrap();
    let client = authorize(&mut reopened, f.root.path(), false);
    let models = [("local-visual".into(), model())].into();
    let read = reopened.inspect_media_observations(&client, f.asset, &models, &Cancellation::default()).unwrap();
    assert_eq!(read.fingerprint, Some(f.baseline.clone()));
    assert_eq!(read.observations.iter().map(|r| r.id).collect::<Vec<_>>(), [asset, scene, first, second, segment, still]);
    assert!(read.observations.iter().all(|r| r.freshness() == Freshness::Current));
    assert_eq!(read.observations[4].observation.related, [first, second]);
    assert_eq!(read.observations[4].observation.provenance, Provenance::Human { author: "native-reviewer".into() });
    assert_eq!(reopened.project.max_id(), still.0);
    assert!(reopened.project.next_id > still.0);
    assert!(crate::project_tools::project_subset(&reopened.project, &Default::default()).observations.is_empty());
    assert_eq!(crate::project_tools::project_subset(&reopened.project, &[f.asset].into()).observations.len(), 6);
    f.session.shutdown();
}

#[test]
fn freshness_tracks_bytes_and_producer_independently() {
    let mut f = Fixture::new(false, true);
    let asset = f.record(f.request()).unwrap();
    f.relate(ObservationKind::Segment, span(SourceStream::Audio(0), 0, 7), vec![asset], model_producer());
    f.relate(ObservationKind::Scene, span(SourceStream::All, 0, 8), vec![asset], ObservationProducer::Human);
    let no_models = f.read(&ActiveModels::new());
    assert_eq!(no_models.observations[1].producer_freshness, Freshness::Unverified);
    for changed in [
        ModelVersion { producer_version: "2".into(), ..model() },
        ModelVersion { model_version: "2".into(), ..model() },
        ModelVersion { configuration_sha256: "b".repeat(64), ..model() },
    ] {
        let result = f.read(&[("local-visual".into(), changed)].into());
        assert_eq!(result.observations[1].source_freshness, Freshness::Current);
        assert_eq!(result.observations[1].freshness(), Freshness::Stale);
        assert_eq!(result.observations[2].freshness(), Freshness::Current);
    }
    let active = [("local-visual".into(), model())].into();
    let moved = f.root.path().join("moved").join("original.mov");
    std::fs::create_dir(moved.parent().unwrap()).unwrap();
    std::fs::copy(f.root.path().join("original.mov"), &moved).unwrap();
    f.relink(&moved, false);
    assert!(f.read(&active).observations.iter().all(|r| r.freshness() == Freshness::Current));
    let quick_hash = |path: &Path| crate::relink::identity_of(&crate::FsServices, path.to_str().unwrap()).unwrap();
    let before = quick_hash(&moved);
    let mut file = std::fs::OpenOptions::new().write(true).open(&moved).unwrap();
    file.seek(SeekFrom::Start(before.size / 2)).unwrap();
    file.write_all(&[1]).unwrap();
    drop(file);
    assert_eq!(before, quick_hash(&moved));
    assert!(f.read(&active).observations.iter().all(|r| r.source_freshness == Freshness::Stale));
    assert_eq!(f.record(f.request()).unwrap_err().code, ErrorCode::MediaChanged);
    let replacement = f.root.path().join("other.mov");
    make_movie(&replacement, DemoScene::CityNight, 32, 18, 4);
    f.relink(&replacement, true);
    let changed = f.read(&active);
    assert!(changed.observations.iter().all(|r| r.source_freshness == Freshness::Stale));
    assert_eq!(changed.observations[2].observation.evidence.location, span(SourceStream::All, 0, 8));
    let incompatible = ObservationRequest { expected_fingerprint: changed.fingerprint.unwrap(), related: vec![asset], ..f.request() };
    assert_eq!(f.record(incompatible).unwrap_err().code, ErrorCode::InvalidArgument);
    std::fs::remove_file(&replacement).unwrap();
    let missing = f.read(&active);
    assert!(missing.fingerprint.is_none());
    assert!(missing.observations.iter().all(|r| r.freshness() == Freshness::Unverified));
    f.session.shutdown();
}

#[test]
fn denied_canceled_conflicting_and_invalid_requests_publish_nothing() {
    let mut f = Fixture::new(false, true);
    let before = f.session.project.clone();
    let request = f.request();
    let outside = tempfile::tempdir().unwrap();
    let confined = authorize(&mut f.session, outside.path(), true);
    let read_only = authorize(&mut f.session, f.root.path(), false);
    let signal = Cancellation::default();
    let inspect_error = f.session.inspect_media_observations(&confined, f.asset, &ActiveModels::new(), &signal).unwrap_err();
    assert_eq!(inspect_error.code, ErrorCode::PermissionRequired);
    assert_eq!(f.session.record_media_observation(&confined, request.clone(), &signal).unwrap_err().code, ErrorCode::PermissionRequired);
    assert_eq!(f.session.record_media_observation(&read_only, request.clone(), &signal).unwrap_err().code, ErrorCode::PermissionDenied);
    let mut scoped = scope(f.root.path(), true);
    scoped.sequences = Some([f.session.state.active_sequence.unwrap()].into());
    let sequence_only = f.session.authorize_editor("sequence-reviewer", scoped).unwrap();
    assert_eq!(f.session.inspect_media_observations(&sequence_only, f.asset, &ActiveModels::new(), &signal).unwrap_err().code, ErrorCode::PermissionDenied);
    let mut budget = scope(f.root.path(), true);
    budget.max_source_bytes = 1;
    let limited = f.session.authorize_editor("limited-reviewer", budget).unwrap();
    assert_eq!(f.session.record_media_observation(&limited, request.clone(), &signal).unwrap_err().code, ErrorCode::ResourceLimit);
    limited.revoke();
    assert_eq!(f.session.inspect_media_observations(&limited, f.asset, &ActiveModels::new(), &signal).unwrap_err().code, ErrorCode::PermissionDenied);
    signal.cancel();
    assert_eq!(f.session.record_media_observation(&f.client, request.clone(), &signal).unwrap_err().code, ErrorCode::Canceled);
    assert_eq!(
        f.record(ObservationRequest { expected_revision: request.expected_revision - 1, ..request.clone() }).unwrap_err().code,
        ErrorCode::RevisionConflict
    );
    for location in [span(SourceStream::Audio(99), 0, 3), span(SourceStream::All, 3, 3), span(SourceStream::Video, 1, 9)] {
        let bad = ObservationRequest { kind: ObservationKind::Segment, location, producer: model_producer(), ..request.clone() };
        assert_eq!(f.record(bad).unwrap_err().code, ErrorCode::InvalidArgument);
    }
    let bad_confidence = ObservationProducer::Model {
        producer: "local-visual".into(),
        version: model(),
        confidence: Confidence { score: f32::NAN, meaning: ConfidenceMeaning::ModelScore },
    };
    assert_eq!(f.record(ObservationRequest { producer: bad_confidence, ..request }).unwrap_err().code, ErrorCode::InvalidArgument);
    assert!(Arc::ptr_eq(&f.session.project, &before));
    f.session.shutdown();
}

#[test]
fn recovery_restores_observations_and_undo_does_not_reuse_an_id() {
    let mut f = Fixture::new(true, true);
    let first = f.record(f.request()).unwrap();
    let revision = f.session.revision;
    let document = f.session.document_id;
    f.session.persistence.take().unwrap().simulate_crash();
    let mut reopened = Session::default();
    enable_recovery(&mut reopened, &f.root.path().join("recovery"));
    reopened.execute("file.recover", json!({})).unwrap();
    assert_eq!((reopened.document_id, reopened.revision), (document, revision));
    let client = authorize(&mut reopened, f.root.path(), true);
    let capture = |s: &Session| s.inspect_media_observations(&client, f.asset, &ActiveModels::new(), &Cancellation::default()).unwrap();
    assert_eq!(capture(&reopened).observations[0].id, first);
    assert_eq!(reopened.undo().as_deref(), Some("Record media observation"));
    assert!(capture(&reopened).observations.is_empty());
    reopened.redo().unwrap();
    assert_eq!(capture(&reopened).observations[0].id, first);
    reopened.undo().unwrap();
    let next = ObservationRequest::verified_asset(f.asset, reopened.revision, f.baseline.clone());
    let second = reopened.record_media_observation(&client, next, &Cancellation::default()).unwrap();
    assert!(second > first);
    reopened.shutdown();
}

#[test]
fn durable_storage_failure_leaves_project_and_revision_unchanged() {
    let mut f = Fixture::new(true, false);
    let saved = f.root.path().join("editing.scproj");
    let original = std::fs::read(&saved).unwrap();
    let request = f.request();
    assert_eq!(f.record(request.clone()).unwrap_err().code, ErrorCode::CapabilityUnavailable);
    enable_recovery(&mut f.session, &f.root.path().join("recovery"));
    let target = f.session.persistence.as_ref().unwrap().session_dir.join("snapshot.scproj");
    std::fs::create_dir(&target).unwrap();
    let before = f.session.project.clone();
    let previous_state = (f.session.revision, f.session.history.undo.len(), f.session.events.len());
    assert_eq!(f.record(request.clone()).unwrap_err().code, ErrorCode::InternalError);
    assert!(Arc::ptr_eq(&f.session.project, &before));
    assert_eq!((f.session.revision, f.session.history.undo.len(), f.session.events.len()), previous_state);
    assert_eq!(std::fs::read(saved).unwrap(), original);
    std::fs::remove_dir(target).unwrap();
    f.record(request).unwrap();
    f.session.shutdown();
}

#[test]
fn project_round_trips_at_the_observation_limit_and_preserves_human_evidence() {
    let mut f = Fixture::new(false, true);
    let human = f.relate(ObservationKind::Segment, span(SourceStream::Audio(0), 1, 2), Vec::new(), ObservationProducer::Human);
    let human_record = f.read(&ActiveModels::new()).observations[0].observation.clone();
    let mut project = (*f.session.project).clone();
    let record = Observation {
        provenance: Provenance::Model {
            producer: "local-visual".into(),
            version: model(),
            confidence: Confidence { score: 0.75, meaning: ConfidenceMeaning::ModelScore },
        },
        summary: Some("a".repeat(shotclutch_project::observations::MAX_SUMMARY_BYTES)),
        related: vec![human],
        ..(*human_record).clone()
    };
    while project.observations.len() + 1 < MAX_OBSERVATIONS {
        let id = ObservationId(project.alloc_id());
        project.observations.insert(id, record.clone()).unwrap();
    }
    f.session.project = Arc::new(project);
    f.record(f.request()).unwrap();
    let revision = f.session.revision;
    assert_eq!(f.record(f.request()).unwrap_err().code, ErrorCode::ResourceLimit);
    assert_eq!(f.session.revision, revision);
    let snapshot = f.session.persistence.as_ref().unwrap().session_dir.join("snapshot.scproj");
    let bytes = std::fs::read(snapshot).unwrap();
    assert!(bytes.len() < 16 * 1024 * 1024);
    let loaded = shotclutch_format::decode(&bytes).unwrap();
    assert_eq!(loaded.project.observations.len(), MAX_OBSERVATIONS);
    assert_eq!(loaded.project.observations.for_asset(f.asset).next(), Some((human, &human_record)));
    eprintln!("AI-001 capacity checkpoint with one large undo state: {} bytes", bytes.len());
    f.session.shutdown();
}

#[test]
fn frame_observations_certify_variable_presentation_times_for_mov_and_matroska() {
    use shotclutch_isobmff::{Brand, Mp4Writer, TrackConfig, WriteSample, WriterOptions};
    use shotclutch_matroska::{MkvWriter, MuxOptions, TrackKind, TrackSpec};
    use std::io::Cursor;

    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("source.mov");
    make_movie(&original, DemoScene::Aurora, 32, 18, 4);
    let original = std::fs::read(original).unwrap();
    let source = shotclutch_isobmff::open(original.clone()).unwrap();
    let video = source.track_of_kind(shotclutch_isobmff::TrackKind::Video).unwrap();
    let mut mov = Mp4Writer::new(Cursor::new(Vec::new()), WriterOptions::new(Brand::Mov)).unwrap();
    let track = mov.add_track(TrackConfig::new(source.tracks[video].entries[0].clone(), 1000)).unwrap();
    let mut mkv_track = TrackSpec::new(TrackKind::Video, "V_MJPEG");
    mkv_track.video_size = Some((32, 18));
    mkv_track.default_duration_ns = Some(40_000_000);
    let mut mkv = MkvWriter::new(Cursor::new(Vec::new()), vec![mkv_track], MuxOptions::default()).unwrap();
    let mut at = 0i64;
    let mut points = Vec::new();
    for (i, duration) in [40, 80, 20, 50].into_iter().enumerate() {
        let bytes = source.read_sample(&original, video, i).unwrap();
        mov.write_sample(track, WriteSample { data: &bytes, duration, composition_offset: 0, is_sync: true }).unwrap();
        let mut jpeg = Vec::new();
        let picture = image::GrayImage::from_pixel(32, 18, image::Luma([20 + i as u8 * 30]));
        image::codecs::jpeg::JpegEncoder::new(&mut jpeg).encode_image(&picture).unwrap();
        mkv.write_frame(0, (500 + at) * 1_000_000, true, &jpeg, Some(u64::from(duration) * 1_000_000)).unwrap();
        points.push(Tick::from_units(at, 1000).0);
        at += i64::from(duration);
    }
    let mov_path = root.path().join("variable.mov");
    let mkv_path = root.path().join("variable.mkv");
    std::fs::write(&mov_path, mov.finish().unwrap().into_inner()).unwrap();
    std::fs::write(&mkv_path, mkv.finish().unwrap().into_inner()).unwrap();
    let (mut session, assets, _) = session_with(&[&mov_path, &mkv_path]);
    enable_recovery(&mut session, &root.path().join("recovery"));
    let client = authorize(&mut session, root.path(), true);
    let cancellation = Cancellation::default();
    let models = ActiveModels::new();
    for asset in assets {
        let read = session.inspect_media_observations(&client, asset, &models, &cancellation).unwrap();
        let mut draft = ObservationRequest::verified_asset(asset, read.revision, read.fingerprint.unwrap());
        draft.kind = ObservationKind::Frame;
        draft.producer = ObservationProducer::Human;
        for pts in [points[3], points[1], points[2]] {
            draft.location = SourceLocation::VideoPresentation { pts: pts - 1 };
            assert_eq!(session.record_media_observation(&client, draft.clone(), &cancellation).unwrap_err().code, ErrorCode::VerificationFailed);
            draft.location = SourceLocation::VideoPresentation { pts };
            session.record_media_observation(&client, draft.clone(), &cancellation).unwrap();
            draft.expected_revision = session.revision;
        }
        let read = session.inspect_media_observations(&client, asset, &models, &cancellation).unwrap();
        assert_eq!(
            read.observations.iter().map(|record| record.observation.evidence.location).collect::<Vec<_>>(),
            [points[3], points[1], points[2]].map(|pts| SourceLocation::VideoPresentation { pts })
        );
    }
    session.shutdown();
}

#[test]
#[ignore = "requires the installed Tiny English offline model and a short spoken WAV"]
fn qualified_local_speech_observation_survives_reopen_and_model_change() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("speech.wav");
    std::fs::copy(std::env::var_os("SHOTCLUTCH_AI001_SPEECH_FIXTURE").unwrap(), &source).unwrap();
    let mut session = Session::default();
    let imported = session.execute("file.import", json!({"paths": [&source]})).unwrap();
    let asset = ItemId(imported["items"][0].as_u64().unwrap());
    let end = session.project.item(asset).unwrap().duration();
    enable_recovery(&mut session, &root.path().join("recovery"));
    let client = authorize(&mut session, root.path(), true);
    let signal = Cancellation::default();
    let active = [(LOCAL_SPEECH_PRODUCER.into(), local_speech_profile(shotclutch_speech::fast_whisper::TINY_EN).unwrap())].into();
    let read = session.inspect_media_observations(&client, asset, &active, &signal).unwrap();
    let start = Tick::from_units(250, 1000);
    let draft = SpeechObservationRequest {
        asset,
        expected_revision: read.revision,
        expected_fingerprint: read.fingerprint.unwrap(),
        range: shotclutch_time::TimeRange::from_bounds(start, end),
        audio_stream: 0,
        model: shotclutch_speech::fast_whisper::TINY_EN.into(),
    };
    let cloud = SpeechObservationRequest { model: "assemblyai".into(), ..draft.clone() };
    assert_eq!(session.record_local_speech_observation(&client, cloud, &signal).unwrap_err().code, ErrorCode::CapabilityUnavailable);
    let id = session.record_local_speech_observation(&client, draft.clone(), &signal).unwrap().expect("speech model returned no words");
    let saved = root.path().join("observed.scproj");
    session.execute("file.saveAs", json!({"path": &saved})).unwrap();
    session.shutdown();
    let mut session = Session::default();
    session.execute("file.open", json!({"path": &saved})).unwrap();
    let client = authorize(&mut session, root.path(), false);
    let result = session.inspect_media_observations(&client, asset, &active, &signal).unwrap();
    let record = &result.observations[0];
    assert_eq!(record.id, id);
    assert_eq!(record.freshness(), Freshness::Current);
    assert_eq!(record.observation.kind, ObservationKind::Segment);
    assert!(record.observation.summary.as_deref().is_some_and(|text| text.split_whitespace().count() >= 3));
    let SourceLocation::SourceRange { stream: SourceStream::Audio(0), start: observed, end: stop } = record.observation.evidence.location else {
        panic!("expected an original audio source range");
    };
    assert!(start.0 <= observed && observed < stop && stop <= end.0);
    let phrase = record.observation.summary.as_ref().unwrap().split_whitespace().skip(1).take(2).collect::<Vec<_>>().join(" ");
    let hits = session.search_media(&client, &crate::media_search::SearchQuery::new(phrase), &active, &signal).unwrap();
    assert_eq!(hits.hits[0].best.evidence, crate::media_search::EvidenceId::Observation(id));
    assert_eq!(
        hits.hits[0].best.location,
        SourceLocation::SourceRange {
            stream: SourceStream::Audio(0),
            start: record.observation.word_times[1].start,
            end: record.observation.word_times[2].end
        }
    );
    let updated = [(LOCAL_SPEECH_PRODUCER.into(), local_speech_profile(shotclutch_speech::fast_whisper::BASE_EN).unwrap())].into();
    let result = session.inspect_media_observations(&client, asset, &updated, &signal).unwrap();
    assert_eq!(result.observations[0].source_freshness, Freshness::Current);
    assert_eq!(result.observations[0].producer_freshness, Freshness::Stale);
}
