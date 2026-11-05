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
