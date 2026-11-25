use super::*;
use crate::autosave::AutosaveConfig;
use crate::media_observations::{ObservationProducer, ObservationRequest};
use crate::media_test_util::{make_movie, session_with};
use crate::service::{Cancellation, EditorClient, EditorScope};
use serde::Deserialize;
use serde_json::json;
use shotclutch_media::DemoScene;
use shotclutch_project::observations::{Confidence, ConfidenceMeaning, ModelVersion, ObservationKind, WordTime};
use shotclutch_project::{Speaker, Word};
use shotclutch_time::{FrameRate, Tick};
use std::{path::Path, time::Instant};

#[derive(Deserialize)]
struct Corpus {
    version: u32,
    records: Vec<(usize, String, String, i64, i64)>,
    queries: Vec<(String, Vec<(usize, i64, i64)>)>,
}
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("corpus_v1.json")).unwrap()
}
fn tick(i: i64) -> i64 {
    FrameRate::FPS_24.tick_of(i).0
}
fn grant(s: &mut Session, root: &Path, write: bool, budget: u64) -> EditorClient {
    s.authorize_editor(
        "reviewer",
        EditorScope {
            allow_edits: write,
            media_roots: vec![root.to_owned()],
            max_source_bytes: budget,
            sequences: None,
            lifetime: std::time::Duration::from_secs(120),
        },
    )
    .unwrap()
}
fn models() -> ActiveModels {
    [(
        "speech-fixture".into(),
        ModelVersion { model: "english".into(), model_version: "1".into(), producer_version: "1".into(), configuration_sha256: "a".repeat(64) },
    )]
    .into()
}
struct Fixture {
    root: tempfile::TempDir,
    s: Session,
    ids: Vec<ItemId>,
    client: EditorClient,
    bin: BinId,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let paths = [root.path().join("a.mov"), root.path().join("b.mov")];
        for (path, scene) in paths.iter().zip([DemoScene::Aurora, DemoScene::CityNight]) {
            make_movie(path, scene, 32, 18, 8);
        }
        let (mut s, ids, _) = session_with(&[&paths[0], &paths[1]]);
        let project = Arc::make_mut(&mut s.project);
        let bin = project.add_bin("First", None);
        project.move_to_bin(&ids[..1], Some(bin));
        project.transcripts.insert(
            ids[1],
            Arc::new(Transcript {
                source: "manual".into(),
                speakers: vec![Speaker { name: "Narrator".into() }],
                words: vec![
                    Word { speaker: Some(0), ..Word::new("Legacy", Tick(2), Tick(8)) },
                    Word { speaker: Some(0), ..Word::new("phrase.", Tick(9), Tick(16)) },
                ],
                ..Default::default()
            }),
        );
        s.start_autosave(AutosaveConfig::new(&root.path().join("recovery"))).unwrap();
        let client = grant(&mut s, root.path(), true, 64 << 20);
        for (asset, role, text, start, end) in corpus().records {
            let asset = ids[asset];
            let fp = s.inspect_media_observations(&client, asset, &models(), &Default::default()).unwrap().fingerprint.unwrap();
            let mut req = ObservationRequest::verified_asset(asset, s.revision, fp);
            req.kind = ObservationKind::Segment;
            req.location = SourceLocation::SourceRange { stream: SourceStream::Video, start: tick(start), end: tick(end) };
            req.producer = ObservationProducer::Human;
            req.text_role = match role.as_str() {
                "transcript" => {
                    req.location = SourceLocation::SourceRange { stream: SourceStream::Audio(0), start: tick(start), end: tick(end) };
                    req.word_times = (start..end).map(|i| WordTime { start: tick(i), end: tick(i + 1) }).collect();
                    req.producer = ObservationProducer::Model {
                        producer: "speech-fixture".into(),
                        version: models()["speech-fixture"].clone(),
                        confidence: Confidence { score: 0.95, meaning: ConfidenceMeaning::ModelScore },
                    };
                    Some(TextRole::Transcript)
                }
                "ocr" => {
                    req.kind = ObservationKind::Frame;
                    req.location = SourceLocation::VideoPresentation { pts: tick(start) };
                    Some(TextRole::Ocr)
                }
                "caption" => Some(TextRole::Caption),
                "tag" => Some(TextRole::Tag),
                "summary" => None,
                _ => panic!("unknown fixture type"),
            };
            req.summary = Some(text);
            s.record_media_observation(&client, req, &Default::default()).unwrap();
        }
        let path = root.path().join("corpus.scproj");
        s.execute("file.saveAs", json!({"path":path})).unwrap();
        s.shutdown();
        let mut s = Session::default();
        s.execute("file.open", json!({"path":path})).unwrap();
        let client = grant(&mut s, root.path(), false, 64 << 20);
        Self { root, s, ids, client, bin }
    }
    fn run(&self, q: &SearchQuery) -> SearchResults {
        self.s.search_media(&self.client, q, &models(), &Default::default()).unwrap()
    }
}

#[test]
fn corpus_v1_retrieval() {
    let f = Fixture::new();
    let corpus = corpus();
    assert_eq!(corpus.version, 1);
    let path = f.root.path().join("corpus.scproj");
    let saved = std::fs::read(&path).unwrap();
    let revision = f.s.revision;
    let mut precision = [0.; 2];
    let mut recall = [0.; 2];
    let mut positives = 0;
    let mut localization = Vec::new();
    let mut warm = Vec::new();
    for (query, labels) in corpus.queries {
        let q = SearchQuery::new(query);
        let start = Instant::now();
        let r = f.run(&q);
        if warm.is_empty() {
            eprintln!("cold query (including build/hash) {:?}", start.elapsed());
        }
        let start = Instant::now();
        let repeated = f.run(&q);
        warm.push(start.elapsed());
        let ids = |r: &SearchResults| r.hits.iter().map(|h| (h.best.asset, h.best.evidence.clone(), h.best.location)).collect::<Vec<_>>();
        assert_eq!(ids(&r), ids(&repeated));
        assert_eq!(r.revision, revision);
        assert!(r.coverage.unavailable.contains(&EvidenceType::Embedding));
        if labels.is_empty() {
            assert!(r.hits.is_empty());
            continue;
        }
        positives += 1;
        let relevant = |hit: &Hit| {
            labels.iter().find(|(asset, start, end)| {
                let (a, b) = bounds(hit.best.location);
                hit.best.asset == f.ids[*asset] && a == tick(*start) && (hit.best.location == SourceLocation::VideoPresentation { pts: a } || b == tick(*end))
            })
        };
        for (slot, k) in [1, 5].into_iter().enumerate() {
            let hits = r.hits.iter().take(k).filter(|h| relevant(h).is_some()).count() as f64;
            precision[slot] += hits / k as f64;
            recall[slot] += hits / labels.len() as f64;
        }
        let top = &r.hits[0].best;
        assert!(top.exact_phrase && top.score == 2.0);
        assert!(top.fingerprint.is_some());
        assert_eq!(top.source_freshness, Freshness::Current);
        for (pos, (asset, start, end)) in labels.iter().enumerate() {
            let hit = &r.hits[pos].best;
            assert_eq!(hit.asset, f.ids[*asset]);
            let (a, b) = bounds(hit.location);
            let observed_end = if matches!(hit.location, SourceLocation::VideoPresentation { .. }) { a + tick(1) } else { b };
            localization.push((a - tick(*start)).abs().max((observed_end - tick(*end)).abs()));
        }
        if q.text == "saffron fixture" {
            assert_eq!(r.hits[0].alternatives.len(), 1);
        }
        if q.text == "EXIT 17" {
            assert_eq!(top.location, SourceLocation::VideoPresentation { pts: tick(3) });
            assert!(!r.hits[1].best.exact_phrase);
        }
    }
    assert_eq!(*localization.iter().max().unwrap(), 0);
    let index = f.s.search_cache.lock().unwrap();
    let i = index.as_ref().unwrap();
    let posting_count: usize = i.postings.values().map(Vec::len).sum();
    let lexical_bytes: usize = i.entries.iter().flat_map(|e| &e.tokens).map(String::len).sum::<usize>()
        + i.postings.keys().map(String::len).sum::<usize>()
        + posting_count * std::mem::size_of::<usize>();
    eprintln!(
        "macro P@1/5 {:?}, R@1/5 {:?}, max endpoint error 0 ticks, warm queries {:?}, {} docs / {} tokens / {} postings / {} lexical payload bytes, coverage {:?}",
        precision.map(|v| v / positives as f64),
        recall.map(|v| v / positives as f64),
        warm,
        i.entries.len(),
        i.token_count,
        posting_count,
        lexical_bytes,
        i.coverage
    );
    assert_eq!(f.s.revision, revision);
    assert_eq!(std::fs::read(path).unwrap(), saved);
}

#[test]
fn filters_freshness_and_grants() {
    let mut f = Fixture::new();
    let q = SearchQuery::new("lift the amber");
    let legacy = SearchQuery::new("legacy phrase");
    assert!(f.run(&legacy).hits.is_empty());
    let mut legacy = SearchQuery { include_unverified: true, ..legacy };
    legacy.filters.speaker = Some("Narrator".into());
    let old = f.run(&legacy);
    assert_eq!(old.hits[0].best.evidence, EvidenceId::TranscriptWords { first: 0, end: 2 });
    assert_eq!(bounds(old.hits[0].best.location), (2, 16));
    assert!(old.hits[0].best.fingerprint.is_none());
    legacy.filters.speaker = Some("Other".into());
    assert!(f.run(&legacy).hits.is_empty());
    let mut scoped = q.clone();
    scoped.filters = SearchFilters {
        bin: Some(f.bin),
        asset: Some(f.ids[0]),
        evidence_type: Some(EvidenceType::Transcript),
        media_kind: Some(MediaKind::Movie),
        source_range: Some((tick(1), tick(2))),
        coverage: Some((EvidenceType::Tag, true)),
        model: Some("english".into()),
        producer: Some("speech-fixture".into()),
        ..Default::default()
    };
    assert_eq!(f.run(&scoped).hits.len(), 1);
    scoped.filters.source_range = Some((tick(4), tick(5)));
    assert!(f.run(&scoped).hits.is_empty());
    let original = f.root.path().join("a.mov");
    let bytes = std::fs::metadata(&original).unwrap().len();
    let limited = grant(&mut f.s, f.root.path(), false, bytes);
    let r = f.s.search_media(&limited, &q, &models(), &Default::default()).unwrap();
    assert_eq!(r.coverage.verified_bytes, bytes);
    assert_eq!(r.hits.len(), 1);
    assert_eq!(r.coverage.budget_limited.len(), 1);
    let outside = tempfile::tempdir().unwrap();
    let denied = grant(&mut f.s, outside.path(), false, 64 << 20);
    assert_eq!(f.s.search_media(&denied, &q, &models(), &Default::default()).unwrap_err().code, ErrorCode::PermissionRequired);
    let cancel = Cancellation::default();
    cancel.cancel();
    assert_eq!(f.s.search_media(&f.client, &q, &models(), &cancel).unwrap_err().code, ErrorCode::Canceled);
    let mut changed = models();
    changed.get_mut("speech-fixture").unwrap().producer_version = "2".into();
    assert!(f.s.search_media(&f.client, &q, &changed, &Default::default()).unwrap().hits.is_empty());
    let q = SearchQuery { include_stale: true, filters: SearchFilters { asset: Some(f.ids[0]), ..Default::default() }, ..q };
    let stale = f.s.search_media(&f.client, &q, &changed, &Default::default()).unwrap();
    assert_eq!(stale.hits[0].best.producer_freshness, Freshness::Stale);
    let folder = f.root.path().join("moved");
    std::fs::create_dir(&folder).unwrap();
    let moved = folder.join("a.mov");
    std::fs::copy(&original, &moved).unwrap();
    f.s.execute("media.relink", json!({"item":f.ids[0].0,"path":moved,"force":false,"relinkOthers":false})).unwrap();
    assert_eq!(f.run(&q).hits[0].best.source_freshness, Freshness::Current);
    let revision = f.s.revision;
    let mut data = std::fs::read(&moved).unwrap();
    let mid = data.len() / 2;
    data[mid] ^= 1;
    std::fs::write(&moved, data).unwrap();
    assert_eq!(f.run(&q).hits[0].best.source_freshness, Freshness::Stale);
    assert_eq!(f.s.revision, revision);
    let other = f.root.path().join("b.mov");
    f.s.execute("media.relink", json!({"item":f.ids[0].0,"path":other,"force":true,"relinkOthers":false})).unwrap();
    assert_eq!(f.run(&q).hits[0].best.source_freshness, Freshness::Stale);
    std::fs::remove_file(other).unwrap();
    let missing = f.run(&SearchQuery { include_unverified: true, ..q });
    assert!(missing.coverage.missing_originals.contains(&f.ids[0]));
    assert_eq!(missing.hits[0].best.source_freshness, Freshness::Unverified);
}

#[test]
fn new_revision_publishes_untimed_speech_at_its_original_precision() {
    let mut f = Fixture::new();
    let mut q = SearchQuery::new("distinct utterance");
    q.filters.evidence_type = Some(EvidenceType::Transcript);
    assert!(f.run(&q).hits.is_empty());
    let asset = f.ids[0];
    let fp = f.s.inspect_media_observations(&f.client, asset, &models(), &Default::default()).unwrap().fingerprint.unwrap();
    let mut req = ObservationRequest::verified_asset(asset, f.s.revision, fp);
    let location = SourceLocation::SourceRange { stream: SourceStream::Audio(0), start: tick(1), end: tick(8) };
    req.kind = ObservationKind::Segment;
    req.location = location;
    req.summary = Some("a distinct utterance here".into());
    let version = models()["speech-fixture"].clone();
    req.producer = ObservationProducer::Model {
        producer: LOCAL_SPEECH_PRODUCER.into(),
        version: version.clone(),
        confidence: Confidence { score: 0.8, meaning: ConfidenceMeaning::ModelScore },
    };
    f.s.start_autosave(AutosaveConfig::new(&f.root.path().join("reopened-recovery"))).unwrap();
    let writer = grant(&mut f.s, f.root.path(), true, 64 << 20);
    let id = f.s.record_media_observation(&writer, req, &Default::default()).unwrap();
    let active = [(LOCAL_SPEECH_PRODUCER.into(), version)].into();
    let result = f.s.search_media(&f.client, &q, &active, &Default::default()).unwrap();
    assert_eq!(result.hits[0].best.evidence, EvidenceId::Observation(id));
    assert_eq!(result.hits[0].best.location, location);
    assert_eq!(result.hits[0].best.text, "a distinct utterance here");
    f.s.shutdown();
}
