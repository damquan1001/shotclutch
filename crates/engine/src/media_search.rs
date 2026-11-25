//! Revision-local lexical retrieval over original-media evidence.
use crate::Session;
use crate::media_observations::{ActiveModels, Freshness, LOCAL_SPEECH_PRODUCER, evidence_freshness};
use crate::service::{ErrorCode, ServiceError, error};
use crate::service_sources::{ReadContext, authorize_search_source, search_fingerprint};
use shotclutch_media::MediaKind;
use shotclutch_project::observations::{Observation, ObservationId, Provenance, SourceFingerprint, SourceLocation, SourceStream, TextRole};
use shotclutch_project::transcript::normalize_word;
use shotclutch_project::{BinId, ItemId, MediaRef, Project, Transcript};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Weak};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EvidenceType {
    Transcript,
    Summary,
    Tag,
    Ocr,
    Caption,
    Embedding,
}

#[derive(Clone, Debug, Default)]
pub struct SearchFilters {
    pub asset: Option<ItemId>,
    /// Includes sub-bins.
    pub bin: Option<BinId>,
    pub source_range: Option<(i64, i64)>,
    pub speaker: Option<String>,
    pub media_kind: Option<MediaKind>,
    pub evidence_type: Option<EvidenceType>,
    /// Restrict to assets with (true) or without (false) indexed evidence of this type.
    pub coverage: Option<(EvidenceType, bool)>,
    pub producer: Option<String>,
    pub model: Option<String>,
}
#[derive(Clone, Debug)]
pub struct SearchQuery {
    pub text: String,
    pub limit: usize,
    pub filters: SearchFilters,
    pub include_stale: bool,
    pub include_unverified: bool,
}
impl SearchQuery {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), limit: 20, filters: Default::default(), include_stale: false, include_unverified: false }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EvidenceId {
    Observation(ObservationId),
    /// Half-open word indices in this revision's asset transcript.
    TranscriptWords {
        first: usize,
        end: usize,
    },
}
#[derive(Clone, Debug)]
pub struct Match {
    pub asset: ItemId,
    pub fingerprint: Option<SourceFingerprint>,
    pub evidence: EvidenceId,
    pub evidence_type: EvidenceType,
    pub location: SourceLocation,
    pub text: String,
    pub provenance: Option<Provenance>,
    pub transcript_source: Option<String>,
    pub source_freshness: Freshness,
    pub producer_freshness: Freshness,
    /// Fraction of distinct query terms matched, plus 1 for an adjacent exact phrase.
    pub score: f64,
    pub exact_phrase: bool,
    /// The reported range comes from the first and last matching word.
    pub word_timed: bool,
}
#[derive(Clone, Debug)]
pub struct Hit {
    pub best: Match,
    pub alternatives: Vec<Match>,
}
#[derive(Clone, Debug, Default)]
pub struct SearchCoverage {
    /// Scoped original assets, including those with no indexed text.
    pub assets: BTreeMap<ItemId, BTreeSet<EvidenceType>>,
    pub unavailable: BTreeSet<EvidenceType>,
    pub budget_limited: BTreeSet<ItemId>,
    pub missing_originals: BTreeSet<ItemId>,
    /// Lexical matches omitted by the caller's freshness policy.
    pub omitted_stale: usize,
    pub omitted_unverified: usize,
    pub verified_bytes: u64,
}
#[derive(Clone, Debug)]
pub struct SearchResults {
    pub revision: u64,
    pub hits: Vec<Hit>,
    pub score_meaning: ScoreMeaning,
    pub coverage: SearchCoverage,
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScoreMeaning {
    /// Fraction of distinct terms matched in the evidence, plus one for an exact phrase.
    LexicalTermCoverageWithPhraseBoost,
}

enum Origin {
    Observation(ObservationId, Arc<Observation>),
    Transcript(Arc<Transcript>),
}
struct Entry {
    asset: ItemId,
    kind: EvidenceType,
    origin: Origin,
    tokens: Vec<String>,
}
impl Entry {
    fn producer(&self) -> (Option<&str>, Option<&str>) {
        match &self.origin {
            Origin::Observation(_, obs) => match &obs.provenance {
                Provenance::Model { producer, version, .. } => (Some(producer), Some(&version.model)),
                Provenance::Human { author } => (Some(author), None),
                _ => (None, None),
            },
            Origin::Transcript(t) => (Some(&t.source), None),
        }
    }
}
pub(crate) struct Index {
    revision: u64,
    project: Weak<Project>,
    entries: Vec<Entry>,
    postings: BTreeMap<String, Vec<usize>>,
    coverage: BTreeMap<ItemId, BTreeSet<EvidenceType>>,
    token_count: usize,
}
impl Index {
    fn push(&mut self, entry: Entry) {
        let id = self.entries.len();
        for term in &entry.tokens {
            if term.is_empty() {
                continue;
            }
            let docs = self.postings.entry(term.clone()).or_default();
            if docs.last() != Some(&id) {
                docs.push(id);
            }
        }
        if entry.tokens.iter().any(|t| !t.is_empty()) {
            self.coverage.entry(entry.asset).or_default().insert(entry.kind);
        }
        self.token_count += entry.tokens.len();
        self.entries.push(entry);
    }
    fn build(s: &Session, ctx: &ReadContext<'_>) -> Result<Self, ServiceError> {
        let mut index = Self {
            revision: s.revision,
            project: Arc::downgrade(&s.project),
            entries: Vec::new(),
            postings: BTreeMap::new(),
            coverage: BTreeMap::new(),
            token_count: 0,
        };
        for (id, obs) in s.project.observations.iter() {
            ctx.check()?;
            let Some(summary) = &obs.summary else { continue };
            let kind = match obs.text_role {
                Some(TextRole::Tag) => EvidenceType::Tag,
                Some(TextRole::Caption) => EvidenceType::Caption,
                Some(TextRole::Ocr) => EvidenceType::Ocr,
                Some(TextRole::Transcript) => EvidenceType::Transcript,
                None if matches!(&obs.provenance,Provenance::Model{producer,..} if producer==LOCAL_SPEECH_PRODUCER) => EvidenceType::Transcript,
                None => EvidenceType::Summary,
            };
            index.push(Entry {
                asset: obs.evidence.asset,
                origin: Origin::Observation(id, obs.clone()),
                kind,
                tokens: summary.split_whitespace().map(normalize_word).collect(),
            });
        }
        for (&asset, t) in &s.project.transcripts {
            ctx.check()?;
            if t.words.len().saturating_add(index.token_count) > 2_000_000 {
                return Err(error(ErrorCode::ResourceLimit, "transcript index exceeds two million words"));
            }
            let mut tokens = Vec::with_capacity(t.words.len());
            for chunk in t.words.chunks(512) {
                ctx.check()?;
                tokens.extend(chunk.iter().map(|w| w.normalized()));
            }
            index.push(Entry { asset, origin: Origin::Transcript(t.clone()), kind: EvidenceType::Transcript, tokens });
        }
        Ok(index)
    }
    fn get(s: &Session, ctx: &ReadContext<'_>) -> Result<Arc<Self>, ServiceError> {
        let mut cache = s.search_cache.lock().map_err(|_| error(ErrorCode::CapabilityUnavailable, "search cache unavailable"))?;
        if let Some(index) = cache.as_ref()
            && index.revision == s.revision
            && index.project.ptr_eq(&Arc::downgrade(&s.project))
        {
            return Ok(index.clone());
        }
        let replacement = Arc::new(Self::build(s, ctx)?);
        ctx.check()?;
        *cache = Some(replacement.clone());
        Ok(replacement)
    }
}

fn bounds(location: SourceLocation) -> (i64, i64) {
    match location {
        SourceLocation::WholeAsset => (0, i64::MAX),
        SourceLocation::SourceRange { start, end, .. } => (start, end),
        SourceLocation::VideoPresentation { pts } => (pts, pts.saturating_add(1)),
    }
}
fn stream(location: SourceLocation) -> SourceStream {
    match location {
        SourceLocation::WholeAsset => SourceStream::All,
        SourceLocation::VideoPresentation { .. } => SourceStream::Video,
        SourceLocation::SourceRange { stream, .. } => stream,
    }
}
fn rank(a: &Match, b: &Match) -> std::cmp::Ordering {
    b.score.total_cmp(&a.score).then(a.asset.cmp(&b.asset)).then(bounds(a.location).cmp(&bounds(b.location))).then(a.evidence.cmp(&b.evidence))
}
fn group(mut matches: Vec<Match>, limit: usize) -> (Vec<Hit>, bool) {
    fn key(m: &Match) -> (ItemId, Option<&str>, SourceStream) {
        (m.asset, m.fingerprint.as_ref().map(|fp| fp.sha256.as_str()), stream(m.location))
    }
    matches.sort_by(|a, b| key(a).cmp(&key(b)).then(bounds(a.location).cmp(&bounds(b.location))));
    let mut hits: Vec<Hit> = Vec::new();
    let mut group_end = 0;
    for m in matches {
        let (start, end) = bounds(m.location);
        if let Some(hit) = hits.last_mut()
            && key(&hit.best) == key(&m)
            && start < group_end
        {
            group_end = group_end.max(end);
            if rank(&m, &hit.best).is_lt() {
                hit.alternatives.push(std::mem::replace(&mut hit.best, m));
            } else {
                hit.alternatives.push(m);
            }
        } else {
            hits.push(Hit { best: m, alternatives: Vec::new() });
            group_end = end;
        }
    }
    hits.sort_by(|a, b| rank(&a.best, &b.best));
    let truncated = hits.len() > limit;
    hits.truncate(limit);
    for hit in &mut hits {
        hit.alternatives.sort_by(rank);
    }
    (hits, truncated)
}

struct LexicalMatch {
    first: usize,
    end: usize,
    exact: bool,
    score: f64,
}
/// Timed evidence yields adjacent runs. Untimed summaries retain their recorded location.
fn lexical(
    entry: &Entry,
    terms: &[String],
    distinct: &BTreeSet<&str>,
    speaker: Option<&str>,
    ctx: &ReadContext<'_>,
    mut emit: impl FnMut(LexicalMatch) -> Result<(), ServiceError>,
) -> Result<(), ServiceError> {
    let timed = match &entry.origin {
        Origin::Transcript(_) => true,
        Origin::Observation(_, o) => !o.word_times.is_empty(),
    };
    let eligible = |i: usize| {
        speaker.is_none_or(|label| match &entry.origin {
            Origin::Transcript(t) => t.speaker_name(&t.words[i]).as_deref() == Some(label),
            Origin::Observation(_, observation) => observation.speaker.as_deref() == Some(label),
        })
    };
    let tokens = &entry.tokens;
    let mut i = 0;
    let mut found = BTreeSet::new();
    let mut phrase = false;
    let mut covered_until = 0;
    while i < tokens.len() {
        if i % 256 == 0 {
            ctx.check()?;
        }
        if !eligible(i) || !distinct.contains(tokens[i].as_str()) {
            i += 1;
            continue;
        }
        let exact = tokens[i..].starts_with(terms) && (i..i + terms.len()).all(eligible);
        if timed {
            let end = if exact { i + terms.len() } else { i + 1 };
            if exact || i >= covered_until {
                emit(LexicalMatch { first: i, end, exact, score: if exact { 2.0 } else { 1.0 / distinct.len() as f64 } })?;
                covered_until = end;
            }
            i += 1;
        } else {
            found.insert(tokens[i].as_str());
            phrase |= exact;
            i += 1;
        }
    }
    if !timed && !found.is_empty() {
        emit(LexicalMatch { first: 0, end: tokens.len(), exact: phrase, score: found.len() as f64 / distinct.len() as f64 + f64::from(phrase) })?;
    }
    Ok(())
}

fn materialize(entry: &Entry, m: LexicalMatch) -> Option<Match> {
    let mut hit = Match {
        asset: entry.asset,
        evidence: EvidenceId::TranscriptWords { first: m.first, end: m.end },
        evidence_type: entry.kind,
        location: SourceLocation::WholeAsset,
        text: String::new(),
        fingerprint: None,
        provenance: None,
        transcript_source: None,
        source_freshness: Freshness::Unverified,
        producer_freshness: Freshness::Unverified,
        score: m.score,
        exact_phrase: m.exact,
        word_timed: false,
    };
    match &entry.origin {
        Origin::Transcript(t) => {
            let words = &t.words[m.first..m.end];
            let start = words.first()?.start.0;
            let end = words.last()?.end.0;
            if start < 0 || end <= start {
                return None;
            }
            hit.location = SourceLocation::SourceRange { stream: SourceStream::All, start, end };
            hit.text = words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
            hit.transcript_source = Some(t.source.clone());
            hit.word_timed = true;
        }
        Origin::Observation(id, obs) => {
            hit.evidence = EvidenceId::Observation(*id);
            hit.provenance = Some(obs.provenance.clone());
            hit.fingerprint = Some(obs.evidence.fingerprint.clone());
            hit.location = match obs.evidence.location {
                SourceLocation::WholeAsset if obs.evidence.duration_ticks > 0 => {
                    SourceLocation::SourceRange { stream: SourceStream::All, start: 0, end: obs.evidence.duration_ticks }
                }
                location => location,
            };
            let summary = obs.summary.as_deref()?;
            if let (Some(first), Some(last)) = (obs.word_times.get(m.first), obs.word_times.get(m.end - 1)) {
                hit.word_timed = true;
                hit.location = SourceLocation::SourceRange { stream: stream(hit.location), start: first.start, end: last.end };
                hit.text = summary.split_whitespace().skip(m.first).take(m.end - m.first).collect::<Vec<_>>().join(" ");
            } else {
                hit.text = summary.to_owned();
            }
        }
    }
    Some(hit)
}

pub(crate) fn search(s: &Session, q: &SearchQuery, models: &ActiveModels, ctx: &ReadContext<'_>) -> Result<SearchResults, ServiceError> {
    search_scoped(s, q, models, ctx, None)
}

pub(crate) fn search_scoped(
    s: &Session,
    q: &SearchQuery,
    models: &ActiveModels,
    ctx: &ReadContext<'_>,
    allowed: Option<&BTreeSet<ItemId>>,
) -> Result<SearchResults, ServiceError> {
    ctx.check()?;
    let f = &q.filters;
    if !(1..=100).contains(&q.limit) || q.text.len() > 512 || f.source_range.is_some_and(|(a, b)| a < 0 || a >= b) {
        return Err(error(ErrorCode::InvalidArgument, "invalid result limit, query length or source range"));
    }
    let terms: Vec<_> = q.text.split_whitespace().map(normalize_word).filter(|t| !t.is_empty()).collect();
    if terms.is_empty() {
        return Err(error(ErrorCode::InvalidArgument, "search requires lexical terms"));
    }
    let distinct = terms.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let index = Index::get(s, ctx)?;
    let mut coverage = SearchCoverage::default();
    coverage.unavailable.insert(EvidenceType::Embedding);
    let bin = if let Some(id) = f.bin {
        let mut ids = Vec::new();
        s.project.root.find_bin(id).ok_or_else(|| error(ErrorCode::NotFound, "bin not found"))?.all_items(&mut ids);
        Some(ids.into_iter().collect::<BTreeSet<_>>())
    } else {
        None
    };
    let mut eligible = BTreeMap::new();
    for (&id, item) in &s.project.items {
        ctx.check()?;
        if allowed.is_some_and(|assets| !assets.contains(&id)) || f.asset.is_some_and(|a| a != id) || bin.as_ref().is_some_and(|b| !b.contains(&id)) {
            continue;
        }
        let Some(media) = item.as_media() else { continue };
        let MediaRef::File { path } = &media.media else { continue };
        if media.info.kind == MediaKind::ImageSequence || f.media_kind.is_some_and(|k| k != media.info.kind) {
            continue;
        }
        authorize_search_source(s, path, ctx)?;
        let types = index.coverage.get(&id).cloned().unwrap_or_default();
        if f.coverage.is_some_and(|(kind, wanted)| types.contains(&kind) != wanted) {
            continue;
        }
        coverage.assets.insert(id, types);
        eligible.insert(id, (path, media.offline));
    }
    let mut remaining = ctx.budget;
    let mut originals = BTreeMap::new();
    let mut matches = Vec::new();
    let candidates = distinct.iter().filter_map(|t| index.postings.get(*t)).flatten().copied().collect::<BTreeSet<_>>();
    for doc in candidates {
        ctx.check()?;
        let entry = &index.entries[doc];
        let Some(&(path, offline)) = eligible.get(&entry.asset) else { continue };
        let (producer, model) = entry.producer();
        if f.evidence_type.is_some_and(|k| k != entry.kind)
            || f.producer.as_deref().is_some_and(|p| Some(p) != producer)
            || f.model.as_deref().is_some_and(|m| Some(m) != model)
        {
            continue;
        }
        lexical(entry, &terms, &distinct, f.speaker.as_deref(), ctx, |m| {
            let Some(mut hit) = materialize(entry, m) else { return Ok(()) };
            let (start, end) = bounds(hit.location);
            if f.source_range.is_some_and(|(a, b)| a >= end || b <= start) {
                return Ok(());
            }
            if let Origin::Observation(_, obs) = &entry.origin {
                if let std::collections::btree_map::Entry::Vacant(slot) = originals.entry(entry.asset) {
                    let fingerprint = match search_fingerprint(s, path, offline, ctx, &mut remaining) {
                        Ok(fp) => Some(fp),
                        Err(e) if e.code == ErrorCode::ResourceLimit => {
                            coverage.budget_limited.insert(entry.asset);
                            None
                        }
                        Err(e) if e.code == ErrorCode::MediaChanged => {
                            coverage.missing_originals.insert(entry.asset);
                            None
                        }
                        Err(e) => return Err(e),
                    };
                    slot.insert(fingerprint);
                }
                (hit.source_freshness, hit.producer_freshness) = evidence_freshness(obs, originals.get(&entry.asset).and_then(Option::as_ref), models);
            }
            let stale = hit.source_freshness == Freshness::Stale || hit.producer_freshness == Freshness::Stale;
            let unknown = hit.source_freshness == Freshness::Unverified || hit.producer_freshness == Freshness::Unverified;
            if stale && !q.include_stale {
                coverage.omitted_stale += 1;
                return Ok(());
            }
            if !stale && unknown && !q.include_unverified {
                coverage.omitted_unverified += 1;
                return Ok(());
            }
            if matches.len() >= 100_000 {
                return Err(error(ErrorCode::ResourceLimit, "too many lexical matches; narrow the search"));
            }
            matches.push(hit);
            Ok(())
        })?;
    }
    coverage.verified_bytes = ctx.budget - remaining;
    let (hits, truncated) = group(matches, q.limit);
    ctx.check()?;
    Ok(SearchResults { score_meaning: ScoreMeaning::LexicalTermCoverageWithPhraseBoost, revision: s.revision, hits, coverage, truncated })
}

#[cfg(test)]
mod tests;
