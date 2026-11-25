use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Limits {
    pub elapsed_ms: u64,
    pub tool_calls: u64,
    pub source_bytes: u64,
    pub frames: u64,
    /// Cumulative duration of inspected source clips and composed audio windows.
    pub inspected_ticks: u64,
    pub retries: u64,
    /// Total explicit clip targets for the single split transaction.
    pub transaction_clips: usize,
}

impl Limits {
    pub(super) fn validate(self) -> Result<Self, ServiceError> {
        if !(1..=3_600_000).contains(&self.elapsed_ms) || self.retries > 1 || !(1..=128).contains(&self.transaction_clips) {
            return Err(error(ErrorCode::InvalidArgument, "invalid elapsed, retry, or transaction limit"));
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub tool_calls: u64,
    pub source_bytes: u64,
    pub frames: u64,
    pub inspected_ticks: u64,
    pub retries: u64,
}

#[derive(Clone, Copy)]
pub(crate) enum Work {
    Tool,
    SourceBytes,
    Frames,
    InspectedTicks,
}

pub(in crate::service) struct Meter {
    pub(in crate::service) deadline: Instant,
    pub(in crate::service) run_id: DocumentId,
    path: PathBuf,
    state: Mutex<State>,
}

struct State {
    /// Durable upper bound on work if the process stops during this dispatch.
    reserved: Checkpoint,
    actual: Usage,
    exhausted: Option<&'static str>,
}

impl Meter {
    pub(super) fn new(run: &AgentRun) -> Self {
        let elapsed = now_ms().saturating_sub(run.checkpoint.started_ms).max(run.checkpoint.status.elapsed_ms);
        let remaining = run.checkpoint.limits.elapsed_ms.saturating_sub(elapsed);
        Self {
            deadline: run.deadline.min(Instant::now() + Duration::from_millis(remaining.min(3_600_000))),
            run_id: run.checkpoint.status.id,
            path: run.path.clone(),
            state: Mutex::new(State { reserved: run.checkpoint.clone(), actual: run.checkpoint.status.usage, exhausted: None }),
        }
    }

    pub(in crate::service) fn check(&self) -> Result<(), ServiceError> {
        if Instant::now() >= self.deadline {
            return Err(error(ErrorCode::ResourceLimit, "agent elapsed time limit reached"));
        }
        if let Some(message) = self.state.lock().unwrap_or_else(|err| err.into_inner()).exhausted {
            return Err(error(ErrorCode::ResourceLimit, message));
        }
        Ok(())
    }

    pub(super) fn usage(&self) -> Usage {
        self.state.lock().unwrap_or_else(|err| err.into_inner()).actual
    }

    pub(in crate::service) fn charge(&self, work: Work, units: u64) -> Result<(), ServiceError> {
        self.check()?;
        let mut state = self.state.lock().unwrap_or_else(|err| err.into_inner());
        let State { reserved, actual, exhausted } = &mut *state;
        let limits = &reserved.limits;
        let saved = &mut reserved.status.usage;
        let (used, saved, limit, block, message) = match work {
            Work::Tool => (&mut actual.tool_calls, &mut saved.tool_calls, limits.tool_calls, 1, "agent tool call budget exhausted"),
            Work::SourceBytes => (&mut actual.source_bytes, &mut saved.source_bytes, limits.source_bytes, 1024 * 1024, "agent source byte budget exhausted"),
            Work::Frames => (&mut actual.frames, &mut saved.frames, limits.frames, 4, "agent frame budget exhausted"),
            Work::InspectedTicks => {
                (&mut actual.inspected_ticks, &mut saved.inspected_ticks, limits.inspected_ticks, 1, "agent inspected duration budget exhausted")
            }
        };
        let Some(next) = used.checked_add(units).filter(|next| *next <= limit) else {
            *exhausted = Some(message);
            return Err(error(ErrorCode::ResourceLimit, message));
        };
        if next > *saved {
            *saved = next.saturating_add(block - 1).min(limit);
            write_checkpoint(&self.path, reserved)?;
        }
        *used = next;
        Ok(())
    }
}
