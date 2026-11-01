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
