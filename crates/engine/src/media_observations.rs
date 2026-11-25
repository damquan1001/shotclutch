//! Trusted, local observation ingestion. Producers submit source-aligned claims as data.

use std::collections::BTreeMap;
use std::sync::Arc;

use shotclutch_media::{FrameRequest, MediaInfo, MediaKind, SharedSource};
use shotclutch_project::observations::{
    ASSET_VERIFIER_VERSION, Confidence, MAX_OBSERVATIONS, ModelVersion, Observation, ObservationId, ObservationKind, Provenance, SourceEvidence,
    SourceFingerprint, SourceLocation,
};
use shotclutch_project::{ItemId, MediaClip, MediaRef};
use shotclutch_time::Tick;

use crate::Session;
use crate::service::{ErrorCode, ServiceError, error, transaction_error};
use crate::service_sources::{ReadContext, observation_file};

mod speech;
pub(crate) use speech::record_speech;
pub use speech::{LOCAL_SPEECH_PRODUCER, SpeechObservationRequest, local_speech_profile};

#[derive(Clone, Debug)]
pub enum ObservationProducer {
    VerifiedAsset,
    /// The editor grant supplies the author identity.
    Human,
    Model {
        producer: String,
        version: ModelVersion,
        confidence: Confidence,
    },
}

/// Trusted producer input. The expected fingerprint is the original the producer observed.
/// Summaries are display text. Credentials and raw provider payloads do not belong here.
#[derive(Clone, Debug)]
pub struct ObservationRequest {
    pub asset: ItemId,
    pub expected_revision: u64,
    pub expected_fingerprint: SourceFingerprint,
    pub kind: ObservationKind,
    pub location: SourceLocation,
    pub producer: ObservationProducer,
    pub summary: Option<String>,
    pub text_role: Option<shotclutch_project::observations::TextRole>,
    pub word_times: Vec<shotclutch_project::observations::WordTime>,
    pub speaker: Option<String>,
    pub related: Vec<ObservationId>,
}

impl ObservationRequest {
    /// Record the engine's verified facts about an imported original using a prior inspection.
    pub fn verified_asset(asset: ItemId, expected_revision: u64, expected_fingerprint: SourceFingerprint) -> Self {
        Self {
            asset,
            expected_revision,
            expected_fingerprint,
            kind: ObservationKind::Asset,
            location: SourceLocation::WholeAsset,
            producer: ObservationProducer::VerifiedAsset,
            summary: None,
            text_role: None,
            word_times: Vec::new(),
            speaker: None,
            related: Vec::new(),
        }
    }
}

/// Active producer profiles supplied by the trusted host, keyed by stable producer identity.
pub type ActiveModels = BTreeMap<String, ModelVersion>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Freshness {
    Current,
    Stale,
    Unverified,
}

#[derive(Clone, Debug)]
pub struct InspectedObservation {
    pub id: ObservationId,
    pub observation: Arc<Observation>,
    pub source_freshness: Freshness,
    pub producer_freshness: Freshness,
}

impl InspectedObservation {
    pub fn freshness(&self) -> Freshness {
        match (self.source_freshness, self.producer_freshness) {
            (Freshness::Stale, _) | (_, Freshness::Stale) => Freshness::Stale,
            (Freshness::Current, Freshness::Current) => Freshness::Current,
            _ => Freshness::Unverified,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ObservationInspection {
    pub revision: u64,
    pub fingerprint: Option<SourceFingerprint>,
    pub observations: Vec<InspectedObservation>,
}

pub(crate) fn original(session: &Session, asset: ItemId) -> Result<(&MediaClip, &str), ServiceError> {
    let media = session.project.item(asset).and_then(|item| item.as_media()).ok_or_else(|| error(ErrorCode::NotFound, "original asset not found"))?;
    if let MediaRef::File { path } = &media.media
        && media.info.kind != MediaKind::ImageSequence
    {
        return Ok((media, path));
    }
    Err(error(ErrorCode::CapabilityUnavailable, "observations require a single original media file"))
}

pub(crate) fn inspect(
    session: &Session,
    asset: ItemId,
    active_models: &ActiveModels,
    context: &ReadContext<'_>,
) -> Result<ObservationInspection, ServiceError> {
    context.check()?;
    let (media, path) = original(session, asset)?;
    let fingerprint = match observation_file(session, path, None, media.offline, context, None) {
        Ok(fingerprint) => Some(fingerprint),
        Err(e) if e.code == ErrorCode::MediaChanged => None,
        Err(e) => return Err(e),
    };
    let observations = session
        .project
        .observations
        .for_asset(asset)
        .map(|(id, observation)| {
            let (source_freshness, producer_freshness) = evidence_freshness(observation, fingerprint.as_ref(), active_models);
            InspectedObservation { id, observation: observation.clone(), source_freshness, producer_freshness }
        })
        .collect();
    context.check()?;
    Ok(ObservationInspection { revision: session.revision, fingerprint, observations })
}

pub(crate) fn evidence_freshness(observation: &Observation, fingerprint: Option<&SourceFingerprint>, active_models: &ActiveModels) -> (Freshness, Freshness) {
    let source_freshness = match fingerprint {
        Some(current) if *current == observation.evidence.fingerprint => Freshness::Current,
        Some(_) => Freshness::Stale,
        None => Freshness::Unverified,
    };
    let producer_freshness = match &observation.provenance {
        Provenance::Human { .. } => Freshness::Current,
        Provenance::VerifiedAsset { version } if *version == ASSET_VERIFIER_VERSION => Freshness::Current,
        Provenance::VerifiedAsset { .. } => Freshness::Stale,
        Provenance::Model { producer, version, .. } => match active_models.get(producer) {
            Some(current) if current == version => Freshness::Current,
            Some(_) => Freshness::Stale,
            None => Freshness::Unverified,
        },
    };
    (source_freshness, producer_freshness)
}

pub(crate) fn record(session: &mut Session, request: ObservationRequest, actor: &str, context: &ReadContext<'_>) -> Result<ObservationId, ServiceError> {
    check_write(session, request.expected_revision, context)?;
    let (media, path) = original(session, request.asset)?;
    let provenance = match request.producer {
        ObservationProducer::VerifiedAsset => Provenance::VerifiedAsset { version: ASSET_VERIFIER_VERSION },
        ObservationProducer::Human => Provenance::Human { author: actor.to_owned() },
        ObservationProducer::Model { producer, version, confidence } => Provenance::Model { producer, version, confidence },
    };
    let observation = Observation {
        kind: request.kind,
        evidence: source_evidence(request.asset, request.expected_fingerprint, request.location, &media.info)?,
        provenance,
        created_unix_seconds: crate::autosave::unix_now(),
        summary: request.summary,
        text_role: request.text_role,
        word_times: request.word_times,
        speaker: request.speaker,
        related: request.related,
    };
    observation.check_fields().map_err(|message| error(ErrorCode::InvalidArgument, message))?;
    let mut snapshot = Vec::new();
    verify_source(session, media, path, &observation.evidence.fingerprint, context, (request.kind == ObservationKind::Frame).then_some(&mut snapshot))?;
    if let SourceLocation::VideoPresentation { pts } = request.location {
        let verified = context.cancellation.with_media(|| verify_frame(session, path, snapshot, Tick(pts)));
        context.check()?;
        verified?;
    }
    commit_observation(session, observation, context)
}

fn check_write(session: &Session, expected_revision: u64, context: &ReadContext<'_>) -> Result<(), ServiceError> {
    context.check()?;
    if session.revision != expected_revision {
        return Err(error(ErrorCode::RevisionConflict, "project changed since the observation was prepared"));
    }
    if session.persistence.is_none() || !session.prefs.auto_save.recovery_journal {
        return Err(error(ErrorCode::CapabilityUnavailable, "durable recovery must be enabled by the host"));
    }
    if session.project.observations.len() >= MAX_OBSERVATIONS {
        return Err(error(ErrorCode::ResourceLimit, "project observation storage is full"));
    }
    Ok(())
}

fn source_evidence(asset: ItemId, fingerprint: SourceFingerprint, location: SourceLocation, info: &MediaInfo) -> Result<SourceEvidence, ServiceError> {
    Ok(SourceEvidence {
        asset,
        fingerprint,
        location,
        duration_ticks: info.duration.0,
        has_video: info.has_video(),
        audio_stream_count: u16::try_from(info.audio_streams.len()).map_err(|_| error(ErrorCode::ResourceLimit, "too many source streams"))?,
    })
}

fn verify_source(
    session: &Session,
    media: &MediaClip,
    path: &str,
    expected: &SourceFingerprint,
    context: &ReadContext<'_>,
    snapshot: Option<&mut Vec<u8>>,
) -> Result<(), ServiceError> {
    let fingerprint = observation_file(session, path, media.identity, media.offline, context, snapshot)?;
    if &fingerprint != expected {
        return Err(error(ErrorCode::MediaChanged, "original content changed since the observation was prepared"));
    }
    Ok(())
}

fn commit_observation(session: &mut Session, observation: Observation, context: &ReadContext<'_>) -> Result<ObservationId, ServiceError> {
    context.check()?;
    let mut project = (*session.project).clone();
    project.next_id = project.next_id.max(session.id_watermark);
    if project.next_id >= shotclutch_project::Project::MAX_NEXT_ID {
        return Err(error(ErrorCode::ResourceLimit, "project ID space exhausted"));
    }
    let id = ObservationId(project.alloc_id());
    project.observations.insert(id, observation).map_err(|message| error(ErrorCode::InvalidArgument, message))?;
    let prepared = session.prepare_project("Record media observation", project, Vec::new()).map_err(|e| error(ErrorCode::InvalidArgument, e.to_string()))?;
    context.check()?;
    session.commit_edits_durable(prepared).map_err(transaction_error)?;
    Ok(id)
}

pub(crate) fn open_snapshot(session: &Session, path: &str, snapshot: Vec<u8>) -> Result<SharedSource, ServiceError> {
    let basename = path.rsplit(['/', '\\']).next().unwrap_or_default();
    session.media.open_bytes(basename, snapshot.into()).map_err(|_| error(ErrorCode::CapabilityUnavailable, "original media reader is unavailable"))
}

fn verify_frame(session: &Session, path: &str, snapshot: Vec<u8>, pts: Tick) -> Result<(), ServiceError> {
    let source = open_snapshot(session, path, snapshot)?;
    let video = source.info().video.as_ref().ok_or_else(|| error(ErrorCode::CapabilityUnavailable, "original has no decodable video stream"))?;
    if u64::from(video.width) * u64::from(video.height) > 4096 * 2160 {
        return Err(error(ErrorCode::ResourceLimit, "frame observation exceeds the decoded pixel budget"));
    }
    if pts >= source.info().duration || source.video_edit_boundary(pts, false) != Some(pts) {
        return Err(error(ErrorCode::VerificationFailed, "requested time is not a certified source presentation boundary"));
    }
    let frame = source.video_frame(FrameRequest::full(pts)).map_err(|_| error(ErrorCode::VerificationFailed, "original frame could not be decoded"))?;
    if frame.pts != pts {
        return Err(error(ErrorCode::VerificationFailed, "decoded frame presentation time differs from the certified boundary"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
