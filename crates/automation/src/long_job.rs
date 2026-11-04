//! Long exports over MCP (docs/agents.md § Long exports): `file.exportMedia` with `wait: true`
//! runs as a background engine job; the server reports its progress as MCP
//! `notifications/progress` (when the request carried a `progressToken`) and stops it on
//! `notifications/cancelled`. The engine owns staging cleanup and final-file publication. The session lock is only held for
//! short polls, so other requests are answered while the export runs.

use std::time::Duration;

use rmcp::RoleServer;
use rmcp::model::{CallToolResult, ContentBlock as Content, ProgressNotificationParam, ProgressToken};
use rmcp::service::{Peer, RequestContext};
use serde_json::{Value, json};

use crate::server::ShotClutchMcp;

/// Engine commands that block until a render is written when called with `wait: true`.
pub const LONG_COMMANDS: &[&str] = &["file.exportMedia"];

/// How often the job is polled (and at most how often progress is reported).
const POLL: Duration = Duration::from_millis(100);

/// Whether `command_run {id, params}` is a long call this module runs.
pub fn is_long(id: &str, params: &Value) -> bool {
    LONG_COMMANDS.contains(&id) && params.get("wait").and_then(Value::as_bool) == Some(true)
}

/// Sends `notifications/progress` for one request; progress only ever increases.
struct Reporter {
    peer: Peer<RoleServer>,
    token: Option<ProgressToken>,
    last: f64,
}

impl Reporter {
    async fn report(&mut self, done: f64, total: f64, message: &str) {
        let Some(token) = self.token.clone() else { return };
        if total <= 0.0 || done <= self.last {
            return;
        }
        self.last = done;
        let mut p = ProgressNotificationParam::new(token, done).with_total(total);
        if !message.is_empty() {
            p = p.with_message(message);
        }
        let _ = self.peer.notify_progress(p).await;
    }
}

impl ShotClutchMcp {
    /// Run `id` (a [`LONG_COMMANDS`] entry, `wait: true`) as a job, reporting progress and
    /// honouring cancellation.
    pub(crate) async fn run_long(&self, id: &str, mut params: Value, context: &RequestContext<RoleServer>) -> CallToolResult {
        if let Some(p) = params.as_object_mut() {
            p.insert("wait".into(), json!(false));
        }
        let start = match self.run(id, params).await {
            Ok(v) => v,
            Err(e) => return CallToolResult::error(vec![Content::text(e.to_string())]),
        };
        let Some(job) = start.get("job").and_then(Value::as_u64) else {
            return CallToolResult::error(vec![Content::text(format!("{id} started no job: {start}"))]);
        };
        let mut rep = Reporter { peer: context.peer.clone(), token: context.meta.get_progress_token(), last: 0.0 };
        loop {
            let state = self.job_state(job).await;
            let finished = state.as_ref().is_none_or(|j| j["finished"].as_bool() == Some(true));
            if let Some(j) = &state {
                let (done, total) = (j["done"].as_f64().unwrap_or(0.0), j["total"].as_f64().unwrap_or(0.0));
                rep.report(done, total, j["status"].as_str().unwrap_or_default()).await;
            }
            if finished {
                return completed(start, state);
            }
            tokio::select! {
                _ = context.ct.cancelled() => break,
                _ = tokio::time::sleep(POLL) => {}
            }
        }
        // The worker owns its private staging files. Never infer ownership of a final path
        // from its timestamp: it may be an existing delivery or a completed racing export.
        let _ = self.run("jobs.cancel", json!({"job": job})).await;
        for _ in 0..600 {
            let state = self.job_state(job).await;
            if state.as_ref().is_none_or(|j| j["finished"].as_bool() == Some(true)) {
                return completed(start, state);
            }
            tokio::time::sleep(POLL).await;
        }
        CallToolResult::error(vec![Content::text(format!(
            "CANCELED: cancellation requested for job {job}; the worker has not confirmed completion; inspect jobs.list"
        ))])
    }

    /// The job's `jobs.list` entry.
    async fn job_state(&self, job: u64) -> Option<Value> {
        let jobs = self.run("jobs.list", json!({})).await.ok()?;
        jobs.as_array()?.iter().find(|j| j["id"].as_u64() == Some(job)).cloned()
    }
}

/// Forward the actual engine result, including a delivery that committed before cancellation.
fn completed(mut start: Value, state: Option<Value>) -> CallToolResult {
    let Some(state) = state else {
        return CallToolResult::error(vec![Content::text("NOT_FOUND: export job is unavailable; inspect jobs.list before retrying")]);
    };
    let result = state["result"].clone();
    if let Some(error) = result.get("error").and_then(Value::as_str) {
        return CallToolResult::error(vec![Content::text(format!("export failed: {error}"))]);
    }
    if !result.is_object() {
        return CallToolResult::error(vec![Content::text("INTERNAL_ERROR: export has no confirmed result; inspect jobs.list")]);
    }
    start["result"] = result;
    for field in ["offlineMedia", "binding"] {
        if let Some(value) = state.get(field) {
            start[field] = value.clone();
        }
    }
    CallToolResult::success(vec![Content::text(serde_json::to_string_pretty(&start).unwrap_or_default())])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn m1_completed_export_receipt_wins_a_cancellation_race() {
        let result = completed(
            json!({"job": 4, "path": "delivery.mov"}),
            Some(json!({"finished": true, "result": {"frames": 12}, "binding": {"projectId": "project", "publication": "atomicFile"}, "offlineMedia": []})),
        );
        assert_ne!(result.is_error, Some(true));
        let wire = serde_json::to_value(result).unwrap();
        let receipt: Value = serde_json::from_str(wire["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(receipt["binding"]["publication"], "atomicFile");
        assert_eq!(receipt["result"]["frames"], 12);
    }
    #[test]
    fn m1_missing_or_incomplete_jobs_cannot_claim_export_success() {
        for state in [None, Some(json!({"finished": true})), Some(json!({"finished": true, "result": {"error": "cancelled"}}))] {
            assert_eq!(completed(json!({"job": 1}), state).is_error, Some(true));
        }
    }
}
