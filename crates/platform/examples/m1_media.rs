//! Strict local M1 qualification. FFmpeg is an external oracle, never a runtime decoder.
//! Run through scripts/m1/qualify-media.py. This executable is not part of routine hosted CI.
use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use shotclutch_codecs::{CodecError, DecodedFrame, VideoDecoder};
use shotclutch_frame::VideoFrame;
use shotclutch_isobmff::SampleEntry;
use shotclutch_media::FrameRequest;
use shotclutch_time::Tick;

type Result<T> = std::result::Result<T, String>;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Case {
    name: String,
    kind: String,
    file: String,
    #[serde(default)]
    pts: Vec<String>,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
    #[serde(default)]
    rgba: String,
    #[serde(default)]
    samples: usize,
    #[serde(default)]
    channels: usize,
    #[serde(default)]
    sample_rate: u32,
    #[serde(default)]
    pcm: String,
    #[serde(default)]
    force_fallback: bool,
    #[serde(default)]
    exact_pixels: bool,
}
struct Failing {
    inner: Box<dyn VideoDecoder>,
    left: usize,
}
impl VideoDecoder for Failing {
    fn decode(&mut self, data: &[u8], pts: i64) -> shotclutch_codecs::Result<Vec<DecodedFrame>> {
        if self.left == 0 {
            return Err(CodecError::Decode("M1 injected hardware failure after eight samples".into()));
        }
        self.left -= 1;
        self.inner.decode(data, pts)
    }
    fn flush(&mut self) -> Vec<DecodedFrame> {
        self.inner.flush()
    }
    fn reset(&mut self) {
        self.left = 8;
        self.inner.reset();
    }
    fn name(&self) -> &str {
        "M1 simulated hardware (software pixels, injected fault)"
    }
    fn set_draft(&mut self, on: bool) {
        self.inner.set_draft(on);
    }
}
fn failing_factory(entry: &SampleEntry) -> Option<shotclutch_codecs::Result<Box<dyn VideoDecoder>>> {
    let info = shotclutch_codecs::hw::NalStreamInfo::from_entry(entry)?;
    Some(info.and_then(|info| {
        let inner = shotclutch_codecs::software_video_decoder(entry)?;
        Ok(Box::new(shotclutch_platform::HybridDecoder::new(Box::new(Failing { inner, left: 8 }), entry.clone(), info)) as Box<dyn VideoDecoder>)
    }))
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn member(root: &Path, name: &str) -> Result<std::path::PathBuf> {
    if name.is_empty() || name.contains(['/', '\\', ':']) || name == "." || name == ".." {
        return Err("oracle members must be single local filenames".into());
    }
    Ok(root.join(name))
}
fn frame(frame: &VideoFrame, pts: i64, width: u32, height: u32) -> Result<Vec<u8>> {
    if frame.pts != Tick(pts) {
        return Err(format!("frame PTS mismatch: requested source identity {pts}, returned {}", frame.pts.0));
    }
    if (frame.width, frame.height) != (width, height) {
        return Err(format!("frame size mismatch: {}x{}", frame.width, frame.height));
    }
    Ok(frame.to_rgba8())
}
fn run(root: &Path, c: &Case, mode: &str) -> Result<Value> {
    let input = fs::read(member(root, &c.file)?).map_err(|e| e.to_string())?;
    if input.len() > 64 * 1024 * 1024 {
        return Err("compressed source exceeds local qualification budget".into());
    }
    let sha = hash(&input);
    let bytes: Arc<[u8]> = input.into();
    let hw0 = shotclutch_codecs::hw::hw_stats();
    let start = Instant::now();
    let opened = shotclutch_codecs::open_bytes(&c.file, bytes.clone());
    if c.kind == "error" {
        let error = match opened {
            Err(e) => e.to_string(),
            Ok(src) => match src.video_frame(FrameRequest::full(Tick::ZERO)) {
                Err(e) => e.to_string(),
                Ok(_) => return Err("malformed source unexpectedly decoded".into()),
            },
        };
        return Ok(json!({"name": c.name, "status": "passed", "sourceSha256": sha, "recoverableError": error}));
    }
    let src = opened.map_err(|e| e.to_string())?;
    let open_ms = start.elapsed().as_secs_f64() * 1000.0;
    if c.kind == "audio" {
        if !(257..=192_000).contains(&c.samples) || !(1..=8).contains(&c.channels) {
            return Err("invalid audio oracle limits".into());
        }
        let oracle = fs::read(member(root, &c.pcm)?).map_err(|e| e.to_string())?;
        if oracle.len() != c.samples * c.channels * 4 {
            return Err("PCM oracle size mismatch".into());
        }
        let audio = src.audio(0, c.samples, c.sample_rate).map_err(|e| e.to_string())?;
        if audio.frames() != c.samples || audio.channel_count() != c.channels {
            return Err("PCM frame/channel count mismatch".into());
        }
        let actual = audio.interleaved();
        let mut max_error = 0.0f32;
        for (a, b) in actual.iter().zip(oracle.chunks_exact(4)) {
            let b = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            if !a.is_finite() || !b.is_finite() {
                return Err("nonfinite PCM sample".into());
            }
            max_error = max_error.max((a - b).abs());
        }
        if max_error > 0.000001 {
            return Err(format!("PCM oracle error {max_error} exceeds 1e-6"));
        }
        for offset in [c.samples - 257, 0, 73, c.samples / 2] {
            let audio = src.audio(offset as i64, 211, c.sample_rate).map_err(|e| e.to_string())?;
            if audio.frames() != 211 || audio.channel_count() != c.channels {
                return Err("random PCM window size mismatch".into());
            }
            for (i, sample) in audio.interleaved().iter().enumerate() {
                if !sample.is_finite() || (*sample - actual[offset * c.channels + i]).abs() > 0.000001 {
                    return Err("random PCM window differs from independent full-stream oracle".into());
                }
            }
        }
        return Ok(
            json!({"name": c.name, "status": "passed", "sourceSha256": sha, "samples": c.samples, "channels": c.channels, "sampleRate": c.sample_rate, "maxPcmError": max_error, "randomWindows": 4}),
        );
    }
    if c.kind != "video" || c.pts.is_empty() || c.pts.len() > 120 || c.width == 0 || c.width > 1920 || c.height == 0 || c.height > 1080 {
        return Err("invalid video oracle limits".into());
    }
    let pts = c.pts.iter().map(|p| p.parse::<i64>().map_err(|e| e.to_string())).collect::<Result<Vec<_>>>()?;
    if pts.windows(2).any(|p| p[0] >= p[1]) {
        return Err("PTS oracle is not strictly ordered".into());
    }
    let frame_bytes = c.width as usize * c.height as usize * 4;
    let mut oracle = fs::File::open(member(root, &c.rgba)?).map_err(|e| e.to_string())?;
    if oracle.metadata().map_err(|e| e.to_string())?.len() != (pts.len() * frame_bytes) as u64 {
        return Err("RGBA oracle frame count mismatch".into());
    }
    let mut expected = vec![0u8; frame_bytes];
    let mut hashes = Vec::new();
    let mut min_psnr = 100.0f64;
    let mut max_error = 0u8;
    let mut decode_ms = 0.0;
    let mut first_ms = 0.0;
    for (i, &time) in pts.iter().enumerate() {
        let t = Instant::now();
        let picture = src.video_frame(FrameRequest::full(Tick(time))).map_err(|e| e.to_string())?;
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        decode_ms += ms;
        if i == 0 {
            first_ms = open_ms + ms;
        }
        let actual = frame(&picture, time, c.width, c.height)?;
        oracle.read_exact(&mut expected).map_err(|e| e.to_string())?;
        let mse = actual
            .iter()
            .zip(&expected)
            .map(|(&a, &b)| {
                max_error = max_error.max(a.abs_diff(b));
                f64::from(a.abs_diff(b)).powi(2)
            })
            .sum::<f64>()
            / frame_bytes as f64;
        let psnr = if mse == 0.0 { 100.0 } else { 10.0 * (255.0 * 255.0 / mse).log10() };
        if psnr < 30.0 || c.exact_pixels && actual != expected {
            return Err(format!("frame {i} does not match oracle: PSNR {psnr:.3} dB, max error {max_error}"));
        }
        min_psnr = min_psnr.min(psnr);
        hashes.push(hash(&actual));
    }
    // Warm randomized access and half-open presentation intervals must retain actual frame identity.
    let mut intervals = 0;
    for i in (0..pts.len()).rev() {
        let picture = src.video_frame(FrameRequest::full(Tick(pts[i]))).map_err(|e| e.to_string())?;
        if hash(&frame(&picture, pts[i], c.width, c.height)?) != hashes[i] {
            return Err("warm seek changed source frame".into());
        }
        if i + 1 < pts.len() {
            let middle = pts[i] + (pts[i + 1] - pts[i]) / 2;
            let picture = src.video_frame(FrameRequest::full(Tick(middle))).map_err(|e| e.to_string())?;
            if hash(&frame(&picture, pts[i], c.width, c.height)?) != hashes[i] {
                return Err(format!("half-open interval lookup returned wrong source frame at {middle}"));
            }
            intervals += 1;
        }
    }
    let mut seeks = Vec::new();
    for i in [pts.len() - 1, 0, pts.len() / 2, 1.min(pts.len() - 1)] {
        let t = Instant::now();
        let cold = shotclutch_codecs::open_bytes(&c.file, bytes.clone()).map_err(|e| e.to_string())?;
        let picture = cold.video_frame(FrameRequest::full(Tick(pts[i]))).map_err(|e| e.to_string())?;
        seeks.push(t.elapsed().as_secs_f64() * 1000.0);
        if hash(&frame(&picture, pts[i], c.width, c.height)?) != hashes[i] {
            return Err("cold seek changed source frame".into());
        }
    }
    let hw = shotclutch_codecs::hw::hw_stats();
    let fallbacks = hw.fallbacks - hw0.fallbacks;
    if mode == "forced-fallback" && c.force_fallback && fallbacks == 0 {
        return Err("injected decoder fault did not exercise fallback".into());
    }
    // Count only verified requests; errors never masquerade as nominal decoded frames/FPS.
    Ok(
        json!({"name": c.name, "status": "passed", "sourceSha256": sha, "framesVerified": pts.len(), "pts": pts.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "frameHashes": hashes, "intervalsVerified": intervals, "coldSeekMs": seeks, "minRgbaPsnrDb": min_psnr, "maxRgbaError": max_error,
        "openMs": open_ms, "firstFrameMs": first_ms, "sequentialRequestMs": decode_ms, "queryFps": pts.len() as f64 / ((open_ms + decode_ms) / 1000.0).max(0.000001),
        "hwFrames": hw.frames - hw0.frames, "hwSessions": hw.sessions - hw0.sessions, "hwDeclined": hw.declined - hw0.declined, "fallbacks": fallbacks, "info": src.info()}),
    )
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: m1_media CORPUS.json software|auto|forced-fallback REPORT.json".into());
    }
    let mode = args[2].as_str();
    match mode {
        "auto" => {
            shotclutch_platform::register();
        }
        "software" => shotclutch_codecs::hw::set_hardware_decoding(false),
        "forced-fallback" => {
            shotclutch_codecs::register_video_decoder(failing_factory);
        }
        _ => return Err("invalid decoder mode".into()),
    }
    let manifest = Path::new(&args[1]);
    let root = manifest.parent().ok_or("manifest needs a parent directory")?;
    let cases: Vec<Case> = serde_json::from_slice(&fs::read(manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if cases.is_empty() || cases.len() > 32 {
        return Err("expected 1..32 corpus cases".into());
    }
    let mut failed = false;
    let rows: Vec<Value> = cases
        .iter()
        .map(|c| match run(root, c, mode) {
            Ok(row) => row,
            Err(error) => {
                failed = true;
                json!({"name": c.name, "status": "failed", "error": error})
            }
        })
        .collect();
    let report = json!({"version": 1, "mode": mode, "simulatedHardware": mode == "forced-fallback", "registeredBackend": shotclutch_codecs::hw::hw_backend(), "status": if failed { "failed" } else { "passed" }, "cases": rows});
    fs::write(&args[3], serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if failed { Err("media qualification failed; inspect the report".into()) } else { Ok(()) }
}
