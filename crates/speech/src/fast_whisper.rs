//! Isolated, pinned, offline faster-whisper/CTranslate2 CPU INT8 worker.
use crate::{Options, ProgressFn, SAMPLE_RATE, SpeechError, Transcriber};
use serde_json::{Value, json};
use shotclutch_project::{Transcript, Word};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

pub const TINY_EN: &str = "faster-whisper-tiny.en";
pub const BASE_EN: &str = "faster-whisper-base.en";
pub const PROFILE: &str = "faster-whisper-1.2.1/ct2-4.8.3/int8/beam5/silero/words/no-context/v1";
const SCRIPT: &str = include_str!("fast_whisper.py");
const REQUIREMENTS: &str = include_str!("fast_whisper_requirements.txt");

pub fn supports(id: &str) -> bool {
    matches!(id, TINY_EN | BASE_EN)
}
pub fn runtime_dir(models: &Path) -> PathBuf {
    models.parent().unwrap_or(models).join("runtime").join("faster-whisper")
}
fn python_path(runtime: &Path) -> PathBuf {
    runtime.join(if cfg!(windows) { "Scripts/python.exe" } else { "bin/python" })
}

fn hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(not(windows))]
    let _ = command;
}

fn checked_process(mut command: Command, progress: &mut dyn FnMut(u64, u64, &str) -> bool, phase: &str) -> Result<(), SpeechError> {
    hidden(&mut command);
    let mut child = command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|_| {
        SpeechError::Unavailable("Python 3.10 or newer is required to install fast local speech; install Python or set SHOTCLUTCH_PYTHON".into())
    })?;
    let started = Instant::now();
    loop {
        let cancelled = !progress(0, 0, phase);
        if cancelled || started.elapsed() > Duration::from_secs(900) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(if cancelled { SpeechError::Cancelled } else { SpeechError::Model(format!("{phase} timed out")) });
        }
        if let Some(status) = child.try_wait()? {
            return if status.success() {
                Ok(())
            } else {
                Err(SpeechError::Model(format!("{phase} failed; check Python availability and the package connection")))
            };
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub fn install_runtime(models: &Path, progress: &mut dyn FnMut(u64, u64, &str) -> bool) -> Result<(), SpeechError> {
    let runtime = runtime_dir(models);
    std::fs::create_dir_all(&runtime)?;
    let worker = runtime.join("worker.py");
    std::fs::write(&worker, SCRIPT)?;
    let python = python_path(&runtime);
    if !python.is_file() {
        let bootstrap = std::env::var_os("SHOTCLUTCH_PYTHON").unwrap_or_else(|| if cfg!(windows) { "python".into() } else { "python3".into() });
        let mut command = shotclutch_env::worker_command(bootstrap);
        command.args(["-m", "venv"]).arg(&runtime);
        checked_process(command, progress, "Creating local speech runtime")?;
    }
    let mut check = shotclutch_env::worker_command(&python);
    check.arg("-B").arg(&worker).arg("--check");
    match checked_process(check, progress, "Checking local speech runtime") {
        Ok(()) => {}
        Err(SpeechError::Cancelled) => return Err(SpeechError::Cancelled),
        Err(_) => {
            let requirements = runtime.join("requirements.txt");
            std::fs::write(&requirements, REQUIREMENTS)?;
            let mut install = shotclutch_env::worker_command(&python);
            install
                .args(["-m", "pip", "install", "--disable-pip-version-check", "--no-input", "--cache-dir"])
                .arg(runtime.join("pip-cache"))
                .arg("-r")
                .arg(requirements);
            checked_process(install, progress, "Installing pinned CPU speech runtime")?;
            let mut verify = shotclutch_env::worker_command(&python);
            verify.arg("-B").arg(&worker).arg("--check");
            checked_process(verify, progress, "Verifying local speech runtime")?;
        }
    }
    Ok(())
}

struct Worker {
    child: Child,
    input: ChildStdin,
    events: mpsc::Receiver<Result<Value, String>>,
    ready: bool,
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Worker {
    fn spawn(models: &Path, id: &str) -> Result<Self, SpeechError> {
        let runtime = runtime_dir(models);
        let python = python_path(&runtime);
        if !python.is_file() {
            return Err(SpeechError::Unavailable("Install the fast local speech runtime and model first".into()));
        }
        let worker = runtime.join("worker.py");
        std::fs::write(&worker, SCRIPT)?;
        let mut command = shotclutch_env::worker_command(python);
        command
            .arg("-B")
            .arg("-u")
            .arg(worker)
            .arg("--model-dir")
            .arg(models.join(id))
            .env("HF_HUB_OFFLINE", "1")
            .env("TRANSFORMERS_OFFLINE", "1")
            .env("HF_HUB_DISABLE_TELEMETRY", "1")
            .env("PYTHONIOENCODING", "utf-8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        hidden(&mut command);
        let mut child = command.spawn()?;
        let input = child.stdin.take().ok_or_else(|| SpeechError::Model("Missing local speech input pipe".into()))?;
        let output = child.stdout.take().ok_or_else(|| SpeechError::Model("Missing local speech output pipe".into()))?;
        let (send, events) = mpsc::sync_channel(4);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let mut line = Vec::new();
                // A bounded chunk's transcript is limited to 8 MiB of JSON.
                let read = reader.by_ref().take(8 * 1024 * 1024 + 1).read_until(b'\n', &mut line);
                match read {
                    Ok(0) => break,
                    Ok(_) if line.len() <= 8 * 1024 * 1024 && line.ends_with(b"\n") => {
                        let value = serde_json::from_slice(&line).map_err(|_| "Invalid local speech response".into());
                        if send.send(value).is_err() {
                            break;
                        }
                    }
                    _ => {
                        let _ = send.send(Err("Local speech response exceeds its limit".into()));
                        break;
                    }
                }
            }
        });
        Ok(Self { child, input, events, ready: false })
    }
    fn next(&mut self, progress: ProgressFn, status: &str, fraction: f32) -> Result<Value, SpeechError> {
        let started = Instant::now();
        loop {
            if !progress(fraction, status) {
                return Err(SpeechError::Cancelled);
            }
            if started.elapsed() > Duration::from_secs(900) {
                return Err(SpeechError::Model("Local speech worker timed out".into()));
            }
            match self.events.recv_timeout(Duration::from_millis(50)) {
                Ok(Ok(event)) if event["event"] == "error" => {
                    return Err(SpeechError::Model("Offline speech worker failed; reinstall or verify the local runtime and model".into()));
                }
                Ok(Ok(event)) => return Ok(event),
                Ok(Err(error)) => return Err(SpeechError::Model(error)),
                Err(mpsc::RecvTimeoutError::Disconnected) => return Err(SpeechError::Model("Local speech worker stopped".into())),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }
}

pub struct FastWhisper {
    id: String,
    models: PathBuf,
    worker: Mutex<Option<Worker>>,
}
impl FastWhisper {
    pub fn new(models: &Path, id: &str) -> Result<Self, SpeechError> {
        if !supports(id) {
            return Err(SpeechError::UnknownModel(id.into()));
        }
        Ok(Self { id: id.into(), models: models.into(), worker: Mutex::new(None) })
    }
}

fn decode(event: Value, id: &str, samples: usize) -> Result<Transcript, SpeechError> {
    let end = crate::sample_tick(samples as i64);
    let values = event["words"].as_array().ok_or_else(|| SpeechError::Model("Missing speech words".into()))?;
    if values.len() > 50_000 {
        return Err(SpeechError::Model("Too many speech words in a chunk".into()));
    }
    let mut result = Transcript { language: "en".into(), source: id.into(), ..Default::default() };
    for value in values {
        let (Some(text), Some(start), Some(stop), Some(confidence)) =
            (value["text"].as_str(), value["start"].as_f64(), value["end"].as_f64(), value["confidence"].as_f64())
        else {
            return Err(SpeechError::Model("Invalid speech word response".into()));
        };
        if text.len() > 4096
            || !start.is_finite()
            || !stop.is_finite()
            || start < 0.0
            || stop <= start
            || !confidence.is_finite()
            || !(0.0..=1.0).contains(&confidence)
        {
            return Err(SpeechError::Model("Invalid speech timing or confidence".into()));
        }
        let mut word = Word::new(text, crate::seconds_tick(start), crate::seconds_tick(stop).min(end));
        word.confidence = confidence as f32;
        if word.end > word.start && word.start < end {
            result.words.push(word);
        }
    }
    result.normalize();
    Ok(result)
}

impl Transcriber for FastWhisper {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn transcribe(&self, audio: &[f32], opts: &Options, progress: ProgressFn) -> Result<Transcript, SpeechError> {
        if !progress(0.0, "Preparing local transcription") {
            return Err(SpeechError::Cancelled);
        }
        if audio.is_empty() || audio.len() > 302 * SAMPLE_RATE as usize || audio.iter().any(|v| !v.is_finite()) {
            return Err(SpeechError::Model("Invalid bounded speech audio".into()));
        }
        if opts.language.as_deref().is_some_and(|lang| lang != "en") {
            return Err(SpeechError::Model("This fast speech profile is English only".into()));
        }
        let mut slot = self.worker.lock().unwrap_or_else(|e| e.into_inner());
        let result = (|| {
            if slot.is_none() {
                *slot = Some(Worker::spawn(&self.models, &self.id)?);
            }
            let worker = slot.as_mut().ok_or_else(|| SpeechError::Model("No local speech worker".into()))?;
            if !worker.ready {
                let event = worker.next(progress, "Loading English CPU model", 0.0)?;
                if event["event"] != "ready" || event["protocol"] != 1 {
                    return Err(SpeechError::Model("Local speech protocol mismatch".into()));
                }
                worker.ready = true;
            }
            serde_json::to_writer(&mut worker.input, &json!({"samples":audio.len(),"language":"en"})).map_err(|e| SpeechError::Io(e.to_string()))?;
            worker.input.write_all(b"\n")?;
            for chunk in audio.chunks(16_384) {
                if !progress(0.0, "Sending local audio") {
                    return Err(SpeechError::Cancelled);
                }
                let pcm: Vec<_> = chunk.iter().flat_map(|v| v.to_le_bytes()).collect();
                worker.input.write_all(&pcm)?;
            }
            worker.input.flush()?;
            let mut fraction = 0.0;
            loop {
                let event = worker.next(progress, "Transcribing speech locally", fraction)?;
                match event["event"].as_str() {
                    Some("progress") => {
                        let seconds = event["seconds"].as_f64().unwrap_or(0.0);
                        fraction = (seconds * SAMPLE_RATE as f64 / audio.len() as f64).clamp(0.0, 1.0) as f32;
                        if !progress(fraction, "Transcribing speech locally") {
                            return Err(SpeechError::Cancelled);
                        }
                    }
                    Some("result") => return decode(event, &self.id, audio.len()),
                    _ => return Err(SpeechError::Model("Unexpected local speech event".into())),
                }
            }
        })();
        if result.is_err() {
            *slot = None;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_before_loading_does_not_start_a_worker() {
        let model = FastWhisper::new(Path::new("missing-runtime"), TINY_EN).unwrap();
        assert_eq!(model.transcribe(&[0.0], &Options::default(), &mut |_, _| false), Err(SpeechError::Cancelled));
        assert!(model.transcribe(&[f32::NAN], &Options::default(), &mut |_, _| true).is_err());
    }
    #[test]
    fn words_keep_times_and_confidence_and_reject_invalid_values() {
        let event = json!({"words":[{"text":"Hello","start":0.125,"end":0.5,"confidence":0.75}]});
        let transcript = decode(event, TINY_EN, 16000).unwrap();
        assert_eq!(transcript.words[0].start, crate::seconds_tick(0.125));
        assert_eq!(transcript.words[0].confidence, 0.75);
        assert!(decode(json!({"words":[{"text":"x","start":2.0,"end":1.0,"confidence":0.5}]}), TINY_EN, 16000).is_err());
    }
}
