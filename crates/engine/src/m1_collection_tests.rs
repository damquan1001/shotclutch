use super::*;
use shotclutch_time::FrameRate;

struct Hook;
impl Hook {
    fn set(f: impl FnMut(Point, &Progress) -> std::result::Result<(), String> + 'static) -> Self {
        FAULT.with(|slot| *slot.borrow_mut() = Some(Box::new(f)));
        Self
    }
}
impl Drop for Hook {
    fn drop(&mut self) {
        FAULT.with(|slot| slot.borrow_mut().take());
    }
}
struct Fixture {
    s: Session,
    dir: tempfile::TempDir,
    source: PathBuf,
    bytes: Vec<u8>,
    item: ItemId,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("original.wav");
        let bytes = shotclutch_media::wav::write_wav16(&vec![0.125; 48_000], 1, 48_000);
        fs::write(&source, &bytes).unwrap();
        let mut s = Session::default();
        let item = ItemId(s.execute("file.import", json!({"paths": [source]})).unwrap()["items"][0].as_u64().unwrap());
        s.execute("file.newSequence", json!({"video": 1, "audio": 1, "fps": 24})).unwrap();
        s.execute("timeline.place", json!({"item": item.0, "time": 0, "duration": shotclutch_time::TICKS_PER_SECOND})).unwrap();
        s.execute("file.save", json!({"path": dir.path().join("source.scproj")})).unwrap();
        Self { s, dir, source, bytes, item }
    }
    fn dest(&self) -> PathBuf {
        self.dir.path().join("Collected")
    }
    fn params(&self, extra: Value) -> Value {
        let mut p = json!({"destination": self.dest(), "wait": true});
        p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        p
    }
    fn collect(&mut self, extra: Value) -> Result<Value> {
        let p = self.params(extra);
        self.s.execute("file.projectManager", p)
    }
    fn clean(&self) {
        assert!(!fs::read_dir(self.dir.path()).unwrap().any(|p| p.unwrap().file_name().to_string_lossy().starts_with(".shotclutch-collect-")));
    }
}
fn manifest(path: &Path) -> Manifest {
    serde_json::from_slice(&fs::read(path.join(MANIFEST)).unwrap()).unwrap()
}
fn put_manifest(path: &Path, m: &Manifest) {
    fs::write(path.join(MANIFEST), serde_json::to_vec_pretty(m).unwrap()).unwrap();
}
fn render(s: &mut Session, path: &Path) -> Vec<u8> {
    s.execute("file.exportMedia", json!({"path": path, "format": "wav", "wait": true})).unwrap();
    fs::read(path).unwrap()
}

#[test]
fn m1_collection_relocates_without_originals_and_preserves_exact_timeline_audio() {
    let mut f = Fixture::new();
    let original = f.s.project.clone();
    let doc = f.s.document_id;
    let revision = f.s.revision;
    let before = render(&mut f.s, &f.dir.path().join("before.wav"));
    let out = f.collect(json!({})).unwrap();
    assert_eq!(out["state"], "committed");
    assert_eq!(out["documentMode"], "derivedCopy");
    assert_ne!(out["projectId"], json!(doc));
    assert!(Arc::ptr_eq(&original, &f.s.project));
    assert_eq!((f.s.document_id, f.s.revision), (doc, revision));
    let m = manifest(&f.dest());
    assert_eq!(m.assets[0].sha256, digest(&f.bytes));
    assert_eq!(m.assets[0].role, "original");
    assert!(relative(&m.assets[0].path));
    assert!(!fs::read_to_string(f.dest().join(MANIFEST)).unwrap().contains(f.source.to_str().unwrap()));
    let relocated = tempfile::tempdir().unwrap();
    let moved = relocated.path().join("Different-root");
    fs::rename(f.dest(), &moved).unwrap();
    f.s.media.clear();
    fs::remove_file(&f.source).unwrap();
    let report = f.s.execute("file.verifyCollection", json!({"path": moved})).unwrap();
    assert_eq!(report["verified"], true);
    let mut opened = Session::default();
    let result = opened.execute("file.open", json!({"path": report["project"]})).unwrap();
    assert_eq!(result["missingMedia"], 0);
    assert_eq!(opened.history.undo.len(), 0);
    assert_eq!(before, render(&mut opened, &relocated.path().join("after.wav")));
    f.clean();
}

#[test]
fn m1_collection_full_document_rewrites_history_and_preserves_undo_redo_identity() {
    let mut f = Fixture::new();
    // A historical source absent from the current project is still essential for undo.
    let old = f.dir.path().join("historical.wav");
    let bytes = shotclutch_media::wav::write_wav16(&vec![0.25; 48_000], 1, 48_000);
    fs::write(&old, bytes).unwrap();
    let mut history = (*f.s.project).clone();
    if let ItemKind::Media(m) = &mut history.items.get_mut(&f.item).unwrap().kind {
        m.media = MediaRef::File { path: old.to_string_lossy().into_owned() };
        m.identity = Some(crate::relink::identity_of(&crate::FsServices, old.to_str().unwrap()).unwrap());
    }
    history.notes = "historical state".into();
    f.s.history.undo.push(("Historical take".into(), Arc::new(history.clone())));
    f.s.history.redo.push(("Redo take".into(), Arc::new(history)));
    let doc = f.s.document_id;
    let revision = f.s.revision;
    let undo = f.s.history.undo.len();
    let out = f.collect(json!({"excludeUnused": false})).unwrap();
    assert_eq!(out["documentMode"], "fullDocument");
    assert_eq!(out["projectId"], json!(doc));
    let m = manifest(&f.dest());
    assert_eq!(m.assets.len(), 2);
    assert_eq!(m.undo_states, undo);
    let moved = f.dir.path().join("Moved");
    fs::rename(f.dest(), &moved).unwrap();
    f.s.media.clear();
    fs::remove_file(old).unwrap();
    fs::remove_file(&f.source).unwrap();
    let mut opened = Session::default();
    opened.execute("file.open", json!({"path": moved.join(&m.project)})).unwrap();
    assert_eq!((opened.document_id, opened.revision), (doc, revision));
    assert_eq!(opened.history.undo.len(), undo);
    assert_eq!(opened.history.redo.len(), 1);
    assert_eq!(opened.undo().as_deref(), Some("Historical take"));
    assert_eq!(opened.project.notes, "historical state");
    let output = render(&mut opened, &f.dir.path().join("undo.wav"));
    let audio = shotclutch_codecs::open_bytes("undo.wav", output.into()).unwrap();
    assert!((audio.audio(0, 48_000, 48_000).unwrap().peaks()[0] - 0.25).abs() < 0.001);
    opened.redo().unwrap();
    assert_ne!(opened.project.notes, "historical state");
    verify(&moved, &Progress::default()).unwrap();
}

#[test]
fn m1_collection_idempotent_retry_uses_verified_receipt_even_after_source_disappears() {
    let mut f = Fixture::new();
    let operation = DocumentId::default();
    let out = f.collect(json!({"operationId": operation})).unwrap();
    f.s.media.clear();
    fs::remove_file(&f.source).unwrap();
    let retry = f.collect(json!({"operationId": operation})).unwrap();
    assert_eq!(retry["replayed"], true);
    assert_eq!(out["project"], retry["project"]);
    assert_eq!(f.s.jobs.len(), 1);
    assert!(f.collect(json!({"operationId": operation, "includeProxies": false})).unwrap_err().to_string().contains("OPERATION_CONFLICT"));
    assert!(f.collect(json!({"operationId": DocumentId::default()})).is_err());
}

#[test]
fn m1_collection_tampered_assets_or_project_do_not_pass_receipt_replay() {
    let mut f = Fixture::new();
    let operation = DocumentId::default();
    f.collect(json!({"operationId": operation})).unwrap();
    let m = manifest(&f.dest());
    fs::write(f.dest().join(&m.assets[0].path), b"other take").unwrap();
    assert!(verify(&f.dest(), &Progress::default()).unwrap_err().contains("hash mismatch"));
    assert!(f.collect(json!({"operationId": operation, "overwrite": true})).is_err());
    fs::write(f.dest().join(&m.assets[0].path), &f.bytes).unwrap();
    fs::write(f.dest().join(&m.project), b"corrupt project").unwrap();
    assert!(verify(&f.dest(), &Progress::default()).unwrap_err().contains("project hash"));
}

#[test]
fn m1_collection_rejects_traversal_and_platform_specific_names_before_writing() {
    let mut f = Fixture::new();
    for name in ["../escape", "x/y", "x\\y", "C:ads", "CON", "nul.scproj", "trailing.", "", "x\n"] {
        assert!(f.collect(json!({"projectName": name})).is_err(), "{name}");
        assert!(!f.dest().exists());
        assert!(f.collect(json!({"projectName": name, "mode": "consolidate"})).is_err(), "{name}");
    }
    f.clean();
}

#[test]
fn m1_collection_manifest_rejects_escapes_duplicate_paths_and_unlisted_references() {
    let mut f = Fixture::new();
    f.collect(json!({})).unwrap();
    let original = manifest(&f.dest());
    for path in ["../original.wav", "Media/../original.wav", "C:/original.wav", "\\\\server\\share\\a", "Media/source.wav:ads", "/original.wav"] {
        let mut m = original.clone();
        m.assets[0].path = path.into();
        put_manifest(&f.dest(), &m);
        assert!(verify(&f.dest(), &Progress::default()).is_err(), "{path}");
    }
    let mut m = original.clone();
    m.assets.push(m.assets[0].clone());
    put_manifest(&f.dest(), &m);
    assert!(verify(&f.dest(), &Progress::default()).unwrap_err().contains("colliding"));
    let mut m = original;
    m.assets.clear();
    put_manifest(&f.dest(), &m);
    assert!(verify(&f.dest(), &Progress::default()).unwrap_err().contains("unlisted"));
}

#[test]
fn m1_collection_cancellation_and_copy_failure_publish_nothing_and_clean_stage() {
    for point in [Point::Copied, Point::Verified] {
        let mut f = Fixture::new();
        let _hook = Hook::set(move |p, progress| {
            if p == point {
                progress.cancel.store(true, Ordering::Release);
            }
            Ok(())
        });
        assert!(f.collect(json!({})).unwrap_err().to_string().contains("CANCELED"));
        assert!(!f.dest().exists());
        assert_eq!(fs::read(&f.source).unwrap(), f.bytes);
        f.clean();
    }
    let mut f = Fixture::new();
    let _hook = Hook::set(|p, _| if p == Point::Copied { Err("injected storage failure".into()) } else { Ok(()) });
    assert!(f.collect(json!({})).is_err());
    assert!(!f.dest().exists());
    f.clean();
}

#[test]
fn m1_collection_concurrent_destination_is_never_replaced_even_when_empty() {
    for empty in [true, false] {
        let mut f = Fixture::new();
        let destination = f.dest();
        let _hook = Hook::set(move |p, _| {
            if p == Point::Verified {
                fs::create_dir(&destination).unwrap();
                if !empty {
                    fs::write(destination.join("owner.txt"), b"keep").unwrap();
                }
            }
            Ok(())
        });
        assert!(f.collect(json!({"overwrite": true})).is_err());
        assert_eq!(fs::read_dir(f.dest()).unwrap().count(), if empty { 0 } else { 1 });
        if !empty {
            assert_eq!(fs::read(f.dest().join("owner.txt")).unwrap(), b"keep");
        }
        f.clean();
    }
}

#[test]
fn m1_collection_source_change_after_copy_aborts_without_publication() {
    let mut f = Fixture::new();
    let source = f.source.clone();
    let _hook = Hook::set(move |p, _| {
        if p == Point::Copied {
            fs::write(&source, b"changed").unwrap();
        }
        Ok(())
    });
    assert!(f.collect(json!({})).unwrap_err().to_string().contains("MEDIA_CHANGED"));
    assert!(!f.dest().exists());
    f.clean();
}

#[test]
fn m1_collection_missing_original_is_explicit_and_dry_run_has_no_writes() {
    let mut f = Fixture::new();
    let out = f.collect(json!({"dryRun": true})).unwrap();
    assert_eq!(out["state"], "planned");
    assert!(!f.dest().exists());
    assert!(f.s.jobs.is_empty());
    f.s.media.clear();
    fs::remove_file(&f.source).unwrap();
    assert!(f.collect(json!({})).unwrap_err().to_string().contains("MEDIA_CHANGED"));
    assert!(!f.dest().exists());
    f.clean();
}

#[test]
fn m1_collection_unsafe_history_selection_and_bad_sequences_are_rejected() {
    let mut f = Fixture::new();
    for options in
        [json!({"includeHistory": true}), json!({"sequences": "all"}), json!({"sequences": ["1"]}), json!({"sequences": []}), json!({"sequences": [999999]})]
    {
        assert!(f.collect(options).is_err());
        assert!(!f.dest().exists());
    }
    assert!(f.collect(json!({"includePreviews": true})).unwrap_err().to_string().contains("CAPABILITY_UNAVAILABLE"));
}

#[test]
fn m1_collection_owned_original_is_essential_even_without_a_disk_file() {
    let mut f = Fixture::new();
    let virtual_path = f.dir.path().join("virtual.wav");
    let item = crate::commands::import_bytes(&mut f.s, virtual_path.to_str().unwrap(), f.bytes.clone().into(), None).unwrap();
    f.s.execute("timeline.place", json!({"item": item.0, "time": FrameRate::FPS_24.tick_of(24).0, "duration": shotclutch_time::TICKS_PER_SECOND})).unwrap();
    let out = f.collect(json!({})).unwrap();
    let m = manifest(&f.dest());
    let owned = m.assets.iter().find(|a| a.role == "ownedOriginal").unwrap();
    assert_eq!(owned.sha256, digest(&f.bytes));
    assert_eq!(fs::read(f.dest().join(&owned.path)).unwrap(), f.bytes);
    assert!(!virtual_path.exists());
    let mut opened = Session::default();
    assert_eq!(opened.execute("file.open", json!({"path": out["project"]})).unwrap()["missingMedia"], 0);
}

#[test]
fn m1_collection_image_sequence_keeps_all_members_holes_and_playback_after_relocation() {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("Stills");
    fs::create_dir(&original).unwrap();
    let first = original.join("take001.png");
    let last = original.join("take003.png");
    image::RgbaImage::from_pixel(32, 18, image::Rgba([255u8, 0, 0, 255])).save(&first).unwrap();
    image::RgbaImage::from_pixel(32, 18, image::Rgba([0u8, 255, 0, 255])).save(&last).unwrap();
    let mut s = Session::default();
    let item = s.execute("file.import", json!({"paths": [first], "imageSequence": true})).unwrap()["items"][0].clone();
    s.execute("file.newSequence", json!({"video": 1, "audio": 1, "width": 32, "height": 18})).unwrap();
    s.execute("timeline.place", json!({"item": item, "time": 0, "duration": FrameRate::FPS_24.tick_of(3).0})).unwrap();
    let before: Vec<_> = (0..3).map(|f| crate::media_test_util::frame_rgba(&mut s, f, 1.0).2).collect();
    let dest = dir.path().join("Collected");
    s.execute("file.projectManager", json!({"destination": dest, "projectName": "Images", "wait": true})).unwrap();
    let m = manifest(&dest);
    assert_eq!(m.assets.len(), 2);
    let collected_first = m.assets.iter().find(|a| a.path.ends_with("take001.png")).unwrap();
    let frames = crate::media_pool::image_sequence_frames(dest.join(&collected_first.path).to_str().unwrap(), &crate::FsServices).unwrap();
    assert_eq!(frames.len(), 3);
    assert!(frames[1].is_none());
    let moved = dir.path().join("Moved");
    fs::rename(&dest, &moved).unwrap();
    s.media.clear();
    fs::rename(&original, dir.path().join("Originals-hidden")).unwrap();
    verify(&moved, &Progress::default()).unwrap();
    let mut opened = Session::default();
    assert_eq!(opened.execute("file.open", json!({"path": moved.join(&m.project)})).unwrap()["missingMedia"], 0);
    let after: Vec<_> = (0..3).map(|f| crate::media_test_util::frame_rgba(&mut opened, f, 1.0).2).collect();
    assert_eq!(before, after);
    let mut mislabeled = m.clone();
    let member = mislabeled.assets.iter_mut().find(|a| a.path.ends_with("take003.png")).unwrap();
    member.role = "proxy".into();
    member.essential = false;
    put_manifest(&moved, &mislabeled);
    assert!(verify(&moved, &Progress::default()).unwrap_err().contains("role/source"));
    put_manifest(&moved, &m);
    // Adding a matching frame would change playback even though all original hashes still match.
    let missing = moved.join(&collected_first.path).parent().unwrap().join("take002.png");
    fs::copy(moved.join(&collected_first.path), missing).unwrap();
    assert!(verify(&moved, &Progress::default()).unwrap_err().contains("unlisted"));
}

#[test]
fn m1_collection_image_membership_change_during_copy_aborts() {
    let mut f = Fixture::new();
    let first = f.dir.path().join("take001.png");
    image::RgbaImage::from_pixel(8, 8, image::Rgba([255u8, 0, 0, 255])).save(&first).unwrap();
    let item = f.s.execute("file.import", json!({"paths": [first], "imageSequence": true})).unwrap()["items"][0].clone();
    f.s.execute("timeline.place", json!({"item": item, "time": 0, "duration": FrameRate::FPS_24.tick_of(1).0})).unwrap();
    let new_frame = f.dir.path().join("take002.png");
    let _hook = Hook::set(move |p, _| {
        if p == Point::Copied {
            image::RgbaImage::from_pixel(8, 8, image::Rgba([0u8, 255, 0, 255])).save(&new_frame).unwrap();
        }
        Ok(())
    });
    assert!(f.collect(json!({})).unwrap_err().to_string().contains("membership changed"));
    assert!(!f.dest().exists());
    f.clean();
}

#[test]
fn m1_collection_same_named_sources_proxies_and_generated_settings_are_portable() {
    let mut f = Fixture::new();
    let other = f.dir.path().join("Other");
    fs::create_dir(&other).unwrap();
    let other_file = other.join("original.wav");
    let different = shotclutch_media::wav::write_wav16(&vec![0.4; 48_000], 1, 48_000);
    fs::write(&other_file, &different).unwrap();
    let other_id = f.s.execute("file.import", json!({"paths": [other_file]})).unwrap()["items"][0].clone();
    f.s.execute("timeline.place", json!({"item": other_id, "time": shotclutch_time::TICKS_PER_SECOND, "duration": shotclutch_time::TICKS_PER_SECOND})).unwrap();
    let mut project = (*f.s.project).clone();
    let ItemKind::Media(media) = &mut project.items.get_mut(&f.item).unwrap().kind else { panic!() };
    media.proxy = Some(MediaRef::File { path: other_file.to_string_lossy().into_owned() });
    project.settings.ingest.enabled = true;
    project.settings.ingest.destination = Some("X:/private/Ingest".into());
    project.settings.scratch.auto_save = Some("X:/private/Recovery".into());
    project.luts.push(shotclutch_project::ProjectLut {
        id: "embedded".into(),
        name: "Embedded LUT".into(),
        source_path: Some("X:/private/look.cube".into()),
        format: "cube".into(),
        text: "LUT_1D_SIZE 2\n0 0 0\n1 1 1\n".into(),
    });
    project.narrations.insert(
        f.item,
        shotclutch_project::Narration {
            text: "Generated narration".into(),
            language: "en-US".into(),
            voice: "local".into(),
            pitch: Default::default(),
            pace: 1.0,
            speech_duration: shotclutch_time::Tick(shotclutch_time::TICKS_PER_SECOND),
        },
    );
    project.notes = "Keep my authored notes".into();
    f.s.project = Arc::new(project);
    let original = f.s.project.clone();
    f.collect(json!({})).unwrap();
    let m = verify(&f.dest(), &Progress::default()).unwrap();
    assert_eq!(m.assets.len(), 3);
    assert_eq!(m.assets.iter().filter(|a| a.role == "proxy" && !a.essential).count(), 1);
    assert_eq!(m.assets.iter().filter(|a| a.role == "generatedOriginal" && a.essential).count(), 1);
    assert!(m.assets.iter().any(|a| a.sha256 == digest(&f.bytes)));
    assert!(m.assets.iter().any(|a| a.sha256 == digest(&different)));
    let loaded = shotclutch_format::decode(&fs::read(f.dest().join(m.project)).unwrap()).unwrap();
    assert_eq!(loaded.project.notes, "Keep my authored notes");
    assert_eq!(loaded.project.luts[0].text, original.luts[0].text);
    assert_eq!(loaded.project.settings.scratch, Default::default());
    assert!(!loaded.project.settings.ingest.enabled);
    assert!(Arc::ptr_eq(&original, &f.s.project));
}

#[test]
fn m1_collection_missing_requested_proxy_requires_explicit_omission() {
    let mut f = Fixture::new();
    let mut project = (*f.s.project).clone();
    let ItemKind::Media(media) = &mut project.items.get_mut(&f.item).unwrap().kind else { panic!() };
    media.proxy = Some(MediaRef::File { path: f.dir.path().join("missing-proxy.wav").to_string_lossy().into_owned() });
    f.s.project = Arc::new(project);
    assert!(f.collect(json!({})).is_err());
    assert!(!f.dest().exists());
    let out = f.collect(json!({"includeProxies": false})).unwrap();
    let loaded = shotclutch_format::decode(&fs::read(out["project"].as_str().unwrap()).unwrap()).unwrap();
    assert!(loaded.project.item(f.item).unwrap().as_media().unwrap().proxy.is_none());
    assert_eq!(manifest(&f.dest()).assets.len(), 1);
}

#[test]
fn m1_collection_conflicting_historical_identity_fails_before_writes() {
    let mut f = Fixture::new();
    let mut project = (*f.s.project).clone();
    let ItemKind::Media(media) = &mut project.items.get_mut(&f.item).unwrap().kind else { panic!() };
    media.identity.as_mut().unwrap().fingerprint ^= 1;
    f.s.history.undo.push(("Different take".into(), Arc::new(project)));
    assert!(f.collect(json!({"excludeUnused": false})).unwrap_err().to_string().contains("MEDIA_CHANGED"));
    assert!(!f.dest().exists());
    f.clean();
}

#[test]
fn m1_collection_full_document_keeps_durable_operation_receipts_but_grants_no_authority() {
    use crate::service::{EditRequest, EditorScope};
    use crate::transactions::{Edge, TimelineEdit, TrimMode};
    let mut f = Fixture::new();
    f.s.start_autosave(crate::autosave::AutosaveConfig::new(f.dir.path().join("Recovery"))).unwrap();
    let sequence = f.s.state.active_sequence.unwrap();
    let clip = f.s.project.sequence(sequence).unwrap().audio_tracks[0].items[0].id;
    let mut scope = EditorScope::sequence(sequence);
    scope.allow_edits = true;
    scope.media_roots.push(f.dir.path().to_path_buf());
    let client = f.s.authorize_editor("collection-test", scope).unwrap();
    let request = EditRequest {
        api_version: "1".into(),
        operation_id: "collect-operation".into(),
        idempotency_key: "collect-key".into(),
        project_id: f.s.document_id,
        expected_revision: f.s.revision,
        sequence_id: sequence,
        label: "Portable trim".into(),
        edits: vec![TimelineEdit::Trim { clip, edge: Edge::Out, mode: TrimMode::Regular, delta: (-FrameRate::FPS_24.tick_of(1).0).to_string(), linked: true }],
    };
    let preview = f.s.execute_as(&client, "transaction.preview", serde_json::to_value(&request).unwrap()).unwrap();
    let result =
        f.s.execute_as(&client, "transaction.commit", json!({"apiVersion": "1", "projectId": f.s.document_id, "transactionId": preview["transactionId"]}))
            .unwrap();
    let out = f.collect(json!({"excludeUnused": false})).unwrap();
    assert_eq!(out["operationReceipts"], 1);
    let mut opened = Session::default();
    opened.execute("file.open", json!({"path": out["project"]})).unwrap();
    assert_eq!(opened.operation_records.len(), 1);
    assert_eq!(opened.operation_records[0].result, result);
    // A serialized receipt cannot revive the prior host's grant, even with the same document ID.
    assert!(opened.execute_as(&client, "project.info", json!({"apiVersion": "1", "projectId": opened.document_id})).is_err());
    assert!(f.s.execute_as(&client, "file.verifyCollection", json!({"path": f.dest()})).is_err());
    f.s.shutdown();
}

#[test]
fn m1_collection_unknown_extension_dependencies_fail_closed_without_losing_original_data() {
    let mut f = Fixture::new();
    let mut project = (*f.s.project).clone();
    project
        .extensions
        .insert("thirdparty.files".into(), shotclutch_project::ProjectExtension { version: 1, required: false, data: json!({"file": "external.data"}) });
    f.s.project = Arc::new(project);
    assert!(f.collect(json!({})).unwrap_err().to_string().contains("CAPABILITY_UNAVAILABLE"));
    assert!(f.s.project.extensions.contains_key("thirdparty.files"));
    assert!(!f.dest().exists());
}

#[cfg(windows)]
#[test]
fn m1_collection_verifier_refuses_junction_escape() {
    let mut f = Fixture::new();
    f.collect(json!({})).unwrap();
    let m = manifest(&f.dest());
    let asset = f.dest().join(&m.assets[0].path);
    let folder = asset.parent().unwrap();
    let outside = f.dir.path().join("Outside");
    fs::create_dir(&outside).unwrap();
    fs::copy(&asset, outside.join(asset.file_name().unwrap())).unwrap();
    fs::remove_file(&asset).unwrap();
    fs::remove_dir(folder).unwrap();
    let status = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(folder.to_string_lossy().replace('/', "\\"))
        .arg(outside.to_string_lossy().replace('/', "\\"))
        .status()
        .unwrap();
    assert!(status.success());
    assert!(verify(&f.dest(), &Progress::default()).is_err());
    // Remove only the known junction itself; never recurse through its target.
    fs::remove_dir(folder).unwrap();
    assert!(outside.join(asset.file_name().unwrap()).exists());
}

#[test]
fn m1_collection_verifier_rejects_unlisted_files_and_malformed_options() {
    let mut f = Fixture::new();
    assert!(f.collect(json!({"includeHistory": "false"})).is_err());
    assert!(f.collect(json!({"projectName": ["name"]})).is_err());
    f.collect(json!({})).unwrap();
    fs::write(f.dest().join("unlisted-secret.txt"), b"not a declared asset").unwrap();
    assert!(verify(&f.dest(), &Progress::default()).unwrap_err().contains("unlisted"));
}

#[test]
fn m1_collection_history_only_network_paths_are_refused_before_host_io() {
    struct Spy(Arc<std::sync::atomic::AtomicUsize>);
    impl Services for Spy {
        fn read_file(&self, path: &str) -> std::io::Result<Vec<u8>> {
            crate::FsServices.read_file(path)
        }
        fn write_file(&self, _: &str, _: &[u8]) -> std::io::Result<()> {
            Err(std::io::ErrorKind::PermissionDenied.into())
        }
        fn file_size(&self, path: &str) -> std::io::Result<u64> {
            if path.starts_with("\\\\") || path.contains("://") {
                self.0.fetch_add(1, Ordering::Relaxed);
                return Err(std::io::ErrorKind::PermissionDenied.into());
            }
            crate::FsServices.file_size(path)
        }
    }
    let mut f = Fixture::new();
    let mut old = (*f.s.project).clone();
    let ItemKind::Media(media) = &mut old.items.get_mut(&f.item).unwrap().kind else { panic!() };
    media.media = MediaRef::File { path: "\\\\192.0.2.1\\share\\unapproved.wav".into() };
    f.s.history.undo.push(("Unapproved history source".into(), Arc::new(old)));
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    f.s.services = Arc::new(Spy(calls.clone()));
    assert!(f.collect(json!({"excludeUnused": false})).unwrap_err().to_string().contains("source path refused"));
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert!(!f.dest().exists());
    f.clean();
}

#[test]
fn m1_collection_intentional_offline_byte_source_retains_its_encoded_original() {
    let mut f = Fixture::new();
    let virtual_path = f.dir.path().join("offline-owned.wav");
    let item = crate::commands::import_bytes(&mut f.s, virtual_path.to_str().unwrap(), f.bytes.clone().into(), None).unwrap();
    f.s.execute("timeline.place", json!({"item": item.0, "time": shotclutch_time::TICKS_PER_SECOND, "duration": shotclutch_time::TICKS_PER_SECOND})).unwrap();
    let mut project = (*f.s.project).clone();
    let ItemKind::Media(media) = &mut project.items.get_mut(&item).unwrap().kind else { panic!() };
    media.offline = true;
    assert!(f.s.media.owned_binding(item, media).is_none(), "export still respects intentional offline");
    f.s.project = Arc::new(project);
    let out = f.collect(json!({})).unwrap();
    let m = verify(&f.dest(), &Progress::default()).unwrap();
    let asset = m.assets.iter().find(|a| a.source_id == item.0.to_string()).unwrap();
    assert!(asset.essential);
    assert_eq!(fs::read(f.dest().join(&asset.path)).unwrap(), f.bytes);
    let loaded = shotclutch_format::decode(&fs::read(out["project"].as_str().unwrap()).unwrap()).unwrap();
    assert!(loaded.project.item(item).unwrap().as_media().unwrap().offline);
}
