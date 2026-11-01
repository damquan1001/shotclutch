use super::*;

fn linked() -> (Session, ItemId, ClipId, ClipId) {
    let mut s = Session::default();
    s.execute("file.openDemoProject", json!({})).unwrap();
    s.execute("file.newSequence", json!({"name": "M1", "video": 1, "audio": 1})).unwrap();
    let source = s.project.items.values().find(|i| i.has_video() && i.has_audio() && i.as_media().is_some()).unwrap().id;
    s.execute("timeline.place", json!({"item": source.0, "frame": 0, "duration": s.sequence_rate().tick_of(48).0})).unwrap();
    let q = s.active_sequence().unwrap();
    let (video, audio) = (q.video_tracks[0].items[0].id, q.audio_tracks[0].items[0].id);
    let sequence = s.state.active_sequence.unwrap();
    (s, sequence, video, audio)
}

fn trim(clip: ClipId, delta: i64) -> TimelineEdit {
    TimelineEdit::Trim { clip, edge: Edge::Out, mode: TrimMode::Regular, delta: delta.to_string(), linked: true }
}

#[test]
fn staged_linked_trim_is_private_and_matches_the_existing_command() {
    let (mut s, sequence, video, audio) = linked();
    let before = s.project.clone();
    let revision = s.revision;
    let undo = s.history.undo.len();
    let journal = s.journal.clone();
    let events = s.events.len();
    let delta = -s.sequence_rate().tick_of(5).0;
    let prepared = s.prepare_edits(revision, sequence, "Trim linked pair", &[trim(video, delta)]).unwrap();
    assert!(Arc::ptr_eq(&s.project, &before));
    assert_eq!(s.revision, revision);
    assert_eq!(s.history.undo.len(), undo);
    assert_eq!(s.journal, journal);
    assert_eq!(s.events.len(), events);
    for clip in [video, audio] {
        let old = before.sequence(sequence).unwrap().find_item(clip).unwrap().1;
        let new = prepared.project().sequence(sequence).unwrap().find_item(clip).unwrap().1;
        assert_eq!(new.duration.0, old.duration.0 + delta);
    }
    let expected = prepared.project().clone();
    s.execute("timeline.trim", json!({"clip": video.0, "edge": "out", "mode": "regular", "delta": delta, "linked": true})).unwrap();
    assert_eq!(*s.project, *expected);
}

#[test]
fn grouped_commit_undo_redo_and_abort() {
    let (mut s, sequence, video, _) = linked();
    let before = s.project.clone();
    let revision = s.revision;
    let undo = s.history.undo.len();
    let delta = -s.sequence_rate().tick_of(5).0;
    let edits = [trim(video, delta), trim(video, delta)];
    let aborted = s.prepare_edits(revision, sequence, "Abort", &edits).unwrap();
    drop(aborted);
    assert!(Arc::ptr_eq(&s.project, &before));
    let prepared = s.prepare_edits(revision, sequence, "Two trims", &edits).unwrap();
    let expected = prepared.project().clone();
    s.commit_edits(prepared).unwrap();
    assert_eq!(*s.project, *expected);
    assert_eq!(s.history.undo.len(), undo + 1);
    assert_eq!(s.revision, revision + 1);
    assert_eq!(s.undo().as_deref(), Some("Two trims"));
    assert_eq!(*s.project, *before);
    assert_eq!(s.revision, revision + 2);
    s.redo().unwrap();
    assert_eq!(*s.project, *expected);
    assert_eq!(s.revision, revision + 3);
}

#[test]
fn stale_preview_cannot_overwrite_a_human_edit_or_another_project() {
    let (mut s, sequence, video, _) = linked();
    let prepared = s.prepare_edits(s.revision, sequence, "Trim", &[trim(video, -1)]).unwrap();
    s.execute("file.newBin", json!({"name": "Human edit"})).unwrap();
    let current = s.project.clone();
    assert!(matches!(s.commit_edits(prepared), Err(TransactionError::RevisionConflict)));
    assert!(Arc::ptr_eq(&s.project, &current));
    let prepared = s.prepare_edits(s.revision, sequence, "Trim", &[trim(video, -1)]).unwrap();
    // Even a project with the same revision and identical contents is a different document.
    s.project = Arc::new((*s.project).clone());
    assert!(matches!(s.commit_edits(prepared), Err(TransactionError::RevisionConflict)));
}

#[test]
fn invalid_later_edit_discards_the_entire_group() {
    let (s, sequence, video, _) = linked();
    let original = s.project.clone();
    let edits = [trim(video, -s.sequence_rate().tick_of(5).0), trim(ClipId(u64::MAX), -1)];
    assert!(s.prepare_edits(s.revision, sequence, "Fail", &edits).is_err());
    assert!(Arc::ptr_eq(&s.project, &original));
    assert!(s.prepare_edits(s.revision - 1, sequence, "Stale", &edits).is_err());
}

#[test]
fn explicit_split_and_locked_targets() {
    let (mut s, sequence, video, audio) = linked();
    let time = s.sequence_rate().tick_of(12).0.to_string();
    let prepared = s.prepare_edits(s.revision, sequence, "Split linked pair", &[TimelineEdit::Split { clips: vec![video, audio], time }]).unwrap();
    s.commit_edits(prepared).unwrap();
    let q = s.active_sequence().unwrap();
    assert_eq!(q.video_tracks[0].items.len(), 2);
    assert_eq!(q.audio_tracks[0].items.len(), 2);
    s.edit("Lock", |p, _| {
        p.sequence_mut(sequence).unwrap().video_tracks[0].locked = true;
        Ok(())
    })
    .unwrap();
    assert!(s.prepare_edits(s.revision, sequence, "Locked", &[trim(video, -1)]).is_err());
}

#[test]
fn exact_time_wire_values_do_not_pass_through_floating_point() {
    for n in [i64::MIN, -9_007_199_254_740_993, 0, 9_007_199_254_740_993, i64::MAX] {
        assert_eq!(exact_tick(&n.to_string()).unwrap(), n);
    }
    for invalid in ["1.5", "1e6", "+1", "01", "-0", "9223372036854775808"] {
        assert!(exact_tick(invalid).is_err());
    }
    let command: TimelineEdit =
        serde_json::from_value(json!({"command": "trim", "clip": 12, "edge": "out", "mode": "regular", "delta": "9007199254740993", "linked": true})).unwrap();
    let wire = serde_json::to_value(command).unwrap();
    assert_eq!(wire["delta"], "9007199254740993");
    assert!(serde_json::from_value::<TimelineEdit>(json!({"command": "file.save", "path": "outside.scproj"})).is_err());
}

fn start_recovery(s: &mut Session, data: &std::path::Path) {
    s.start_autosave(crate::autosave::AutosaveConfig::new(data)).unwrap();
    s.persistence.as_ref().unwrap().flush_checked().unwrap();
}

#[test]
fn durable_commit_recovers_grouped_undo_and_identity_after_crash() {
    let dir = tempfile::tempdir().unwrap();
    let (mut s, sequence, video, _) = linked();
    start_recovery(&mut s, dir.path());
    let before = s.project.clone();
    let id = s.document_id;
    let revision = s.revision;
    let prepared = s.prepare_edits(revision, sequence, "Durable linked trim", &[trim(video, -s.sequence_rate().tick_of(5).0)]).unwrap();
    let expected = prepared.project().clone();
    s.commit_edits_durable(prepared).unwrap();
    let bytes = std::fs::read(s.persistence.as_ref().unwrap().session_dir.join("snapshot.scproj")).unwrap();
    let loaded = shotclutch_format::decode(&bytes).unwrap();
    assert_eq!(loaded.edit_state.as_ref().unwrap().revision, revision + 1);
    assert_eq!(loaded.project, *expected);
    s.persistence.take().unwrap().simulate_crash();
    let mut reopened = Session::default();
    start_recovery(&mut reopened, dir.path());
    reopened.execute("file.recover", json!({})).unwrap();
    assert_eq!(reopened.document_id, id);
    assert_eq!(*reopened.project, *expected);
    let recovered_revision = reopened.revision;
    assert_eq!(reopened.undo().as_deref(), Some("Durable linked trim"));
    assert_eq!(*reopened.project, *before);
    assert!(reopened.revision > recovered_revision);
    reopened.redo().unwrap();
    assert_eq!(*reopened.project, *expected);
    reopened.shutdown();
}

#[test]
fn durable_api_fails_closed_without_enabled_storage() {
    let (mut s, sequence, video, _) = linked();
    let before = s.project.clone();
    let prepared = s.prepare_edits(s.revision, sequence, "No worker", &[trim(video, -1)]).unwrap();
    assert!(matches!(s.commit_edits_durable(prepared), Err(TransactionError::CapabilityUnavailable(_))));
    assert!(Arc::ptr_eq(&s.project, &before));
    let dir = tempfile::tempdir().unwrap();
    start_recovery(&mut s, dir.path());
    s.execute("prefs.set", json!({"key": "autoSave.recoveryJournal", "value": false})).unwrap();
    let prepared = s.prepare_edits(s.revision, sequence, "Disabled worker", &[trim(video, -1)]).unwrap();
    assert!(matches!(s.commit_edits_durable(prepared), Err(TransactionError::CapabilityUnavailable(_))));
    assert!(Arc::ptr_eq(&s.project, &before));
    s.shutdown();
}

#[test]
fn crash_after_commit_marker_before_publication_recovers_complete_new_revision() {
    let dir = tempfile::tempdir().unwrap();
    let (mut s, sequence, video, _) = linked();
    start_recovery(&mut s, dir.path());
    let before = s.project.clone();
    let prepared = s.prepare_edits(s.revision, sequence, "Commit window", &[trim(video, -1)]).unwrap();
    let expected = prepared.project().clone();
    crate::document::CRASH_AFTER_COMMIT.with(|fault| fault.set(true));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.commit_edits(prepared)));
    assert!(result.is_err());
    assert!(Arc::ptr_eq(&s.project, &before), "the crash was before live publication");
    s.persistence.take().unwrap().simulate_crash();
    let mut reopened = Session::default();
    start_recovery(&mut reopened, dir.path());
    reopened.execute("file.recover", json!({})).unwrap();
    assert_eq!(*reopened.project, *expected);
    assert_eq!(reopened.undo().as_deref(), Some("Commit window"));
    assert_eq!(*reopened.project, *before);
    reopened.shutdown();
}

#[test]
fn failed_durable_write_publishes_nothing_and_keeps_saved_project() {
    let dir = tempfile::tempdir().unwrap();
    let (mut s, sequence, video, _) = linked();
    let path = dir.path().join("Original.scproj");
    s.execute("file.saveAs", json!({"path": path.to_str().unwrap()})).unwrap();
    let original = std::fs::read(&path).unwrap();
    start_recovery(&mut s, &dir.path().join("data"));
    let target = s.persistence.as_ref().unwrap().session_dir.join("snapshot.scproj");
    std::fs::create_dir(&target).unwrap(); // force atomic promotion to fail, on Windows and Unix
    let before = s.project.clone();
    let undo = s.history.undo.len();
    let revision = s.revision;
    let events = s.events.len();
    let prepared = s.prepare_edits(revision, sequence, "Must fail", &[trim(video, -1)]).unwrap();
    assert!(matches!(s.commit_edits(prepared), Err(TransactionError::PersistenceFailed(_))));
    assert!(Arc::ptr_eq(&s.project, &before));
    assert_eq!((s.history.undo.len(), s.revision, s.events.len()), (undo, revision, events));
    assert_eq!(std::fs::read(&path).unwrap(), original);
    std::fs::remove_dir(target).unwrap();
    s.shutdown();
}

#[test]
fn advisory_metadata_failure_cannot_turn_committed_edit_into_failed_edit() {
    let dir = tempfile::tempdir().unwrap();
    let (mut s, sequence, video, _) = linked();
    let path = dir.path().join("Native.scproj");
    s.execute("file.saveAs", json!({"path": path.to_str().unwrap()})).unwrap();
    start_recovery(&mut s, &dir.path().join("data"));
    std::fs::create_dir(s.persistence.as_ref().unwrap().session_dir.join("session.json")).unwrap();
    let prepared = s.prepare_edits(s.revision, sequence, "Metadata fault", &[trim(video, -1)]).unwrap();
    s.commit_edits(prepared).unwrap();
    let expected = s.project.clone();
    s.persistence.take().unwrap().simulate_crash();
    let mut reopened = Session::default();
    start_recovery(&mut reopened, &dir.path().join("data"));
    reopened.execute("file.recover", json!({})).unwrap();
    assert_eq!(*reopened.project, *expected);
    assert_eq!(reopened.path.as_deref(), path.to_str());
    assert_eq!(reopened.undo().as_deref(), Some("Metadata fault"));
    reopened.shutdown();
}

#[test]
fn save_reopen_preserves_history_and_never_reuses_ids_after_undo() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Identity.scproj");
    let mut s = Session::default();
    s.execute("file.newBin", json!({"name": "A"})).unwrap();
    let next = s.project.next_id;
    let id = s.document_id;
    s.execute("edit.undo", json!({})).unwrap();
    s.execute("history.deleteRedoable", json!({})).unwrap();
    s.execute("file.saveAs", json!({"path": path.to_str().unwrap()})).unwrap();
    let mut reopened = Session::default();
    reopened.execute("file.open", json!({"path": path.to_str().unwrap()})).unwrap();
    assert_eq!(reopened.document_id, id);
    assert!(reopened.project.next_id >= next);
    reopened.execute("file.newBin", json!({"name": "B"})).unwrap();
    assert!(reopened.project.next_id > next);
    reopened.execute("file.save", json!({})).unwrap();
    let mut second = Session::default();
    second.execute("file.open", json!({"path": path.to_str().unwrap()})).unwrap();
    assert_eq!(second.document_id, id);
    assert!(second.undo().is_some());
    assert!(second.project.root.children.is_empty());
    second.execute("file.newProject", json!({})).unwrap();
    assert_ne!(second.document_id, id);
}

#[test]
fn failed_group_cannot_leak_staged_commands_into_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Session::default();
    start_recovery(&mut s, dir.path());
    let before = s.project.clone();
    let revision = s.revision;
    let events = s.events.len();
    let journal = s.journal.len();
    let result: crate::Result<()> = s.edit_group("Failed group", |s| {
        s.execute("file.newBin", json!({"name": "Uncommitted"}))?;
        s.persistence.as_ref().unwrap().flush_checked().unwrap();
        assert!(!s.persistence.as_ref().unwrap().session_dir.join("snapshot.scproj").exists());
        Err(crate::EngineError::Other("injected failure".into()))
    });
    assert!(result.is_err());
    assert!(Arc::ptr_eq(&s.project, &before));
    assert_eq!((s.revision, s.events.len(), s.journal.len()), (revision, events, journal));
    s.edit_group("Two bins", |s| {
        s.execute("file.newBin", json!({"name": "A"}))?;
        s.execute("file.newBin", json!({"name": "B"}))?;
        Ok(())
    })
    .unwrap();
    assert_eq!(s.revision, revision + 1);
    assert_eq!(s.history.undo.len(), 1);
    s.persistence.as_ref().unwrap().flush_checked().unwrap();
    s.persistence.take().unwrap().simulate_crash();
    let mut reopened = Session::default();
    start_recovery(&mut reopened, dir.path());
    reopened.execute("file.recover", json!({})).unwrap();
    assert_eq!(reopened.project.root.children.len(), 2);
    assert_eq!(reopened.undo().as_deref(), Some("Two bins"));
    assert!(reopened.project.root.children.is_empty());
    reopened.shutdown();
}

#[test]
fn failed_recovery_flush_retains_original_candidate_for_retry() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Session::default();
    start_recovery(&mut s, dir.path());
    s.execute("file.newBin", json!({"name": "Retained"})).unwrap();
    s.persistence.take().unwrap().simulate_crash();
    let mut reopened = Session::default();
    start_recovery(&mut reopened, dir.path());
    let original = reopened.recovery_candidates()[0].dir.clone();
    let blocked = reopened.persistence.as_ref().unwrap().session_dir.join("snapshot.scproj");
    std::fs::create_dir(&blocked).unwrap();
    let before = reopened.project.clone();
    let id = reopened.document_id;
    assert!(reopened.execute("file.recover", json!({})).is_err());
    assert!(Arc::ptr_eq(&reopened.project, &before));
    assert_eq!(reopened.document_id, id);
    assert!(original.join("snapshot.scproj").is_file());
    assert_eq!(reopened.recovery_candidates().len(), 1);
    std::fs::remove_dir(blocked).unwrap();
    reopened.execute("file.recover", json!({})).unwrap();
    assert!(reopened.recovery_candidates().is_empty());
    assert!(!original.exists());
    reopened.shutdown();
}

#[test]
fn corrupt_or_recursive_history_is_rejected_before_replacing_live_document() {
    let dir = tempfile::tempdir().unwrap();
    let mut source = Session::default();
    source.execute("file.newBin", json!({"name": "Saved history"})).unwrap();
    let bytes = source.document_snapshot().encode(None, false).unwrap();
    let mut current = Session::default();
    current.execute("file.newBin", json!({"name": "Keep this document"})).unwrap();
    let before = current.project.clone();
    let id = current.document_id;
    let revision = current.revision;
    for bad in [json!(null), serde_json::from_slice(&bytes).unwrap()] {
        let loaded = shotclutch_format::decode(&bytes).unwrap();
        let mut state = loaded.edit_state.unwrap();
        state.undo[0].snapshot = bad;
        let path = dir.path().join("bad.scproj");
        std::fs::write(&path, shotclutch_format::encode_document(&loaded.project, None, Some(&state), None, false)).unwrap();
        assert!(current.execute("file.open", json!({"path": path.to_str().unwrap()})).is_err());
        assert!(Arc::ptr_eq(&current.project, &before));
        assert_eq!((current.document_id, current.revision), (id, revision));
    }
}

#[test]
fn upgrading_v16_preserves_original_before_writing_identity_and_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Previous.scproj");
    let mut historical: Value = serde_json::from_slice(&shotclutch_format::encode(&Project::default(), false)).unwrap();
    historical["schema_version"] = json!(16);
    historical.as_object_mut().unwrap().remove("integrity");
    let bytes = serde_json::to_vec(&historical).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let mut s = Session::default();
    s.execute("file.open", json!({"path": path.to_str().unwrap()})).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes, "opening must not migrate in place");
    s.execute("file.newBin", json!({"name": "New edit"})).unwrap();
    let saved = s.execute("file.save", json!({})).unwrap();
    assert_eq!(std::fs::read(saved["backup"].as_str().unwrap()).unwrap(), bytes);
    let loaded = shotclutch_format::decode(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(loaded.edit_state.unwrap().project_id, s.document_id);
    assert_eq!(loaded.schema_version, 17);
}

#[test]
fn autosave_after_recovery_cannot_overwrite_unbacked_migration_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Previous.scproj");
    let mut historical: Value = serde_json::from_slice(&shotclutch_format::encode(&Project::default(), false)).unwrap();
    historical["schema_version"] = json!(16);
    historical.as_object_mut().unwrap().remove("integrity");
    let bytes = serde_json::to_vec(&historical).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let mut s = Session::default();
    s.execute("file.open", json!({"path": path.to_str().unwrap()})).unwrap();
    start_recovery(&mut s, &dir.path().join("data"));
    s.execute("prefs.set", json!({"key": "autoSave.saveCurrentProject", "value": true})).unwrap();
    s.execute("file.newBin", json!({"name": "Pending migration"})).unwrap();
    s.persistence.as_ref().unwrap().auto_save_now().unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    s.persistence.take().unwrap().simulate_crash();
    let mut reopened = Session::default();
    start_recovery(&mut reopened, &dir.path().join("data"));
    reopened.execute("file.recover", json!({})).unwrap();
    assert_eq!(reopened.loaded_schema, 16);
    reopened.persistence.as_ref().unwrap().auto_save_now().unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes, "recovery must retain the source's migration protection");
    let result = reopened.execute("file.save", json!({})).unwrap();
    assert_eq!(std::fs::read(result["backup"].as_str().unwrap()).unwrap(), bytes);
    reopened.shutdown();
}

#[test]
fn exhausted_revision_or_id_counters_fail_before_publishing_an_edit() {
    let mut s = Session::default();
    let before = s.project.clone();
    s.revision = u64::MAX - 1;
    let error = s.execute("file.newBin", json!({"name": "Cannot commit"})).unwrap_err();
    assert!(error.to_string().contains("RESOURCE_LIMIT"));
    assert!(Arc::ptr_eq(&s.project, &before));
    assert_eq!(s.revision, u64::MAX - 1);
    s.bump_view();
    assert_eq!(s.revision, u64::MAX - 1);
    let mut s = Session::default();
    Arc::make_mut(&mut s.project).next_id = Project::MAX_NEXT_ID;
    let before = s.project.clone();
    let error = s.execute("file.newBin", json!({"name": "Cannot allocate"})).unwrap_err();
    assert!(error.to_string().contains("RESOURCE_LIMIT"));
    assert!(Arc::ptr_eq(&s.project, &before));
    assert!(s.history.undo.is_empty());
}
