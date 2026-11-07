//! AssemblyAI asynchronous transcription of bounded mono PCM chunks.
#[cfg(any(feature = "download", test))]
use crate::SAMPLE_RATE;
use crate::{Options, ProgressFn, SpeechError, Transcriber};
#[cfg(any(feature = "download", test))]
use serde::Deserialize;
use shotclutch_project::Transcript;
#[cfg(any(feature = "download", test))]
use shotclutch_project::Word;
#[cfg(any(feature = "download", test))]
use shotclutch_time::Tick;

pub struct AssemblyAi {
    key: String,
}

impl AssemblyAi {
    pub fn new(key: String) -> Result<Self, SpeechError> {
        if key.trim().is_empty() {
            return Err(SpeechError::Unavailable("enter an AssemblyAI API key".into()));
        }
        if !cfg!(feature = "download") {
            return Err(SpeechError::Unavailable("cloud transcription requires HTTPS support".into()));
        }
        Ok(Self { key: key.trim().into() })
    }
}

#[cfg(any(feature = "download", test))]
#[derive(Deserialize)]
struct TimedWord {
    text: String,
    start: i64,
    end: i64,
    confidence: f32,
}

#[cfg(any(feature = "download", test))]
#[derive(Deserialize)]
struct Response {
    status: String,
    words: Option<Vec<TimedWord>>,
}

#[cfg(any(feature = "download", test))]
fn transcript(response: Response, opts: &Options, samples: usize) -> Result<Transcript, SpeechError> {
    if response.status != "completed" {
        return Err(SpeechError::Model("AssemblyAI transcript is incomplete".into()));
    }
    let end = crate::sample_tick(samples as i64);
    let mut result = Transcript { source: "assemblyai".into(), language: opts.language.clone().unwrap_or_default(), ..Default::default() };
    for word in response.words.unwrap_or_default() {
        if word.start < 0 || word.end < word.start || !word.confidence.is_finite() || !(0.0..=1.0).contains(&word.confidence) {
            return Err(SpeechError::Model("AssemblyAI returned invalid word timing or confidence".into()));
        }
        let mut w = Word::new(word.text, Tick::from_units(word.start, 1000), Tick::from_units(word.end, 1000).min(end));
        if w.start >= end || w.end <= w.start {
            continue;
        }
        w.confidence = word.confidence;
        result.words.push(w);
    }
    result.normalize();
    Ok(result)
}

#[cfg(feature = "download")]
fn http_error(error: ureq::Error) -> SpeechError {
    let message = match error {
        ureq::Error::StatusCode(401 | 403) => "AssemblyAI rejected the API key or project access",
        ureq::Error::StatusCode(429) => "AssemblyAI rate limit reached; wait before resuming",
        _ => "AssemblyAI request failed; check your connection and account",
    };
    SpeechError::Model(message.into())
}

impl Transcriber for AssemblyAi {
    fn id(&self) -> String {
        "assemblyai".into()
    }

    fn transcribe(&self, audio: &[f32], opts: &Options, progress: ProgressFn) -> Result<Transcript, SpeechError> {
        #[cfg(not(feature = "download"))]
        {
            let _ = (audio, opts, progress, &self.key);
            Err(SpeechError::Unavailable("cloud transcription requires HTTPS support".into()))
        }
        #[cfg(feature = "download")]
        {
            use serde_json::{Value, json};
            if !progress(0.0, "Uploading audio to AssemblyAI") {
                return Err(SpeechError::Cancelled);
            }
            if audio.len() > 302 * SAMPLE_RATE as usize {
                return Err(SpeechError::Model("AssemblyAI audio chunk exceeds five minutes of audio plus context".into()));
            }
            let length = audio.len() as u32 * 2;
            let mut wav = Vec::with_capacity(length as usize + 44);
            wav.extend(b"RIFF");
            wav.extend((length + 36).to_le_bytes());
            wav.extend(b"WAVEfmt ");
            wav.extend(16u32.to_le_bytes());
            wav.extend(1u16.to_le_bytes());
            wav.extend(1u16.to_le_bytes());
            wav.extend(SAMPLE_RATE.to_le_bytes());
            wav.extend((SAMPLE_RATE * 2).to_le_bytes());
            wav.extend(2u16.to_le_bytes());
            wav.extend(16u16.to_le_bytes());
            wav.extend(b"data");
            wav.extend(length.to_le_bytes());
            for sample in audio {
                wav.extend(((sample.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
            }
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .tls_config(
                    ureq::tls::TlsConfig::builder()
                        .provider(ureq::tls::TlsProvider::Rustls)
                        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                        .unversioned_rustls_crypto_provider(std::sync::Arc::new(rustls_rustcrypto::provider()))
                        .build(),
                )
                .max_redirects(0)
                .timeout_connect(Some(std::time::Duration::from_secs(10)))
                .timeout_global(Some(std::time::Duration::from_secs(30)))
                .build()
                .into();
            let read = |mut response: ureq::http::Response<ureq::Body>| -> Result<Value, SpeechError> {
                let body = response.body_mut().read_to_string().map_err(|_| SpeechError::Model("Could not read AssemblyAI response".into()))?;
                serde_json::from_str(&body).map_err(|_| SpeechError::Model("Invalid AssemblyAI response".into()))
            };
            let upload = read(
                agent
                    .post("https://api.assemblyai.com/v2/upload")
                    .header("authorization", &self.key)
                    .header("content-type", "audio/wav")
                    .send(wav.as_slice())
                    .map_err(http_error)?,
            )?;
            if !progress(0.15, "Submitting to AssemblyAI") {
                return Err(SpeechError::Cancelled);
            }
            let url = upload["upload_url"].as_str().ok_or_else(|| SpeechError::Model("AssemblyAI upload URL is missing".into()))?;
            if !url.starts_with("https://cdn.assemblyai.com/") {
                return Err(SpeechError::Model("Unexpected AssemblyAI upload URL".into()));
            }
            let request = json!({"audio_url":url,"speech_models":["universal-3-pro","universal-2"],
                "language_code":opts.language,"disfluencies":true,"punctuate":true,"format_text":false});
            let submitted = read(
                agent
                    .post("https://api.assemblyai.com/v2/transcript")
                    .header("authorization", &self.key)
                    .header("content-type", "application/json")
                    .send(request.to_string())
                    .map_err(http_error)?,
            )?;
            let id = submitted["id"]
                .as_str()
                .filter(|id| !id.is_empty() && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-'))
                .ok_or_else(|| SpeechError::Model("AssemblyAI transcript ID is missing".into()))?;
            let endpoint = format!("https://api.assemblyai.com/v2/transcript/{id}");
            let result = (|| {
                let started = std::time::Instant::now();
                loop {
                    if !progress(0.2, "Waiting for AssemblyAI transcription") {
                        return Err(SpeechError::Cancelled);
                    }
                    if started.elapsed().as_secs() > 1800 {
                        return Err(SpeechError::Model("AssemblyAI transcription timed out; resume to try the unfinished chunk again".into()));
                    }
                    let value = read(agent.get(&endpoint).header("authorization", &self.key).call().map_err(http_error)?)?;
                    let response: Response = serde_json::from_value(value).map_err(|_| SpeechError::Model("Invalid AssemblyAI transcript response".into()))?;
                    match response.status.as_str() {
                        "completed" => return transcript(response, opts, audio.len()),
                        "error" => {
                            return Err(SpeechError::Model("AssemblyAI could not transcribe this audio. Check the account dashboard for details.".into()));
                        }
                        "queued" | "processing" => {}
                        _ => return Err(SpeechError::Model("Unknown AssemblyAI transcription status".into())),
                    }
                    for _ in 0..30 {
                        if !progress(0.2, "Waiting for AssemblyAI transcription") {
                            return Err(SpeechError::Cancelled);
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            })();
            // Release the remote transcript after receiving its local word timestamps or canceling.
            let _ = agent.delete(&endpoint).header("authorization", &self.key).call();
            result
        }
    }
}

#[test]
fn timestamps_are_milliseconds_and_confidence_is_preserved() {
    let response = Response { status: "completed".into(), words: Some(vec![TimedWord { text: "um".into(), start: 125, end: 450, confidence: 0.8 }]) };
    let t = transcript(response, &Options::default(), SAMPLE_RATE as usize).unwrap();
    assert_eq!(t.words[0].start, Tick::from_units(125, 1000));
    assert_eq!(t.words[0].end, Tick::from_units(450, 1000));
    assert_eq!(t.words[0].confidence, 0.8);
}

#[test]
fn invalid_word_timing_is_rejected() {
    let response = Response { status: "completed".into(), words: Some(vec![TimedWord { text: "bad".into(), start: 450, end: 125, confidence: 0.8 }]) };
    assert!(transcript(response, &Options::default(), SAMPLE_RATE as usize).is_err());
}

#[cfg(feature = "download")]
#[test]
fn cancellation_before_upload_needs_no_network() {
    let provider = AssemblyAi::new("test-only-key".into()).unwrap();
    let result = provider.transcribe(&[0.0; 160], &Options::default(), &mut |_, _| false);
    assert_eq!(result.unwrap_err(), SpeechError::Cancelled);
}
