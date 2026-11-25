use serde_json::{Value, json};
use std::{
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub(super) struct Server {
    child: Child,
    url: String,
    key: String,
    _files: tempfile::TempDir,
    pub peak_bytes: Option<u64>,
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn command(exe: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut cmd = shotclutch_env::worker_command(exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}
fn request(url: &str, key: &str, body: Option<Vec<u8>>) -> Result<String, String> {
    let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(180))).proxy(None).build().new_agent();
    let auth = format!("Bearer {key}");
    let mut response = if let Some(body) = body {
        agent.post(url).header("Authorization", &auth).header("Content-Type", "application/json").send(body.as_slice())
    } else {
        agent.get(url).header("Authorization", &auth).call()
    }
    .map_err(|e| format!("Local reviewer request failed: {e}"))?;
    response.body_mut().with_config().limit(128 * 1024).read_to_string().map_err(|e| e.to_string())
}
impl Server {
    pub fn start(model: &Path, cancel: &AtomicBool) -> Result<Self, String> {
        let files = tempfile::tempdir().map_err(|e| e.to_string())?;
        let key = uuid::Uuid::new_v4().to_string();
        let auth = files.path().join("auth");
        std::fs::write(&auth, &key).map_err(|e| e.to_string())?;
        let port = std::net::TcpListener::bind(("127.0.0.1", 0)).map_err(|e| e.to_string())?.local_addr().map_err(|e| e.to_string())?.port();
        let path = std::env::var_os("SHOTCLUTCH_LLAMA_SERVER")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                let executable = std::env::current_exe().ok()?;
                let parent = executable.parent()?;
                ["llama-vulkan", "llama"]
                    .into_iter()
                    .map(|dir| parent.join(dir).join(format!("llama-server{}", std::env::consts::EXE_SUFFIX)))
                    .find(|p| p.is_file())
            })
            .unwrap_or_else(|| "llama-server".into());
        let log = std::fs::File::create(files.path().join("server.log")).map_err(|e| e.to_string())?;
        let threads = std::thread::available_parallelism().map_or(4, usize::from).min(8).to_string();
        let child = command(path)
            .arg("-m")
            .arg(model)
            .args(["--host", "127.0.0.1", "--port", &port.to_string(), "--api-key-file"])
            .arg(&auth)
            .args([
                "-c",
                "16384",
                "--parallel",
                "1",
                "-t",
                &threads,
                "-tb",
                &threads,
                "--jinja",
                "--reasoning",
                "off",
                "--chat-template-kwargs",
                r#"{"enable_thinking":false}"#,
                "--fit",
                "on",
                "--fit-target",
                "768",
                "--fit-ctx",
                "16384",
                "--no-warmup",
                "--no-webui",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .map_err(|e| format!("Cannot launch local llama-server: {e}"))?;
        let mut server = Self { child, url: format!("http://127.0.0.1:{port}"), key, _files: files, peak_bytes: None };
        let start = Instant::now();
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err("Take review cancelled".into());
            }
            if server.child.try_wait().map_err(|e| e.to_string())?.is_some() {
                return Err("Local reviewer exited during startup; check its model and available memory".into());
            }
            if std::net::TcpStream::connect_timeout(&format!("127.0.0.1:{port}").parse().unwrap(), Duration::from_millis(100)).is_ok() {
                let url = format!("{}/health", server.url);
                if request(&url, &server.key, None).is_ok() {
                    break;
                }
            }
            if start.elapsed() > Duration::from_secs(240) {
                return Err("Local reviewer startup timed out".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(server)
    }
    pub fn chat(&mut self, system: &str, data: &Value, schema: &Value, cancel: &AtomicBool) -> Result<Value, String> {
        let body = serde_json::to_vec(&json!({"messages":[{"role":"system","content":system},{"role":"user","content":data.to_string()}],
            "temperature":0,"seed":0,"max_tokens":2048,"stream":false,"chat_template_kwargs":{"enable_thinking":false},
            "response_format":{"type":"json_schema","json_schema":{"name":"take_decision","strict":true,"schema":schema}}}))
        .map_err(|e| e.to_string())?;
        let url = self.url.clone();
        let key = self.key.clone();
        let (send, recv) = mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| {
                let payload: Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
                let prompt: Value = serde_json::from_str(&request(
                    &format!("{url}/apply-template"),
                    &key,
                    Some(
                        serde_json::to_vec(
                            &json!({"messages":payload["messages"],"add_generation_prompt":true,"chat_template_kwargs":{"enable_thinking":false}}),
                        )
                        .unwrap(),
                    ),
                )?)
                .map_err(|e| e.to_string())?;
                let tokens: Value = serde_json::from_str(&request(
                    &format!("{url}/tokenize"),
                    &key,
                    Some(serde_json::to_vec(&json!({"content":prompt["prompt"],"add_special":true})).unwrap()),
                )?)
                .map_err(|e| e.to_string())?;
                if tokens["tokens"].as_array().is_none_or(|t| t.len() + 2048 > 16384) {
                    return Err("Complete attempts exceed the model context; no text was truncated".into());
                }
                request(&format!("{url}/v1/chat/completions"), &key, Some(body))
            })();
            let _ = send.send(result);
        });
        loop {
            if cancel.load(Ordering::Relaxed) {
                let _ = self.child.kill();
                let _ = self.child.wait();
                return Err("Take review cancelled".into());
            }
            match recv.recv_timeout(Duration::from_millis(100)) {
                Ok(result) => {
                    let result: Value = serde_json::from_str(&result?).map_err(|e| e.to_string())?;
                    if result["choices"][0]["finish_reason"] != "stop" {
                        return Err("Incomplete reviewer response".into());
                    }
                    let content = result["choices"][0]["message"]["content"].as_str().ok_or("Missing reviewer content")?;
                    return serde_json::from_str(content).map_err(|e| e.to_string());
                }
                Err(mpsc::RecvTimeoutError::Timeout) => (),
                Err(_) => return Err("Local reviewer worker stopped".into()),
            }
        }
    }
    pub fn measure_peak(&mut self) {
        #[cfg(windows)]
        {
            if let Ok(out) = command("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-Command", &format!("(Get-Process -Id {}).PeakWorkingSet64", self.child.id())])
                .output()
            {
                self.peak_bytes = String::from_utf8_lossy(&out.stdout).trim().parse().ok();
            }
        }
        #[cfg(target_os = "linux")]
        {
            if let Ok(s) = std::fs::read_to_string(format!("/proc/{}/status", self.child.id())) {
                self.peak_bytes =
                    s.lines().find(|s| s.starts_with("VmHWM:")).and_then(|s| s.split_whitespace().nth(1)).and_then(|s| s.parse::<u64>().ok()).map(|n| n * 1024);
            }
        }
    }
}
