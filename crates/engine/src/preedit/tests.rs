use super::*;

fn seconds(n: i64) -> Tick {
    Tick(n * TICKS_PER_SECOND)
}

fn range(a: i64, b: i64) -> TimeRange {
    TimeRange::from_bounds(seconds(a), seconds(b))
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

fn fixture() -> (tempfile::TempDir, PathBuf, Session, ItemId) {
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

fn analyze(session: &mut Session, item: ItemId, prior: Option<u64>) -> u64 {
    session.execute("preedit.analyze", json!({"item":item.0.to_string(),"audioStream":0,"previousMap":prior.map(|id| id.to_string())})).unwrap();
    wait(session);
    *session.project.preedit.maps.last_key_value().unwrap_or_else(|| panic!("analysis produced no map: {:?}", session.log.entries)).0
}

fn review_action(session: &mut Session, map: u64, action: Value) -> u64 {
    let params = json!({"map":map.to_string(),"expectedRevision":session.revision.to_string(),"action":action});
    session.execute("preedit.review", params).unwrap()["map"].as_str().unwrap().parse().unwrap()
}

#[test]
fn m2_map_restores_nested_ranges_and_maps_half_open_joins() {
    let map = PreEditMap {
        analysis: 1,
        previous: None,
        reviewed: true,
        candidates: Arc::default(),
        takes: Arc::default(),
        conflicts: Vec::new(),
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
    assert_eq!(session.undo().as_deref(), Some("Apply Smart Rough Cut"));
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
    let (_dir, _, mut session, item) = fixture();
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
    let (draft, sequence) = preview(&session, cut).unwrap();
    let seq = draft.sequence(sequence).unwrap();
    let captions = &seq.caption_tracks[0].captions;
    let text = captions.iter().map(|caption| caption.text.as_str()).collect::<Vec<_>>().join(" ");
    assert!(text.contains("First") && text.contains("Last") && !text.contains("Discard"));
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

#[cfg(feature = "whisper")]
#[test]
#[ignore = "requires the pinned model and the operator's private footage"]
fn m2_private_whisper_qualification() {
    let footage = std::env::var_os("SHOTCLUTCH_M2_FOOTAGE").map(PathBuf::from).expect("set private footage path");
    let stream: usize = std::env::var("SHOTCLUTCH_M2_STREAM").expect("select the audited audio stream").parse().unwrap();
    let model_dir = crate::transcript::models_dir().expect("set the private data directory");
    let model = shotclutch_speech::models::find("whisper-base").unwrap();
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
                "item":item.id.0.to_string(),"audioStream":stream,"model":"whisper-base","language":"en","range":range
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
        let error = session.log.entries.iter().rev().find(|entry| entry.source == "Smart Rough Cut").map(|entry| &entry.message);
        panic!("no durable analysis: {error:?}")
    });
    assert_eq!(
        analysis.status,
        AnalysisStatus::Complete,
        "private analysis did not complete: {:?}",
        session.log.entries.iter().rev().find(|entry| entry.source == "Smart Rough Cut").map(|entry| &entry.message)
    );
    let words = analysis.chunks.iter().filter_map(|c| c.transcript.as_ref()).map(|t| t.words.len()).sum::<usize>();
    assert!(words > 0, "selected stream produced no transcript");
    let (map_id, map) = session.project.preedit.maps.last_key_value().expect("no proposal");
    let (draft, sequence) = preview(&session, *map_id).expect("rough-cut preview failed");
    let result = draft.sequence(sequence).unwrap();
    assert_eq!(result.duration(), analysis.source.range.duration);
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
    assert_eq!(refreshed.analyzer_version, 2);
    assert_eq!(refreshed.chunks[0].transcript, old.chunks[0].transcript);
    let mut wrong = args;
    wrong["language"] = json!("fr");
    assert!(session.execute("preedit.analyze", wrong).unwrap_err().to_string().contains("same source"));
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
