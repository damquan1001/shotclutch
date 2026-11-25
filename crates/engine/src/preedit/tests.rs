use super::*;

fn seconds(n: i64) -> Tick {
    Tick(n * TICKS_PER_SECOND)
}

fn range(a: i64, b: i64) -> TimeRange {
    TimeRange::from_bounds(seconds(a), seconds(b))
}

#[test]
fn m2_audio_cut_snapping_respects_inward_and_outward_sample_boundaries() {
    for sr in [44_100, 48_000] {
        let sample = Tick::from_units(1, sr);
        for head in [1, 3] {
            for tail in [1, 3] {
                let requested =
                    TimeRange::from_bounds(Tick::from_units(100, sr) + Tick(sample.0 * head / 4), Tick::from_units(200, sr) + Tick(sample.0 * tail / 4));
                let inward = assembly::snap_decision(None, sr as u32, range(0, 1), requested, true).unwrap();
                let outward = assembly::snap_decision(None, sr as u32, range(0, 1), requested, false).unwrap();
                assert_eq!(inward, TimeRange::from_bounds(Tick::from_units(101, sr), Tick::from_units(200, sr)));
                assert_eq!(outward, TimeRange::from_bounds(Tick::from_units(100, sr), Tick::from_units(201, sr)));
            }
        }
        let sub_sample = TimeRange::from_bounds(Tick::from_units(100, sr) + Tick(sample.0 / 4), Tick::from_units(100, sr) + Tick(sample.0 * 3 / 4));
        assert!(assembly::snap_decision(None, sr as u32, range(0, 1), sub_sample, true).is_err());
    }
}

fn waveform(path: &std::path::Path) {
    let mut wav = Vec::new();
    let samples = 16_000u32 * 4;
    let length = samples * 2;
    wav.extend(b"RIFF");
    wav.extend((36 + length).to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16u32.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(16_000u32.to_le_bytes());
    wav.extend(32_000u32.to_le_bytes());
    wav.extend(2u16.to_le_bytes());
    wav.extend(16u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend(length.to_le_bytes());
    for i in 0..samples {
        let v = if i < 16_000 || i >= 48_000 { ((i as f64 * 0.1).sin() * 8000.0) as i16 } else { 0 };
        wav.extend(v.to_le_bytes());
    }
    std::fs::write(path, wav).unwrap();
}

pub(crate) fn fixture() -> (tempfile::TempDir, PathBuf, Session, ItemId) {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("voice.wav");
    waveform(&source);
    let mut session = Session::default();
    session.start_autosave(crate::autosave::AutosaveConfig::new(&dir.path().join("data"))).unwrap();
    session.execute("file.import", json!({"paths":[source]})).unwrap();
    let item = session.project.items.values().find(|item| item.as_media().is_some()).unwrap().id;
    (dir, source, session, item)
}

fn wait(session: &mut Session) {
    let start = std::time::Instant::now();
    while session.preedit_job.is_some() || session.preedit_apply.is_some() {
        session.poll_persistence();
        assert!(start.elapsed().as_secs() < 15, "background rough-cut job did not complete");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

pub(crate) fn analyze(session: &mut Session, item: ItemId, prior: Option<u64>) -> u64 {
    session.execute("preedit.analyze", json!({"item":item.0.to_string(),"audioStream":0,"previousMap":prior.map(|id| id.to_string())})).unwrap();
    wait(session);
    *session.project.preedit.maps.last_key_value().unwrap_or_else(|| panic!("analysis produced no map: {:?}", session.log.entries)).0
}

fn review_action(session: &mut Session, map: u64, action: Value) -> u64 {
    let params = json!({"map":map.to_string(),"expectedRevision":session.revision.to_string(),"action":action});
    session.execute("preedit.review", params).unwrap()["map"].as_str().unwrap().parse().unwrap()
}

#[test]
fn m2_gling_compatible_matches_opengling_synthetic_corpus() {
    let corpus: Value = serde_json::from_str(include_str!("../../test-data/opengling-cleanup-v2.json")).unwrap();
    for case in corpus.as_array().unwrap() {
        let tick = |value: &Value| Tick::from_seconds_f64(value.as_f64().unwrap());
        let duration = tick(&case["duration"]);
        let words = case["segments"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|s| s["words"].as_array().unwrap())
            .map(|w| shotclutch_project::Word::new(w["word"].as_str().unwrap(), tick(&w["start"]), tick(&w["end"])))
            .collect();
        let analysis = PreEditAnalysis {
            source: SourceBinding {
                item: ItemId(1),
                range: TimeRange::new(Tick::ZERO, duration),
                audio_stream: 0,
                channel: None,
                sha256: "0".repeat(64),
                bytes: 1,
            },
            settings: AnalysisSettings { model: None, language: "en".into(), min_pause: Tick::from_units(500, 1000), handle: Tick::from_units(150, 1000) },
            analyzer_version: 3,
            model_digest: None,
            status: AnalysisStatus::Complete,
            retake_recheck: None,
            chunks: vec![Arc::new(AnalysisChunk {
                coverage: TimeRange::new(Tick::ZERO, duration),
                transcript: Some(shotclutch_project::Transcript { language: "en".into(), words, ..Default::default() }),
                quiet: Vec::new(),
                edge_quiet: Vec::new(),
                peaks: Vec::new(),
            })],
        };
        let result = gling::propose(1, &analysis, CleanupSettings { mode: CleanupMode::GlingCompatible, ..Default::default() });
        let retained: std::collections::BTreeSet<_> = result.takes.iter().filter_map(|g| g.alternatives.last().copied()).collect();
        let kind = |kind| match kind {
            CandidateKind::Pause => 0,
            CandidateKind::Filler => 1,
            _ => 2,
        };
        let mut got: Vec<_> = result
            .candidates
            .iter()
            .enumerate()
            .filter(|(i, _)| !retained.contains(i))
            .map(|(_, c)| (c.range.start.0, c.range.end().0, kind(c.kind)))
            .collect();
        let mut expected: Vec<_> = case["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    tick(&p["start"]).0,
                    tick(&p["end"]).0,
                    match p["type"].as_str().unwrap() {
                        "silence" => 0,
                        "filler" => 1,
                        _ => 2,
                    },
                )
            })
            .collect();
        got.sort_unstable();
        expected.sort_unstable();
        assert_eq!(got, expected, "{}", case["name"]);
    }
}

#[test]
fn m2_map_restores_nested_ranges_and_maps_half_open_joins() {
    let map = PreEditMap {
        take_review_report: None,
        analysis: 1,
        previous: None,
        reviewed: true,
        candidates: Arc::default(),
        takes: Arc::default(),
        conflicts: Vec::new(),
        corrections: Default::default(),
        automatic: None,
        cleanup: None,
        overrides: vec![Override { range: range(1, 5), decision: Decision::Remove }, Override { range: range(2, 3), decision: Decision::Keep }],
    };
    let segments = map.segments(range(0, 6)).unwrap();
    assert_eq!(segments.iter().map(|s| s.source).collect::<Vec<_>>(), [range(0, 1), range(2, 3), range(5, 6)]);
    assert_eq!(segments.last().unwrap().output.end(), seconds(3));
    assert_eq!(segments[0].source_at(seconds(1)), None);
    assert_eq!(segments[1].source_at(seconds(1)), Some(seconds(2)));
    assert_eq!(segments[2].output_at(seconds(5)), Some(seconds(2)));
    assert!(map.segments(TimeRange::new(Tick(i64::MAX), Tick(1))).is_err());
}

#[test]
fn m2_explicit_analysis_review_apply_reopen_retry_and_changed_result() {
    let (dir, _, mut session, item) = fixture();
    assert!(session.preedit_job.is_none());
    assert!(session.project.preedit.is_empty());
    let proposed = analyze(&mut session, item, None);
    let record = &session.project.preedit.maps[&proposed];
    assert_eq!(record.candidates.len(), 1);
    assert_eq!(record.candidates[0].kind, CandidateKind::Pause);
    assert_eq!(session.project.preedit.analyses[&record.analysis].status, AnalysisStatus::Complete);
    let kept = review_action(&mut session, proposed, json!({"kind":"candidate","index":0,"decision":"Keep"}));
    let proposed = analyze(&mut session, item, Some(kept));
    assert_eq!(session.project.preedit.maps[&proposed].overrides, session.project.preedit.maps[&kept].overrides);
    let cut = review_action(&mut session, proposed, json!({"kind":"candidate","index":0,"decision":"Remove"}));
    let map = review_action(&mut session, cut, json!({"kind":"markReviewed"}));
    let (_, preview_sequence) = preview(&session, map).unwrap();
    assert!(session.project.sequence(preview_sequence).is_none());
    let params = json!({"map":map.to_string(),"expectedRevision":session.revision.to_string(),"operationId":"m2-apply"});
    let reply = session.execute("preedit.applyBackground", params.clone()).unwrap();
    assert_eq!(reply["state"], "verifying");
    wait(&mut session);
    let reply = session.preedit_apply_result.take().unwrap().unwrap();
    let sequence = ItemId(reply["sequenceId"].as_str().unwrap().parse().unwrap());
    let track = &session.project.sequence(sequence).unwrap().audio_tracks[0];
    assert_eq!(track.items.len(), 2);
    assert_eq!(track.items[0].end(), track.items[1].start);
    assert_eq!(track.transitions.len(), 4);
    assert_eq!(session.execute("preedit.apply", params.clone()).unwrap(), reply);
    let saved = dir.path().join("rough-cut.scproj");
    session.execute("file.saveAs", json!({"path":saved})).unwrap();
    let mut reopened = Session::default();
    reopened.execute("file.open", json!({"path":saved})).unwrap();
    assert_eq!(reopened.project.preedit, session.project.preedit);
    assert_eq!(reopened.execute("preedit.apply", params.clone()).unwrap(), reply);
    assert_eq!(session.undo().as_deref(), Some("Apply Smart Cut"));
    assert!(session.project.sequence(sequence).is_none());
    assert_eq!(session.execute("preedit.apply", params).unwrap(), reply);
    session.redo().unwrap();
    session
        .edit("Manual result edit", |project, _| {
            let item = project.item_mut(sequence).unwrap();
            let shotclutch_project::ItemKind::Sequence(q) = &mut item.kind else { panic!("missing generated sequence") };
            Arc::make_mut(q).master_volume_db = -3.0;
            Ok(())
        })
        .unwrap();
    let replacement = json!({"map":map.to_string(),"expectedRevision":session.revision.to_string(),"operationId":"replace","replace":reply["application"]});
    let failure = session.execute("preedit.apply", replacement).unwrap_err().to_string();
    assert!(failure.contains("REVISION_CONFLICT"), "{failure}");
}

#[test]
fn m2_changed_source_fails_before_application() {
    let (_dir, source, mut session, item) = fixture();
    let proposed = analyze(&mut session, item, None);
    let map = review_action(&mut session, proposed, json!({"kind":"markReviewed"}));
    let mut bytes = std::fs::read(&source).unwrap();
    bytes[100] ^= 1;
    std::fs::write(&source, bytes).unwrap();
    let params = json!({"map":map.to_string(),"expectedRevision":session.revision.to_string(),"operationId":"changed-source"});
    let error = session.execute("preedit.apply", params).unwrap_err().to_string();
    assert!(error.contains("MEDIA_CHANGED"), "{error}");
    assert!(session.project.preedit.applications.is_empty());
}

#[test]
fn m2_cancellation_preserves_completed_checkpoints_for_resume() {
    let (_dir, _, mut session, item) = fixture();
    let request = json!({"item":item.0.to_string(),"audioStream":0});
    session.execute("preedit.analyze", request.clone()).unwrap();
    let started = std::time::Instant::now();
    let (id, chunk) = loop {
        session.poll_persistence();
        if let Some((&id, analysis)) = session.project.preedit.analyses.last_key_value()
            && let Some(chunk) = analysis.chunks.first()
        {
            break (id, chunk.clone());
        }
        assert!(session.preedit_job.is_some() && started.elapsed().as_secs() < 15, "{:?}", session.log.entries);
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    session.execute("preedit.cancel", json!({})).unwrap();
    wait(&mut session);
    assert_eq!(session.project.preedit.analyses[&id].status, AnalysisStatus::Canceled);
    let saved = session.persistence.as_ref().unwrap().session_dir.join("snapshot.scproj");
    let loaded = shotclutch_format::decode(&std::fs::read(saved).unwrap()).unwrap();
    assert_eq!(loaded.project.preedit.analyses[&id].chunks[0], chunk);
    let mut resume = request;
    resume["resume"] = json!(id.to_string());
    session.execute("preedit.analyze", resume).unwrap();
    wait(&mut session);
    let resumed = &session.project.preedit.analyses[&id];
    assert_eq!(resumed.status, AnalysisStatus::Complete);
    assert_eq!(resumed.chunks.as_slice(), &[chunk]);
}

#[test]
fn m2_checkpoint_disk_wait_is_nonblocking_and_rebases_concurrent_review() {
    let (_dir, _, mut session, item) = fixture();
    let old = analyze(&mut session, item, None);
    let analysis_count = session.project.preedit.analyses.len();
    let undo_count = session.history.undo.len();
    let (ready, release) = session.persistence.as_ref().unwrap().pause_for_test();
    ready.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
    session.execute("preedit.analyze", json!({"item":item.0.to_string(),"audioStream":0,"previousMap":old.to_string()})).unwrap();
    let started = std::time::Instant::now();
    loop {
        let poll_start = std::time::Instant::now();
        session.poll_persistence();
        assert!(poll_start.elapsed() < std::time::Duration::from_millis(250), "UI poll waited on checkpoint storage");
        if session.preedit_job.as_ref().is_some_and(|job| job.checkpoint.is_some()) {
            break;
        }
        assert!(started.elapsed().as_secs() < 5);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!(session.project.preedit.analyses.len(), analysis_count, "unacknowledged data must not be published");
    let reviewed =
        review_action(&mut session, old, json!({"kind":"range","decision":"Keep","range":{"start":seconds(1).0.to_string(),"end":seconds(2).0.to_string()}}));
    let manual = session.project.preedit.maps[&reviewed].overrides.clone();
    session.execute("preedit.cancel", json!({})).unwrap();
    release.send(()).unwrap();
    wait(&mut session);
    assert!(session.project.preedit.maps.contains_key(&reviewed), "a checkpoint must not overwrite a concurrent review or reuse its ID");
    let (id, record) = session.project.preedit.analyses.last_key_value().unwrap();
    assert_eq!(record.status, AnalysisStatus::Canceled);
    assert_eq!(session.project.preedit.maps.last_key_value().unwrap().1.overrides, manual);
    assert_eq!(session.history.undo.len(), undo_count + 1, "analysis chunks are not individual undo items");
    let snapshot = session.persistence.as_ref().unwrap().session_dir.join("snapshot.scproj");
    let saved = shotclutch_format::decode(&std::fs::read(snapshot).unwrap()).unwrap();
    assert_eq!(saved.project.preedit.analyses[id], *record);
    assert_eq!(saved.project.preedit.maps[&reviewed].overrides, manual);
}

#[test]
fn m2_checkpoint_rebases_after_recovery_preferences_change_without_an_edit() {
    let (_dir, _, mut session, item) = fixture();
    let (ready, release) = session.persistence.as_ref().unwrap().pause_for_test();
    ready.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
    session.execute("preedit.analyze", json!({"item":item.0.to_string(),"audioStream":0})).unwrap();
    let started = std::time::Instant::now();
    while session.preedit_job.as_ref().is_none_or(|job| job.checkpoint.is_none()) {
        session.poll_persistence();
        assert!(started.elapsed().as_secs() < 5);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let revision = session.revision;
    assert!(session.preedit_job.as_ref().unwrap().checkpoint.as_ref().unwrap().commit.is_current(&session));
    let mut prefs = session.prefs.clone();
    prefs.auto_save.recovery_journal = false;
    session.set_prefs(prefs.clone()).unwrap();
    prefs.auto_save.recovery_journal = true;
    session.set_prefs(prefs).unwrap();
    assert_eq!(session.revision, revision);
    assert!(!session.preedit_job.as_ref().unwrap().checkpoint.as_ref().unwrap().commit.is_current(&session));
    release.send(()).unwrap();
    wait(&mut session);
    assert_eq!(session.project.preedit.analyses.last_key_value().unwrap().1.status, AnalysisStatus::Complete);
}

#[test]
fn m2_failed_async_checkpoint_does_not_publish_unpersisted_analysis() {
    let (_dir, _, mut session, item) = fixture();
    let before = session.project.preedit.clone();
    let mut prefs = session.prefs.clone();
    prefs.auto_save.recovery_journal = false;
    session.set_prefs(prefs).unwrap();
    session.execute("preedit.analyze", json!({"item":item.0.to_string(),"audioStream":0})).unwrap();
    wait(&mut session);
    assert_eq!(session.project.preedit, before);
}

#[test]
fn m2_background_apply_rejects_an_intervening_edit() {
    let (_dir, _, mut session, item) = fixture();
    let proposed = analyze(&mut session, item, None);
    let map = review_action(&mut session, proposed, json!({"kind":"markReviewed"}));
    let request = json!({"map":map.to_string(),"expectedRevision":session.revision.to_string(),"operationId":"stale"});
    session.execute("preedit.applyBackground", request).unwrap();
    session.execute("file.newBin", json!({"name":"Concurrent user edit"})).unwrap();
    wait(&mut session);
    let result = session.preedit_apply_result.take().unwrap().unwrap_err();
    assert!(result.contains("REVISION_CONFLICT"), "{result}");
    assert!(session.project.preedit.applications.is_empty());
}

#[test]
fn m2_result_captions_exclude_words_in_removed_ranges() {
    let (dir, _, mut session, item) = fixture();
    let proposed = analyze(&mut session, item, None);
    session
        .edit("Test transcript fixture", |project, _| {
            let analysis = project.preedit.maps[&proposed].analysis;
            let analysis = Arc::make_mut(project.preedit.analyses.get_mut(&analysis).unwrap());
            let chunk = Arc::make_mut(&mut analysis.chunks[0]);
            chunk.transcript = Some(shotclutch_project::Transcript {
                language: "en".into(),
                source: "fixture".into(),
                words: [("First", 0), ("Discard", 1), ("Last", 3)]
                    .into_iter()
                    .map(|(word, time)| shotclutch_project::Word::new(word, seconds(time), seconds(time) + Tick(TICKS_PER_SECOND / 4)))
                    .collect(),
                ..Default::default()
            });
            Ok(())
        })
        .unwrap();
    let cut = review_action(
        &mut session,
        proposed,
        json!({
            "kind":"range","decision":"Remove","range":{"start":seconds(1).0.to_string(),"end":seconds(3).0.to_string()}
        }),
    );
    let corrected = review_action(
        &mut session,
        cut,
        json!({
            "kind":"correctText","words":[{"start":seconds(3).0.to_string(),"text":"Corrected"}]
        }),
    );
    let saved = dir.path().join("correction.scproj");
    session.execute("file.saveAs", json!({"path":saved})).unwrap();
    session.execute("file.open", json!({"path":saved})).unwrap();
    let (draft, sequence) = preview(&session, corrected).unwrap();
    let seq = draft.sequence(sequence).unwrap();
    let captions = &seq.caption_tracks[0].captions;
    let text = captions.iter().map(|caption| caption.text.as_str()).collect::<Vec<_>>().join(" ");
    assert!(text.contains("First") && text.contains("Corrected") && !text.contains("Discard") && !text.contains("Last"));
    assert!(captions.iter().all(|caption| caption.end() <= seconds(2)));
}

#[test]
fn m2_linked_cut_preview_and_export_preserve_frame_and_sample_counts() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("indexed.mov");
    crate::media_test_util::make_movie(&source, shotclutch_media::DemoScene::Aurora, 32, 18, 8);
    let original = std::fs::read(&source).unwrap();
    let mut session = Session::default();
    session.start_autosave(crate::autosave::AutosaveConfig::new(&dir.path().join("data"))).unwrap();
    let imported = session.execute("file.import", json!({"paths":[source]})).unwrap();
    let item = ItemId(imported["items"][0].as_u64().unwrap());
    let proposed = analyze(&mut session, item, None);
    let rate = shotclutch_time::FrameRate::FPS_24;
    let map = review_action(
        &mut session,
        proposed,
        json!({
            "kind":"range","decision":"Remove",
            "range":{"start":rate.tick_of(2).0.to_string(),"end":rate.tick_of(4).0.to_string()}
        }),
    );
    let map = review_action(&mut session, map, json!({"kind":"markReviewed"}));
    let (draft, preview_id) = preview(&session, map).unwrap();
    let preview = draft.sequence(preview_id).unwrap();
    assert_eq!(preview.duration(), rate.tick_of(6));
    assert_eq!(preview.audio_tracks[0].items[1].source_in, rate.tick_of(4));
    assert_eq!(preview.video_tracks[0].items[1].link, preview.audio_tracks[0].items[1].link);
    let provider = session.media.full_res_provider(draft.clone(), session.services.clone());
    let sample = shotclutch_render::render_sequence(&draft, preview_id, rate.tick_of(2), Default::default(), &provider);
    let request = json!({"map":map.to_string(),"expectedRevision":session.revision.to_string(),"operationId":"av-output"});
    let result = session.execute("preedit.apply", request).unwrap();
    let sequence = result["sequenceId"].as_str().unwrap().parse::<u64>().unwrap();
    session.execute("sequence.open", json!({"item":sequence})).unwrap();
    assert_eq!(session.render_program_at(1.0, rate.tick_of(2)).unwrap().to_rgba8(), sample.to_rgba8());
    let path = dir.path().join("rough-cut.mov");
    session.execute("file.exportMedia", json!({"path":path,"format":"prores","wait":true})).unwrap();
    let output = shotclutch_isobmff::open(std::fs::read(path).unwrap()).unwrap();
    let video = output.track_of_kind(shotclutch_isobmff::TrackKind::Video).unwrap();
    let audio = output.track_of_kind(shotclutch_isobmff::TrackKind::Audio).unwrap();
    assert_eq!(output.tracks[video].samples.len(), 6);
    assert_eq!(output.tracks[audio].timescale, 48_000);
    assert_eq!(output.tracks[audio].samples.iter().map(|sample| u64::from(sample.duration)).sum::<u64>(), 12_000);
    assert_eq!(std::fs::read(source).unwrap(), original);
}

#[test]
fn m2_smart_cut_audio_preview_and_applied_sequence_have_click_free_edges() {
    let (_dir, _, mut session, item) = fixture();
    let proposed = analyze(&mut session, item, None);
    let half = Tick::from_units(1, 2);
    let map = review_action(
        &mut session,
        proposed,
        json!({
            "kind":"range", "decision":"Remove", "range":{"start":half.0.to_string(),"end":(seconds(3)+half).0.to_string()}
        }),
    );
    let map = review_action(&mut session, map, json!({"kind":"markReviewed"}));
    let (draft, sequence) = preview(&session, map).unwrap();
    let seq = draft.sequence(sequence).unwrap();
    assert_eq!(seq.duration(), seconds(1));
    let provider = session.media.full_res_provider(draft.clone(), session.services.clone());
    let audio = shotclutch_render::audio::mix_sequence(&draft, seq, 0, 16_000, &provider);
    for channel in &audio.channels {
        for edge in [0, 7999, 8000, 15999] {
            assert_eq!(channel[edge], 0.0, "audible sample discontinuity at output sample {edge}");
        }
        assert!(channel[4000].abs() > 0.01, "retained speech was attenuated away from the join");
    }
    let result =
        session.execute("preedit.apply", json!({"map":map.to_string(),"expectedRevision":session.revision.to_string(),"operationId":"smooth-join"})).unwrap();
    let applied = ItemId(result["sequenceId"].as_str().unwrap().parse().unwrap());
    let provider = session.media.full_res_provider(session.project.clone(), session.services.clone());
    let applied_audio = shotclutch_render::audio::mix_sequence(&session.project, session.project.sequence(applied).unwrap(), 0, 16_000, &provider);
    assert_eq!(audio.channels, applied_audio.channels);
}

#[cfg(feature = "whisper")]
#[test]
#[ignore = "requires the pinned model and the operator's private footage"]
fn m2_private_whisper_qualification() {
    private_speech_qualification("whisper-base", false);
}

#[test]
#[ignore = "requires pinned fast runtime, model and private footage"]
fn m2_private_fast_whisper_qualification() {
    private_speech_qualification("faster-whisper-tiny.en", true);
}

fn private_speech_qualification(model_id: &str, auto_cut: bool) {
    let footage = std::env::var_os("SHOTCLUTCH_M2_FOOTAGE").map(PathBuf::from).expect("set private footage path");
    let stream: usize = std::env::var("SHOTCLUTCH_M2_STREAM").expect("select the audited audio stream").parse().unwrap();
    let model_dir = crate::transcript::models_dir().expect("set the private data directory");
    let model = shotclutch_speech::models::find(model_id).unwrap();
    assert!(shotclutch_speech::models::installed(&model_dir, model), "pinned model is missing or invalid");
    let recovery = tempfile::tempdir().unwrap();
    let mut session = Session::default();
    session.start_autosave(crate::autosave::AutosaveConfig::new(recovery.path())).unwrap();
    session.execute("file.import", json!({"paths":[footage]})).map_err(|_| "private footage import failed").unwrap();
    let item = session.project.items.values().find(|i| i.as_media().is_some()).expect("no imported source");
    let total = item.as_media().unwrap().duration();
    let range = std::env::var("SHOTCLUTCH_M2_EXCERPT_SECONDS").ok().map(|raw| {
        let (start, end) = raw.split_once(':').expect("use start:end seconds");
        let tick = |value: &str| Tick::from_seconds_f64(value.parse().unwrap()).0.to_string();
        json!({"start":tick(start),"end":tick(end)})
    });
    let started = std::time::Instant::now();
    session
        .execute(
            "preedit.analyze",
            json!({
                "item":item.id.0.to_string(),"audioStream":stream,"model":model_id,"language":"en","range":range,
                "autoCut":auto_cut,"cleanup":{"mode":"glingCompatible","minPauseMs":500,"handleMs":150}
            }),
        )
        .unwrap();
    let mut reported = 0;
    while session.preedit_job.is_some() {
        session.poll_persistence();
        if let Some((_, analysis)) = session.project.preedit.analyses.last_key_value()
            && analysis.chunks.len() / 10 > reported
        {
            reported = analysis.chunks.len() / 10;
            println!("M2 analysis checkpoint {} · {:.1}s elapsed", analysis.chunks.len(), started.elapsed().as_secs_f64());
        }
        assert!(started.elapsed().as_secs() < 7200, "private qualification exceeded two hours");
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    let (_, analysis) = session.project.preedit.analyses.last_key_value().unwrap_or_else(|| {
        let error = session.log.entries.iter().rev().find(|entry| entry.source == "Smart Cut").map(|entry| &entry.message);
        panic!("no durable analysis: {error:?}")
    });
    assert_eq!(
        analysis.status,
        AnalysisStatus::Complete,
        "private analysis did not complete: {:?}",
        session.log.entries.iter().rev().find(|entry| entry.source == "Smart Cut").map(|entry| &entry.message)
    );
    let words = analysis.chunks.iter().filter_map(|c| c.transcript.as_ref()).map(|t| t.words.len()).sum::<usize>();
    assert!(words > 0, "selected stream produced no transcript");
    let (map_id, map) = session.project.preedit.maps.last_key_value().expect("no proposal");
    let (draft, sequence) = preview(&session, *map_id).expect("rough-cut preview failed");
    let result = draft.sequence(sequence).unwrap();
    assert_eq!(result.duration(), map.segments(analysis.source.range).unwrap().last().unwrap().output.end());
    if auto_cut {
        assert!(result.duration() < analysis.source.range.duration);
    }
    assert_eq!(result.audio_tracks[0].items[0].audio_stream, stream);
    println!(
        "M2 local qualification · {:.3}s source · {:.3}s coverage · stream {} · {} chunks · {} words · {} candidates · {} take groups · {:.1}s elapsed · preview {}",
        total.0 as f64 / TICKS_PER_SECOND as f64,
        analysis.source.range.duration.0 as f64 / TICKS_PER_SECOND as f64,
        stream + 1,
        analysis.chunks.len(),
        words,
        map.candidates.len(),
        map.takes.len(),
        started.elapsed().as_secs_f64(),
        sequence.0
    );
    println!("M2 profile {} · result {:.3}s", model_id, result.duration().0 as f64 / TICKS_PER_SECOND as f64);
    if let Some(path) = std::env::var_os("SHOTCLUTCH_M2_REVIEW_PROJECT") {
        session.execute("file.saveAs", json!({"path":PathBuf::from(path)})).map_err(|_| "could not save private review project").unwrap();
    }
}

#[test]
fn m2_auto_cut_opens_without_review_and_restoration_survives_reanalysis() {
    let (_dir, _, mut session, item) = fixture();
    let request = |prior: Option<u64>| {
        json!({
            "item": item.0.to_string(), "audioStream": 0, "autoCut": true,
            "previousMap": prior.map(|id| id.to_string())
        })
    };
    session.execute("preedit.analyze", request(None)).unwrap();
    wait(&mut session);
    let (&map_id, map) = session.project.preedit.maps.last_key_value().unwrap();
    assert!(!map.reviewed);
    assert_eq!(map.automatic.as_ref().unwrap().len(), 1);
    assert!(map.overrides.is_empty());
    let cut = map.automatic.as_ref().unwrap()[0].range;
    let (preview, sequence) = super::preview(&session, map_id).unwrap();
    assert_eq!(preview.sequence(sequence).unwrap().duration(), seconds(4) - cut.duration);
    let reply = session
        .execute(
            "preedit.apply",
            json!({
                "map": map_id.to_string(), "expectedRevision": session.revision.to_string(), "operationId": "auto-cut"
            }),
        )
        .unwrap();
    assert_eq!(reply["state"], "committed");
    let restored = review_action(
        &mut session,
        map_id,
        json!({
            "kind": "range", "decision": "Keep", "range": {"start": cut.start.0.to_string(), "end": cut.end().0.to_string()}
        }),
    );
    session.execute("preedit.analyze", request(Some(restored))).unwrap();
    wait(&mut session);
    let (_, recut) = session.project.preedit.maps.last_key_value().unwrap();
    assert_eq!(recut.automatic.as_ref().unwrap().len(), 1);
    assert_eq!(recut.segments(range(0, 4)).unwrap()[0].source, range(0, 4));
}

#[test]
fn m2_cleanup_switches_reuse_completed_analysis() {
    let (_dir, _, mut session, item) = fixture();
    session
        .execute(
            "preedit.analyze",
            json!({
                "item": item.0.to_string(), "audioStream": 0, "autoCut": true,
                "cleanup": {"cutBadTakes": false, "cutFillers": false, "cutPauses": false}
            }),
        )
        .unwrap();
    wait(&mut session);
    let (&initial, map) = session.project.preedit.maps.last_key_value().unwrap();
    let analysis = map.analysis;
    assert!(map.automatic.as_ref().unwrap().is_empty());
    let recut = review_action(
        &mut session,
        initial,
        json!({
            "kind": "cleanUp", "options": {"cutBadTakes": false, "cutFillers": false, "cutPauses": true, "minPauseMs": 1000}
        }),
    );
    let map = &session.project.preedit.maps[&recut];
    assert_eq!(map.analysis, analysis);
    assert_eq!(map.automatic.as_ref().unwrap().len(), 1);
    let no_cuts = review_action(
        &mut session,
        recut,
        json!({
            "kind": "cleanUp", "options": {"cutPauses": true, "minPauseMs": 3000}
        }),
    );
    let map = &session.project.preedit.maps[&no_cuts];
    assert_eq!(map.analysis, analysis);
    assert!(map.automatic.as_ref().unwrap().is_empty());
    assert_eq!(session.project.preedit.analyses.len(), 1);
}

#[test]
fn m2_cleanup_finds_repeated_opening_and_protects_timed_speech_in_quiet_audio() {
    let (_dir, _, mut session, item) = fixture();
    let map_id = analyze(&mut session, item, None);
    let map = &session.project.preedit.maps[&map_id];
    let mut saved = (*session.project.preedit.analyses[&map.analysis]).clone();
    let time = |ms| Tick::from_units(ms, 1000);
    let mut words: Vec<_> = ["here", "we", "go", "here", "we", "go", "uhm"]
        .into_iter()
        .enumerate()
        .map(|(i, text)| shotclutch_project::Word::new(text, time(i as i64 * 120), time(i as i64 * 120 + 80)))
        .collect();
    words.push(shotclutch_project::Word::new("hello", time(2100), time(2200)));
    Arc::make_mut(&mut saved.chunks[0]).transcript = Some(shotclutch_project::Transcript { words, ..Default::default() });
    let proposed = analysis::propose(map.analysis, &saved, None);
    assert!(proposed.candidates.iter().any(|c| c.kind == CandidateKind::Restart && c.range.end() == time(320)));
    assert!(proposed.candidates.iter().any(|c| c.kind == CandidateKind::Filler && c.range.start == time(720)));
    assert!(proposed.candidates.iter().any(|c| c.kind == CandidateKind::Pause && c.range.end() < time(2100)));
    assert!(!proposed.candidates.iter().any(|c| c.kind == CandidateKind::Pause && c.range.contains(time(2150))));
}

#[test]
fn m2_adaptive_silence_keeps_audible_islands_and_constant_audio() {
    let mut db = vec![-32.0; 600]; // room noise above the old -42 dB ceiling
    db[..100].fill(-12.0);
    db[500..].fill(-12.0);
    db[290..310].fill(-18.0); // a breath / untranscribed sound
    let quiet = cleanup::quiet_intervals(range(0, 6), &db);
    assert!(quiet.iter().any(|r| r.contains(seconds(2))));
    assert!(quiet.iter().any(|r| r.contains(seconds(4))));
    assert!(!quiet.iter().any(|r| r.contains(seconds(3))));
    for level in [-32.0, -55.0, -15.0] {
        assert!(cleanup::quiet_intervals(range(0, 6), &vec![level; 600]).is_empty());
    }
}

#[test]
fn m2_natural_pause_padding_is_per_speech_gap_and_transcript_mode_is_exact() {
    let (_dir, _, mut session, item) = fixture();
    let id = analyze(&mut session, item, None);
    let map = &session.project.preedit.maps[&id];
    let mut saved = (*session.project.preedit.analyses[&map.analysis]).clone();
    let t = |v| Tick::from_units(v, 1000);
    let chunk = Arc::make_mut(&mut saved.chunks[0]);
    chunk.transcript = Some(shotclutch_project::Transcript {
        words: vec![shotclutch_project::Word::new("Hello.", t(100), t(1000)), shotclutch_project::Word::new("Again", t(3000), t(3800))],
        ..Default::default()
    });
    chunk.quiet = vec![TimeRange::from_bounds(t(1000), t(1800)), TimeRange::from_bounds(t(1900), t(3000))];
    let options = CleanupSettings { min_pause_ms: Some(500), handle_ms: Some(100), ..Default::default() };
    let natural = analysis::propose(map.analysis, &saved, Some(options));
    let cuts: Vec<_> = natural.candidates.iter().filter(|c| c.kind == CandidateKind::Pause).collect();
    assert_eq!(cuts.len(), 2);
    assert_eq!(cuts[0].range, TimeRange::from_bounds(t(1270), t(1800)));
    assert_eq!(cuts[1].range, TimeRange::from_bounds(t(1900), t(2820)));
    let transcript = analysis::propose(map.analysis, &saved, Some(CleanupSettings { mode: CleanupMode::Transcript, ..options }));
    assert!(transcript.candidates.iter().any(|c| c.range == TimeRange::from_bounds(t(1000), t(3000))));
    assert!(transcript.candidates.iter().any(|c| c.range == TimeRange::from_bounds(t(0), t(100))));
    assert!(transcript.candidates.iter().any(|c| c.range == TimeRange::from_bounds(t(3800), t(4000))));
}

#[test]
fn m2_natural_word_cuts_guard_neighbors_and_require_quiet_boundaries() {
    let t = |v| Tick::from_units(v, 1000);
    let owned = [
        shotclutch_project::Word::new("hello", t(300), t(600)),
        shotclutch_project::Word::new("um", t(1100), t(1300)),
        shotclutch_project::Word::new("world", t(1800), t(2200)),
    ];
    let words: Vec<_> = owned.iter().collect();
    let quiet = [TimeRange::from_bounds(t(600), t(1100)), TimeRange::from_bounds(t(1300), t(1800))];
    assert_eq!(cleanup::natural_word_range(owned[1].range(), &words, &quiet, range(0, 3)), Some(TimeRange::from_bounds(t(720), t(1720))));
    assert_eq!(cleanup::natural_word_range(owned[1].range(), &words, &[], range(0, 3)), None);
    assert_eq!(cleanup::transcript_word_range(owned[1].range(), &words, t(500)), TimeRange::from_bounds(t(600), t(1800)));
    assert_eq!(cleanup::transcript_word_range(owned[1].range(), &words, t(400)), owned[1].range());
}

#[test]
fn m2_transcript_rules_detect_varied_take_openings_and_extended_fillers() {
    let (_dir, _, mut session, item) = fixture();
    let id = analyze(&mut session, item, None);
    let map = &session.project.preedit.maps[&id];
    let mut saved = (*session.project.preedit.analyses[&map.analysis]).clone();
    let time = |v| Tick::from_units(v, 1000);
    let mut words: Vec<_> = ["this", "camera", "looks", "very", "good", "this", "camera", "looks", "really", "great", "ummm"]
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let start = i as i64 * 140 + if i >= 5 { 650 } else { 0 };
            shotclutch_project::Word::new(*text, time(start), time(start + 100))
        })
        .collect();
    words.last_mut().unwrap().end = time(2200);
    Arc::make_mut(&mut saved.chunks[0]).transcript = Some(shotclutch_project::Transcript { words, ..Default::default() });
    let options = CleanupSettings { mode: CleanupMode::Transcript, min_pause_ms: Some(500), ..Default::default() };
    let map = analysis::propose(map.analysis, &saved, Some(options));
    assert_eq!(map.takes.len(), 1);
    assert!(map.candidates.iter().any(|c| c.kind == CandidateKind::Filler));
    let source = session.media.full_res_source(&session.project, item, &*session.services).unwrap();
    let cuts = automatic_cuts(&map, &saved, &*source);
    assert!(cuts.iter().any(|c| c.range.start == time(0) && c.range.end() == time(660)));
    assert!(!cuts.iter().any(|c| c.range.contains(time(1400))));
}

#[test]
fn m2_audio_refresh_reuses_words_preserves_restoration_and_checks_binding() {
    let (_dir, _, mut session, item) = fixture();
    let id = analyze(&mut session, item, None);
    let kept = review_action(&mut session, id, json!({"kind":"candidate","index":0,"decision":"Keep"}));
    let analysis_id = session.project.preedit.maps[&kept].analysis;
    session
        .edit("Attach timed test transcript", |project, _| {
            let analysis = Arc::make_mut(project.preedit.analyses.get_mut(&analysis_id).unwrap());
            analysis.analyzer_version = 1;
            Arc::make_mut(&mut analysis.chunks[0]).transcript =
                Some(shotclutch_project::Transcript { words: vec![shotclutch_project::Word::new("test", Tick::ZERO, seconds(1))], ..Default::default() });
            Ok(())
        })
        .unwrap();
    let old = session.project.preedit.analyses[&analysis_id].clone();
    let args = json!({"item":item.0.to_string(),"audioStream":0,"reuseTranscript":analysis_id.to_string(),"previousMap":kept.to_string(),"autoCut":true});
    session.execute("preedit.analyze", args.clone()).unwrap();
    wait(&mut session);
    let (_, map) = session.project.preedit.maps.last_key_value().unwrap();
    assert_eq!(map.overrides, session.project.preedit.maps[&kept].overrides);
    let refreshed = &session.project.preedit.analyses[&map.analysis];
    assert_eq!(refreshed.analyzer_version, analysis::ANALYZER_VERSION);
    assert_eq!(refreshed.chunks[0].transcript, old.chunks[0].transcript);
    let mut wrong = args;
    wrong["language"] = json!("fr");
    assert!(session.execute("preedit.analyze", wrong).unwrap_err().to_string().contains("same source"));
}

#[test]
fn m2_gling_recut_preserves_manual_restoration_and_saved_mode() {
    let (dir, _, mut session, item) = fixture();
    let id = analyze(&mut session, item, None);
    let analysis_id = session.project.preedit.maps[&id].analysis;
    session
        .edit("Timed Gling fixture", |project, _| {
            let analysis = Arc::make_mut(project.preedit.analyses.get_mut(&analysis_id).unwrap());
            Arc::make_mut(&mut analysis.chunks[0]).transcript = Some(shotclutch_project::Transcript {
                words: [("this", 200, 400), ("camera", 400, 800), ("this", 2000, 2200), ("camera", 2200, 2600), ("works", 2600, 3200)]
                    .into_iter()
                    .map(|(text, start, end)| shotclutch_project::Word::new(text, Tick::from_units(start, 1000), Tick::from_units(end, 1000)))
                    .collect(),
                ..Default::default()
            });
            Ok(())
        })
        .unwrap();
    let options = json!({"mode":"glingCompatible","minPauseMs":500});
    let cut = review_action(&mut session, id, json!({"kind":"cleanUp","options":options}));
    let removed = session.project.preedit.maps[&cut].segments(range(0, 4)).unwrap();
    assert_eq!(removed[0].source.start, seconds(2));
    let restored = review_action(&mut session, cut, json!({"kind":"range","decision":"Keep","range":{"start":"0","end":seconds(1).0.to_string()}}));
    let recut = review_action(&mut session, restored, json!({"kind":"cleanUp","options":options}));
    assert_eq!(session.project.preedit.maps[&recut].segments(range(0, 4)).unwrap()[0].source, range(0, 1));
    let path = dir.path().join("gling.scproj");
    session.execute("file.saveAs", json!({"path":path})).unwrap();
    let mut reopened = Session::default();
    reopened.execute("file.open", json!({"path":path})).unwrap();
    assert_eq!(reopened.project.preedit.maps[&recut], session.project.preedit.maps[&recut]);
    assert_eq!(reopened.project.preedit.maps[&recut].cleanup.as_ref().unwrap().mode, CleanupMode::GlingCompatible);
    assert!(!session.project.preedit.maps[&cut].takes.is_empty());
}

#[test]
fn m2_fast_profile_checkpoint_bound_matches_project_validation() {
    let (_dir, _, mut session, item) = fixture();
    let id = analyze(&mut session, item, None);
    let analysis_id = session.project.preedit.maps[&id].analysis;
    let mut preedit = session.project.preedit.clone();
    preedit.maps.clear();
    let analysis = Arc::make_mut(preedit.analyses.get_mut(&analysis_id).unwrap());
    analysis.source.range = range(0, 300);
    let chunk = Arc::make_mut(&mut analysis.chunks[0]);
    chunk.coverage = range(0, 300);
    chunk.edge_quiet.clear();
    chunk.peaks = vec![0; 6000];
    for model in ["faster-whisper-tiny.en", "faster-whisper-base.en", "assemblyai"] {
        Arc::make_mut(preedit.analyses.get_mut(&analysis_id).unwrap()).settings.model = Some(model.into());
        assert!(preedit.check().is_ok(), "{model}");
    }
    let analysis = Arc::make_mut(preedit.analyses.get_mut(&analysis_id).unwrap());
    Arc::make_mut(&mut analysis.chunks[0]).peaks.resize(6002, 0);
    assert!(preedit.check().is_err());
    let analysis = Arc::make_mut(preedit.analyses.get_mut(&analysis_id).unwrap());
    Arc::make_mut(&mut analysis.chunks[0]).peaks.resize(6000, 0);
    analysis.settings.model = Some("whisper-base".into());
    assert_eq!(analysis.settings.chunk_seconds(), 20);
    assert!(preedit.check().is_err());
}

#[test]
fn m2_gling_natural_pacing_preserves_short_pauses_and_sentence_context() {
    let (dir, _, mut session, item) = fixture();
    let id = analyze(&mut session, item, None);
    let analysis_id = session.project.preedit.maps[&id].analysis;
    let t = |value| Tick::from_units(value, 1000);
    session
        .edit("Natural pause fixture", |project, _| {
            let analysis = Arc::make_mut(project.preedit.analyses.get_mut(&analysis_id).unwrap());
            Arc::make_mut(&mut analysis.chunks[0]).transcript = Some(shotclutch_project::Transcript {
                words: [("one", 200, 400), ("two", 920, 1100), ("three.", 1740, 1900), ("four", 3000, 3200)]
                    .into_iter()
                    .map(|(text, start, end)| shotclutch_project::Word::new(text, t(start), t(end)))
                    .collect(),
                ..Default::default()
            });
            Ok(())
        })
        .unwrap();
    let options = json!({"mode":"glingCompatible","minPauseMs":500,"handleMs":150,"preservePausesMs":800});
    let paced = review_action(&mut session, id, json!({"kind":"cleanUp","options":options}));
    let map = &session.project.preedit.maps[&paced];
    let cuts = map.automatic.as_ref().unwrap();
    for time in [150, 400, 600, 920, 1100, 1500, 1740, 1950, 2000, 2950, 3200, 3250] {
        assert!(!cuts.iter().any(|cut| cut.range.contains(t(time))), "speech/pause context removed at {time} ms");
    }
    assert!(cuts.iter().any(|cut| cut.range.start == t(2050) && cut.range.end() == t(2900)));
    assert!(cuts.iter().any(|cut| cut.range.start == t(3275) && cut.range.end() == t(4000)));
    assert!(cuts.iter().all(|cut| cut.range.duration >= t(120)));
    let restored =
        review_action(&mut session, paced, json!({"kind":"range","decision":"Keep","range":{"start":t(1900).0.to_string(),"end":t(3000).0.to_string()}}));
    let recut = review_action(&mut session, restored, json!({"kind":"cleanUp","options":options}));
    assert!(
        session.project.preedit.maps[&recut]
            .segments(range(0, 4))
            .unwrap()
            .iter()
            .any(|segment| segment.source.start <= t(1900) && segment.source.end() >= t(3000))
    );
    let path = dir.path().join("paced.scproj");
    session.execute("file.saveAs", json!({"path":path})).unwrap();
    let mut reopened = Session::default();
    reopened.execute("file.open", json!({"path":path})).unwrap();
    assert_eq!(reopened.project.preedit.maps[&recut].cleanup.unwrap().preserve_pauses_ms, Some(800));
}

#[test]
fn m2_gling_pacing_keeps_take_detection_and_only_restores_quiet_word_handles() {
    let (_dir, _, mut session, item) = fixture();
    let id = analyze(&mut session, item, None);
    let analysis_id = session.project.preedit.maps[&id].analysis;
    let mut analysis = (*session.project.preedit.analyses[&analysis_id]).clone();
    let t = |value| Tick::from_units(value, 1000);
    let transcript = |values: &[(&str, i64, i64)]| shotclutch_project::Transcript {
        words: values.iter().map(|(text, start, end)| shotclutch_project::Word::new(*text, t(*start), t(*end))).collect(),
        ..Default::default()
    };
    Arc::make_mut(&mut analysis.chunks[0]).transcript =
        Some(transcript(&[("this", 200, 400), ("camera", 400, 800), ("this", 2000, 2200), ("camera", 2200, 2600), ("works", 2600, 3200)]));
    let raw_options = CleanupSettings { mode: CleanupMode::GlingCompatible, handle_ms: Some(150), ..Default::default() };
    let paced_options = CleanupSettings { preserve_pauses_ms: Some(800), ..raw_options };
    let raw = analysis::propose(analysis_id, &analysis, Some(raw_options));
    let paced = analysis::propose(analysis_id, &analysis, Some(paced_options));
    assert!(!raw.takes.is_empty());
    assert_eq!(raw.takes, paced.takes);
    assert_eq!(
        raw.candidates.iter().filter(|c| c.kind != CandidateKind::Pause).collect::<Vec<_>>(),
        paced.candidates.iter().filter(|c| c.kind != CandidateKind::Pause).collect::<Vec<_>>()
    );
    let reader = session.media.full_res_source(&session.project, item, &*session.services).unwrap();
    let mut result = paced.clone();
    result.automatic = Some(automatic_cuts(&paced, &analysis, &*reader));
    let segments = result.segments(analysis.source.range).unwrap();
    assert_eq!(segments.len(), 1, "discarded takes must not leave isolated quiet clips");
    assert_eq!(segments[0].source, TimeRange::from_bounds(t(1925), t(3275)));
    Arc::make_mut(&mut analysis.chunks[0]).transcript = Some(transcript(&[
        ("um", 200, 400),
        ("um", 500, 700),
        ("this", 800, 1000),
        ("camera", 1100, 1300),
        ("this", 1400, 1600),
        ("camera", 1700, 1900),
        ("works", 2000, 2200),
    ]));
    let adjacent = analysis::propose(analysis_id, &analysis, Some(paced_options));
    assert!(adjacent.candidates.iter().any(|c| c.kind == CandidateKind::Filler));
    let mut joined = adjacent.clone();
    joined.automatic = Some(automatic_cuts(&adjacent, &analysis, &*reader));
    let segments = joined.segments(analysis.source.range).unwrap();
    assert_eq!(segments.len(), 1, "adjacent filler/take cuts must not retain padding between discarded words");
    assert_eq!(segments[0].source, TimeRange::from_bounds(t(1325), t(2275)));
    Arc::make_mut(&mut analysis.chunks[0]).transcript = Some(transcript(&[("hello", 0, 200), ("um", 400, 600), ("world", 800, 1000)]));
    let paced = analysis::propose(analysis_id, &analysis, Some(paced_options));
    let filler = paced.candidates.iter().find(|c| c.kind == CandidateKind::Filler).unwrap();
    assert_eq!(filler.range, TimeRange::from_bounds(t(275), t(725)));
    assert!(filler.range.contains(t(500)));
    let options = CleanupSettings { handle_ms: Some(800), preserve_pauses_ms: Some(200), ..paced_options };
    Arc::make_mut(&mut analysis.chunks[0]).transcript = Some(transcript(&[("before", 0, 200), ("after", 1050, 1300)]));
    let paced = analysis::propose(analysis_id, &analysis, Some(options));
    assert!(!paced.candidates.iter().any(|c| c.kind == CandidateKind::Pause && c.range.start < t(1050)));
    assert!(!serde_json::from_value::<CleanupSettings>(json!({"preservePausesMs":0})).unwrap().valid());
    let chunk = Arc::make_mut(&mut analysis.chunks[0]);
    chunk.transcript = None;
    chunk.quiet = vec![TimeRange::from_bounds(t(400), t(920)), TimeRange::from_bounds(t(1100), t(1740)), TimeRange::from_bounds(t(1900), t(3000))];
    let paced = analysis::propose(analysis_id, &analysis, Some(paced_options));
    assert!(!paced.candidates.iter().any(|c| c.kind == CandidateKind::Pause && c.range.start < t(1900)));
    assert!(paced.candidates.iter().any(|c| c.range == TimeRange::from_bounds(t(1975), t(2925))));
}

#[test]
fn m2_gling_pacing_merges_adjoining_removals_before_video_frame_snapping() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("joined.mov");
    crate::media_test_util::make_movie(&file, shotclutch_media::DemoScene::Aurora, 32, 18, 8);
    let mut session = Session::default();
    session.start_autosave(crate::autosave::AutosaveConfig::new(&dir.path().join("data"))).unwrap();
    let imported = session.execute("file.import", json!({"paths":[file]})).unwrap();
    let item = ItemId(imported["items"][0].as_u64().unwrap());
    let id = analyze(&mut session, item, None);
    let mut map = (*session.project.preedit.maps[&id]).clone();
    let analysis = session.project.preedit.analyses[&map.analysis].clone();
    let t = |value| Tick::from_units(value, 1000);
    map.cleanup = Some(CleanupSettings { mode: CleanupMode::GlingCompatible, preserve_pauses_ms: Some(800), ..Default::default() });
    map.candidates = Arc::new(vec![
        Candidate { range: TimeRange::from_bounds(t(10), t(140)), kind: CandidateKind::Take, reason: "discarded words".into(), confidence: None },
        Candidate { range: TimeRange::from_bounds(t(140), t(210)), kind: CandidateKind::Pause, reason: "adjoining discarded gap".into(), confidence: None },
    ]);
    map.takes = Arc::default();
    let source = session.media.full_res_source(&session.project, item, &*session.services).unwrap();
    let cuts = automatic_cuts(&map, &analysis, &*source);
    let rate = shotclutch_time::FrameRate::FPS_24;
    assert_eq!(cuts.as_slice(), &[Override { range: TimeRange::from_bounds(rate.tick_of(1), rate.tick_of(5)), decision: Decision::Remove }]);
}

#[test]
fn m2_ordered_cleanup_is_explicit_durable_and_preserves_manual_actions() {
    let legacy: CleanupSettings = serde_json::from_value(json!({"mode":"glingCompatible"})).unwrap();
    assert_eq!(legacy.take_matching, TakeMatching::Legacy);
    assert!(serde_json::to_value(legacy).unwrap().get("takeMatching").is_none());
    assert!(serde_json::from_value::<CleanupSettings>(json!({"takeMatching":"unqualifiedFutureVersion"})).is_err());
    let (_dir, _, mut session, item) = fixture();
    let id = analyze(&mut session, item, None);
    let analysis_id = session.project.preedit.maps[&id].analysis;
    let mut analysis = (*session.project.preedit.analyses[&analysis_id]).clone();
    let t = |v| Tick::from_units(v, 1000);
    let old = ["this", "camera", "records"];
    let fresh = ["this", "camera", "records", "clear", "video."];
    let words = old
        .iter()
        .enumerate()
        .map(|(i, key)| shotclutch_project::Word::new(*key, t(200 + i as i64 * 200), t(350 + i as i64 * 200)))
        .chain(fresh.iter().enumerate().map(|(i, key)| shotclutch_project::Word::new(*key, t(1700 + i as i64 * 200), t(1850 + i as i64 * 200))))
        .collect();
    Arc::make_mut(&mut analysis.chunks[0]).transcript = Some(shotclutch_project::Transcript { words, ..Default::default() });
    Arc::make_mut(&mut session.project).preedit.analyses.insert(analysis_id, Arc::new(analysis));
    let map = review_action(&mut session, id, json!({"kind":"range","decision":"Keep","range":{"start":t(300).0.to_string(),"end":t(500).0.to_string()}}));
    let before = session.project.preedit.maps[&map].clone();
    let options = CleanupSettings {
        mode: CleanupMode::GlingCompatible,
        take_matching: TakeMatching::OrderedV1,
        cut_pauses: false,
        preserve_pauses_ms: Some(800),
        ..Default::default()
    };
    let recut = review_action(&mut session, map, json!({"kind":"cleanUp","options":options}));
    let changed = session.project.preedit.maps[&recut].clone();
    assert_eq!(changed.overrides, before.overrides);
    assert_eq!(session.project.preedit.maps[&map], before, "old immutable map must remain unchanged");
    assert_eq!(changed.takes.len(), 1);
    assert!(changed.candidates.iter().all(|c| c.confidence.is_none()), "rule evidence is not calibrated probability");
    let segments = changed.segments(range(0, 4)).unwrap();
    assert!(segments.iter().any(|s| s.source.contains(t(400))), "manual restoration must win");
    assert!(!segments.iter().any(|s| s.source.contains(t(650))), "matched abandoned attempt must be cut");
    session.undo().unwrap();
    assert!(!session.project.preedit.maps.contains_key(&recut));
    session.redo().unwrap();
    assert_eq!(session.project.preedit.maps[&recut], changed);
    let path = _dir.path().join("ordered.scproj");
    session.execute("file.saveAs", json!({"path":path})).unwrap();
    let mut reopened = Session::default();
    reopened.execute("file.open", json!({"path":path})).unwrap();
    assert_eq!(reopened.project.preedit.maps[&recut], changed);
    assert_eq!(changed.cleanup.unwrap().take_matching, TakeMatching::OrderedV1);
    // Independent pacing settings must not reclassify takes.
    let saved = &reopened.project.preedit.analyses[&analysis_id];
    let a = analysis::propose(analysis_id, saved, Some(options));
    let b = analysis::propose(analysis_id, saved, Some(CleanupSettings { min_pause_ms: Some(3000), preserve_pauses_ms: Some(1000), ..options }));
    assert_eq!(a.takes.len(), b.takes.len());
    let pairs = |m: &PreEditMap| m.takes.iter().map(|g| g.alternatives.iter().map(|&i| m.candidates[i].range).collect::<Vec<_>>()).collect::<Vec<_>>();
    assert_eq!(pairs(&a), pairs(&b));
}

#[test]
fn m2_automatic_union_matches_ordered_edits_with_manual_overrides() {
    let (_dir, _, mut session, item) = fixture();
    let id = analyze(&mut session, item, None);
    let mut map = (*session.project.preedit.maps[&id]).clone();
    let cuts: Vec<_> = (0..300)
        .map(|i| {
            let start = (i * 73 % 3500) as i64;
            Override { range: TimeRange::from_bounds(Tick::from_units(start, 1000), Tick::from_units(start + 230, 1000)), decision: Decision::Remove }
        })
        .collect();
    map.overrides = vec![Override { range: range(1, 3), decision: Decision::Keep }, Override { range: range(2, 3), decision: Decision::Remove }];
    map.automatic = Some(Arc::new(cuts.clone()));
    let fast = map.segments(range(0, 4)).unwrap();
    map.automatic = None;
    map.overrides.splice(0..0, cuts);
    assert_eq!(fast, map.segments(range(0, 4)).unwrap());
}

#[test]
fn m2_opposite_phase_stereo_does_not_become_silence() {
    let (dir, original, mut session, _) = fixture();
    let mono = std::fs::read(original).unwrap();
    let mut stereo = mono[..44].to_vec();
    stereo[4..8].copy_from_slice(&((mono.len() as u32 - 44) * 2 + 36).to_le_bytes());
    stereo[22..24].copy_from_slice(&2u16.to_le_bytes());
    stereo[28..32].copy_from_slice(&64000u32.to_le_bytes());
    stereo[32..34].copy_from_slice(&4u16.to_le_bytes());
    stereo[40..44].copy_from_slice(&((mono.len() as u32 - 44) * 2).to_le_bytes());
    for sample in mono[44..].chunks_exact(2) {
        let v = i16::from_le_bytes([sample[0], sample[1]]);
        stereo.extend(v.to_le_bytes());
        stereo.extend((-v).to_le_bytes());
    }
    let path = dir.path().join("stereo.wav");
    std::fs::write(&path, stereo).unwrap();
    session.execute("file.import", json!({"paths":[path]})).unwrap();
    let item = session.project.items.values().find(|i| i.name == "stereo.wav").unwrap().id;
    let id = analyze(&mut session, item, None);
    let map = &session.project.preedit.maps[&id];
    assert_eq!(map.candidates.len(), 1);
    assert_eq!(map.candidates[0].range, range(1, 3));
}

#[test]
fn m2_source_edges_ignore_overlong_alignment_but_protect_real_words() {
    let mut db = vec![-85.0; 400];
    db[150..250].fill(-25.0);
    let measured = cleanup::source_edge_quiet(range(0, 4), range(0, 4), &db);
    assert_eq!(
        measured,
        vec![TimeRange::from_bounds(seconds(0), Tick::from_units(1500, 1000)), TimeRange::from_bounds(Tick::from_units(2500, 1000), seconds(4))]
    );
    assert!(cleanup::source_edge_quiet(range(0, 4), range(0, 4), &vec![-65.0; 400]).is_empty());
    let (_dir, _, mut session, item) = fixture();
    let id = analyze(&mut session, item, None);
    let map = &session.project.preedit.maps[&id];
    let mut saved = (*session.project.preedit.analyses[&map.analysis]).clone();
    let chunk = Arc::make_mut(&mut saved.chunks[0]);
    chunk.edge_quiet = measured.clone();
    chunk.transcript = Some(shotclutch_project::Transcript {
        words: vec![shotclutch_project::Word::new("Aligned", Tick::from_units(100, 1000), Tick::from_units(3800, 1000))],
        ..Default::default()
    });
    let proposed = analysis::propose(map.analysis, &saved, None);
    let source = session.media.full_res_source(&session.project, item, &*session.services).unwrap();
    let cuts = automatic_cuts(&proposed, &saved, &*source);
    assert!(cuts.iter().any(|cut| cut.range.start == Tick::ZERO && cut.range.end() > seconds(1)));
    let restored = PreEditMap { automatic: Some(cuts), overrides: vec![Override { range: measured[0], decision: Decision::Keep }], ..proposed };
    assert_eq!(restored.segments(range(0, 4)).unwrap()[0].source.start, Tick::ZERO);
    Arc::make_mut(&mut saved.chunks[0]).transcript.as_mut().unwrap().words =
        vec![shotclutch_project::Word::new("Quiet", Tick::from_units(100, 1000), Tick::from_units(500, 1000))];
    let protected = analysis::propose(map.analysis, &saved, None);
    assert!(!protected.candidates.iter().any(|cut| cut.range.start == Tick::ZERO && cut.range.end() > seconds(1)));
}

#[test]
fn m2_automatic_take_cleanup_preserves_changed_negation_numbers_and_uncertain_endings() {
    let words = |text: &str| text.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
    for (earlier, later) in [
        ("this camera is good", "this camera is not good"),
        ("this camera costs 549 dollars", "this camera costs 399 dollars"),
        ("this camera isn't working", "this camera is working"),
    ] {
        assert!(!cleanup::safe_take_removal(&words(earlier), &words(later), true));
        assert!(!cleanup::safe_take_removal(&words(earlier), &words(later), false));
    }
    assert!(!cleanup::safe_take_removal(&words("this camera looks very good"), &words("this camera looks really great"), true));
    assert!(cleanup::safe_take_removal(&words("this camera looks good"), &words("this camera looks good"), true));
    assert!(cleanup::safe_take_removal(&words("this camera"), &words("this camera looks good"), true));
}

#[test]
fn m2_reject_group_restores_alternatives_and_undo_is_one_action() {
    let (_dir, _, mut session, item) = fixture();
    let id = analyze(&mut session, item, None);
    session
        .edit("Take group fixture", |project, _| {
            let map = Arc::make_mut(project.preedit.maps.get_mut(&id).unwrap());
            map.candidates = Arc::new(vec![
                Candidate { range: range(0, 1), kind: CandidateKind::Take, reason: "earlier".into(), confidence: None },
                Candidate { range: range(3, 4), kind: CandidateKind::Take, reason: "later".into(), confidence: None },
            ]);
            map.takes = Arc::new(vec![TakeGroup { alternatives: vec![0, 1], explanation: "comparison".into() }]);
            map.automatic = Some(Arc::new(vec![Override { range: range(0, 1), decision: Decision::Remove }]));
            Ok(())
        })
        .unwrap();
    let before = session.project.preedit.maps[&id].clone();
    let rejected = review_action(&mut session, id, json!({"kind":"rejectGroup","group":0}));
    let map = &session.project.preedit.maps[&rejected];
    assert!(map.takes.is_empty());
    assert_eq!(map.segments(range(0, 4)).unwrap()[0].source, range(0, 4));
    session.undo().unwrap();
    assert_eq!(session.project.preedit.maps[&id], before);
    assert!(!session.project.preedit.maps.contains_key(&rejected));
    session.redo().unwrap();
    assert!(session.project.preedit.maps[&rejected].takes.is_empty());
}

#[test]
fn m2_collection_and_relink_keep_removed_source_available_for_restoration() {
    let (dir, source, mut session, item) = fixture();
    let original = std::fs::read(&source).unwrap();
    let id = analyze(&mut session, item, None);
    let cut =
        review_action(&mut session, id, json!({"kind":"range","decision":"Remove","range":{"start":seconds(1).0.to_string(),"end":seconds(3).0.to_string()}}));
    let map = review_action(&mut session, cut, json!({"kind":"markReviewed"}));
    session.execute("preedit.apply", json!({"map":map.to_string(),"expectedRevision":session.revision.to_string(),"operationId":"collect-cut"})).unwrap();
    let output = session.execute("file.projectManager", json!({"destination":dir.path().join("Collected"),"wait":true})).unwrap();
    session.media.clear();
    std::fs::remove_file(source).unwrap();
    let mut reopened = Session::default();
    reopened.start_autosave(crate::autosave::AutosaveConfig::new(&dir.path().join("reopened-data"))).unwrap();
    reopened.execute("file.open", json!({"path":output["project"]})).unwrap();
    assert_eq!(reopened.project.preedit.maps[&map], session.project.preedit.maps[&map]);
    let shotclutch_project::MediaRef::File { path } = &reopened.project.item(item).unwrap().as_media().unwrap().media else {
        panic!("missing collected original")
    };
    let collected = PathBuf::from(output["project"].as_str().unwrap()).parent().unwrap().join(path);
    assert_eq!(std::fs::read(&collected).unwrap(), original);
    let moved = dir.path().join("Relinked");
    std::fs::create_dir(&moved).unwrap();
    let moved = moved.join(collected.file_name().unwrap());
    std::fs::rename(&collected, &moved).unwrap();
    reopened.media.clear();
    reopened.execute("media.relink", json!({"item":item.0,"path":moved})).unwrap();
    let restored =
        review_action(&mut reopened, map, json!({"kind":"range","decision":"Keep","range":{"start":seconds(1).0.to_string(),"end":seconds(3).0.to_string()}}));
    let restored = review_action(&mut reopened, restored, json!({"kind":"markReviewed"}));
    let (draft, preview_id) = preview(&reopened, restored).unwrap();
    let result = reopened
        .execute("preedit.apply", json!({"map":restored.to_string(),"expectedRevision":reopened.revision.to_string(),"operationId":"restore-after-relink"}))
        .unwrap();
    let sequence = ItemId(result["sequenceId"].as_str().unwrap().parse().unwrap());
    assert_eq!(reopened.project.sequence(sequence).unwrap().duration(), seconds(4));
    assert_eq!(draft.sequence(preview_id).unwrap().duration(), reopened.project.sequence(sequence).unwrap().duration());
    assert_eq!(std::fs::read(moved).unwrap(), original);
}

#[test]
fn m2_scoped_clients_cannot_access_owner_preedit_commands() {
    let (_dir, _, mut session, item) = fixture();
    let id = analyze(&mut session, item, None);
    let map = review_action(&mut session, id, json!({"kind":"markReviewed"}));
    let result =
        session.execute("preedit.apply", json!({"map":map.to_string(),"expectedRevision":session.revision.to_string(),"operationId":"scope-fixture"})).unwrap();
    let sequence = ItemId(result["sequenceId"].as_str().unwrap().parse().unwrap());
    let mut scope = crate::service::EditorScope::sequence(sequence);
    scope.allow_edits = true;
    let client = session.authorize_editor("scoped-editor", scope).unwrap();
    let before = session.project.clone();
    let revision = session.revision;
    for name in [
        "preedit.inspect",
        "preedit.analyze",
        "preedit.cancel",
        "preedit.review",
        "preedit.preview",
        "preedit.apply",
        "preedit.applyBackground",
        "preedit.cancelApply",
    ] {
        assert_eq!(session.execute_as(&client, name, json!({})).unwrap_err().code, crate::service::ErrorCode::PermissionDenied);
    }
    assert_eq!(session.revision, revision);
    assert!(Arc::ptr_eq(&before, &session.project));
}

#[test]
fn m2_first_pass_recorded_word_and_interval_oracle() {
    let (_dir, _, mut session, item) = fixture();
    let id = analyze(&mut session, item, None);
    let map = &session.project.preedit.maps[&id];
    let mut a = (*session.project.preedit.analyses[&map.analysis]).clone();
    let tick = |n: &Value| Tick::from_units(n.as_i64().unwrap(), 1_000_000);
    for row in include_str!("../../test-data/opengling-rule-oracle.jsonl").lines() {
        let case: Value = serde_json::from_str(row).unwrap();
        a.settings.language = case["lang"].as_str().unwrap().into();
        a.source.range = TimeRange::from_bounds(Tick::ZERO, tick(&case["duration"]));
        let words: Vec<_> =
            case["words"].as_array().unwrap().iter().map(|w| shotclutch_project::Word::new(w[0].as_str().unwrap(), tick(&w[1]), tick(&w[2]))).collect();
        let chunk = Arc::make_mut(&mut a.chunks[0]);
        chunk.coverage = a.source.range;
        chunk.transcript = Some(shotclutch_project::Transcript { words: words.clone(), ..Default::default() });
        for mode in ["takes", "fillers", "combined"] {
            for matching in [TakeMatching::Legacy, TakeMatching::OrderedV7] {
                let map = gling::propose(
                    id,
                    &a,
                    CleanupSettings {
                        mode: CleanupMode::GlingCompatible,
                        take_matching: matching,
                        min_pause_ms: Some(500),
                        cut_fillers: mode != "takes",
                        cut_pauses: mode == "combined",
                        cut_bad_takes: mode != "fillers",
                        ..Default::default()
                    },
                );
                let keep: std::collections::BTreeSet<_> = map.takes.iter().filter_map(|g| g.alternatives.last()).copied().collect();
                let cuts: Vec<_> = map.candidates.iter().enumerate().filter(|(i, _)| !keep.contains(i)).map(|(_, c)| c).collect();
                let got: String = words
                    .iter()
                    .map(|w| {
                        cuts.iter()
                            .find(|c| c.range.start <= w.start && c.range.end() >= w.end)
                            .map_or('.', |c| if c.kind == CandidateKind::Filler { 'f' } else { 'b' })
                    })
                    .collect();
                let expected = case[mode][0].as_str().unwrap();
                if matching == TakeMatching::OrderedV7 {
                    assert!(expected.chars().zip(got.chars()).all(|(e, g)| e != 'b' || g == 'b'), "reference retake lost: {} {mode}", case["name"]);
                    continue;
                }
                assert_eq!(got, expected, "{} {mode}", case["name"]);
                let mut ranges: Vec<_> = cuts.iter().map(|c| (c.range.start, c.range.end())).collect();
                ranges.sort_unstable();
                let mut merged: Vec<(Tick, Tick)> = Vec::new();
                for (s, e) in ranges {
                    if let Some(last) = merged.last_mut()
                        && s <= last.1
                    {
                        last.1 = last.1.max(e)
                    } else {
                        merged.push((s, e))
                    }
                }
                let expected: Vec<_> = case[mode][1].as_array().unwrap().iter().map(|r| (tick(&r[0]), tick(&r[1]))).collect();
                assert_eq!(merged, expected, "{} {mode}", case["name"]);
            }
        }
    }
}

/// Compare rule proposals on an optional local qualification snapshot without opening or changing it.
#[test]
#[ignore = "requires a private saved transcript and an output path"]
fn m2_export_first_pass_intervals() {
    let doc: Value = serde_json::from_slice(&std::fs::read(std::env::var_os("SHOTCLUTCH_RULE_PROJECT").unwrap()).unwrap()).unwrap();
    let state: PreEdit = serde_json::from_value(doc["project"]["preedit"].clone()).unwrap();
    let (_, map) = state.maps.last_key_value().unwrap();
    let analysis = &state.analyses[&map.analysis];
    let (_tmp, _, session, item) = fixture();
    let source = session.media.full_res_source(&session.project, item, &*session.services).unwrap();
    let mut runs = serde_json::Map::new();
    for (name, take_matching) in
        [("v6", TakeMatching::OrderedV6), ("reference", TakeMatching::Legacy), ("v7", TakeMatching::OrderedV7), ("v8", TakeMatching::OrderedV8)]
    {
        let settings = CleanupSettings { take_matching, ..map.cleanup.unwrap() };
        let p = gling::propose(map.analysis, analysis, settings);
        let keep: std::collections::BTreeSet<_> = p.takes.iter().filter_map(|g| g.alternatives.last().copied()).collect();
        let ranges: Vec<_> = p
            .candidates
            .iter()
            .enumerate()
            .filter(|(i, c)| !keep.contains(i) && matches!(c.kind, CandidateKind::Restart | CandidateKind::Take))
            .map(|(_, c)| [c.range.start.0, c.range.end().0])
            .collect();
        let cut = automatic_cuts(&p, analysis, &*source);
        let bare = PreEditMap { automatic: Some(cut), overrides: vec![], ..p.clone() };
        let kept = bare.segments(analysis.source.range).unwrap();
        runs.insert(
            format!("{name}_automatic"),
            json!({"retained":kept.iter().map(|s|[s.source.start.0,s.source.end().0]).collect::<Vec<_>>(),"duration":kept.last().map(|s|s.output.end().0)}),
        );
        runs.insert(name.into(), json!(ranges));
    }
    std::fs::write(std::env::var_os("SHOTCLUTCH_RULE_REPORT").unwrap(), serde_json::to_vec(&runs).unwrap()).unwrap();
}

#[test]
fn m2_take_review_stamp_detects_source_changes_without_hashing_on_the_editor_thread() {
    let (_dir, path, mut session, item) = fixture();
    let map = analyze(&mut session, item, None);
    let initial = take_review_stamp(&session, map).unwrap();
    let again = take_review_stamp(&session, map).unwrap();
    assert_eq!(serde_json::to_value(&initial).unwrap(), serde_json::to_value(again).unwrap());
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(2)).unwrap();
    let changed = take_review_stamp(&session, map).unwrap();
    assert_ne!(initial.modified_ns, changed.modified_ns);
    assert_eq!(initial.identity, changed.identity);
}
