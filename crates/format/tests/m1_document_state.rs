use serde_json::json;
use shotclutch_format::{
    decode, encode, encode_document,
    session::{DocumentId, DocumentState, HistorySnapshot},
};
use shotclutch_project::Project;

fn state() -> DocumentState {
    DocumentState { version: 1, project_id: DocumentId::default(), revision: 9_007_199_254_740_993, next_id: 9_007_199_254_740_993, undo: vec![], redo: vec![] }
}

#[test]
fn identity_revision_and_history_share_the_versioned_envelope() {
    let project = Project::new("Current");
    let mut state = state();
    state.undo.push(HistorySnapshot { label: "Edit".into(), snapshot: serde_json::from_slice(&encode(&Project::new("Before"), false)).unwrap() });
    let bytes = encode_document(&project, None, Some(&state), None, false);
    let wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(wire["edit_state"]["revision"], "9007199254740993");
    let loaded = decode(&bytes).unwrap();
    let restored = loaded.edit_state.unwrap();
    assert_eq!(restored.project_id, state.project_id);
    assert_eq!(restored.revision, state.revision);
    assert_eq!(restored.next_id, state.next_id);
    assert_eq!(restored.undo[0].label, "Edit");
    assert_eq!(restored.undo[0].snapshot["project"]["name"], "Before");
}

#[test]
fn invalid_document_state_is_not_silently_dropped_like_optional_view_state() {
    let bytes = encode_document(&Project::default(), None, Some(&state()), None, false);
    let original: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for revision in [json!(42), json!("01"), json!("+1"), json!("0"), json!("18446744073709551615")] {
        let mut wire = original.clone();
        wire["edit_state"]["revision"] = revision;
        assert!(decode(&serde_json::to_vec(&wire).unwrap()).is_err());
    }
    let mut future = state();
    future.version = 2;
    let bytes = encode_document(&Project::default(), None, Some(&future), None, false);
    assert!(decode(&bytes).unwrap_err().to_string().contains("CAPABILITY_UNAVAILABLE"));
}

#[test]
fn valid_json_corruption_of_project_or_history_fails_integrity_validation() {
    let bytes = encode_document(&Project::default(), None, Some(&state()), None, false);
    let original: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for path in [["project", "notes"], ["edit_state", "revision"]] {
        let mut changed = original.clone();
        changed[path[0]][path[1]] = json!("1234");
        let error = decode(&serde_json::to_vec(&changed).unwrap()).unwrap_err();
        assert!(error.to_string().contains("checksum mismatch"));
    }
}

#[test]
fn v16_migrates_without_fabricating_persisted_history() {
    let mut doc: serde_json::Value = serde_json::from_slice(&encode(&Project::default(), false)).unwrap();
    doc["schema_version"] = json!(16);
    doc.as_object_mut().unwrap().remove("integrity");
    let loaded = decode(&serde_json::to_vec(&doc).unwrap()).unwrap();
    assert!(loaded.migrated());
    assert!(loaded.edit_state.is_none());
}

#[test]
fn checksum_tracks_wire_floats_without_widening_f32_or_rounding_f64() {
    use shotclutch_project::{CaptionFormat, CaptionTrack, SequenceSettings, TrackId};
    let mut project = Project::default();
    let sequence = project.new_sequence("Floating point", SequenceSettings::default(), 1, 1, None);
    let mut track = CaptionTrack::new(TrackId(project.alloc_id()), "Captions".into(), CaptionFormat::Subtitle);
    track.style.margin = 0.07;
    track.style.line_spacing = 1.234567;
    project.sequence_mut(sequence).unwrap().caption_tracks.push(track);
    project.sequence_mut(sequence).unwrap().master_volume_db = -9.990000000000002;
    for pretty in [true, false] {
        let bytes = encode(&project, pretty);
        let loaded = decode(&bytes).unwrap();
        assert_eq!(loaded.project, project);
        assert_eq!(encode(&loaded.project, pretty), bytes);
        // Parsing and reformatting the file as generic JSON must not invalidate its digest.
        let wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(decode(&serde_json::to_vec_pretty(&wire).unwrap()).is_ok());
    }
}
