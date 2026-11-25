use super::*;
use crate::autosave::AutosaveConfig;
use crate::media_observations::{ObservationProducer, ObservationRequest};
use crate::media_search::SearchQuery;
use crate::media_test_util::{make_movie, session_with};
use crate::service::{Cancellation, EditorClient, EditorScope};
use serde_json::json;
use shotclutch_media::DemoScene;
use shotclutch_project::TrackKind;
use shotclutch_project::observations::{ObservationKind, SourceLocation, SourceStream, TextRole};
use shotclutch_time::FrameRate;
use std::path::Path;

fn grant(s: &mut Session, root: &Path, sequences: Option<std::collections::BTreeSet<ItemId>>, write: bool, budget: u64) -> EditorClient {
    s.authorize_editor(
        "inspector",
        EditorScope {
            sequences,
            allow_edits: write,
            media_roots: vec![root.to_owned()],
            lifetime: std::time::Duration::from_secs(120),
            max_source_bytes: budget,
        },
    )
    .unwrap()
}
fn tick(index: i64) -> Tick {
    FrameRate::FPS_24.tick_of(index)
}
fn source_result(s: &Session, client: &EditorClient, asset: ItemId, fp: &SourceFingerprint, view: SourceView) -> Result<SourceResult, ServiceError> {
    let result =
        s.inspect_media(client, &Request::Source(SourceRequest { asset, expected_fingerprint: fp.clone(), view }), &Default::default(), &Default::default())?;
    let Inspection::Source(result) = result else { panic!("expected source result") };
    assert_eq!(result.asset, asset);
    assert_eq!(&result.fingerprint, fp);
    Ok(result.evidence)
}

#[test]
fn search_hit_to_contact_sheet_clip_and_exact_crop() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("original.mov");
    make_movie(&path, DemoScene::Aurora, 32, 18, 12);
    let (mut s, assets, _) = session_with(&[&path]);
    let asset = assets[0];
    s.start_autosave(AutosaveConfig::new(&root.path().join("recovery"))).unwrap();
    let client = grant(&mut s, root.path(), None, true, 64 << 20);
    let read = s.inspect_media_observations(&client, asset, &Default::default(), &Default::default()).unwrap();
    let mut request = ObservationRequest::verified_asset(asset, read.revision, read.fingerprint.unwrap());
    let range = TimeRange::from_bounds(Tick(tick(2).0 + 1), tick(6));
    request.kind = ObservationKind::Segment;
    request.location = SourceLocation::SourceRange { stream: SourceStream::Video, start: range.start.0, end: range.end().0 };
    request.producer = ObservationProducer::Human;
    request.text_role = Some(TextRole::Caption);
    request.summary = Some("Aurora display".into());
    s.record_media_observation(&client, request, &Default::default()).unwrap();
    let found = s.search_media(&client, &SearchQuery::new("aurora display"), &Default::default(), &Default::default()).unwrap();
    let hit = &found.hits[0].best;
    let fp = hit.fingerprint.as_ref().unwrap();
    let SourceLocation::SourceRange { start, end, .. } = hit.location else { panic!("expected source-time range") };
    let range = TimeRange::from_bounds(Tick(start), Tick(end));
    let read_only = grant(&mut s, root.path(), None, false, 64 << 20);
    let read = |view| source_result(&s, &read_only, asset, fp, view).unwrap();
    let SourceResult::Summary { observations, .. } = read(SourceView::Summary) else { panic!("expected summary") };
    assert_eq!(observations.observations.len(), 1);
    let too_many_tiles = SourceView::ContactSheet { range, samples: 17, max_edge: 16 };
    assert_eq!(source_result(&s, &read_only, asset, fp, too_many_tiles).unwrap_err().code, ErrorCode::ResourceLimit);
    let SourceResult::ContactSheet(tiles) = read(SourceView::ContactSheet { range, samples: 16, max_edge: 16 }) else { panic!("expected tiles") };
    assert_eq!(tiles.iter().map(|tile| tile.pts).collect::<Vec<_>>(), [tick(2), tick(3), tick(4), tick(5)]);
    assert_eq!((tiles[0].pixels.width, tiles[0].pixels.height), (16, 9));
    let SourceResult::Clip(clip) = read(SourceView::Clip { range, max_edge: 32, audio_stream: Some(0) }) else { panic!("expected clip") };
    assert_eq!(clip.frames[0].visible, TimeRange::from_bounds(range.start, tick(3)));
    assert_eq!(clip.frames[0].frame.pts, tick(2));
    assert_eq!(clip.frames.last().unwrap().visible.end(), range.end());
    let pcm = clip.audio.unwrap();
    assert_eq!(pcm.first_sample, 4001);
    assert_eq!(pcm.buffer.frames(), 7999);
    assert!(pcm.buffer.channels.iter().flatten().any(|s| s.abs() > 0.001));
    let view = |time, rounding, crop| SourceView::Frame { time, rounding, max_edge: 32, crop };
    let SourceResult::Frame(full) = read(view(tick(3), Rounding::Exact, None)) else { panic!("expected frame") };
    let region = Region { x: 3, y: 2, w: 4, h: 4 };
    let SourceResult::Frame(crop) = read(view(range.start, Rounding::AtOrAfter, Some(region))) else { panic!("expected crop") };
    assert_eq!(crop.pts, tick(3));
    assert_eq!(crop.region, region);
    use shotclutch_geom::Vec2;
    assert_eq!(crop.output_to_original().apply(Vec2::new(0.5, 0.5)), Vec2::new(3.5, 2.5));
    assert_eq!(tiles[0].output_to_original().apply(Vec2::new(0.5, 0.5)), Vec2::new(1.0, 1.0));
    let offset = (region.y * 32 + region.x) * 4;
    assert_eq!(&crop.pixels.rgba[..16], &full.pixels.rgba[offset..offset + 16]);
    assert_eq!(source_result(&s, &read_only, asset, fp, view(range.start, Rounding::Exact, None)).unwrap_err().code, ErrorCode::VerificationFailed);
    let SourceResult::Frame(before) = read(view(range.start, Rounding::AtOrBefore, None)) else { panic!("expected earlier frame") };
    assert_eq!(before.pts, tick(2));

    let moved = root.path().join("moved");
    std::fs::create_dir(&moved).unwrap();
    let moved = moved.join("original.mov");
    std::fs::copy(path, &moved).unwrap();
    s.execute("media.relink", json!({"item":asset.0,"path":moved,"force":false,"relinkOthers":false})).unwrap();
    source_result(&s, &read_only, asset, fp, view(tick(3), Rounding::Exact, None)).unwrap();
    let mut bytes = std::fs::read(&moved).unwrap();
    let mid = bytes.len() / 2;
    bytes[mid] ^= 1;
    std::fs::write(moved, bytes).unwrap();
    assert_eq!(source_result(&s, &read_only, asset, fp, view(tick(3), Rounding::Exact, None)).unwrap_err().code, ErrorCode::MediaChanged);
    s.shutdown();
}

#[test]
fn variable_frame_clip_preserves_presentation_durations() {
    use shotclutch_matroska::{MkvWriter, MuxOptions, TrackKind, TrackSpec};
    use std::io::Cursor;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("vfr.mkv");
    let mut spec = TrackSpec::new(TrackKind::Video, "V_MJPEG");
    spec.video_size = Some((16, 16));
    spec.default_duration_ns = Some(40_000_000);
    let mut mkv = MkvWriter::new(Cursor::new(Vec::new()), vec![spec], MuxOptions::default()).unwrap();
    let mut at = 0;
    for (tone, duration) in [(20, 40), (50, 80), (100, 20), (150, 50)] {
        let mut jpeg = Vec::new();
        let picture = image::GrayImage::from_pixel(16, 16, image::Luma([tone]));
        image::codecs::jpeg::JpegEncoder::new(&mut jpeg).encode_image(&picture).unwrap();
        mkv.write_frame(0, at * 1_000_000, true, &jpeg, Some(duration * 1_000_000)).unwrap();
        at += duration as i64;
    }
    std::fs::write(&path, mkv.finish().unwrap().into_inner()).unwrap();
    let mut s = Session::default();
    let asset = ItemId(s.execute("file.import", json!({"paths":[path]})).unwrap()["items"][0].as_u64().unwrap());
    let client = grant(&mut s, root.path(), None, false, 64 << 20);
    let fp = s.inspect_media_observations(&client, asset, &Default::default(), &Default::default()).unwrap().fingerprint.unwrap();
    let ms = |n| Tick::from_units(n, 1000);
    let range = TimeRange::from_bounds(ms(50), ms(155));
    let SourceResult::Clip(clip) = source_result(&s, &client, asset, &fp, SourceView::Clip { range, max_edge: 16, audio_stream: None }).unwrap() else {
        panic!("expected clip")
    };
    let spans = clip.frames.iter().map(|f| (f.frame.pts, f.visible.start, f.visible.end())).collect::<Vec<_>>();
    assert_eq!(spans, [(ms(40), ms(50), ms(120)), (ms(120), ms(120), ms(140)), (ms(140), ms(140), ms(155))]);
    assert!(clip.audio.is_none());
}

#[test]
fn scoped_timeline_inspects_nested_picture_and_mix() {
    let root = tempfile::tempdir().unwrap();
    let granted = root.path().join("granted");
    std::fs::create_dir(&granted).unwrap();
    let path = root.path().join("original.mov");
    make_movie(&path, DemoScene::CityNight, 32, 18, 12);
    let (mut s, assets, nested) = session_with(&[&path]);
    let project = std::sync::Arc::make_mut(&mut s.project);
    let settings = project.sequence(nested).unwrap().settings.clone();
    let parent = project.new_sequence("Parent", settings, 1, 1, None);
    let length = TimeRange::new(Tick::ZERO, tick(6));
    for kind in [TrackKind::Video, TrackKind::Audio] {
        let offset = tick(2);
        let mut clip = project.make_track_item(nested, kind, offset, length, FrameRate::FPS_24).unwrap();
        for effect in &mut clip.effects {
            shotclutch_project::resolve_auto_points(effect, (32, 18), (32, 18));
        }
        project.sequence_mut(parent).unwrap().tracks_mut(kind)[0].items.push(clip);
    }
    let saved = root.path().join("timeline.scproj");
    s.execute("file.saveAs", json!({"path":saved})).unwrap();
    let saved_bytes = std::fs::read(&saved).unwrap();
    s.state.active_sequence = Some(nested);
    s.set_playhead(tick(1));
    let active = s.state.active_sequence;
    let playhead = s.playhead();
    let revision = s.revision;
    let range = TimeRange::from_bounds(tick(3), tick(5));
    let request =
        Request::Timeline(TimelineRequest { sequence: parent, expected_revision: revision, view: TimelineView::Clip { range, max_edge: 32, audio: true } });
    let client = grant(&mut s, root.path(), Some([parent].into()), false, 64 << 20);
    let confined = grant(&mut s, &granted, Some([parent].into()), false, 64 << 20);
    let wrong_sequence = grant(&mut s, root.path(), Some([nested].into()), false, 64 << 20);
    let small = grant(&mut s, root.path(), Some([parent].into()), false, 1);
    let read = |client, request| s.inspect_media(client, request, &Default::default(), &Default::default());
    assert_eq!(read(&confined, &request).unwrap_err().code, ErrorCode::PermissionRequired);
    assert_eq!(read(&wrong_sequence, &request).unwrap_err().code, ErrorCode::PermissionDenied);
    assert_eq!(read(&small, &request).unwrap_err().code, ErrorCode::ResourceLimit);
    let stale =
        Request::Timeline(TimelineRequest { sequence: parent, expected_revision: revision + 1, view: TimelineView::Frame { time: tick(3), max_edge: 32 } });
    assert_eq!(read(&client, &stale).unwrap_err().code, ErrorCode::RevisionConflict);
    let cancelled = Cancellation::default();
    cancelled.cancel();
    assert_eq!(s.inspect_media(&client, &request, &Default::default(), &cancelled).unwrap_err().code, ErrorCode::Canceled);
    let Inspection::Timeline(evidence) = read(&client, &request).unwrap() else { panic!("expected timeline") };
    assert_eq!(evidence.sequence, parent);
    assert_eq!(evidence.originals.keys().copied().collect::<Vec<_>>(), assets);
    assert_eq!(evidence.revision, revision);
    let TimelineResult::Clip(clip) = evidence.evidence else { panic!("expected clip") };
    let provider = s.media.provider(s.project.clone(), s.services.clone());
    let first = &clip.frames[0].frame;
    assert_eq!(first.time, tick(3));
    let reference = shotclutch_render::render_sequence(&s.project, parent, tick(3), render_options(), &provider);
    assert_eq!(first.pixels.rgba, reference.to_rgba8());
    let picture = shotclutch_render::render_sequence(&s.project, nested, tick(1), render_options(), &provider);
    assert_eq!(first.pixels.rgba, picture.to_rgba8());
    let frame_request = Request::Timeline(TimelineRequest {
        sequence: parent,
        expected_revision: revision,
        view: TimelineView::Frame { time: Tick(tick(3).0 + 1), max_edge: 16 },
    });
    let Inspection::Timeline(frame) = read(&client, &frame_request).unwrap() else { panic!("expected timeline evidence") };
    let TimelineResult::Frame(frame) = frame.evidence else { panic!("expected timeline frame") };
    assert_eq!(frame.time, tick(3));
    assert_eq!((frame.pixels.width, frame.pixels.height), (16, 9));
    let pcm = clip.audio.unwrap();
    let reference =
        shotclutch_render::mixer::mix_graph(&s.project, s.project.sequence(parent).unwrap(), pcm.first_sample, pcm.buffer.frames(), &provider, None);
    assert_eq!(pcm.buffer, reference);
    assert!(pcm.buffer.channels.iter().flatten().any(|s| s.abs() > 0.001));
    assert_eq!(s.revision, revision);
    assert_eq!(s.state.active_sequence, active);
    assert_eq!(s.playhead(), playhead);
    assert_eq!(std::fs::read(saved).unwrap(), saved_bytes);
    let mut broken = s.project.clone();
    std::sync::Arc::make_mut(&mut broken).sequence_mut(nested).unwrap().audio_tracks[0].items[0].audio_stream = 9;
    s.project = broken;
    assert_eq!(s.inspect_media(&client, &request, &Default::default(), &Default::default()).unwrap_err().code, ErrorCode::VerificationFailed);
    std::fs::remove_file(path).unwrap();
    assert_eq!(s.inspect_media(&client, &request, &Default::default(), &Default::default()).unwrap_err().code, ErrorCode::MediaChanged);
}

fn render_options() -> shotclutch_render::RenderOptions {
    shotclutch_render::RenderOptions { captions: true, ..Default::default() }
}
