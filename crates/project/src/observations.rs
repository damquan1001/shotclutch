//! Durable semantic observations tied to the bytes and source time of an original asset.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use shotclutch_media::MAX_AUDIO_STREAMS;
use shotclutch_time::Tick;

use crate::{ItemId, MediaRef, Project};

pub const MAX_OBSERVATIONS: usize = 4096;
const MAX_RELATIONSHIPS: usize = 16;
pub const MAX_SUMMARY_BYTES: usize = 512;
pub const ASSET_VERIFIER_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ObservationId(#[serde(with = "decimal")] pub u64);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFingerprint {
    pub sha256: String,
    #[serde(with = "decimal")]
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationKind {
    Asset,
    Scene,
    Shot,
    Segment,
    Frame,
}

/// Stream numbers use `MediaSource`'s stream ordering. Video means its primary video stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceStream {
    All,
    Video,
    Audio(u16),
}

/// All coordinates are at `shotclutch_time::TICKS_PER_SECOND` in the original's media time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "domain", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceLocation {
    WholeAsset,
    SourceRange {
        stream: SourceStream,
        #[serde(with = "decimal")]
        start: i64,
        #[serde(with = "decimal")]
        end: i64,
    },
    VideoPresentation {
        #[serde(with = "decimal")]
        pts: i64,
    },
}

/// The content fingerprint and source locator together identify the evidence. Keep the media
/// bounds from capture so replacing the linked file cannot invalidate the stored record itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceEvidence {
    #[serde(with = "asset_id")]
    pub asset: ItemId,
    pub fingerprint: SourceFingerprint,
    pub location: SourceLocation,
    #[serde(with = "decimal")]
    pub duration_ticks: i64,
    pub has_video: bool,
    pub audio_stream_count: u16,
}

impl SourceEvidence {
    pub fn valid_location(&self, kind: ObservationKind) -> bool {
        let duration = self.duration_ticks;
        if duration < 0 || usize::from(self.audio_stream_count) > MAX_AUDIO_STREAMS {
            return false;
        }
        match (kind, self.location) {
            (ObservationKind::Asset, SourceLocation::WholeAsset) => true,
            (ObservationKind::Scene | ObservationKind::Shot | ObservationKind::Segment, SourceLocation::SourceRange { stream, start, end }) => {
                let available = match stream {
                    SourceStream::All => true,
                    SourceStream::Video => self.has_video,
                    SourceStream::Audio(index) => index < self.audio_stream_count,
                };
                available && start >= Tick::ZERO.0 && start < end && end <= duration
            }
            (ObservationKind::Frame, SourceLocation::VideoPresentation { pts }) => self.has_video && pts >= Tick::ZERO.0 && pts < duration,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelVersion {
    pub producer_version: String,
    pub model: String,
    pub model_version: String,
    pub configuration_sha256: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceMeaning {
    /// Normalized producer score with no calibration claim.
    ModelScore,
    HeuristicScore,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Confidence {
    pub score: f32,
    pub meaning: ConfidenceMeaning,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "origin", rename_all = "snake_case", deny_unknown_fields)]
pub enum Provenance {
    VerifiedAsset { version: u32 },
    Human { author: String },
    Model { producer: String, version: ModelVersion, confidence: Confidence },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextRole {
    Transcript,
    Tag,
    Ocr,
    Caption,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WordTime {
    #[serde(with = "decimal")]
    pub start: i64,
    #[serde(with = "decimal")]
    pub end: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_role: Option<TextRole>,
    /// Source-time intervals aligned to whitespace-separated words in the summary.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub word_times: Vec<WordTime>,
    /// Attributed speaker for every word in this transcript observation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
    pub kind: ObservationKind,
    pub evidence: SourceEvidence,
    pub provenance: Provenance,
    #[serde(with = "decimal")]
    pub created_unix_seconds: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Explicit same-source relationships. Ranges can overlap or cross related boundaries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<ObservationId>,
}

impl Observation {
    pub fn check_fields(&self) -> Result<(), &'static str> {
        if self.text_role.is_some() && self.summary.is_none() {
            return Err("typed text requires a summary");
        }
        if self
            .speaker
            .as_ref()
            .is_some_and(|speaker| self.word_times.is_empty() || speaker.trim().is_empty() || speaker.len() > 128 || speaker.chars().any(char::is_control))
        {
            return Err("a speaker label requires bounded word-timed transcript evidence");
        }
        if !self.word_times.is_empty() {
            let SourceLocation::SourceRange { start, end, stream: SourceStream::Audio(_) } = self.evidence.location else {
                return Err("word timing requires an audio range");
            };
            let words = self.summary.as_deref().unwrap_or_default().split_whitespace().count();
            if self.text_role != Some(TextRole::Transcript) || self.word_times.len() != words {
                return Err("timing must identify each transcript word");
            }
            let mut previous = start;
            for time in &self.word_times {
                if time.start < previous || time.start >= time.end || time.end > end {
                    return Err("invalid transcript word timing");
                }
                previous = time.end;
            }
        }
        let valid_provenance = match &self.provenance {
            Provenance::VerifiedAsset { version } => *version > 0 && self.kind == ObservationKind::Asset && self.summary.is_none(),
            Provenance::Human { author } => identifier(author),
            Provenance::Model { producer, version, confidence } => {
                identifier(producer)
                    && identifier(&version.producer_version)
                    && identifier(&version.model)
                    && identifier(&version.model_version)
                    && sha256(&version.configuration_sha256)
                    && (0.0..=1.0).contains(&confidence.score)
            }
        };
        if !valid_provenance {
            return Err("invalid observation provenance or confidence");
        }
        if self.evidence.asset.0 == 0
            || self.evidence.fingerprint.bytes == 0
            || !sha256(&self.evidence.fingerprint.sha256)
            || !self.evidence.valid_location(self.kind)
        {
            return Err("invalid observation evidence or source coordinates");
        }
        if self.created_unix_seconds < 0
            || self.summary.as_ref().is_some_and(|s| s.trim().is_empty() || s.len() > MAX_SUMMARY_BYTES || s.chars().any(char::is_control))
            || self.related.len() > MAX_RELATIONSHIPS
        {
            return Err("observation timestamp, summary or relationship count is invalid");
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Observations(Arc<BTreeMap<ObservationId, Arc<Observation>>>);

impl Observations {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (ObservationId, &Arc<Observation>)> {
        self.0.iter().map(|(id, record)| (*id, record))
    }

    pub fn for_asset(&self, asset: ItemId) -> impl Iterator<Item = (ObservationId, &Arc<Observation>)> {
        self.0.iter().filter(move |(_, record)| record.evidence.asset == asset).map(|(id, record)| (*id, record))
    }

    pub(crate) fn max_id(&self) -> u64 {
        self.0.last_key_value().map_or(0, |(id, _)| id.0)
    }

    pub fn insert(&mut self, id: ObservationId, record: Observation) -> Result<(), &'static str> {
        if self.len() >= MAX_OBSERVATIONS || self.0.contains_key(&id) {
            return Err("observation count or identifier is invalid");
        }
        self.check_record(id, &record)?;
        Arc::make_mut(&mut self.0).insert(id, Arc::new(record));
        Ok(())
    }

    fn check_record(&self, id: ObservationId, record: &Observation) -> Result<(), &'static str> {
        record.check_fields()?;
        if id.0 == 0 || id.0 <= record.evidence.asset.0 {
            return Err("invalid observation identifier");
        }
        let mut seen = BTreeSet::new();
        for related in &record.related {
            // Only previously issued IDs can be related. This also prevents loaded cycles.
            if *related >= id || !seen.insert(related) {
                return Err("invalid observation relationship");
            }
            let other = self.0.get(related).ok_or("observation relationship is missing")?;
            if record.evidence.asset != other.evidence.asset || record.evidence.fingerprint != other.evidence.fingerprint {
                return Err("observation relationship refers to a different source");
            }
        }
        Ok(())
    }

    pub fn check(&self, project: &Project) -> Result<(), &'static str> {
        if self.len() > MAX_OBSERVATIONS {
            return Err("project has too many observations");
        }
        for (&id, record) in self.0.iter() {
            self.check_record(id, record)?;
            if !project.item(record.evidence.asset).and_then(|item| item.as_media()).is_some_and(|media| matches!(media.media, MediaRef::File { .. })) {
                return Err("observation original asset is missing");
            }
        }
        Ok(())
    }

    pub fn remove_source(&mut self, asset: ItemId) {
        if self.for_asset(asset).next().is_some() {
            Arc::make_mut(&mut self.0).retain(|_, record| record.evidence.asset != asset);
        }
    }

    pub fn retain_sources(&mut self, assets: &BTreeSet<ItemId>) {
        if self.0.values().any(|record| !assets.contains(&record.evidence.asset)) {
            Arc::make_mut(&mut self.0).retain(|_, record| assets.contains(&record.evidence.asset));
        }
    }
}

fn sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && value.bytes().all(|b| b.is_ascii_alphanumeric() || b"_.:-".contains(&b))
}

mod asset_id {
    use serde::{Deserializer, Serializer};

    use super::{ItemId, decimal};

    pub fn serialize<S: Serializer>(id: &ItemId, serializer: S) -> Result<S::Ok, S::Error> {
        decimal::serialize(&id.0, serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<ItemId, D::Error> {
        decimal::deserialize(deserializer).map(ItemId)
    }
}

mod decimal {
    use std::fmt::Display;
    use std::str::FromStr;

    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn serialize<T: Display, S: Serializer>(value: &T, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(value)
    }

    pub fn deserialize<'de, T: FromStr + Display, D: Deserializer<'de>>(deserializer: D) -> Result<T, D::Error> {
        let text = String::deserialize(deserializer)?;
        let value: T = text.parse().map_err(|_| D::Error::custom("invalid decimal integer"))?;
        if value.to_string() != text {
            return Err(D::Error::custom("noncanonical decimal integer"));
        }
        Ok(value)
    }
}
