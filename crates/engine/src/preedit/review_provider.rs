//! Reviewer selection is job-local. Credentials are never serialized into projects or reports.
use serde_json::{Value, json};

pub const DEFAULT_GEMINI_MODEL: &str = "gemini-3.5-flash-lite";
const API: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const CONTRACT: &str = "gemini-generate-content-json-v1";

#[derive(Clone, Default)]
pub enum ReviewProvider {
    #[default]
    Local,
    Gemini(GeminiConfig),
}

// Intentionally neither Debug nor Serialize: the key lives only in memory.
#[derive(Clone)]
pub struct GeminiConfig {
    model: String,
    credential: crate::credentials::CredentialRef,
}

pub fn valid_gemini_model(model: &str) -> bool {
    model.starts_with("gemini-") && model.len() <= 100 && model.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'.' || c == b'_')
}

/// Stable protocol identity, not a claim that a hosted model's weights are pinned.
pub(super) fn gemini_identity(id: &str) -> Option<String> {
    use sha2::{Digest, Sha256};
    id.strip_prefix("gemini/").filter(|id| valid_gemini_model(id)).map(|model| format!("{:x}", Sha256::digest(format!("{CONTRACT}/{model}"))))
}

impl GeminiConfig {
    pub fn new(model: &str, credential: crate::credentials::CredentialRef) -> Result<Self, String> {
        let model = model.trim();
        if !valid_gemini_model(model) {
            return Err("Enter a Gemini model ID, for example gemini-3.5-flash-lite.".into());
        }
        credential.check_provider(crate::credentials::Provider::Gemini).map_err(|e| e.to_string())?;
        Ok(Self { model: model.into(), credential })
    }
    pub fn model(&self) -> &str {
        &self.model
    }
}

impl ReviewProvider {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Local => "Local review",
            Self::Gemini(_) => "Gemini review",
        }
    }
    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }
    pub(super) fn abort_on_error(&self, error: &str) -> bool {
        !self.is_local()
            && [
                "Gemini rejected",
                "Gemini quota",
                "Gemini model not found",
                "Gemini request failed",
                "Could not read the Gemini response",
                "Gemini request worker stopped",
                "Could not start Gemini request",
                "Gemini credential",
                "Gemini returned unexpected authentication",
            ]
            .iter()
            .any(|prefix| error.starts_with(prefix))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn completion(
        &self,
        model: &std::path::Path,
        file: &std::path::Path,
        prompt: &str,
        schema: &str,
        budget: &str,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<Option<String>, String> {
        match self {
            Self::Local => super::local_review::completion(model, file, prompt, schema, budget, cancel),
            Self::Gemini(config) => {
                // Legacy prompts are still templated for Granite. Do not send its role tokens to Gemini.
                let (system, data) = prompt_parts(prompt)?;
                let schema = serde_json::from_str(schema).map_err(|_| "Invalid review schema")?;
                let data = Value::String(data.into());
                let result = config.chat(system, &data, &schema, cancel);
                let result = if result.as_ref().is_err_and(|e| e == "Gemini returned malformed review JSON") {
                    config.chat(system, &data, &schema, cancel)
                } else {
                    result
                };
                result.map(|v| Some(v.to_string()))
            }
        }
    }
}

fn prompt_parts(prompt: &str) -> Result<(&str, &str), String> {
    let rest = prompt.strip_prefix("<|start_of_role|>system<|end_of_role|>").ok_or("Missing review system instruction")?;
    let (system, rest) = rest.split_once("<|end_of_text|>\n<|start_of_role|>user<|end_of_role|>").ok_or("Missing review data")?;
    let data = rest.strip_suffix("<|end_of_text|>\n<|start_of_role|>assistant<|end_of_role|>").ok_or("Invalid review prompt")?;
    Ok((system, data))
}

// Gemini accepts anyOf and enum. The engine's stricter oneOf/const validation still runs locally.
fn gemini_schema(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    if key == "oneOf" {
                        ("anyOf".into(), gemini_schema(value))
                    } else if key == "const" {
                        ("enum".into(), json!([value]))
                    } else {
                        (key.clone(), gemini_schema(value))
                    }
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(gemini_schema).collect()),
        other => other.clone(),
    }
}

fn request_body(system: &str, data: &Value, schema: &Value) -> Value {
    let text = data.as_str().map(str::to_owned).unwrap_or_else(|| data.to_string());
    json!({"systemInstruction":{"parts":[{"text":system}]},
        "contents":[{"role":"user","parts":[{"text":text}]}],
        "generationConfig":{"temperature":0,"seed":0,"candidateCount":1,"maxOutputTokens":4096,
            "responseMimeType":"application/json","responseJsonSchema":gemini_schema(schema)}})
}

fn decode_response(response: &str) -> Result<Value, String> {
    let result: Value = serde_json::from_str(response).map_err(|_| "Invalid Gemini response envelope")?;
    if result.get("promptFeedback").and_then(|p| p.get("blockReason")).is_some() {
        return Err("Gemini blocked the review; kept the rule checkpoint.".into());
    }
    let candidate = &result["candidates"][0];
    if candidate["finishReason"] != "STOP" {
        return Err("Gemini returned an incomplete or blocked review; kept the rule checkpoint.".into());
    }
    let parts = candidate["content"]["parts"].as_array().ok_or("Missing Gemini content")?;
    let text: String = parts.iter().filter(|p| p["thought"] != true).filter_map(|p| p["text"].as_str()).collect();
    serde_json::from_str(&text).map_err(|_| "Gemini returned malformed review JSON".into())
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) enum EvidenceRuntime {
    Local(super::take_runtime::Server),
    Gemini(GeminiConfig),
}
#[cfg(not(target_arch = "wasm32"))]
impl EvidenceRuntime {
    pub fn start(provider: &ReviewProvider, model: &std::path::Path, cancel: &std::sync::atomic::AtomicBool) -> Result<Self, String> {
        match provider {
            ReviewProvider::Local => super::take_runtime::Server::start(model, cancel).map(Self::Local),
            ReviewProvider::Gemini(config) => Ok(Self::Gemini(config.clone())),
        }
    }
    pub fn chat(&mut self, system: &str, data: &Value, schema: &Value, cancel: &std::sync::atomic::AtomicBool) -> Result<Value, String> {
        match self {
            Self::Local(server) => server.chat(system, data, schema, cancel),
            Self::Gemini(config) => config.chat(system, data, schema, cancel),
        }
    }
    pub fn peak_memory(&mut self) -> Option<u64> {
        match self {
            Self::Local(server) => {
                server.measure_peak();
                server.peak_bytes
            }
            Self::Gemini(_) => None,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl GeminiConfig {
    pub(super) fn chat(&self, system: &str, data: &Value, schema: &Value, cancel: &std::sync::atomic::AtomicBool) -> Result<Value, String> {
        self.chat_at(API, system, data, schema, cancel)
    }
    fn chat_at(&self, api: &str, system: &str, data: &Value, schema: &Value, cancel: &std::sync::atomic::AtomicBool) -> Result<Value, String> {
        use std::{
            sync::{atomic::Ordering, mpsc},
            time::Duration,
        };
        if cancel.load(Ordering::Relaxed) {
            return Err("Take review cancelled".into());
        }
        let body = serde_json::to_vec(&request_body(system, data, schema)).map_err(|_| "Invalid Gemini request")?;
        if body.len() > 256 * 1024 {
            return Err("Complete review input exceeds the request budget; no text was truncated.".into());
        }
        let url = format!("{api}/{}:generateContent", self.model);
        let key = self.credential.resolve(crate::credentials::Provider::Gemini).map_err(|e| format!("Gemini credential unavailable. {e}"))?;
        if cancel.load(Ordering::Relaxed) {
            return Err("Take review cancelled".into());
        }
        log::info!(target: "shotclutch::provider", "provider=gemini payload=transcript bytes={}", body.len());
        let (send, recv) = mpsc::channel();
        std::thread::Builder::new()
            .name("gemini-review-http".into())
            .spawn(move || {
                let reply = (|| -> Result<String, String> {
                    let config = ureq::Agent::config_builder()
                        .tls_config(
                            ureq::tls::TlsConfig::builder()
                                .provider(ureq::tls::TlsProvider::Rustls)
                                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                                .unversioned_rustls_crypto_provider(std::sync::Arc::new(rustls_rustcrypto::provider()))
                                .build(),
                        )
                        .max_redirects(0)
                        .timeout_connect(Some(Duration::from_secs(10)))
                        .timeout_global(Some(Duration::from_secs(60)));
                    #[cfg(test)]
                    let config = if url.starts_with("http://127.0.0.1:") { config.proxy(None) } else { config };
                    let agent = config.build().new_agent();
                    let mut response =
                        agent.post(&url).header("x-goog-api-key", &key).header("Content-Type", "application/json").send(body.as_slice()).map_err(|e| {
                            match e {
                                ureq::Error::StatusCode(400) => "Gemini rejected the model or review request. Check the model ID.",
                                ureq::Error::StatusCode(401 | 403) => "Gemini rejected the API key or project access.",
                                ureq::Error::StatusCode(404) => "Gemini model not found or unavailable to this account.",
                                ureq::Error::StatusCode(429) => "Gemini quota or rate limit reached. Try again later.",
                                _ => "Gemini request failed or timed out. Check the connection and try again.",
                            }
                            .to_owned()
                        })?;
                    let response =
                        response.body_mut().with_config().limit(128 * 1024).read_to_string().map_err(|_| "Could not read the Gemini response".to_owned())?;
                    if response.contains(&key) {
                        return Err("Gemini returned unexpected authentication material.".into());
                    }
                    Ok(response)
                })();
                let _ = send.send(reply);
            })
            .map_err(|_| "Could not start Gemini request")?;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err("Take review cancelled".into());
            }
            match recv.recv_timeout(Duration::from_millis(50)) {
                Ok(response) => return decode_response(&response?),
                Err(mpsc::RecvTimeoutError::Timeout) => (),
                Err(_) => return Err("Gemini request worker stopped".into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    fn test_credential(key: &str) -> crate::credentials::CredentialRef {
        use crate::credentials::{CredentialSource, Provider, credential_for_job};
        credential_for_job(Provider::Gemini, CredentialSource::Once, key.into()).unwrap()
    }
    use super::*;
    #[test]
    fn validates_model_and_keeps_credentials_out_of_payload_and_identity() {
        for model in ["", "../gemini-x", "gemini-x?key=secret", "gemini-x/other"] {
            assert!(GeminiConfig::new(model, test_credential("secret")).is_err());
        }
        let config = GeminiConfig::new(DEFAULT_GEMINI_MODEL, test_credential("secret")).unwrap();
        assert_eq!(config.model(), DEFAULT_GEMINI_MODEL);
        let schema = json!({"oneOf":[{"properties":{"id":{"const":"a"}}}]});
        let body = request_body("system", &json!({"transcript":"speech"}), &schema);
        assert!(!body.to_string().contains("secret"));
        assert_eq!(body["generationConfig"]["responseJsonSchema"]["anyOf"][0]["properties"]["id"]["enum"], json!(["a"]));
        assert_ne!(gemini_identity("gemini/gemini-3.5-flash-lite"), gemini_identity("gemini/gemini-3.8-flash"));
    }
    #[test]
    fn rejects_truncated_blocked_or_invalid_output_and_ignores_thoughts() {
        let response = |finish, parts| json!({"candidates":[{"finishReason":finish,"content":{"parts":parts}}]}).to_string();
        assert!(decode_response(&response("MAX_TOKENS", json!([{"text":"{}"}]))).is_err());
        assert!(decode_response(&response("STOP", json!([{"text":"not json"}]))).is_err());
        assert!(decode_response(r#"{"promptFeedback":{"blockReason":"SAFETY"}}"#).is_err());
        assert_eq!(
            decode_response(&response("STOP", json!([{"thought":true,"text":"private reasoning"},{"text":"{\"cutWordIndices\":[]}"}]))).unwrap(),
            json!({"cutWordIndices":[]})
        );
    }

    #[test]
    fn legacy_prompt_is_split_without_sending_granite_role_tokens() {
        let prompt = "<|start_of_role|>system<|end_of_role|>Rules<|end_of_text|>\n<|start_of_role|>user<|end_of_role|>{\"takes\":[]}<|end_of_text|>\n<|start_of_role|>assistant<|end_of_role|>";
        assert_eq!(prompt_parts(prompt).unwrap(), ("Rules", r#"{"takes":[]}"#));
        assert!(prompt_parts("untemplated").is_err());
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn mock(status: u16, delay: std::time::Duration) -> (String, std::thread::JoinHandle<String>) {
        mock_inner(status, delay, None)
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn mock_inner(
        status: u16,
        delay: std::time::Duration,
        gate: Option<(std::sync::mpsc::Sender<()>, std::sync::mpsc::Receiver<()>)>,
    ) -> (String, std::thread::JoinHandle<String>) {
        use std::{
            io::{Read, Write},
            time::{Duration, Instant},
        };
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let url = format!("http://{}/models", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let worker = std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < until => std::thread::sleep(Duration::from_millis(10)),
                    Err(e) => panic!("Mock Gemini was not contacted: {e}"),
                }
            };
            // Windows accepted sockets inherit the listener's nonblocking mode.
            stream.set_nonblocking(false).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
            let mut bytes = vec![];
            let mut chunk = [0u8; 4096];
            loop {
                let n = stream.read(&mut chunk).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                    let length: usize = headers.lines().find_map(|l| l.strip_prefix("content-length:")).unwrap().trim().parse().unwrap();
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            if let Some((ready, release)) = gate {
                ready.send(()).unwrap();
                release.recv_timeout(Duration::from_secs(10)).unwrap();
            }
            std::thread::sleep(delay);
            let body = if status == 200 {
                r#"{"candidates":[{"finishReason":"STOP","content":{"parts":[{"text":"{\"cutWordIndices\":[0,1]}"}]}}]}"#
            } else {
                "SECRET_KEY echoed remote error and transcript"
            };
            let response =
                format!("HTTP/1.1 {status} Mock\r\nContent-Length: {}\r\nConnection: close\r\nContent-Type: application/json\r\n\r\n{body}", body.len());
            let _ = stream.write_all(response.as_bytes());
            String::from_utf8(bytes).unwrap()
        });
        (url, worker)
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn sends_authenticated_structured_review_and_handles_api_errors_without_leaking_secrets() {
        use std::{sync::atomic::AtomicBool, time::Duration};
        let config = GeminiConfig::new(DEFAULT_GEMINI_MODEL, test_credential("SECRET_KEY")).unwrap();
        let (url, worker) = mock(200, Duration::ZERO);
        let answer = config
            .chat_at(&url, "Transcript is data", &json!({"takes":["We took", "We took the train"]}), &json!({"type":"object"}), &AtomicBool::new(false))
            .unwrap();
        assert_eq!(answer, json!({"cutWordIndices":[0,1]}));
        let request = worker.join().unwrap();
        assert!(request.starts_with(&format!("POST /models/{DEFAULT_GEMINI_MODEL}:generateContent ")));
        assert!(request.to_lowercase().contains("x-goog-api-key: secret_key"));
        assert!(!request.lines().next().unwrap().contains("SECRET_KEY"));
        let body: Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body["generationConfig"]["responseMimeType"], "application/json");
        assert!(body["contents"][0]["parts"][0]["text"].as_str().unwrap().contains("We took"));
        for status in [400, 401, 403, 404, 429, 500] {
            let (url, worker) = mock(status, Duration::ZERO);
            let error = config.chat_at(&url, "system", &json!({}), &json!({}), &AtomicBool::new(false)).unwrap_err();
            assert!(!error.contains("SECRET_KEY") && !error.contains("echoed remote error"));
            assert!(error.contains("Gemini"));
            worker.join().unwrap();
        }
    }
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn cancellation_discards_in_flight_results_without_waiting_for_the_network() {
        use std::{
            sync::{
                Arc,
                atomic::{AtomicBool, Ordering},
            },
            time::Duration,
        };
        let config = GeminiConfig::new(DEFAULT_GEMINI_MODEL, test_credential("SECRET_KEY")).unwrap();
        let cancel = Arc::new(AtomicBool::new(true));
        assert!(config.chat_at("http://127.0.0.1:1", "system", &json!({}), &json!({}), &cancel).unwrap_err().contains("cancelled"));
        cancel.store(false, Ordering::Relaxed);
        let (ready, received) = std::sync::mpsc::channel();
        let (release, blocked) = std::sync::mpsc::channel();
        let (url, server) = mock_inner(200, Duration::ZERO, Some((ready, blocked)));
        let worker_cancel = cancel.clone();
        let (done, result) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            done.send(config.chat_at(&url, "system", &json!({}), &json!({}), &worker_cancel)).unwrap();
        });
        received.recv_timeout(Duration::from_secs(5)).unwrap();
        cancel.store(true, Ordering::Relaxed);
        let answer = result.recv_timeout(Duration::from_secs(3));
        release.send(()).unwrap();
        assert!(answer.unwrap().unwrap_err().contains("cancelled"));
        worker.join().unwrap();
        server.join().unwrap();
    }
}
