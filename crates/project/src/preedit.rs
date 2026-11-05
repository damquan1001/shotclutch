//! Durable review state for a nondestructive rough cut.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use shotclutch_time::{Tick, TimeRange};

use crate::{ClipId, ItemId, Transcript};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PreEdit {
    pub analyses: BTreeMap<u64, Arc<PreEditAnalysis>>,
    pub maps: BTreeMap<u64, Arc<PreEditMap>>,
    pub applications: BTreeMap<u64, Arc<PreEditApplication>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceBinding {
    pub item: ItemId,
    pub range: TimeRange,
    pub audio_stream: usize,
    /// The channel used for recognition, or None to analyze an equal-weight mono downmix.
    pub channel: Option<u16>,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisSettings {
    pub model: Option<String>,
    pub language: String,
    pub min_pause: Tick,
    pub handle: Tick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalysisStatus {
    Running,
    Complete,
    Canceled,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnalysisChunk {
    pub coverage: TimeRange,
    /// Absolute source time. None denotes a silence-only chunk.
    pub transcript: Option<Transcript>,
    pub quiet: Vec<TimeRange>,
    /// Peak amplitude in equal-duration bins, quantized for the waveform display.
    #[serde(default)]
    pub peaks: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreEditAnalysis {
    pub source: SourceBinding,
    pub settings: AnalysisSettings,
    pub analyzer_version: u32,
    pub model_digest: Option<String>,
    pub status: AnalysisStatus,
    pub chunks: Vec<Arc<AnalysisChunk>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CandidateKind {
    Pause,
    Filler,
    Restart,
    Take,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub range: TimeRange,
    pub kind: CandidateKind,
    pub reason: String,
    pub confidence: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TakeGroup {
    /// Indices into the immutable candidate list of this map.
    pub alternatives: Vec<usize>,
    pub explanation: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    Keep,
    Remove,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Override {
    pub range: TimeRange,
    pub decision: Decision,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreEditMap {
    pub analysis: u64,
    pub previous: Option<u64>,
    pub reviewed: bool,
    pub candidates: Arc<Vec<Candidate>>,
    pub takes: Arc<Vec<TakeGroup>>,
    /// Ordered user actions. The most recent decision wins where ranges overlap.
    pub overrides: Vec<Override>,
    pub conflicts: Vec<Override>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    pub source: TimeRange,
    pub output: TimeRange,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreEditApplication {
    pub map: u64,
    pub sequence: ItemId,
    pub sequence_digest: String,
    /// Clip IDs for each retained source segment, in assembly order.
    pub clips: Vec<Vec<ClipId>>,
}

fn valid(range: TimeRange) -> bool {
    range.start.0 >= 0 && range.duration.0 > 0 && range.start.0.checked_add(range.duration.0).is_some()
}

pub fn encloses(outer: TimeRange, inner: TimeRange) -> bool {
    valid(outer) && valid(inner) && outer.start <= inner.start && inner.end() <= outer.end()
}

impl PreEditMap {
    /// Assemble exact half-open ranges. Suggestions have no effect until explicitly accepted.
    pub fn segments(&self, source: TimeRange) -> Result<Vec<Segment>, String> {
        if !valid(source) || self.overrides.iter().any(|entry| !encloses(source, entry.range)) {
            return Err("invalid rough-cut source or decision range".into());
        }
        let mut retained = vec![source];
        for entry in &self.overrides {
            let cut = entry.range;
            match entry.decision {
                Decision::Remove => {
                    let mut next = Vec::with_capacity(retained.len() + 1);
                    for range in retained {
                        if cut.end() <= range.start || cut.start >= range.end() {
                            next.push(range);
                        } else {
                            if range.start < cut.start {
                                next.push(TimeRange::from_bounds(range.start, cut.start));
                            }
                            if cut.end() < range.end() {
                                next.push(TimeRange::from_bounds(cut.end(), range.end()));
                            }
                        }
                    }
                    retained = next;
                }
                Decision::Keep => {
                    retained.push(cut);
                    retained.sort_unstable_by_key(|range| range.start);
                    let mut merged: Vec<TimeRange> = Vec::with_capacity(retained.len());
                    for range in retained {
                        if let Some(last) = merged.last_mut()
                            && last.end() >= range.start
                        {
                            *last = TimeRange::from_bounds(last.start, last.end().max(range.end()));
                        } else {
                            merged.push(range);
                        }
                    }
                    retained = merged;
                }
            }
        }
        let mut start = Tick::ZERO;
        Ok(retained
            .into_iter()
            .map(|source| {
                let output = TimeRange::from_bounds(start, start + source.duration);
                start = output.end();
                Segment { source, output }
            })
            .collect())
    }
}

impl Segment {
    pub fn source_at(&self, output: Tick) -> Option<Tick> {
        self.output.contains(output).then(|| self.source.start + (output - self.output.start))
    }

    pub fn output_at(&self, source: Tick) -> Option<Tick> {
        self.source.contains(source).then(|| self.output.start + (source - self.source.start))
    }
}

impl PreEdit {
    pub fn is_empty(&self) -> bool {
        self.analyses.is_empty() && self.maps.is_empty() && self.applications.is_empty()
    }

    pub fn max_id(&self) -> u64 {
        self.analyses.keys().chain(self.maps.keys()).chain(self.applications.keys()).copied().max().unwrap_or(0)
    }

    /// Keep review metadata only for included originals when making a project subset.
    pub fn retain_sources(&mut self, items: &BTreeSet<ItemId>) {
        self.analyses.retain(|_, analysis| items.contains(&analysis.source.item));
        self.maps.retain(|_, map| self.analyses.contains_key(&map.analysis));
        let maps: BTreeSet<_> = self.maps.keys().copied().collect();
        for map in self.maps.values_mut() {
            if map.previous.is_some_and(|id| !maps.contains(&id)) {
                Arc::make_mut(map).previous = None;
            }
        }
        self.applications.retain(|_, application| self.maps.contains_key(&application.map) && items.contains(&application.sequence));
    }

    pub fn check(&self) -> Result<(), String> {
        for analysis in self.analyses.values() {
            if !valid(analysis.source.range)
                || analysis.source.sha256.len() != 64
                || !analysis.source.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
                || analysis.source.bytes == 0
                || analysis.source.audio_stream >= shotclutch_media::MAX_AUDIO_STREAMS
                || analysis.source.range.duration.0 > 4 * 3600 * shotclutch_time::TICKS_PER_SECOND
                || analysis.chunks.len() > 721
                || analysis.settings.min_pause.0 <= 0
                || analysis.settings.min_pause.0 > 10 * shotclutch_time::TICKS_PER_SECOND
                || analysis.settings.handle.0 < 0
                || analysis.settings.handle.0 > shotclutch_time::TICKS_PER_SECOND
            {
                return Err("invalid rough-cut analysis settings or source".into());
            }
            let mut end = analysis.source.range.start;
            for chunk in &analysis.chunks {
                if chunk.coverage.start != end
                    || !encloses(analysis.source.range, chunk.coverage)
                    || chunk.peaks.len() > 401
                    || chunk.quiet.iter().any(|range| !encloses(chunk.coverage, *range))
                {
                    return Err("invalid rough-cut analysis coverage".into());
                }
                if let Some(transcript) = &chunk.transcript {
                    transcript.check().map_err(|_| "invalid rough-cut transcript timing or speaker references")?;
                    if transcript.words.iter().any(|word| !encloses(chunk.coverage, word.range())) {
                        return Err("rough-cut transcript exceeds its coverage".into());
                    }
                }
                end = chunk.coverage.end();
            }
            if analysis.status == AnalysisStatus::Complete && end != analysis.source.range.end() {
                return Err("complete rough-cut analysis has missing coverage".into());
            }
        }
        for (id, map) in &self.maps {
            let source = self.analyses.get(&map.analysis).ok_or("rough-cut map references missing analysis")?.source.range;
            if map.previous.is_some_and(|old| old >= *id || !self.maps.contains_key(&old))
                || map.overrides.len() > 4096
                || map.conflicts.len() > 4096
                || map.candidates.iter().any(|c| !encloses(source, c.range) || c.confidence.is_some_and(|p| !(0.0..=1.0).contains(&p)))
                || map.takes.iter().any(|g| g.alternatives.len() < 2 || g.alternatives.iter().any(|&i| i >= map.candidates.len()))
                || map.overrides.iter().any(|entry| !encloses(source, entry.range))
                || (map.reviewed && !map.conflicts.is_empty())
            {
                return Err("invalid rough-cut map".into());
            }
        }
        if self.applications.values().any(|application| !self.maps.contains_key(&application.map)) {
            return Err("rough-cut application references a missing map".into());
        }
        Ok(())
    }
}
