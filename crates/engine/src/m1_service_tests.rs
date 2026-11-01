use super::*;
use crate::transactions::{Edge, TrimMode};
use shotclutch_project::ClipId;

fn fixture() -> (tempfile::TempDir, Session, ItemId, ClipId) {
    let dir = tempfile::tempdir().unwrap();
    let mut session = Session::default();
    session.execute("file.openDemoProject", json!({})).unwrap();
    let sequence = session.state.active_sequence.unwrap();
    let clip = session.project.sequence(sequence).unwrap().video_tracks[0].items[0].id;
    session.start_autosave(crate::autosave::AutosaveConfig::new(dir.path())).unwrap();
    (dir, session, sequence, clip)
}
fn scope(sequence: ItemId, edit: bool) -> EditorScope {
    let mut scope = EditorScope::sequence(sequence);
    scope.allow_edits = edit;
    scope
}
fn request(s: &Session, sequence: ItemId, clip: ClipId, name: &str) -> Value {
    serde_json::to_value(EditRequest {
        api_version: "1".into(),
        operation_id: format!("op-{name}"),
        idempotency_key: format!("key-{name}"),
        project_id: s.document_id,
        expected_revision: s.revision,
        sequence_id: sequence,
        label: "Scoped trim".into(),
        edits: vec![TimelineEdit::Trim { clip, edge: Edge::Out, mode: TrimMode::Regular, delta: (-s.sequence_rate().tick_of(1).0).to_string(), linked: true }],
    })
    .unwrap()
}
fn reference(s: &Session, preview: &Value) -> Value {
    json!({"apiVersion": "1", "projectId": s.document_id, "transactionId": preview["transactionId"]})
}

#[test]
fn owner_and_scoped_adapters_use_the_same_typed_commands_and_grouped_undo() {
    let (_dir, mut s, sequence, clip) = fixture();
    let before = s.project.clone();
    let client = s.authorize_editor("agent", scope(sequence, true)).unwrap();
    let params = request(&s, sequence, clip, "parity");
    let preview = s.execute_as(&client, "transaction.preview", params).unwrap();
    assert_eq!(preview["state"], "preview_ready");
    assert_eq!(*s.project, *before);
    let result = s.execute_as(&client, "transaction.commit", reference(&s, &preview)).unwrap();
    assert_eq!(result["state"], "committed");
    let mut equivalent = Session { project: before.clone(), media: s.media.clone(), ..Default::default() };
    equivalent.state.active_sequence = Some(sequence);
    equivalent
        .execute("timeline.trim", json!({"clip": clip.0, "edge": "out", "mode": "regular", "delta": -s.sequence_rate().tick_of(1).0, "linked": true}))
        .unwrap();
    assert_eq!(*s.project, *equivalent.project);
    assert_eq!(s.undo().as_deref(), Some("Scoped trim"));
    assert_eq!(*s.project, *before);
    s.shutdown();
}

#[test]
fn duplicate_preview_and_commit_return_existing_results_without_reapplying() {
    let (_dir, mut s, sequence, clip) = fixture();
    let client = s.authorize_editor("agent", scope(sequence, true)).unwrap();
    let params = request(&s, sequence, clip, "retry");
    let preview = s.execute_as(&client, "transaction.preview", params.clone()).unwrap();
    assert_eq!(s.execute_as(&client, "transaction.preview", params.clone()).unwrap(), preview);
    let result = s.execute_as(&client, "transaction.commit", reference(&s, &preview)).unwrap();
    let revision = s.revision;
    assert_eq!(s.execute_as(&client, "transaction.commit", reference(&s, &preview)).unwrap(), result);
    assert_eq!(s.execute_as(&client, "transaction.preview", params).unwrap(), result);
    assert_eq!(s.revision, revision);
    s.undo().unwrap();
    let undone = s.project.clone();
    assert_eq!(s.execute_as(&client, "transaction.commit", reference(&s, &preview)).unwrap(), result);
    assert!(Arc::ptr_eq(&s.project, &undone), "retry must not replay an already-undone edit");
    s.shutdown();
}

#[test]
fn changed_idempotency_payload_and_duplicate_operation_ids_are_rejected() {
    let (_dir, mut s, sequence, clip) = fixture();
    let client = s.authorize_editor("agent", scope(sequence, true)).unwrap();
    let params = request(&s, sequence, clip, "conflict");
    let preview = s.execute_as(&client, "transaction.preview", params.clone()).unwrap();
    for field in ["label", "operationId", "idempotencyKey"] {
        let mut changed = params.clone();
        changed[field] = json!("different");
        assert_eq!(s.execute_as(&client, "transaction.preview", changed).unwrap_err().code, ErrorCode::InvalidArgument);
    }
    s.execute_as(&client, "transaction.commit", reference(&s, &preview)).unwrap();
    let mut changed = params;
    changed["edits"][0]["delta"] = json!("-1");
    assert_eq!(s.execute_as(&client, "transaction.preview", changed).unwrap_err().code, ErrorCode::InvalidArgument);
    s.shutdown();
}

#[test]
fn scoped_clients_cannot_impersonate_actors_or_invoke_raw_mutations() {
    let (_dir, mut s, sequence, clip) = fixture();
    let read = s.authorize_editor("reader", scope(sequence, false)).unwrap();
    let edit = s.authorize_editor("editor", scope(sequence, true)).unwrap();
    for command in ["file.open", "file.import", "file.save", "timeline.trim", "prefs.set", "ui.menu.invoke"] {
        assert_eq!(s.execute_as(&edit, command, json!({})).unwrap_err().code, ErrorCode::PermissionDenied);
    }
    assert_eq!(s.execute_as(&read, "transaction.preview", request(&s, sequence, clip, "read")).unwrap_err().code, ErrorCode::PermissionDenied);
    let mut spoof = request(&s, sequence, clip, "spoof");
    spoof["actor"] = json!("local-owner");
    assert_eq!(s.execute_as(&edit, "transaction.preview", spoof).unwrap_err().code, ErrorCode::InvalidArgument);
    s.shutdown();
}

#[test]
fn stale_revision_cross_project_and_other_sequence_access_fail_closed() {
    let (_dir, mut s, sequence, clip) = fixture();
    let client = s.authorize_editor("agent", scope(sequence, true)).unwrap();
    let params = request(&s, sequence, clip, "stale");
    let preview = s.execute_as(&client, "transaction.preview", params).unwrap();
    s.execute("file.newSequence", json!({"name": "Outside scope"})).unwrap();
    let other = s.state.active_sequence.unwrap();
    assert_eq!(s.execute_as(&client, "sequence.inspect", json!({"item": other.0.to_string()})).unwrap_err().code, ErrorCode::PermissionDenied);
    assert_eq!(s.execute_as(&client, "transaction.commit", reference(&s, &preview)).unwrap_err().code, ErrorCode::RevisionConflict);
    s.execute("file.newProject", json!({})).unwrap();
    assert_eq!(s.execute_as(&client, "project.inspect", json!({})).unwrap_err().code, ErrorCode::PermissionDenied);
    s.shutdown();
}

#[test]
fn revocation_expiry_and_draft_ownership_are_rechecked_at_commit() {
    let (_dir, mut s, sequence, clip) = fixture();
    let client = s.authorize_editor("agent", scope(sequence, true)).unwrap();
    let other = s.authorize_editor("agent", scope(sequence, true)).unwrap();
    let preview = s.execute_as(&client, "transaction.preview", request(&s, sequence, clip, "revoke")).unwrap();
    assert_eq!(s.execute_as(&other, "transaction.commit", reference(&s, &preview)).unwrap_err().code, ErrorCode::PermissionDenied);
    let id: DocumentId = serde_json::from_value(preview["transactionId"].clone()).unwrap();
    s.service.drafts.get_mut(&id).unwrap().expires = Instant::now();
    assert_eq!(s.execute_as(&client, "transaction.commit", reference(&s, &preview)).unwrap_err().code, ErrorCode::Canceled);
    let preview = s.execute_as(&client, "transaction.preview", request(&s, sequence, clip, "revoke-again")).unwrap();
    client.revoke();
    assert_eq!(s.execute_as(&client, "transaction.commit", reference(&s, &preview)).unwrap_err().code, ErrorCode::PermissionDenied);
    s.service.clients.get_mut(&other.id).unwrap().expires = Instant::now();
    assert_eq!(s.execute_as(&other, "project.inspect", json!({})).unwrap_err().code, ErrorCode::PermissionDenied);
    s.shutdown();
}

#[test]
fn abort_and_cancellation_do_not_commit_or_consume_receipt_slots() {
    let (_dir, mut s, sequence, clip) = fixture();
    let client = s.authorize_editor("agent", scope(sequence, true)).unwrap();
    let before = s.project.clone();
    let preview = s.execute_as(&client, "transaction.preview", request(&s, sequence, clip, "abort")).unwrap();
    assert_eq!(s.execute_as(&client, "transaction.abort", reference(&s, &preview)).unwrap()["state"], "aborted");
    let preview = s.execute_as(&client, "transaction.preview", request(&s, sequence, clip, "cancel")).unwrap();
    let cancellation = Cancellation::default();
    cancellation.cancel();
    assert_eq!(s.execute_as_cancellable(&client, "transaction.commit", reference(&s, &preview), &cancellation).unwrap_err().code, ErrorCode::Canceled);
    assert!(Arc::ptr_eq(&s.project, &before));
    assert!(s.operation_records.is_empty());
    s.shutdown();
}

#[test]
fn operation_receipt_survives_crash_recovery_and_actor_reauthorization() {
    let (dir, mut s, sequence, clip) = fixture();
    let client = s.authorize_editor("agent", scope(sequence, true)).unwrap();
    let params = request(&s, sequence, clip, "recover");
    let preview = s.execute_as(&client, "transaction.preview", params.clone()).unwrap();
    let result = s.execute_as(&client, "transaction.commit", reference(&s, &preview)).unwrap();
    let expected = s.project.clone();
    s.persistence.take().unwrap().simulate_crash();
    let mut restored = Session::default();
    restored.start_autosave(crate::autosave::AutosaveConfig::new(dir.path())).unwrap();
    restored.execute("file.recover", json!({})).unwrap();
    let client = restored.authorize_editor("agent", scope(sequence, true)).unwrap();
    let revision = restored.revision;
    assert_eq!(restored.execute_as(&client, "transaction.preview", params).unwrap(), result);
    assert_eq!(*restored.project, *expected);
    assert_eq!(restored.revision, revision);
    let operation =
        restored.execute_as(&client, "operation.inspect", json!({"apiVersion": "1", "projectId": restored.document_id, "operationId": "op-recover"})).unwrap();
    assert_eq!(operation["result"], result);
    restored.shutdown();
    s.shutdown();
}

#[test]
fn crash_after_commit_marker_preserves_receipt_before_live_publication() {
    let (dir, mut s, sequence, clip) = fixture();
    let client = s.authorize_editor("agent", scope(sequence, true)).unwrap();
    let params = request(&s, sequence, clip, "receipt-window");
    let preview = s.execute_as(&client, "transaction.preview", params.clone()).unwrap();
    crate::document::CRASH_AFTER_COMMIT.with(|fault| fault.set(true));
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.execute_as(&client, "transaction.commit", reference(&s, &preview)))).is_err());
    s.persistence.take().unwrap().simulate_crash();
    let mut recovered = Session::default();
    recovered.start_autosave(crate::autosave::AutosaveConfig::new(dir.path())).unwrap();
    recovered.execute("file.recover", json!({})).unwrap();
    let access = recovered.authorize_editor("agent", scope(sequence, true)).unwrap();
    let before = recovered.project.clone();
    let result = recovered.execute_as(&access, "transaction.preview", params).unwrap();
    assert_eq!(result["state"], "committed");
    assert_eq!(result["transactionId"], preview["transactionId"]);
    assert!(Arc::ptr_eq(&recovered.project, &before));
    recovered.shutdown();
    s.shutdown();
}

#[test]
fn wire_ids_and_times_are_exact_and_preview_capacity_is_bounded() {
    let (_dir, mut s, sequence, clip) = fixture();
    let client = s.authorize_editor("agent", scope(sequence, true)).unwrap();
    let params = request(&s, sequence, clip, "wire");
    assert!(params["sequenceId"].is_string());
    assert!(params["edits"][0]["clip"].is_string());
    let mut invalid = params;
    invalid["sequenceId"] = json!(sequence.0);
    assert_eq!(s.execute_as(&client, "transaction.preview", invalid).unwrap_err().code, ErrorCode::InvalidArgument);
    for i in 0..8 {
        s.execute_as(&client, "transaction.preview", request(&s, sequence, clip, &format!("slot-{i}"))).unwrap();
    }
    assert_eq!(s.execute_as(&client, "transaction.preview", request(&s, sequence, clip, "overflow")).unwrap_err().code, ErrorCode::ResourceLimit);
    s.shutdown();
}

fn audio_fixture(root: &std::path::Path) -> (Session, ItemId, ClipId, PathBuf) {
    let path = root.join("take.wav");
    std::fs::write(&path, shotclutch_media::wav::write_wav16(&vec![0.1; 2 * 1024 * 1024], 1, 48_000)).unwrap();
    let mut s = Session::default();
    let imported = s.execute("file.import", json!({"paths": [path.to_str().unwrap()]})).unwrap();
    s.execute("file.newSequence", json!({"video": 1, "audio": 1})).unwrap();
    s.execute("timeline.place", json!({"item": imported["items"][0], "frame": 0, "duration": s.sequence_rate().tick_of(48).0})).unwrap();
    let sequence = s.state.active_sequence.unwrap();
    let clip = s.project.sequence(sequence).unwrap().audio_tracks[0].items[0].id;
    s.start_autosave(crate::autosave::AutosaveConfig::new(root.join("data"))).unwrap();
    (s, sequence, clip, path)
}

#[test]
fn source_folder_grants_and_read_budgets_are_enforced_before_commit() {
    let dir = tempfile::tempdir().unwrap();
    let (mut s, sequence, clip, _) = audio_fixture(dir.path());
    let before = s.project.clone();
    let ungranted = s.authorize_editor("ungranted", scope(sequence, true)).unwrap();
    assert_eq!(s.execute_as(&ungranted, "transaction.preview", request(&s, sequence, clip, "no-root")).unwrap_err().code, ErrorCode::PermissionRequired);
    let mut limited = scope(sequence, true);
    limited.media_roots.push(dir.path().into());
    limited.max_source_bytes = 1;
    let limited = s.authorize_editor("limited", limited).unwrap();
    assert_eq!(s.execute_as(&limited, "transaction.preview", request(&s, sequence, clip, "budget")).unwrap_err().code, ErrorCode::ResourceLimit);
    let mut granted = scope(sequence, true);
    granted.media_roots.push(dir.path().into());
    let granted = s.authorize_editor("granted", granted).unwrap();
    let preview = s.execute_as(&granted, "transaction.preview", request(&s, sequence, clip, "source")).unwrap();
    assert_eq!(preview["sourceCount"], 1);
    assert!(Arc::ptr_eq(&s.project, &before));
    s.execute_as(&granted, "transaction.commit", reference(&s, &preview)).unwrap();
    s.shutdown();
}

#[test]
fn same_size_middle_of_file_changes_invalidate_preview_even_when_fast_identity_matches() {
    use std::io::{Seek, SeekFrom, Write};
    let dir = tempfile::tempdir().unwrap();
    let (mut s, sequence, clip, path) = audio_fixture(dir.path());
    let mut access = scope(sequence, true);
    access.media_roots.push(dir.path().into());
    let access = s.authorize_editor("source-check", access).unwrap();
    let preview = s.execute_as(&access, "transaction.preview", request(&s, sequence, clip, "middle")).unwrap();
    let fast = crate::relink::identity_of(&*s.services, path.to_str().unwrap()).unwrap();
    let mut file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.seek(SeekFrom::Start(2 * 1024 * 1024)).unwrap();
    file.write_all(&[0x7f]).unwrap();
    file.sync_all().unwrap();
    drop(file);
    assert_eq!(
        crate::relink::identity_of(&*s.services, path.to_str().unwrap()).unwrap(),
        fast,
        "the changed sample is outside the inherited head/tail fingerprint"
    );
    let before = s.project.clone();
    assert_eq!(s.execute_as(&access, "transaction.commit", reference(&s, &preview)).unwrap_err().code, ErrorCode::MediaChanged);
    assert!(Arc::ptr_eq(&s.project, &before));
    assert!(s.operation_records.is_empty());
    s.shutdown();
}

#[test]
fn traversal_and_directory_link_escapes_cannot_read_sources_outside_granted_root() {
    let dir = tempfile::tempdir().unwrap();
    let allowed = dir.path().join("allowed");
    let outside = dir.path().join("outside");
    std::fs::create_dir(&allowed).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let (mut s, sequence, clip, path) = audio_fixture(&outside);
    let mut access = scope(sequence, true);
    access.media_roots.push(allowed.clone());
    let access = s.authorize_editor("confined", access).unwrap();
    let item = s.project.sequence(sequence).unwrap().find_item(clip).unwrap().1.item;
    let stored = "../outside/take.wav";
    s.edit("Relative path", |p, _| {
        if let shotclutch_project::ItemKind::Media(media) = &mut p.items.get_mut(&item).unwrap().kind {
            media.media = shotclutch_project::MediaRef::File { path: stored.into() };
        }
        Ok(())
    })
    .unwrap();
    s.media.add_stored(&s.project, Some(allowed.join("cut.scproj").to_str().unwrap()));
    assert_eq!(s.execute_as(&access, "transaction.preview", request(&s, sequence, clip, "traversal")).unwrap_err().code, ErrorCode::PermissionRequired);
    let link = allowed.join("linked");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let result = std::process::Command::new("cmd").args(["/C", "mklink", "/J"]).arg(&link).arg(&outside).creation_flags(0x08000000).output().unwrap();
        assert!(result.status.success(), "could not create junction fixture: {}", String::from_utf8_lossy(&result.stderr));
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    s.edit("Linked path", |p, _| {
        if let shotclutch_project::ItemKind::Media(media) = &mut p.items.get_mut(&item).unwrap().kind {
            media.media = shotclutch_project::MediaRef::File { path: link.join("take.wav").to_string_lossy().into_owned() };
        }
        Ok(())
    })
    .unwrap();
    assert!(path.is_file());
    assert!(matches!(
        s.execute_as(&access, "transaction.preview", request(&s, sequence, clip, "link")).unwrap_err().code,
        ErrorCode::PermissionDenied | ErrorCode::PermissionRequired
    ));
    s.shutdown();
}

#[test]
fn receipt_capacity_does_not_evict_unexpired_operations() {
    let (_dir, mut s, sequence, clip) = fixture();
    let client = s.authorize_editor("agent", scope(sequence, true)).unwrap();
    for i in 0..128 {
        s.operation_records.push(OperationRecord {
            actor: "peer".into(),
            operation_id: format!("op-{i}"),
            idempotency_key: format!("key-{i}"),
            request: "0".repeat(64),
            sequence: sequence.0,
            retained_until: crate::autosave::unix_now() + 3600,
            result: json!({"apiVersion": "1", "transactionId": DocumentId::default(), "projectId": s.document_id, "operationId": format!("op-{i}"), "state": "committed", "priorRevision": "1", "revision": "2", "sequenceId": sequence.0.to_string(), "retainedUntil": crate::autosave::unix_now() + 3600}),
        });
    }
    let preview = s.execute_as(&client, "transaction.preview", request(&s, sequence, clip, "full")).unwrap();
    let before = s.project.clone();
    assert_eq!(s.execute_as(&client, "transaction.commit", reference(&s, &preview)).unwrap_err().code, ErrorCode::ResourceLimit);
    assert_eq!(s.operation_records.len(), 128);
    assert!(Arc::ptr_eq(&s.project, &before));
    for record in &mut s.operation_records {
        record.retained_until = 1;
    }
    s.execute_as(&client, "transaction.commit", reference(&s, &preview)).unwrap();
    assert_eq!(s.operation_records.len(), 1);
    s.shutdown();
}

#[test]
fn grants_and_authorization_handles_never_enter_project_files() {
    let dir = tempfile::tempdir().unwrap();
    let private = dir.path().join("unused-private-root");
    std::fs::create_dir(&private).unwrap();
    let (_data, mut s, sequence, _) = fixture();
    let mut scope = scope(sequence, true);
    scope.media_roots.push(private.clone());
    let client = s.authorize_editor("agent", scope).unwrap();
    let bytes = s.document_snapshot().encode(None, false).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(!text.contains(&client.id.to_string()));
    assert!(!text.contains("unused-private-root"));
    s.revoke_editor(&client);
    assert_eq!(s.execute_as(&client, "project.inspect", json!({})).unwrap_err().code, ErrorCode::PermissionDenied);
    s.shutdown();
}

#[test]
fn scoped_queries_keep_exact_ids_and_times_and_catalog_effective_permissions() {
    let (_dir, mut s, sequence, _) = fixture();
    let read = s.authorize_editor("reader", scope(sequence, false)).unwrap();
    let query = s.execute_as(&read, "sequence.inspect", json!({"item": sequence.0.to_string()})).unwrap();
    assert_eq!(query["observedRevision"], s.revision.to_string());
    assert_eq!(query["id"], sequence.0.to_string());
    assert!(query["duration"].is_string());
    let clip = &query["video"][0]["items"][0];
    for field in ["clip", "item", "start", "end", "duration", "sourceIn", "sourceOut"] {
        assert!(clip[field].is_string(), "{field}");
    }
    let catalog = s.execute_as(&read, "command.list", json!({})).unwrap();
    assert_eq!(catalog.as_array().unwrap().iter().find(|c| c["id"] == "transaction.commit").unwrap()["enabled"], false);
    assert!(!catalog.to_string().contains("file.save"));
    s.shutdown();
    assert_eq!(s.execute_as(&read, "project.inspect", json!({})).unwrap_err().code, ErrorCode::PermissionDenied);
}

#[test]
fn invalid_recovered_receipts_are_rejected_before_project_replacement() {
    let (_dir, mut s, sequence, clip) = fixture();
    let access = s.authorize_editor("agent", scope(sequence, true)).unwrap();
    let preview = s.execute_as(&access, "transaction.preview", request(&s, sequence, clip, "receipt-shape")).unwrap();
    s.execute_as(&access, "transaction.commit", reference(&s, &preview)).unwrap();
    let good = s.operation_records.clone();
    validate_records(&good, s.document_id).unwrap();
    for field in ["transactionId", "priorRevision", "revision", "sequenceId", "retainedUntil", "apiVersion"] {
        let mut bad = good.clone();
        bad[0].result[field] = Value::Null;
        assert!(validate_records(&bad, s.document_id).is_err(), "{field}");
    }
    s.shutdown();
}

#[test]
fn missing_source_items_cannot_receive_successful_structural_verification() {
    let (_dir, mut s, sequence, clip) = fixture();
    let access = s.authorize_editor("agent", scope(sequence, true)).unwrap();
    s.edit("Damaged source reference", |p, _| {
        p.sequence_mut(sequence).unwrap().video_tracks[0].items[0].item = ItemId(u64::MAX - 5);
        Ok(())
    })
    .unwrap();
    assert_eq!(s.execute_as(&access, "transaction.preview", request(&s, sequence, clip, "missing-item")).unwrap_err().code, ErrorCode::MediaChanged);
    s.shutdown();
}

#[test]
fn numbered_image_sources_require_complete_source_manifests() {
    let dir = tempfile::tempdir().unwrap();
    let (mut s, sequence, clip, _) = audio_fixture(dir.path());
    let item = s.project.sequence(sequence).unwrap().find_item(clip).unwrap().1.item;
    s.edit("Image sequence metadata", |p, _| {
        if let shotclutch_project::ItemKind::Media(media) = &mut p.items.get_mut(&item).unwrap().kind {
            media.info.kind = shotclutch_media::MediaKind::ImageSequence;
        }
        Ok(())
    })
    .unwrap();
    let mut grant = scope(sequence, true);
    grant.media_roots.push(dir.path().into());
    let access = s.authorize_editor("agent", grant).unwrap();
    assert_eq!(s.execute_as(&access, "transaction.preview", request(&s, sequence, clip, "image-manifest")).unwrap_err().code, ErrorCode::CapabilityUnavailable);
    s.shutdown();
}
