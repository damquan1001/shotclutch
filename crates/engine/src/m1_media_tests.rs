//! Small, real-container regressions for source PTS, edit offsets and half-open VFR lookup.
//! Independent expected time tables; no external fixture generator in routine CI.
use std::io::Cursor;
use std::sync::Arc;

use shotclutch_isobmff::{Brand, Mp4Writer, TrackConfig, WriteSample, WriterOptions};
use shotclutch_media::{DemoScene, FrameRequest};
use shotclutch_time::{FrameRate, Tick};

#[test]
fn m1_native_source_frames_keep_actual_pts_through_vfr_offsets_and_cache_seeks() {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("source.mov");
    crate::media_test_util::make_movie(&original, DemoScene::Aurora, 32, 18, 6);
    let bytes = std::fs::read(&original).unwrap();
    let file = shotclutch_isobmff::open(bytes.clone()).unwrap();
    let track = file.track_of_kind(shotclutch_isobmff::TrackKind::Video).unwrap();
    let entry = file.tracks[track].entries[0].clone();
    let source = shotclutch_codecs::open_bytes("source.mov", bytes.clone().into()).unwrap();
    let pictures: Vec<_> = (0..6).map(|i| source.video_frame(FrameRequest::full(FrameRate::FPS_24.tick_of(i))).unwrap().to_rgba8()).collect();
    for (timescale, durations, offset) in
        [(24u32, [1, 1, 1, 1, 1, 1], 0), (1000u32, [40, 80, 20, 100, 40, 120], 0), (24000u32, [1001, 1001, 1001, 1001, 1001, 1001], 2002)]
    {
        let mut mux = Mp4Writer::new(Cursor::new(Vec::new()), WriterOptions::new(Brand::Mov)).unwrap();
        let mut config = TrackConfig::new(entry.clone(), timescale);
        config.media_start = Some(offset);
        let output = mux.add_track(config).unwrap();
        let mut at = 0;
        let mut expected = Vec::new();
        for (i, &duration) in durations.iter().enumerate() {
            expected.push(Tick::from_units(at, timescale as i64));
            let sample = file.read_sample(&bytes, track, i).unwrap();
            mux.write_sample(output, WriteSample { data: &sample, duration, composition_offset: offset as i32, is_sync: true }).unwrap();
            at += duration as i64;
        }
        let encoded: Arc<[u8]> = mux.finish().unwrap().into_inner().into();
        let source = shotclutch_codecs::open_bytes("mapped.mov", encoded.clone()).unwrap();
        for i in [5, 0, 3, 1, 4, 2] {
            let f = source.video_frame(FrameRequest::full(expected[i])).unwrap();
            assert_eq!(f.pts, expected[i], "clock {timescale}, offset {offset}, picture {i}");
            assert_eq!(f.to_rgba8(), pictures[i]);
            if i + 1 < expected.len() {
                for time in [Tick(expected[i].0 + (expected[i + 1].0 - expected[i].0) / 2), expected[i + 1] - Tick(1)] {
                    let f = source.video_frame(FrameRequest::full(time)).unwrap();
                    assert_eq!(f.pts, expected[i], "half-open source interval at {time:?}, clock {timescale}");
                    assert_eq!(f.to_rgba8(), pictures[i]);
                }
            }
            let cold = shotclutch_codecs::open_bytes("mapped.mov", encoded.clone()).unwrap();
            assert_eq!(cold.video_frame(FrameRequest::full(expected[i])).unwrap().pts, expected[i]);
        }
    }
}
