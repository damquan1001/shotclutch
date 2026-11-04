use super::*;
use crate::Session;
use std::io::{Seek, SeekFrom, Write};

struct Hook;
impl Hook {
    fn set(f: impl FnMut(FaultPoint) -> Result<(), String> + 'static) -> Self {
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
    seq: ItemId,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("original.wav");
        let bytes = shotclutch_media::wav::write_wav16(&vec![0.1; 2 * 1024 * 1024], 1, 48_000);
        fs::write(&source, &bytes).unwrap();
        let mut s = Session::default();
        let item = s.execute("file.import", json!({"paths": [source]})).unwrap()["items"][0].clone();
        s.execute("file.newSequence", json!({"video": 1, "audio": 1, "fps": 24})).unwrap();
        s.execute("timeline.place", json!({"item": item, "time": 0, "duration": shotclutch_time::TICKS_PER_SECOND})).unwrap();
        let seq = s.state.active_sequence.unwrap();
        Self { s, dir, source, bytes, seq }
    }
    fn output(&self) -> PathBuf {
        self.dir.path().join("delivery.wav")
    }
    fn export(&mut self) -> crate::Result<Value> {
        self.s.execute("file.exportMedia", json!({"path": self.output(), "format": "wav", "wait": true}))
    }
    fn clean(&self) {
        assert!(!fs::read_dir(self.dir.path()).unwrap().any(|p| p.unwrap().file_name().to_string_lossy().starts_with(".shotclutch-export-")));
    }
}
fn change(path: &Path, offset: u64) {
    let mut file = fs::OpenOptions::new().write(true).open(path).unwrap();
    file.seek(SeekFrom::Start(offset)).unwrap();
    file.write_all(&[0x7f, 0x1f]).unwrap();
    file.sync_all().unwrap();
}

#[test]
fn m1_export_binds_originals_and_complete_output_with_exact_provenance() {
    let mut f = Fixture::new();
    fs::write(f.output(), b"previous delivery").unwrap();
    let revision = f.s.revision.to_string();
    let doc = f.s.document_id;
    let result = f.export().unwrap();
    assert_eq!(result["binding"]["projectId"], json!(doc));
    assert_eq!(result["binding"]["revision"], revision);
    assert_eq!(result["binding"]["publication"], "atomicFile");
    assert_eq!(result["binding"]["sources"][0]["sha256"], format!("{:x}", Sha256::digest(&f.bytes)));
    let output = fs::read(f.output()).unwrap();
    assert_eq!(result["binding"]["outputs"][0]["sha256"], format!("{:x}", Sha256::digest(&output)));
    assert_eq!(result["binding"]["outputs"][0]["bytes"], output.len().to_string());
    let audio = shotclutch_codecs::open_bytes("delivery.wav", output.into()).unwrap();
    assert_eq!(audio.info().duration.to_units_floor(48_000), 48_000);
    assert_eq!(audio.info().audio().unwrap().channels, 2);
    assert!((audio.audio(0, 48_000, 48_000).unwrap().peaks()[0] - 0.1).abs() < 0.001);
    assert_eq!(fs::read(&f.source).unwrap(), f.bytes);
    assert!(!result["binding"].to_string().contains(".shotclutch-export-"));
    f.clean();
}

#[test]
fn m1_original_change_after_render_preserves_existing_delivery() {
    let mut f = Fixture::new();
    fs::write(f.output(), b"approved delivery").unwrap();
    let source = f.source.clone();
    let _hook = Hook::set(move |point| {
        if point == FaultPoint::Encoded {
            change(&source, 2 * 1024 * 1024);
        }
        Ok(())
    });
    let error = f.export().unwrap_err().to_string();
    assert!(error.contains("MEDIA_CHANGED"), "{error}");
    assert_eq!(fs::read(f.output()).unwrap(), b"approved delivery");
    assert!(f.s.export_bindings.values().all(|b| b.lock().unwrap().is_none()));
    f.clean();
}

#[test]
fn m1_changed_then_restored_decoder_bytes_cannot_pass_final_verification() {
    let mut f = Fixture::new();
    let source = f.source.clone();
    let original = f.bytes.clone();
    let _hook = Hook::set(move |point| {
        if point == FaultPoint::InputsBound {
            change(&source, 60_000);
        }
        if point == FaultPoint::Encoded {
            fs::write(&source, &original).unwrap();
        }
        Ok(())
    });
    let error = f.export().unwrap_err().to_string();
    assert!(error.contains("MEDIA_CHANGED"), "{error}");
    assert!(!f.output().exists());
    assert_eq!(fs::read(&f.source).unwrap(), f.bytes);
    f.clean();
}

#[test]
fn m1_cancellation_before_publication_keeps_prior_file_and_unfinished_status() {
    let f = Fixture::new();
    fs::write(f.output(), b"keep").unwrap();
    let progress = Arc::new(Progress::default());
    let cancel = progress.clone();
    let _hook = Hook::set(move |point| {
        if point == FaultPoint::Encoded {
            assert!(!cancel.finished.load(Ordering::Acquire), "encoding alone must not publish job completion");
            cancel.cancel.store(true, Ordering::Release);
        }
        Ok(())
    });
    let result = run(
        &f.s.project,
        f.seq,
        &ExportSettings { path: f.output().to_string_lossy().into(), format: Format::Wav, ..Default::default() },
        f.s.media.export_snapshot(),
        f.s.services.clone(),
        &progress,
        (f.s.document_id, f.s.revision),
        &Mutex::default(),
        &Mutex::default(),
    );
    assert!(result.unwrap_err().contains("CANCELED"));
    assert_eq!(fs::read(f.output()).unwrap(), b"keep");
    f.clean();
}

#[test]
fn m1_destination_changed_during_render_is_preserved() {
    let mut f = Fixture::new();
    fs::write(f.output(), b"before").unwrap();
    let output = f.output();
    let _hook = Hook::set(move |point| {
        if point == FaultPoint::Encoded {
            fs::write(&output, b"new human delivery").unwrap();
        }
        Ok(())
    });
    let error = f.export().unwrap_err().to_string();
    assert!(error.contains("REVISION_CONFLICT"), "{error}");
    assert_eq!(fs::read(f.output()).unwrap(), b"new human delivery");
    f.clean();
}

#[test]
fn m1_export_destination_cannot_replace_an_original_or_project_document() {
    let mut f = Fixture::new();
    let error = f.s.execute("file.exportMedia", json!({"path": f.source, "format": "wav", "wait": true})).unwrap_err().to_string();
    assert!(error.contains("PERMISSION_DENIED"), "{error}");
    // The CLI resolves ordinary extensions; direct native hosts must also be prevented from
    // writing a render over a canonical project or a directory.
    for path in [f.dir.path().join("project.scproj"), f.dir.path().to_path_buf()] {
        let settings = ExportSettings { path: path.to_string_lossy().into(), format: Format::Wav, ..Default::default() };
        let error = Delivery::new(&f.s.project, f.seq, &settings, None, &[], &[], &Progress::default()).err().unwrap();
        assert!(error.contains("PERMISSION_DENIED"), "{error}");
    }
    assert_eq!(fs::read(&f.source).unwrap(), f.bytes);
    f.clean();
}

#[test]
fn m1_output_reservations_prevent_concurrent_jobs_and_release_on_abort() {
    let f = Fixture::new();
    let settings = ExportSettings { path: f.output().to_string_lossy().into(), format: Format::Wav, ..Default::default() };
    let progress = Progress::default();
    let first = Delivery::new(&f.s.project, f.seq, &settings, None, &[], &[], &progress).unwrap();
    assert!(Delivery::new(&f.s.project, f.seq, &settings, None, &[], &[], &progress).err().unwrap().contains("RESOURCE_LIMIT"));
    drop(first);
    drop(Delivery::new(&f.s.project, f.seq, &settings, None, &[], &[], &progress).unwrap());
    assert!(!f.output().exists());
    f.clean();
}

#[test]
fn m1_multi_file_publication_rolls_back_existing_and_new_destinations() {
    for existing in [false, true] {
        let f = Fixture::new();
        let settings = ExportSettings { path: f.dir.path().join("delivery.mxf").to_string_lossy().into(), format: Format::MxfOpAtom, ..Default::default() };
        let progress = Progress::default();
        let paths = filenames(&f.s.project, f.seq, &settings, None).unwrap();
        assert!(paths.len() >= 3);
        for path in &paths {
            if existing {
                fs::write(path, b"old").unwrap();
            }
        }
        let delivery = Delivery::new(&f.s.project, f.seq, &settings, None, &[], &[], &progress).unwrap();
        for path in &paths {
            fs::write(delivery.staged(path), b"new").unwrap();
        }
        let _hook = Hook::set(|point| if point == FaultPoint::Publish(1) { Err("injected promotion failure".into()) } else { Ok(()) });
        assert!(delivery.publish(&progress).unwrap_err().contains("injected promotion failure"));
        drop(delivery);
        for path in paths {
            if existing {
                assert_eq!(fs::read(path).unwrap(), b"old");
            } else {
                assert!(!path.exists());
            }
        }
        f.clean();
    }
}

#[test]
fn m1_cancellation_after_the_publication_point_returns_the_completed_delivery() {
    let f = Fixture::new();
    let progress = Arc::new(Progress::default());
    let cancel = progress.clone();
    let _hook = Hook::set(move |point| {
        if point == FaultPoint::Publish(0) {
            cancel.cancel.store(true, Ordering::Release);
        }
        Ok(())
    });
    let binding = Mutex::default();
    let report = run(
        &f.s.project,
        f.seq,
        &ExportSettings { path: f.output().to_string_lossy().into(), format: Format::Wav, ..Default::default() },
        f.s.media.export_snapshot(),
        f.s.services.clone(),
        &progress,
        (f.s.document_id, f.s.revision),
        &Mutex::default(),
        &binding,
    )
    .unwrap();
    assert_eq!(report.path, f.output().to_string_lossy());
    assert!(progress.cancel.load(Ordering::Acquire));
    assert_eq!(binding.lock().unwrap().as_ref().unwrap()["publication"], "atomicFile");
    assert!(f.output().is_file());
    f.clean();
}

#[test]
fn m1_queued_project_identity_survives_switching_the_live_document() {
    let mut f = Fixture::new();
    let media_id = f.s.project.sequence(f.seq).unwrap().audio_tracks[0].items[0].item;
    f.s.edit("Relative project source", |p, _| {
        if let shotclutch_project::ItemKind::Media(media) = &mut p.items.get_mut(&media_id).unwrap().kind {
            media.media = shotclutch_project::MediaRef::File { path: "original.wav".into() };
        }
        Ok(())
    })
    .unwrap();
    f.s.media.add_stored(&f.s.project, Some(f.dir.path().join("project.scproj").to_str().unwrap()));
    let project = f.s.document_id;
    let revision = f.s.revision.to_string();
    f.s.execute("export.queue.add", json!({"path": f.output(), "format": "wav"})).unwrap();
    f.s.execute("file.newProject", json!({"name": "Other project"})).unwrap();
    assert_ne!(f.s.document_id, project);
    f.s.execute("export.queue.start", json!({"wait": true})).unwrap();
    let jobs = f.s.execute("jobs.list", json!({})).unwrap();
    let last = jobs.as_array().unwrap().last().unwrap();
    assert_eq!(last["binding"]["projectId"], json!(project), "{jobs}");
    assert_eq!(last["binding"]["revision"], revision);
    assert_eq!(last["binding"]["sources"][0]["state"], "original");
    assert_eq!(last["binding"]["sources"][0]["sha256"], format!("{:x}", Sha256::digest(&f.bytes)));
    assert!(last["offlineMedia"].as_array().unwrap().is_empty());
    assert!(f.output().is_file());
    f.clean();
}

#[test]
fn m1_export_uses_fresh_originals_after_preview_cached_an_old_file() {
    let mut f = Fixture::new();
    let item = f.s.project.sequence(f.seq).unwrap().audio_tracks[0].items[0].item;
    let cached = f.s.media.full_res_source(&f.s.project, item, &*f.s.services).unwrap();
    // Simulate a historical project without a stored import identity. Even then a job must
    // establish its own content binding and avoid the monitor's retained old file handle.
    f.s.edit("Historical identity", |p, _| {
        if let shotclutch_project::ItemKind::Media(media) = &mut p.items.get_mut(&item).unwrap().kind {
            media.identity = None;
        }
        Ok(())
    })
    .unwrap();
    let replacement = shotclutch_media::wav::write_wav16(&vec![0.7; 2 * 1024 * 1024], 1, 48_000);
    f.s.services.write_file(f.source.to_str().unwrap(), &replacement).unwrap();
    assert!((cached.audio(0, 48_000, 48_000).unwrap().peaks()[0] - 0.1).abs() < 0.001);
    let result = f.export().unwrap();
    assert_eq!(result["binding"]["sources"][0]["sha256"], format!("{:x}", Sha256::digest(&replacement)));
    let audio = shotclutch_codecs::open_bytes("out.wav", fs::read(f.output()).unwrap().into()).unwrap();
    assert!((audio.audio(0, 48_000, 48_000).unwrap().peaks()[0] - 0.7).abs() < 0.001);
    f.clean();
}

#[test]
fn m1_unused_offline_originals_are_protected_from_output_overwrite() {
    let mut f = Fixture::new();
    let other = f.dir.path().join("unused.wav");
    fs::write(&other, &f.bytes).unwrap();
    let item = f.s.execute("file.import", json!({"paths": [other]})).unwrap()["items"][0].as_u64().unwrap();
    f.s.edit("Offline unrelated original", |p, _| {
        if let shotclutch_project::ItemKind::Media(media) = &mut p.items.get_mut(&ItemId(item)).unwrap().kind {
            media.offline = true;
        }
        Ok(())
    })
    .unwrap();
    let error = f.s.execute("file.exportMedia", json!({"path": other, "format": "wav", "wait": true})).unwrap_err().to_string();
    assert!(error.contains("PERMISSION_DENIED"), "{error}");
    assert_eq!(fs::read(other).unwrap(), f.bytes);
    f.clean();
}

#[test]
fn m1_numbered_originals_are_all_bound_and_holes_stay_frozen_for_the_job() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("take001.png");
    let last = dir.path().join("take003.png");
    image::RgbaImage::from_pixel(32, 18, image::Rgba([255u8, 0, 0, 255])).save(&first).unwrap();
    image::RgbaImage::from_pixel(32, 18, image::Rgba([0u8, 255, 0, 255])).save(&last).unwrap();
    let mut s = Session::default();
    let id = s.execute("file.import", json!({"paths": [first], "imageSequence": true})).unwrap()["items"][0].as_u64().unwrap();
    let media = s.project.item(ItemId(id)).unwrap().as_media().unwrap();
    assert_eq!(media.info.kind, shotclutch_media::MediaKind::ImageSequence);
    let rate = media.info.frame_rate();
    s.execute("file.newSequence", json!({"video": 1, "audio": 1})).unwrap();
    s.execute("timeline.place", json!({"item": id, "time": 0, "duration": rate.tick_of(3).0})).unwrap();
    let hole = dir.path().join("take002.png");
    let _hook = Hook::set(move |point| {
        if point == FaultPoint::InputsBound {
            image::RgbaImage::from_pixel(32, 18, image::Rgba([0u8, 0, 255, 255])).save(&hole).unwrap();
        }
        Ok(())
    });
    let result = s.execute("file.exportMedia", json!({"path": dir.path().join("render.png"), "format": "png", "width": 32, "height": 18, "fps": format!("{}/{}", rate.num, rate.den), "wait": true})).unwrap();
    let sources = result["binding"]["sources"].as_array().unwrap();
    assert_eq!(sources.len(), 2, "{result}");
    assert_eq!(sources[0]["frameName"], "take001.png");
    assert_eq!(sources[1]["frameName"], "take003.png");
    assert_eq!(result["result"]["frames"], 3);
    let first_out = image::open(dir.path().join("render000.png")).unwrap().to_rgba8();
    let hole_out = image::open(dir.path().join("render001.png")).unwrap().to_rgba8();
    assert_eq!(first_out, hole_out, "a frame created after source binding must not replace a planned hole");
    assert_eq!(result["binding"]["publication"], "fileSet");
}

#[test]
fn m1_fractional_frame_range_keeps_exact_absolute_sample_boundaries() {
    let mut f = Fixture::new();
    let item = f.s.project.sequence(f.seq).unwrap().audio_tracks[0].items[0].item;
    let rate = shotclutch_time::FrameRate::FPS_29_97;
    f.s.execute("file.newSequence", json!({"video": 1, "audio": 1, "fps": 29.97})).unwrap();
    f.s.execute("timeline.place", json!({"item": item.0, "time": 0, "duration": rate.tick_of(30).0})).unwrap();
    let result =
        f.s.execute("file.exportMedia", json!({"path": f.output(), "format": "wav", "range": "custom", "startFrame": 1, "endFrame": 4, "wait": true})).unwrap();
    let expected = (4 * 48_000u64 * 1001 / 30_000) - (48_000u64 * 1001 / 30_000);
    assert_eq!(expected, 4_805);
    assert_eq!(result["result"]["frames"], expected);
    assert_eq!(result["binding"]["range"]["start"], rate.tick_of(1).0.to_string());
    assert_eq!(result["binding"]["range"]["end"], rate.tick_of(4).0.to_string());
    let audio = shotclutch_codecs::open_bytes("out.wav", fs::read(f.output()).unwrap().into()).unwrap();
    assert_eq!(audio.info().duration.to_units_floor(48_000), expected as i64);
    f.clean();
}

#[test]
fn m1_failed_rollback_retains_prior_files_for_explicit_recovery() {
    let f = Fixture::new();
    let output = f.dir.path().join("delivery.mxf");
    let settings = ExportSettings { path: output.to_string_lossy().into(), format: Format::MxfOpAtom, ..Default::default() };
    let paths = filenames(&f.s.project, f.seq, &settings, None).unwrap();
    for path in &paths {
        fs::write(path, b"approved prior output").unwrap();
    }
    let progress = Progress::default();
    let delivery = Delivery::new(&f.s.project, f.seq, &settings, None, &[], &[], &progress).unwrap();
    let recovery = delivery.stage.path().to_path_buf();
    for path in &paths {
        fs::write(delivery.staged(path), b"replacement").unwrap();
    }
    let blocked = output.clone();
    let _hook = Hook::set(move |point| {
        if point == FaultPoint::Publish(1) {
            fs::remove_file(&blocked).unwrap();
            fs::create_dir(&blocked).unwrap();
            return Err("injected publication/rollback failure".into());
        }
        Ok(())
    });
    let error = delivery.publish(&progress).unwrap_err();
    assert!(error.contains("needs recovery"), "{error}");
    drop(delivery);
    assert!(recovery.join("RECOVERY_REQUIRED").is_file());
    assert_eq!(fs::read(recovery.join("previous").join(output.file_name().unwrap())).unwrap(), b"approved prior output");
    assert!(output.is_dir(), "a concurrent destination must not be deleted during recovery");
}

#[test]
fn m1_queued_byte_original_survives_clearing_the_live_media_cache() {
    let dir = tempfile::tempdir().unwrap();
    let virtual_path = dir.path().join("not-on-disk.wav");
    let bytes: Arc<[u8]> = shotclutch_media::wav::write_wav16(&vec![0.25; 48_000], 1, 48_000).into();
    let mut s = Session::default();
    let item = crate::commands::import_bytes(&mut s, virtual_path.to_str().unwrap(), bytes.clone(), None).unwrap();
    s.execute("file.newSequence", json!({"video": 1, "audio": 1, "fps": 24})).unwrap();
    s.execute("timeline.place", json!({"item": item.0, "time": 0, "duration": shotclutch_time::TICKS_PER_SECOND})).unwrap();
    let output = dir.path().join("out.wav");
    s.execute("export.queue.add", json!({"path": output, "format": "wav"})).unwrap();
    s.media.clear();
    assert!(!virtual_path.exists());
    s.execute("export.queue.start", json!({"wait": true})).unwrap();
    let jobs = s.execute("jobs.list", json!({})).unwrap();
    let last = jobs.as_array().unwrap().last().unwrap();
    assert_eq!(last["binding"]["sources"][0]["state"], "ownedOriginal", "{jobs}");
    assert_eq!(last["binding"]["sources"][0]["sha256"], format!("{:x}", Sha256::digest(&bytes)));
    assert!(last["offlineMedia"].as_array().unwrap().is_empty());
    let audio = shotclutch_codecs::open_bytes("out.wav", fs::read(output).unwrap().into()).unwrap();
    assert!((audio.audio(0, 48_000, 48_000).unwrap().peaks()[0] - 0.25).abs() < 0.001);
}
