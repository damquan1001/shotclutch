//! Native review surface. Only the editor commands can change a rough-cut map.

use std::sync::{Arc, atomic::Ordering, mpsc};

use egui::{Rect, Ui, pos2, vec2};
use serde_json::{Value, json};
use shotclutch_project::preedit::{AnalysisStatus, CandidateKind, CleanupMode, CleanupSettings, PreEditAnalysis, PreEditMap, Segment, TakeMatching};
use shotclutch_project::{ItemId, Project};
use shotclutch_time::{TICKS_PER_SECOND, Tick, TimeRange};

use crate::frames::{FrameKey, Target};
use crate::icons::{self, Icon};
use crate::panels::monitor;
use crate::{AudioOut, ShotClutchApp};

#[derive(Default)]
pub struct Workspace {
    tab: ReviewTab,
    show_cuts: bool,
    search: String,
    review_kind: Option<CandidateKind>,
    review_status: u8,
    review_index: Option<usize>,
    original: bool,
    source: Option<ItemId>,
    stream: usize,
    channel: Option<u16>,
    silence_only: bool,
    assemblyai: bool,
    assemblyai_key: String,
    local_model: String,
    cleanup: CleanupSettings,
    cleanup_initialized: bool,
    applied_cleanup: Option<CleanupSettings>,
    language: String,
    selection_start: String,
    selection_end: String,
    pause_ms: u32,
    handle_ms: u32,
    map: Option<u64>,
    seen_analysis: Option<(u64, bool)>,
    cursor: Tick,
    zoom: f32,
    window_start: Tick,
    zoom_drag: Option<ZoomDrag>,
    follow_cursor: bool,
    mark_in: Option<Tick>,
    mark_out: Option<Tick>,
    timeline: TimelineSelection,
    text_anchor: Option<TimeRange>,
    text_selection: Option<TimeRange>,
    text_dragging: bool,
    correction: Option<TextCorrection>,
    script_layout: Option<(u64, u32, u32)>,
    script_corrections: std::collections::BTreeMap<i64, String>,
    paragraph_heights: std::collections::BTreeMap<Tick, (u32, f32)>,
    segments: Option<(u64, Arc<[Segment]>)>,
    pub(crate) toggle_requested: bool,
    message: String,
    result: Option<ResultView>,
    preview_builds: u64,
    preview_attempt: Option<(u64, u64)>,
    model_install: ModelInstall,
    document: Option<String>,
    local_review: bool,
    review_install: ModelInstall,
    pending_rule_cut: Option<(ItemId, u64)>,
    review_job: Option<LocalJob>,
    evidence_install: ModelInstall,
}

struct LocalJob {
    evidence: bool,
    progress: Option<Arc<std::sync::Mutex<shotclutch_engine::preedit::take_review::ReviewProgress>>>,
    pairs: bool,
    map: u64,
    revision: u64,
    cancel: Arc<std::sync::atomic::AtomicBool>,
    result: mpsc::Receiver<Result<(Value, String), String>>,
}
impl Drop for LocalJob {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed)
    }
}
#[derive(Default)]
struct TimelineSelection {
    range: Option<TimeRange>,
    select_at: Option<Tick>,
    trim: Option<RegionTrim>,
    peaks: Option<WaveformPeaks>,
}

struct WaveformPeaks {
    analysis: u64,
    chunks: usize,
    window: TimeRange,
    bins: Vec<u8>,
}

struct RegionTrim {
    range: TimeRange,
    removed: bool,
    left: bool,
    target: Tick,
    pointer_offset: f32,
}

struct TextCorrection {
    map: u64,
    revision: u64,
    words: Vec<(Tick, String)>,
}

#[derive(Default)]
struct ModelInstall {
    model: Option<String>,
    pending: Option<ModelDownload>,
    confirm: bool,
    result: Option<Result<(), String>>,
}

struct ModelDownload {
    progress: Arc<shotclutch_export::Progress>,
    result: mpsc::Receiver<Result<(), String>>,
}

#[derive(Default, PartialEq)]
enum ReviewTab {
    #[default]
    Script,
    Files,
    Cuts,
}

struct ResultView {
    map: u64,
    revision: u64,
    source: ItemId,
    use_proxies: bool,
    project: Arc<Project>,
    sequence: ItemId,
    time: Tick,
    playing: Option<(f64, Tick, Option<u32>)>,
}

impl Workspace {
    pub(crate) fn preview_stats(&self) -> Value {
        json!({"builds":self.preview_builds,"map":self.result.as_ref().map(|v| v.map),
            "revision":self.result.as_ref().map(|v| v.revision),"playing":self.is_playing()})
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.result.as_ref().is_some_and(|view| view.playing.is_some())
    }

    pub(crate) fn stop(&mut self, audio: Option<&mut (dyn AudioOut + 'static)>) {
        if let Some(result) = &mut self.result
            && result.playing.take().is_some()
            && let Some(audio) = audio
        {
            audio.stop();
        }
    }

    fn command(&mut self, app: &mut ShotClutchApp, id: &str, args: Value) -> Option<Value> {
        match app.session.execute(id, args) {
            Ok(reply) => {
                self.message.clear();
                Some(reply)
            }
            Err(err) => {
                self.message = err.to_string();
                None
            }
        }
    }

    fn review(&mut self, app: &mut ShotClutchApp, map: u64, action: Value) {
        self.stop(app.audio.as_deref_mut());
        app.stop_source();
        let revision = app.session.revision.to_string();
        if let Some(reply) = self.command(app, "preedit.review", json!({"map":map.to_string(),"expectedRevision":revision,"action":action})) {
            self.map = reply["map"].as_str().and_then(|value| value.parse().ok());
            if let Some(s) = reply["reviewSummary"].as_str() {
                self.message = s.into()
            }
        }
    }

    fn audition(&mut self, app: &mut ShotClutchApp, source: ItemId, stream: usize, range: TimeRange) {
        self.stop(app.audio.as_deref_mut());
        self.original = true;
        if let Err(error) = app.audition_rough_cut_source(source, stream, range.start, range.end()) {
            self.message = error;
        }
    }

    fn cleanup_options(&self, cut: bool) -> CleanupSettings {
        CleanupSettings {
            mode: self.cleanup.mode,
            take_matching: self.cleanup.take_matching,
            cut_bad_takes: cut && !self.silence_only && self.cleanup.cut_bad_takes,
            cut_fillers: cut && !self.silence_only && self.cleanup.cut_fillers,
            cut_pauses: cut && self.cleanup.cut_pauses,
            min_pause_ms: Some(self.pause_ms),
            handle_ms: Some(self.handle_ms),
            preserve_pauses_ms: self.cleanup.preserve_pauses_ms,
        }
    }

    fn sync_cleanup(&mut self, saved: Option<CleanupSettings>, defaults: (u32, u32)) {
        let current = self.cleanup_options(true);
        // Follow undo/reopen/external updates only when the form is clean, or when
        // the new map acknowledges this form's Update cut. Preserve pending edits.
        if let Some(settings) = saved
            && (!self.cleanup_initialized || self.applied_cleanup.is_none_or(|previous| previous == current) || settings == current)
        {
            self.cleanup = settings;
            self.pause_ms = settings.min_pause_ms.unwrap_or(defaults.0);
            self.handle_ms = settings.handle_ms.unwrap_or(defaults.1);
            self.applied_cleanup = Some(self.cleanup_options(true));
        }
    }

    fn start(&mut self, app: &mut ShotClutchApp, resume: Option<u64>, cut: bool) {
        let Some(item) = self.source else { return };
        self.stop(app.audio.as_deref_mut());
        app.stop_source();
        self.text_selection = None;
        self.text_anchor = None;
        self.correction = None;
        let model = (!self.silence_only).then(|| if self.assemblyai { "assemblyai".into() } else { self.local_model.clone() });
        let range = if self.selection_start.trim().is_empty() && self.selection_end.trim().is_empty() {
            Value::Null
        } else {
            let parse = |s: &str| s.trim().parse::<f64>().ok().filter(|s| s.is_finite() && *s >= 0.0);
            let (Some(start), Some(end)) = (parse(&self.selection_start), parse(&self.selection_end)) else {
                self.message = "Enter both source range endpoints in seconds, or leave both blank for the full source.".into();
                return;
            };
            json!({"start":Tick::from_seconds_f64(start).0.to_string(),"end":Tick::from_seconds_f64(end).0.to_string()})
        };
        if self
            .command(
                app,
                "preedit.analyze",
                json!({
                    "item":item.0.to_string(),"audioStream":self.stream,"channel":self.channel,"model":model,"language":self.language,"range":range,
                    "apiKey":if self.assemblyai { Some(&self.assemblyai_key) } else { None },"minPauseMs":self.pause_ms,"handleMs":self.handle_ms,"autoCut":true,"cleanup":self.cleanup_options(cut),
                    "previousMap":self.map.map(|id| id.to_string()),"resume":resume.map(|id| id.to_string())
                }),
            )
            .is_some()
        {
            self.pending_rule_cut=(cut && self.local_review && self.cleanup.cut_bad_takes && !self.silence_only).then(||(item,app.session.project.preedit.maps.keys().next_back().copied().unwrap_or(0)));
            self.cleanup_initialized = true;
            self.tab = ReviewTab::Script;
        }
    }

    fn saved_analysis_map(&self, app: &ShotClutchApp) -> Option<u64> {
        self.map.filter(|id| {
            app.session
                .project
                .preedit
                .maps
                .get(id)
                .and_then(|map| app.session.project.preedit.analyses.get(&map.analysis))
                .is_some_and(|analysis| analysis.status == AnalysisStatus::Complete)
        })
    }

    fn update_cut(&mut self, app: &mut ShotClutchApp, id: u64) {
        let old = app
            .session
            .project
            .preedit
            .maps
            .get(&id)
            .and_then(|map| app.session.project.preedit.analyses.get(&map.analysis).map(|a| (map.analysis, a.clone())));
        if let Some((analysis_id, analysis)) = old
            && analysis.analyzer_version < 3
            && self.cleanup.mode == CleanupMode::Natural
        {
            self.refresh_audio(app, id, analysis_id, &analysis);
            return;
        }
        let mut options = self.cleanup_options(true);
        if options.mode == CleanupMode::GlingCompatible
            && !matches!(options.take_matching, TakeMatching::Legacy | TakeMatching::OrderedV7 | TakeMatching::OrderedV8)
        {
            options.take_matching = TakeMatching::OrderedV6;
        }
        self.review(app, id, json!({"kind":"cleanUp","options":options}));
        if self.map != Some(id) {
            if self.local_review
                && options.cut_bad_takes
                && let Some(checkpoint) = self.map
            {
                self.begin_review(app, checkpoint, false);
            }
            self.cleanup.take_matching = options.take_matching;
            self.original = false;
            self.tab = ReviewTab::Script;
        }
    }

    fn begin_review(&mut self, app: &ShotClutchApp, id: u64, pairs: bool) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let map = app.session.project.preedit.maps[&id].clone();
            let analysis = app.session.project.preedit.analyses[&map.analysis].clone();
            let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let worker = cancel.clone();
            let (send, result) = mpsc::channel();
            match std::thread::Builder::new().name("local-take-review".into()).spawn(move || {
                let reply = if pairs {
                    shotclutch_engine::preedit::pair_review::run(id, &map, &analysis, &worker).map(|(r, s)| (json!(r), s))
                } else {
                    shotclutch_engine::preedit::local_review::run(id, &map, &analysis, &worker).map(|(r, s)| (json!(r), s))
                };
                let _ = send.send(reply);
            }) {
                Ok(_) => {
                    self.review_job = Some(LocalJob { evidence: false, progress: None, pairs, map: id, revision: app.session.revision, cancel, result });
                    self.message = "Reviewing takes locally…".into();
                }
                Err(e) => self.message = e.to_string(),
            }
        }
    }
    fn begin_evidence_review(&mut self, app: &mut ShotClutchApp, id: u64) {
        let mut settings = self.cleanup_options(true);
        settings.mode = CleanupMode::GlingCompatible;
        settings.take_matching = TakeMatching::OrderedV6;
        settings.cut_bad_takes = true;
        self.review(app, id, json!({"kind":"cleanUp","options":settings}));
        let Some(checkpoint) = self.map.filter(|&v| v != id) else { return };
        #[cfg(not(target_arch = "wasm32"))]
        {
            let map = app.session.project.preedit.maps[&checkpoint].clone();
            let analysis = app.session.project.preedit.analyses[&map.analysis].clone();
            let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let worker = cancel.clone();
            let (send, result) = mpsc::channel();
            let progress = Arc::new(std::sync::Mutex::new(Default::default()));
            let worker_progress = progress.clone();
            let stamp = match shotclutch_engine::preedit::take_review_stamp(&app.session, checkpoint) {
                Ok(s) => s,
                Err(e) => {
                    self.message = e.to_string();
                    return;
                }
            };
            let model = shotclutch_engine::preedit::local_review::MODEL.id;
            match std::thread::Builder::new().name("evidence-take-review".into()).spawn(move || {
                let result = shotclutch_engine::preedit::take_review::run(checkpoint, &map, &analysis, model, &worker, &worker_progress).map(|mut r| {
                    r.source_stamp = Some(stamp);
                    (json!(r), "Evidence review completed".into())
                });
                let _ = send.send(result);
            }) {
                Ok(_) => {
                    self.review_job = Some(LocalJob {
                        evidence: true,
                        progress: Some(progress),
                        pairs: false,
                        map: checkpoint,
                        revision: app.session.revision,
                        cancel,
                        result,
                    });
                    self.message = "Reviewing spoken attempts locally…".into();
                }
                Err(e) => self.message = e.to_string(),
            }
        }
    }
    fn refresh_audio(&mut self, app: &mut ShotClutchApp, map: u64, id: u64, analysis: &PreEditAnalysis) {
        self.reuse_analysis(app, map, id, analysis, false);
    }

    fn reuse_analysis(&mut self, app: &mut ShotClutchApp, map: u64, id: u64, analysis: &PreEditAnalysis, recheck: bool) {
        self.stop(app.audio.as_deref_mut());
        app.stop_source();
        let source = &analysis.source;
        let mut cleanup = self.cleanup_options(true);
        if recheck && !matches!(cleanup.take_matching, TakeMatching::OrderedV7 | TakeMatching::OrderedV8) {
            cleanup.take_matching = TakeMatching::OrderedV6;
        }
        self.command(
            app,
            "preedit.analyze",
            json!({
                "item":source.item.0.to_string(), "audioStream":source.audio_stream, "channel":source.channel,
                "model":analysis.settings.model, "language":analysis.settings.language,
                "range":{"start":source.range.start.0.to_string(),"end":source.range.end().0.to_string()},
                "reuseTranscript":id.to_string(),"previousMap":map.to_string(),"autoCut":true,
                "minPauseMs":self.pause_ms,"handleMs":self.handle_ms,"cleanup":cleanup,"recheckRetakes":recheck
            }),
        );
    }

    fn inputs(&mut self, app: &mut ShotClutchApp, ui: &mut Ui, primary_above_settings: bool) {
        let items: Vec<_> = app
            .session
            .project
            .items
            .values()
            .filter(|item| item.as_media().is_some_and(|media| media.info.has_audio()))
            .map(|item| (item.id, item.name.clone()))
            .collect();
        if !items.iter().any(|(item, _)| Some(*item) == self.source) {
            self.source = app
                .session
                .state
                .project_selection
                .iter()
                .copied()
                .find(|id| items.iter().any(|(item, _)| id == item))
                .or_else(|| items.first().map(|(id, _)| *id));
        }
        let running = app.session.preedit_job.is_some();
        let tokens = app.tokens;
        ui.horizontal_wrapped(|ui| {
            section_title(ui, &tokens, "01", "Source");
            if ui.add_enabled(!running, egui::Button::new("Import media…")).clicked() {
                if let Err(error) = crate::menus::invoke(app, ui.ctx(), "file.import", json!({})) {
                    self.message = error;
                }
            }
        });
        ui.add_space(4.0);
        let before = self.source;
        ui.add_enabled_ui(!running, |ui| {
            egui::ComboBox::from_id_salt("rough-cut-source")
                .width((ui.available_width() - 20.0).max(80.0))
                .truncate()
                .selected_text(items.iter().find(|(id, _)| Some(*id) == self.source).map_or("Select media", |(_, name)| name))
                .show_ui(ui, |ui| {
                    for (id, name) in &items {
                        ui.selectable_value(&mut self.source, Some(*id), name);
                    }
                });
        });
        if self.source != before {
            self.stream = 0;
            self.channel = None;
            self.selection_start.clear();
            self.selection_end.clear();
            self.map = None;
            self.cleanup_initialized = false;
            self.cursor = Tick::ZERO;
            self.window_start = Tick::ZERO;
            self.mark_in = None;
            self.mark_out = None;
            app.stop_source();
            self.stop(app.audio.as_deref_mut());
            self.result = None;
        }
        let Some(item) = self.source else {
            ui.label(egui::RichText::new("Import a clip with audio to create a transcript and rough cut.").color(tokens.text_dim));
            return;
        };
        let Some(media) = app.session.project.item(item).and_then(|item| item.as_media()) else { return };
        let duration = media.duration();
        let streams = media.info.audio_streams.clone();
        self.stream = self.stream.min(streams.len().saturating_sub(1));
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(!running, |ui| {
                ui.label(if self.stream == 0 { "Source audio".into() } else { format!("Source audio · Track {}", self.stream + 1) });
                ui.checkbox(&mut self.silence_only, "Silence only");
                if ui.button("Listen to source audio").clicked() {
                    let start = if app.session.state.source_item == Some(item) { app.session.state.source_playhead } else { Tick::ZERO };
                    let end = (start + Tick(10 * TICKS_PER_SECOND)).min(duration);
                    if end > start {
                        self.audition(app, item, self.stream, TimeRange::from_bounds(start, end));
                    }
                }
            });
        });
        ui.add_enabled_ui(!running, |ui| {
            ui.collapsing("Advanced source settings", |ui| {
                if streams.len() > 1 {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Audio track");
                        let selected = if self.stream == 0 { "Main audio".into() } else { format!("Track {}", self.stream + 1) };
                        let response = egui::ComboBox::from_id_salt("rough-cut-audio")
                            .selected_text(selected)
                            .show_ui(ui, |ui| {
                                for (index, stream) in streams.iter().enumerate() {
                                    let name = if index == 0 { "Main audio".into() } else { format!("Track {}", index + 1) };
                                    ui.selectable_value(&mut self.stream, index, format!("{name} · {} channels · {} Hz", stream.channels, stream.sample_rate));
                                }
                            })
                            .response;
                        app.auto.add_response("roughCut.source.audioTrack", &response, "Audio track");
                    });
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label("Source start / end (seconds)");
                    ui.add(egui::TextEdit::singleline(&mut self.selection_start).desired_width(55.0));
                    ui.add(egui::TextEdit::singleline(&mut self.selection_end).desired_width(55.0));
                    ui.label("Blank uses the full source");
                });
                ui.horizontal_wrapped(|ui| {
                    let channels = streams.get(self.stream).map_or(0, |s| s.channels.min(32));
                    if self.channel.is_some_and(|c| u32::from(c) >= channels) {
                        self.channel = None;
                    }
                    egui::ComboBox::from_id_salt("rough-cut-channel")
                        .selected_text(self.channel.map_or_else(|| "Downmix for analysis".into(), |c| format!("Analyze channel {}", c + 1)))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.channel, None, "Downmix for analysis");
                            for c in 0..channels as u16 {
                                ui.selectable_value(&mut self.channel, Some(c), format!("Channel {}", c + 1));
                            }
                        });
                    ui.label("Language");
                    ui.add(egui::TextEdit::singleline(&mut self.language).char_limit(2).desired_width(28.0));
                });
            });
        });
        ui.add_space(8.0);
        ui.separator();
        ui.add_enabled_ui(!running && !self.silence_only, |ui| {
            ui.horizontal(|ui| {
                let local = ui.selectable_value(&mut self.assemblyai, false, "Whisper · Local");
                app.auto.add_response("roughCut.provider.local", &local, "Whisper");
                let cloud = ui.selectable_value(&mut self.assemblyai, true, "AssemblyAI · Cloud");
                app.auto.add_response("roughCut.provider.assemblyai", &cloud, "AssemblyAI");
            });
            if self.assemblyai {
                ui.label("Transcribe sends the selected audio to AssemblyAI. API usage is billed to your account.");
                let key = ui.add(egui::TextEdit::singleline(&mut self.assemblyai_key).password(true).hint_text("AssemblyAI API key (or ASSEMBLYAI_API_KEY)"));
                app.auto.add_response("roughCut.assemblyai.key", &key, "AssemblyAI API key");
                ui.label(egui::RichText::new("Key kept for this session. Completed audio sections can be resumed.").small().color(tokens.text_dim));
            } else {
                let profile = egui::ComboBox::from_id_salt("smart-cut-speech-profile")
                    .selected_text(shotclutch_engine::transcript::speech_model(&self.local_model).map_or("Select speech profile", |m| m.name))
                    .show_ui(ui, |ui| {
                        for model in shotclutch_engine::transcript::model_catalogue() {
                            let response = ui.selectable_value(&mut self.local_model, model.id.into(), model.name);
                            app.auto.add_response(&format!("roughCut.profile.{}", model.id), &response, model.name);
                        }
                    })
                    .response;
                app.auto.add_response("roughCut.profile", &profile, "Speech profile");
            }
        });
        section_title(ui, &tokens, "02", "Speech cleanup");
        ui.add_enabled_ui(!running, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (mode, label, id) in [
                    (CleanupMode::GlingCompatible, "Automatic", "gling"),
                    (CleanupMode::Natural, "Conservative", "natural"),
                    (CleanupMode::Transcript, "Legacy transcript", "transcript"),
                ] {
                    let response = ui.selectable_value(&mut self.cleanup.mode, mode, label);
                    app.auto.add_response(&format!("roughCut.mode.{id}"), &response, label);
                }
            });
            ui.label(
                egui::RichText::new(if self.cleanup.mode == CleanupMode::Natural {
                    "Use audio levels to protect speech and keep natural pacing."
                } else if self.cleanup.mode == CleanupMode::GlingCompatible {
                    "Detect repeated takes and fillers from the transcript. Choose pause pacing below."
                } else {
                    "Use the earlier transcript cleanup rules. Review and restore any cut."
                })
                .small()
                .color(tokens.text_dim),
            );
            ui.add_enabled_ui(!self.silence_only, |ui| {
                cleanup_option(ui, &tokens, &mut self.cleanup.cut_bad_takes, "Remove repeated takes", "Detect repeated attempts. Compare or restore alternatives in Review cuts.");
                if self.cleanup.mode == CleanupMode::GlingCompatible {
                    ui.add_enabled_ui(self.cleanup.cut_bad_takes, |ui| {
                        let mut ordered = self.cleanup.take_matching != TakeMatching::Legacy;
                        let response = ui.checkbox(&mut ordered, "Context-aware takes (preview)").on_hover_text("Match repeated phrases across sentence breaks and small wording differences. Update cut uses the latest preview rules.");
                        app.auto.add_response("roughCut.takes.ordered", &response, "Context-aware takes");
                        if response.changed() {
                            self.cleanup.take_matching = if ordered { TakeMatching::OrderedV6 } else { TakeMatching::Legacy };
                        }
                        if ordered {
                            let mut verify=self.cleanup.take_matching==TakeMatching::OrderedV8;
                            if ui.checkbox(&mut verify,"Require coverage by the later reading (experimental)").on_hover_text("Keep unique words after a repeated opening. This stricter check can also keep genuine retakes.").changed(){self.cleanup.take_matching=if verify{TakeMatching::OrderedV8}else{TakeMatching::OrderedV6};}
                            let mut reference_first=matches!(self.cleanup.take_matching,TakeMatching::OrderedV7|TakeMatching::OrderedV8);
                            if ui.checkbox(&mut reference_first,"Apply transcript rules first (experimental)").on_hover_text("Keeps the broad transcript-rule cuts before context matching. A repeated opening can remove intervening speech, including good takes.").changed(){
                                self.cleanup.take_matching=if reference_first {TakeMatching::OrderedV7} else {TakeMatching::OrderedV6};
                            }
                        }
                    });
                }
                if self.cleanup.cut_bad_takes {
                    ui.checkbox(&mut self.local_review,"Review takes with a local model (experimental)");
                    if self.local_review { ui.label("Requires llama.cpp (llama-completion). The transcript stays on this device during review."); self.review_install.show(app,ui,shotclutch_engine::preedit::local_review::MODEL.id); }
                }
                cleanup_option(ui, &tokens, &mut self.cleanup.cut_fillers, "Remove fillers", "Cut hesitation sounds such as “um” and “uh.”");
            });
            let pause_help = if self.cleanup.mode == CleanupMode::GlingCompatible {
                "Cut gaps outside timed speech, including the recording edges."
            } else {
                "Shorten quiet gaps while keeping breathing room."
            };
            cleanup_option(ui, &tokens, &mut self.cleanup.cut_pauses, "Trim silences and pauses", pause_help);
            ui.add_enabled_ui(self.cleanup.cut_pauses, |ui| {
                if self.cleanup.mode == CleanupMode::GlingCompatible {
                    let mut preserve = self.cleanup.preserve_pauses_ms.is_some();
                    let response = ui.checkbox(&mut preserve, "Preserve natural pauses");
                    app.auto.add_response("roughCut.pause.preserve", &response, "Preserve natural pauses");
                    if response.changed() {
                        self.cleanup.preserve_pauses_ms = preserve.then_some(800);
                    }
                    if let Some(floor) = &mut self.cleanup.preserve_pauses_ms {
                        ui.add(egui::DragValue::new(floor).range(200..=10_000).prefix("Keep pauses up to ").suffix(" ms"));
                        ui.label(egui::RichText::new("Between kept speech. Longer gaps retain breathing room; take detection stays unchanged.").small().color(tokens.text_dim));
                    }
                }
                ui.horizontal_wrapped(|ui| {
                    let threshold_label = if self.cleanup.mode == CleanupMode::GlingCompatible && self.cleanup.preserve_pauses_ms.is_some() {
                        "Take separation "
                    } else {
                        "Minimum pause "
                    };
                    ui.add(egui::DragValue::new(&mut self.pause_ms).range(200..=10_000).prefix(threshold_label).suffix(" ms"));
                    ui.add_enabled(
                        self.cleanup.mode != CleanupMode::GlingCompatible || self.cleanup.preserve_pauses_ms.is_some(),
                        egui::DragValue::new(&mut self.handle_ms).range(0..=1000).prefix(if self.cleanup.mode==CleanupMode::GlingCompatible && self.cleanup.preserve_pauses_ms.is_some() {"Breathing room "} else {"Padding "}).suffix(" ms"),
                    )
                    .on_hover_text("Natural pauses keep this total space around a join, with at least 250 ms after sentences. Other modes use per-edge padding; exact transcript rules use none.");
                });
            });
        });
        ui.add_space(8.0);
        ui.separator();
        section_title(ui, &tokens, "03", "Process");
        let saved = self.saved_analysis_map(app);
        ui.horizontal_wrapped(|ui| {
            if running {
                if ui.button("Cancel analysis").clicked() {
                    self.command(app, "preedit.cancel", json!({}));
                }
            } else {
                if let Some(id) = saved.filter(|_| !primary_above_settings) {
                    let recut = ui
                        .add(primary_button(&tokens, "Update cut", true))
                        .on_hover_text("Apply these settings to the saved analysis without transcribing again");
                    app.auto.add_response("roughCut.cleanUpSaved", &recut, "Update cut");
                    if recut.clicked() {
                        self.update_cut(app, id);
                    }
                }
                if let Some(id) = saved {
                    let response = ui
                        .button("Refresh audio & cut")
                        .on_hover_text("Remeasure the original audio using the saved transcript. No speech model inference is needed.");
                    app.auto.add_response("roughCut.refreshAudio", &response, "Refresh audio & cut");
                    if response.clicked() {
                        let map = &app.session.project.preedit.maps[&id];
                        let analysis_id = map.analysis;
                        let analysis = app.session.project.preedit.analyses[&analysis_id].clone();
                        self.refresh_audio(app, id, analysis_id, &analysis);
                    }
                    let map = &app.session.project.preedit.maps[&id];
                    let analysis_id = map.analysis;
                    let analysis = app.session.project.preedit.analyses[&analysis_id].clone();
                    if map.automatic.is_some()
                        && self.review_job.is_none()
                        && ui
                            .button("Compare remaining takes (experimental)")
                            .on_hover_text("Compare nearby surviving readings with the local review model. Keeps existing edit decisions.")
                            .clicked()
                    {
                        self.begin_review(app, id, true);
                    }
                    ui.separator();
                    ui.label("Evidence-based take review (experimental)");
                    ui.label("Local reviewer: Granite 2B");
                    self.evidence_install.show(app, ui, shotclutch_engine::preedit::local_review::MODEL.id);
                    if self.review_job.is_none()
                        && ui
                            .button("Review takes with evidence")
                            .on_hover_text("Starts from conservative rules, then compares attempts and independently checks proposed cuts.")
                            .clicked()
                    {
                        self.begin_evidence_review(app, id);
                    }
                    if let Some(job) = &self.review_job {
                        if let Some(progress) = &job.progress {
                            let p = progress.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                            ui.label(format!("{} · {}/{} groups", p.phase, p.completed, p.total));
                        }
                        if ui.button("Cancel take review").clicked() {
                            job.cancel.store(true, Ordering::Relaxed);
                        }
                    }
                    let cleanup = self.cleanup_options(true);
                    if cleanup.mode == CleanupMode::GlingCompatible
                        && cleanup.take_matching != TakeMatching::Legacy
                        && cleanup.cut_bad_takes
                        && analysis.settings.model.is_some()
                        && analysis.settings.language == "en"
                    {
                        let response =
                            ui.button("Recheck retakes").on_hover_text("Recheck short sections of the original audio with Base English · Fast on this device.");
                        app.auto.add_response("roughCut.recheckRetakes", &response, "Recheck retakes");
                        if response.clicked() {
                            self.reuse_analysis(app, id, analysis_id, &analysis, true);
                        }
                    }
                }
                let label = if self.silence_only { "Analyze & cut pauses" } else { "Transcribe & cut" };
                if !primary_above_settings || saved.is_some() {
                    let cut = ui.add(primary_button(&tokens, label, saved.is_none()));
                    app.auto.add_response("roughCut.transcribeAndCut", &cut, label);
                    if cut.clicked() {
                        self.start(app, None, true);
                    }
                }
                if !self.silence_only {
                    let transcribe = ui.button("Transcribe only").on_hover_text("Create a timed script without automatic cuts");
                    app.auto.add_response("roughCut.transcribeOnly", &transcribe, "Transcribe only");
                    if transcribe.clicked() {
                        self.start(app, None, false);
                    }
                }
                if let Some((&id, record)) = app
                    .session
                    .project
                    .preedit
                    .analyses
                    .iter()
                    .rev()
                    .find(|(_, a)| {
                        a.source.item == item
                            && a.source.audio_stream == self.stream
                            && a.status != AnalysisStatus::Complete
                            && a.settings.model.is_some() != self.silence_only
                            && !a.chunks.is_empty()
                    })
                    .map(|(id, analysis)| (id, analysis.clone()))
                {
                    let cut = ui.button("Resume & cut").clicked();
                    let transcribe = !self.silence_only && ui.button("Resume transcription only").clicked();
                    if !cut && !transcribe {
                        return;
                    }
                    self.assemblyai = record.settings.model.as_deref() == Some("assemblyai");
                    if let Some(model) = record.settings.model.as_ref().filter(|m| m.as_str() != "assemblyai") {
                        self.local_model = model.clone();
                    }
                    self.channel = record.source.channel;
                    self.language.clone_from(&record.settings.language);
                    self.pause_ms = record.settings.min_pause.to_units_floor(1000) as u32;
                    self.handle_ms = record.settings.handle.to_units_floor(1000) as u32;
                    let range = record.source.range;
                    self.selection_start = (range.start.0 as f64 / TICKS_PER_SECOND as f64).to_string();
                    self.selection_end = (range.end().0 as f64 / TICKS_PER_SECOND as f64).to_string();
                    self.start(app, Some(id), cut);
                }
            }
        });
        if !self.silence_only && !self.assemblyai {
            ui.label(if self.language == "en" { "English · Local CPU speech" } else { "Local speech · word-based cleanup is tuned for English" });
            self.model_install.show(app, ui, &self.local_model);
        }
        if let Some((_, analysis)) = app.session.project.preedit.analyses.iter().rev().find(|(_, a)| a.source.item == item) {
            let end = analysis.chunks.last().map_or(analysis.source.range.start, |c| c.coverage.end());
            let covered = end - analysis.source.range.start;
            let fraction = covered.0 as f32 / analysis.source.range.duration.0 as f32;
            let state = match analysis.status {
                AnalysisStatus::Running if !running => "Interrupted",
                AnalysisStatus::Running => "Analyzing",
                AnalysisStatus::Complete => "Complete",
                AnalysisStatus::Canceled => "Canceled",
                AnalysisStatus::Failed => "Failed",
            };
            if running {
                ui.add(egui::ProgressBar::new(fraction).text(format!("{state} · {} analyzed", time_label(covered))));
            } else {
                ui.label(egui::RichText::new(format!("{state} · {} analyzed", time_label(covered))).color(tokens.text_dim));
            }
        } else if running {
            ui.label("Verifying the source and opening the selected stream…");
        }
    }

    fn seek_source(&mut self, app: &mut ShotClutchApp, item: ItemId, time: Tick) {
        self.stop(app.audio.as_deref_mut());
        app.stop_source();
        if app.session.state.source_item != Some(item) {
            self.command(app, "source.open", json!({"item":item.0}));
        }
        self.command(app, "source.setPlayhead", json!({"time":time.0}));
    }

    fn script(&mut self, app: &mut ShotClutchApp, ui: &mut Ui, id: u64, map: &PreEditMap, analysis: &PreEditAnalysis, segments: &[Segment]) {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Find in transcript").desired_width(ui.available_width()));
        });
        ui.label(egui::RichText::new("Click to seek · Drag or Shift-click to select words").small().color(app.tokens.text_dim));
        ui.add_space(4.0);
        let query = self.search.to_lowercase();
        let mut seek = None;
        let mut toolbar = None;
        let mut any = false;
        let (press, down, escape) = ui.input(|i| {
            let press = i.events.iter().find_map(|event| match event {
                egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers } => Some((*pos, modifiers.shift)),
                _ => None,
            });
            (press, i.pointer.primary_down(), i.key_pressed(egui::Key::Escape))
        });
        let mut pressed_word = None;
        let mut pointed_word = None;
        let mut clicked_word = None;
        if escape && self.correction.is_none() {
            self.text_selection = None;
            self.text_anchor = None;
        }
        egui::ScrollArea::vertical().id_salt("rough-cut-script").auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 7.0);
            let layout = (map.analysis, ui.available_width().to_bits(), ui.ctx().pixels_per_point().to_bits());
            if self.script_layout != Some(layout) || self.script_corrections != map.corrections {
                self.script_layout = Some(layout);
                self.script_corrections.clone_from(&map.corrections);
                self.paragraph_heights.clear();
            }
            for chunk in &analysis.chunks {
                let Some(text) = &chunk.transcript else { continue };
                for words in text.words.chunks(32) {
                    if !query.is_empty()
                        && !words
                            .iter()
                            .map(|w| map.corrections.get(&w.start.0).unwrap_or(&w.text).as_str())
                            .collect::<Vec<_>>()
                            .join(" ")
                            .to_lowercase()
                            .contains(&query)
                    {
                        continue;
                    }
                    let Some(first) = words.first() else { continue };
                    any = true;
                    // Cut decisions only affect wrapping when removed words are hidden.
                    let hidden = if self.show_cuts {
                        0
                    } else {
                        words.iter().enumerate().fold(0, |mask, (i, word)| {
                            let midpoint = word.start + Tick(word.range().duration.0 / 2);
                            mask | (u32::from(!retained_at(segments, midpoint)) << i)
                        })
                    };
                    let top = ui.cursor().top();
                    if let Some(&(previous_hidden, height)) = self.paragraph_heights.get(&first.start)
                        && previous_hidden == hidden
                        && (top + height < ui.clip_rect().top() || top > ui.clip_rect().bottom())
                    {
                        ui.add_space(height);
                        continue;
                    }
                    // Stable widget IDs keep selection and focus independent of which paragraphs were laid out.
                    ui.push_id(first.start.0, |ui| {
                        ui.add_space(10.0);
                        if ui.small_button(time_label(first.start)).on_hover_text("Seek original").clicked() {
                            seek = Some(first.start);
                        }
                        ui.horizontal_wrapped(|ui| {
                            for word in words {
                                let midpoint = word.start + Tick(word.range().duration.0 / 2);
                                let removed = !retained_at(segments, midpoint);
                                if removed && !self.show_cuts {
                                    continue;
                                }
                                let selected = self.text_selection.is_some_and(|r| r.contains(word.start));
                                let active = word.range().contains(self.cursor);
                                let text = map.corrections.get(&word.start.0).unwrap_or(&word.text);
                                let mut label = egui::RichText::new(text).size(16.0).color(if removed { app.tokens.text_dim } else { app.tokens.text });
                                if removed {
                                    label = label.strikethrough();
                                }
                                if selected {
                                    label = label.background_color(app.tokens.accent.gamma_multiply(0.55));
                                } else if active {
                                    label = label.background_color(app.tokens.row_selected);
                                }
                                // Wrap between word widgets so each word has one hit rectangle.
                                let response = ui.add(egui::Label::new(label).extend().selectable(false).sense(egui::Sense::click()));
                                if ui.is_rect_visible(response.rect) {
                                    app.auto.add_response(&format!("roughCut.word.{}", word.start.0), &response, text);
                                    if selected {
                                        toolbar = Some(response.rect);
                                    }
                                }
                                if let Some((position, shift)) = press
                                    && response.rect.intersect(ui.clip_rect()).contains(position)
                                    && ui.ctx().layer_id_at(position) == Some(ui.layer_id())
                                {
                                    pressed_word = Some((word.range(), shift));
                                }
                                if ui.rect_contains_pointer(response.rect) {
                                    pointed_word = Some(word.range());
                                }
                                if response.clicked() {
                                    clicked_word = Some((word.start, removed));
                                }
                                if response.secondary_clicked() {
                                    if !selected {
                                        self.text_selection = Some(word.range());
                                    }
                                    toolbar = Some(response.rect);
                                }
                            }
                        });
                    });
                    self.paragraph_heights.insert(first.start, (hidden, ui.cursor().top() - top));
                }
            }
            if !any {
                ui.label(if query.is_empty() { "No speech transcript. Review the cuts in the timeline." } else { "No matching phrases." });
            }
        });
        if self.correction.is_none() {
            let shift = pressed_word.is_some_and(|(_, shift)| shift);
            if let Some((word, _)) = pressed_word {
                self.text_dragging = true;
                if !shift || self.text_anchor.is_none() {
                    self.text_anchor = Some(word);
                    self.text_selection = None;
                }
            }
            if self.text_dragging
                && let Some((anchor, word)) = self.text_anchor.zip(pointed_word)
            {
                self.text_selection = (shift || anchor != word).then(|| TimeRange::from_bounds(anchor.start.min(word.start), anchor.end().max(word.end())));
                ui.ctx().request_repaint();
            }
            if let Some((time, removed)) = clicked_word
                && self.text_selection.is_none()
            {
                if removed {
                    self.original = true;
                    self.stop(app.audio.as_deref_mut());
                }
                seek = Some(time);
            }
        }
        if let Some(time) = seek {
            self.seek(app, analysis.source.item, segments, time);
        }
        self.text_dragging &= down;
        if let Some((range, anchor)) = self.text_selection.zip(toolbar).filter(|_| !self.text_dragging && self.correction.is_none()) {
            let mut action = None;
            egui::Area::new(egui::Id::new("rough-cut-text-actions")).order(egui::Order::Foreground).fixed_pos(pos2(anchor.left(), anchor.top() - 36.0)).show(
                ui.ctx(),
                |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (label, key) in [("Cut", "cut"), ("Uncut", "uncut"), ("Correct text", "correct"), ("Export clip", "export")] {
                                let button = ui.button(label);
                                app.auto.add_response(&format!("roughCut.text.{key}"), &button, label);
                                if button.clicked() {
                                    action = Some(key);
                                }
                            }
                        });
                    });
                },
            );
            match action {
                Some("cut" | "uncut") => {
                    self.review(app, id, range_action(range, if action == Some("cut") { "Remove" } else { "Keep" }));
                }
                Some("correct") => {
                    let words = analysis
                        .chunks
                        .iter()
                        .filter_map(|c| c.transcript.as_ref())
                        .flat_map(|t| &t.words)
                        .filter(|w| range.contains(w.start))
                        .map(|w| (w.start, map.corrections.get(&w.start.0).unwrap_or(&w.text).clone()))
                        .collect();
                    self.correction = Some(TextCorrection { map: id, revision: app.session.revision, words });
                }
                Some("export") => {
                    let video = app.session.project.item(analysis.source.item).and_then(|i| i.as_media()).is_some_and(|m| m.info.has_video());
                    let (filter, ext) = if video { ("MP4 video", "mp4") } else { ("WAV audio", "wav") };
                    let document = app.session.document_id;
                    app.pick_ui(crate::native_dialogs::FileDialog::save_as(filter, &[ext], &format!("Transcript clip.{ext}")), move |app, paths| {
                        if app.session.document_id == document
                            && let Some(path) = paths.into_iter().next()
                        {
                            let result = app.session.execute("preedit.exportClip", json!({"map":id.to_string(),"range":range_json(range),"path":path}));
                            app.rough_cut.message = match result {
                                Ok(_) => "Export started. Follow progress in Jobs.".into(),
                                Err(error) => error.to_string(),
                            };
                        }
                    });
                }
                _ => {}
            }
        }
        self.correct_text_dialog(app, ui, id);
    }

    fn correct_text_dialog(&mut self, app: &mut ShotClutchApp, ui: &Ui, id: u64) {
        let Some(mut edit) = self.correction.take() else { return };
        if edit.map != id || edit.revision != app.session.revision {
            return;
        }
        let mut open = true;
        let mut save = false;
        let mut cancel = false;
        egui::Window::new("Correct transcript text").collapsible(false).open(&mut open).show(ui.ctx(), |ui| {
            ui.label("Edit the words while keeping their timing.");
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for (tick, text) in &mut edit.words {
                    ui.horizontal(|ui| {
                        ui.monospace(time_label(*tick));
                        let field = ui.text_edit_singleline(text);
                        app.auto.add_response(&format!("roughCut.correct.{}", tick.0), &field, "Correct word");
                    });
                }
            });
            ui.horizontal(|ui| {
                let valid = edit.words.iter().all(|(_, text)| !text.trim().is_empty() && text.trim().len() <= 512);
                let apply = ui.add_enabled(valid, egui::Button::new("Apply correction"));
                app.auto.add_response("roughCut.correct.apply", &apply, "Apply correction");
                save = apply.clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if save {
            let words: Vec<_> = edit.words.iter().map(|(tick, text)| json!({"start":tick.0.to_string(),"text":text})).collect();
            self.review(app, id, json!({"kind":"correctText","words":words}));
        } else if open && !cancel && !ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.correction = Some(edit);
        }
    }

    fn cut_review(&mut self, app: &mut ShotClutchApp, ui: &mut Ui, id: u64, map: &PreEditMap, analysis: &PreEditAnalysis, segments: &[Segment]) {
        let tokens = app.tokens;
        ui.horizontal_wrapped(|ui| {
            for (kind, label) in
                [(None, "All"), (Some(CandidateKind::Pause), "Pauses"), (Some(CandidateKind::Filler), "Fillers"), (Some(CandidateKind::Take), "Takes")]
            {
                let response = ui.selectable_value(&mut self.review_kind, kind, label);
                app.auto.add_response(&format!("roughCut.review.filter.{label}"), &response, label);
            }
        });
        ui.horizontal(|ui| {
            for (status, label) in [(0, "All ranges"), (1, "Cut"), (2, "Kept")] {
                let response = ui.selectable_value(&mut self.review_status, status, label);
                app.auto.add_response(&format!("roughCut.review.status.{status}"), &response, label);
            }
        });
        let rows: Vec<_> = map
            .candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                if self.review_kind.is_some_and(|kind| candidate.kind != kind && !(kind == CandidateKind::Take && candidate.kind == CandidateKind::Restart)) {
                    return None;
                }
                let first = segments.partition_point(|s| s.source.end() <= candidate.range.start);
                let kept: i64 = segments[first..]
                    .iter()
                    .take_while(|s| s.source.start < candidate.range.end())
                    .map(|s| (s.source.end().min(candidate.range.end()) - s.source.start.max(candidate.range.start)).0.max(0))
                    .sum();
                let removed = kept < candidate.range.duration.0;
                if (self.review_status == 1 && !removed) || (self.review_status == 2 && removed) {
                    return None;
                }
                Some((index, candidate, removed, kept == 0))
            })
            .collect();
        ui.label(egui::RichText::new(format!("{} ranges · P / N to navigate · R to restore", rows.len())).small().color(tokens.text_dim));
        let mut selected = self.review_index.and_then(|index| rows.iter().position(|r| r.0 == index)).unwrap_or(0);
        let mut navigate = 0i32;
        let mut listen = None;
        let mut action = None;
        ui.horizontal(|ui| {
            for (label, step, key) in [("Previous", -1, "previous"), ("Next", 1, "next")] {
                let response = ui.add_enabled(!rows.is_empty(), egui::Button::new(label));
                app.auto.add_response(&format!("roughCut.review.{key}"), &response, label);
                if response.clicked() {
                    navigate = step;
                }
            }
        });
        if !ui.ctx().egui_wants_keyboard_input() {
            if ui.input(|i| i.key_pressed(egui::Key::N)) {
                navigate = 1;
            }
            if ui.input(|i| i.key_pressed(egui::Key::P)) {
                navigate = -1;
            }
            if ui.input(|i| i.key_pressed(egui::Key::R))
                && let Some(row) = rows.get(selected)
            {
                action = Some(range_action(row.1.range, "Keep"));
            }
        }
        if !rows.is_empty() {
            selected = (selected as i32 + navigate).rem_euclid(rows.len() as i32) as usize;
            self.review_index = Some(rows[selected].0);
            let row = rows[selected];
            ui.horizontal_wrapped(|ui| {
                let response = ui.button("Listen in context");
                app.auto.add_response("roughCut.review.listen", &response, "Listen in context");
                if response.clicked() {
                    listen = Some(row.1.range);
                }
                let response = ui.button(if row.2 { "Restore" } else { "Cut range" });
                app.auto.add_response("roughCut.review.restoreOrCut", &response, if row.2 { "Restore" } else { "Cut range" });
                if response.clicked() {
                    action = Some(range_action(row.1.range, if row.2 { "Keep" } else { "Remove" }));
                }
                if ui.button("Mark boundaries").clicked() {
                    self.mark_in = Some(row.1.range.start);
                    self.mark_out = Some(row.1.range.end());
                    self.original = true;
                    self.seek(app, analysis.source.item, segments, row.1.range.start);
                }
            });
            if let Some((group_index, group)) = map.takes.iter().enumerate().find(|(_, g)| g.alternatives.contains(&row.0)) {
                // Storage ends with the selected alternative, which may now be the earlier
                // performance. Number audition/choice buttons in source order for review.
                let mut display = group.alternatives.clone();
                display.sort_by_key(|&index| map.candidates[index].range.start);
                ui.horizontal_wrapped(|ui| {
                    ui.label("Compare");
                    for (n, &index) in display.iter().enumerate() {
                        let response = ui.button(format!("Take {}", n + 1));
                        app.auto.add_response(&format!("roughCut.review.take.{n}"), &response, "Audition take");
                        if response.clicked() {
                            listen = Some(map.candidates[index].range);
                        }
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    for (n, &index) in display.iter().enumerate() {
                        let response = ui.button(format!("Keep take {}", n + 1));
                        app.auto.add_response(&format!("roughCut.review.keepTake.{n}"), &response, "Keep this take");
                        if response.clicked() {
                            action = Some(json!({"kind":"chooseTake","group":group_index,"retain":[index]}));
                        }
                    }
                    let response = ui.button("Keep all");
                    app.auto.add_response("roughCut.review.keepBoth", &response, "Keep all takes");
                    if response.clicked() {
                        action = Some(json!({"kind":"chooseTake","group":group_index,"retain":group.alternatives}));
                    }
                });
                ui.menu_button("Group options", |ui| {
                    let reject = ui.button("Reject grouping and keep all takes");
                    app.auto.add_response("roughCut.review.rejectGroup", &reject, "Reject grouping");
                    if reject.clicked() {
                        action = Some(json!({"kind":"rejectGroup","group":group_index}));
                        ui.close();
                    }
                    if group.alternatives.len() > 2 {
                        for at in 1..group.alternatives.len() {
                            if ui.button(format!("Split after take {at}")).clicked() {
                                action = Some(json!({"kind":"splitGroup","group":group_index,"at":at}));
                                ui.close();
                            }
                        }
                    }
                });
            }
        }
        ui.separator();
        let mut scroll = egui::ScrollArea::vertical().id_salt("rough-cut-review").auto_shrink([false, false]);
        if navigate != 0 {
            scroll = scroll.vertical_scroll_offset(selected as f32 * 96.0);
        }
        scroll.show_rows(ui, 92.0, rows.len(), |ui, visible| {
            for row in &rows[visible] {
                let candidate = row.1;
                let kind = match candidate.kind {
                    CandidateKind::Pause => "Pause",
                    CandidateKind::Filler => "Filler",
                    CandidateKind::Restart => "Restart",
                    CandidateKind::Take => "Take",
                };
                let state = if row.3 {
                    "Cut"
                } else if row.2 {
                    "Partly cut"
                } else {
                    "Kept"
                };
                let response = ui.selectable_label(
                    self.review_index == Some(row.0),
                    format!("{kind} · {}–{} · {state}", time_label(candidate.range.start), time_label(candidate.range.end())),
                );
                app.auto.add_response(&format!("roughCut.review.row.{}", row.0), &response, state);
                if response.clicked() {
                    self.review_index = Some(row.0);
                }
                let reason = review_reason(&candidate.reason);
                ui.add(egui::Label::new(egui::RichText::new(&reason).small().color(tokens.text_dim)).truncate()).on_hover_text(&reason);
                let text = analysis
                    .chunks
                    .iter()
                    .filter_map(|c| c.transcript.as_ref())
                    .flat_map(|t| &t.words)
                    .filter(|w| w.start < candidate.range.end() && w.end > candidate.range.start)
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                ui.add(egui::Label::new(if text.is_empty() { "Quiet interval" } else { &text }).truncate());
                ui.label(egui::RichText::new("Rule-based detection · confidence unknown").small().color(tokens.text_dim));
                ui.separator();
            }
        });
        if let Some(range) = listen {
            let range = TimeRange::from_bounds(
                (range.start - Tick::from_units(3, 4)).max(analysis.source.range.start),
                (range.end() + Tick::from_units(3, 4)).min(analysis.source.range.end()),
            );
            self.audition(app, analysis.source.item, analysis.source.audio_stream, range);
        }
        if let Some(action) = action {
            self.review(app, id, action);
        }
    }

    fn original_view(&mut self, app: &mut ShotClutchApp, ui: &mut Ui, item: ItemId, range: TimeRange) {
        if app.session.state.source_item != Some(item) {
            self.seek_source(app, item, range.start);
        }
        let Some(view) = shotclutch_engine::clip_ops::source_view(&app.session, item) else { return };
        let Some(video) = app.session.project.item(item).and_then(|i| i.as_media()).and_then(|m| m.info.video.as_ref()) else {
            ui.centered_and_justified(|ui| {
                ui.label("Audio source");
            });
            return;
        };
        let frame_size = (video.width, video.height);
        let par = app.session.project.source_par(item).map_or(1.0, shotclutch_project::par_ratio) as f32;
        let area = ui.available_rect_before_wrap();
        let picture = monitor::fit(area, video.width as f32 * par, video.height as f32);
        let playing = app.source_playback.clock.playing;
        let scale = monitor::render_scale(picture, ui.ctx().pixels_per_point(), frame_size, app.ui.source.effective_res(playing));
        let time = app.session.state.source_playhead.clamp(range.start, range.end() - Tick(1));
        let key = FrameKey {
            target: Target::Item(item),
            frame: view.rate.frame_at(time),
            size: (scale * 1000.0) as u32,
            revision: app.item_revision(item),
            draft: crate::frames::draft_playback(playing, scale, app.session.prefs.playback.draft_decode),
        };
        let late_frames = if playing { review_frame_grace(view.rate) } else { 0 };
        if playing {
            app.frames.schedule_review_playback(key, view.rate, scale, &app.session.project, late_frames);
        } else {
            app.frames.request(key, view.rate.tick_of(key.frame), scale, &app.session.project, 0);
        }
        update_picture(app, ui, "rough-cut-original", key, late_frames);
        if let Some((texture, _)) =
            app.texture_existing("rough-cut-original").filter(|_| app.texture_key("rough-cut-original").is_some_and(|shown| shown.target == key.target))
        {
            paint_picture(ui, texture, picture);
        } else {
            ui.label("Loading original frame…");
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(if app.source_playback.clock.playing { 16 } else { 100 }));
    }

    fn result_surface(&mut self, app: &mut ShotClutchApp, ui: &mut Ui, map: u64, duration: Tick, segments: &[Segment]) {
        ui.separator();
        if self.result.as_ref().is_some_and(|view| {
            view.map != map
                || view.project.item(view.source) != app.session.project.item(view.source)
                || view.project.settings != app.session.project.settings
                || view.use_proxies != app.session.media.use_proxies()
        }) {
            self.stop(app.audio.as_deref_mut());
            self.result = None;
        }
        let attempt = (map, app.session.revision);
        let retry = self.result.is_none() && self.preview_attempt == Some(attempt) && ui.button("Retry result preview").clicked();
        if duration > Tick::ZERO && self.result.is_none() && (self.preview_attempt != Some(attempt) || retry) {
            self.preview_attempt = Some(attempt);
            self.preview_builds += 1;
            self.stop(app.audio.as_deref_mut());
            match shotclutch_engine::preedit::preview(&app.session, map) {
                Ok((project, sequence)) => {
                    let source = app.session.project.preedit.analyses[&app.session.project.preedit.maps[&map].analysis].source.item;
                    self.result = Some(ResultView {
                        map,
                        revision: app.session.revision,
                        source,
                        use_proxies: app.session.media.use_proxies(),
                        project,
                        sequence,
                        time: result_time(segments, self.cursor),
                        playing: None,
                    })
                }
                Err(error) => self.message = error.to_string(),
            }
        }
        let Some(view) = self.result.as_mut() else { return };

        let now = ui.input(|input| input.time);
        if let Some((anchor, start, sample_rate)) = view.playing {
            let elapsed = sample_rate
                .and_then(|rate| app.audio.as_ref()?.played_frames().map(|frames| Tick::from_units(frames as i64, i64::from(rate))))
                .unwrap_or_else(|| Tick::from_seconds_f64((now - anchor).max(0.0)));
            view.time = (start + elapsed).min(duration);
            if view.time >= duration {
                if let Some(audio) = app.audio.as_mut() {
                    audio.stop();
                }
                view.playing = None;
            }
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
        }
        let Some(sequence) = view.project.sequence(view.sequence) else { return };
        let rate = sequence.settings.frame_rate;
        if !sequence.video_tracks.is_empty() {
            let frame_size = (sequence.settings.width, sequence.settings.height);
            let par = shotclutch_project::par_ratio(sequence.settings.par) as f32;
            let picture = monitor::fit(ui.available_rect_before_wrap(), frame_size.0 as f32 * par, frame_size.1 as f32);
            let playing = view.playing.is_some();
            let scale = monitor::render_scale(picture, ui.ctx().pixels_per_point(), frame_size, app.ui.program.effective_res(playing));
            let gpu = app.gpu.is_some() && !crate::panels::menu_dialogs::software_renderer(app);
            let frame = rate.frame_at(view.time.min(duration - Tick(1)));
            let key = FrameKey {
                target: if gpu { Target::SequencePlan(view.sequence) } else { Target::SequenceLive(view.sequence) },
                frame,
                size: (scale * 1000.0) as u32,
                revision: u64::MAX - view.revision,
                draft: crate::frames::draft_playback(playing, scale, app.session.prefs.playback.draft_decode),
            };
            let late_frames = if playing { review_frame_grace(rate) } else { 0 };
            if late_frames > 0 {
                app.frames.schedule_review_playback(key, rate, scale, &view.project, late_frames);
            } else {
                app.frames.request(key, rate.tick_of(frame), scale, &view.project, 0);
            }
            let (texture, shown) = if gpu {
                monitor::gpu_texture(app, key, &[], late_frames)
            } else {
                update_picture(app, ui, "rough-cut-result", key, late_frames);
                (app.texture_existing("rough-cut-result").map(|(texture, _)| texture), app.texture_key("rough-cut-result"))
            };
            if let Some(texture) = texture.filter(|_| shown.is_some_and(|frame| frame.target == key.target && frame.revision == key.revision)) {
                paint_picture(ui, texture, picture);
            } else {
                ui.label("Loading result frame…");
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(30));
            }
        }

        self.cursor = segments
            .iter()
            .find_map(|segment| segment.source_at(view.time))
            .or_else(|| segments.last().map(|segment| segment.source.end()))
            .unwrap_or(self.cursor);
    }

    fn seek(&mut self, app: &mut ShotClutchApp, item: ItemId, segments: &[Segment], time: Tick) {
        self.cursor = time;
        self.follow_cursor = true;
        if self.original {
            self.seek_source(app, item, time);
        } else {
            self.stop(app.audio.as_deref_mut());
            if let Some(view) = &mut self.result {
                view.time = result_time(segments, time);
            }
        }
    }

    fn toggle(&mut self, app: &mut ShotClutchApp, ui: &Ui, item: ItemId, stream: usize, range: TimeRange, duration: Tick) {
        if self.original {
            if app.source_playback.clock.playing {
                app.stop_source();
            } else {
                let start = if self.cursor >= range.end() { range.start } else { self.cursor.max(range.start) };
                self.audition(app, item, stream, TimeRange::from_bounds(start, range.end()));
            }
            return;
        }
        let Some(view) = &mut self.result else { return };
        if view.playing.take().is_some() {
            app.frames.stop_prefetch();
            if let Some(audio) = app.audio.as_mut() {
                audio.stop();
            }
            return;
        }
        if view.time >= duration {
            view.time = Tick::ZERO;
        }
        app.stop();
        app.stop_source();
        let mut rate = None;
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(audio) = app.audio.as_mut() {
            let sr = audio.sample_rate();
            if sr > 0 && audio.channels() > 0 {
                let cursor = view.time.to_units_floor(i64::from(sr));
                let mixer = crate::playback_mix_snapshot(&app.session, view.project.clone(), view.sequence, sr, false);
                let fill = crate::play_ahead::spawn(mixer, cursor, sr, audio.channels(), Default::default());
                if audio.start(fill).is_ok() {
                    rate = Some(sr);
                }
            }
        }
        view.playing = Some((ui.input(|input| input.time), view.time, rate));
    }

    fn open_in_editor(&mut self, app: &mut ShotClutchApp, map: u64, replace: Option<u64>) {
        self.stop(app.audio.as_deref_mut());
        app.stop_source();
        let revision = app.session.revision;
        let operation = format!("rough-cut-{map}-{revision}");
        self.command(
            app,
            "preedit.applyBackground",
            json!({
                "map": map.to_string(), "expectedRevision": revision.to_string(), "operationId": operation, "replace": replace.map(|id| id.to_string())
            }),
        );
    }
}

impl ModelInstall {
    fn show(&mut self, app: &mut ShotClutchApp, ui: &mut Ui, model: &str) {
        if self.pending.is_none() && self.model.as_deref() != Some(model) {
            self.model = Some(model.into());
            self.confirm = false;
            self.result = None;
        }
        let selected = self.model.as_deref().unwrap_or(model).to_owned();
        let Some(info) = shotclutch_engine::preedit::take_review::model(&selected).or_else(|| shotclutch_engine::transcript::speech_model(&selected)) else {
            ui.label("Select a supported speech profile.");
            return;
        };
        if let Some(download) = &self.pending {
            let result = match download.result.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Disconnected) => Err("The model installer stopped.".into()),
                Err(mpsc::TryRecvError::Empty) => {
                    let done = download.progress.done.load(Ordering::Relaxed);
                    let total = download.progress.total.load(Ordering::Relaxed);
                    let status = download.progress.status.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
                    let fraction = if total == 0 { 0.0 } else { (done as f32 / total as f32).min(1.0) };
                    let text = if total == 0 {
                        "Checking local model files…".into()
                    } else {
                        format!("{:.0}%   ·   {:.1} / {:.1} MB", fraction * 100.0, done as f64 / 1e6, total as f64 / 1e6)
                    };
                    let bar = ui.add(egui::ProgressBar::new(fraction).animate(total == 0).text(&text));
                    app.auto.add_response("roughCut.model.progress", &bar, &text);
                    if total > 0 {
                        ui.label(if done >= total { "Verifying model files…" } else { &status });
                    } else {
                        ui.label(&status);
                    }
                    let cancel = ui.button("Cancel installation");
                    app.auto.add_response("roughCut.model.cancel", &cancel, "Cancel installation");
                    if cancel.clicked() {
                        download.progress.cancel.store(true, Ordering::Relaxed);
                    }
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
                    return;
                }
            };
            self.pending = None;
            self.result = Some(result);
        }
        match &self.result {
            Some(Ok(())) => {
                let label = format!("{} installed", info.name);
                let status = ui.label(&label);
                app.auto.add_response("roughCut.model.installed", &status, &label);
                return;
            }
            Some(Err(error)) => {
                ui.label(format!("Could not install {}. {error}", info.name));
            }
            None => {}
        }
        if !self.confirm {
            let label = if self.result.is_some() {
                "Retry model download…"
            } else if selected == shotclutch_engine::preedit::local_review::MODEL.id {
                "Install local review model…"
            } else {
                "Install local speech model…"
            };
            let response = ui.button(label);
            app.auto.add_response("roughCut.model.install", &response, label);
            self.confirm = response.clicked();
        }
        if self.confirm {
            ui.label(format!("{} · {:.0} MB · {}", info.name, info.size() as f64 / 1e6, info.license));
            ui.hyperlink_to("Model source", info.source);
            ui.hyperlink_to("Model license", info.license_url);
            if selected.starts_with("faster-whisper-") {
                ui.label("Installs an isolated Python CPU speech runtime and the selected model. Inference stays on this device.");
            }
            ui.horizontal(|ui| {
                let response = ui.button("Download model");
                app.auto.add_response("roughCut.model.download", &response, "Download model");
                if response.clicked() {
                    self.start(ui.ctx().clone(), selected.clone());
                }
                if ui.button("Cancel").clicked() {
                    self.confirm = false;
                }
            });
        }
    }

    fn start(&mut self, ctx: egui::Context, model: String) {
        let (send, result) = mpsc::channel();
        let progress = Arc::new(shotclutch_export::Progress::default());
        let worker = progress.clone();
        let started = std::thread::Builder::new().name("rough-cut-model".into()).spawn(move || {
            let result = shotclutch_engine::transcript::install_model(&model, &mut |done, total, file| {
                worker.done.store(done, Ordering::Relaxed);
                worker.total.store(total, Ordering::Relaxed);
                *worker.status.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = if total == 0 { file.into() } else { format!("Downloading {file}") };
                !worker.cancel.load(Ordering::Relaxed)
            });
            let _ = send.send(result.map(|_| ()).map_err(|error| error.to_string()));
            ctx.request_repaint();
        });
        self.result = None;
        match started {
            Ok(_) => self.pending = Some(ModelDownload { progress, result }),
            Err(error) => self.result = Some(Err(error.to_string())),
        }
        self.confirm = false;
    }
}

pub fn show(app: &mut ShotClutchApp, ui: &mut Ui, rect: Rect) {
    if cfg!(target_arch = "wasm32") {
        ui.put(rect, egui::Label::new("Smart Cut requires the native desktop app."));
        return;
    }
    let mut workspace = std::mem::take(&mut app.rough_cut);
    let document = app.session.document_id.to_string();
    if workspace.document.as_ref() != Some(&document) {
        workspace.stop(app.audio.as_deref_mut());
        workspace = Workspace {
            model_install: std::mem::take(&mut workspace.model_install),
            document: Some(document),
            language: "en".into(),
            local_model: "faster-whisper-tiny.en".into(),
            cleanup: CleanupSettings { mode: CleanupMode::GlingCompatible, preserve_pauses_ms: Some(800), ..Default::default() },
            show_cuts: true,
            zoom: 1.0,
            original: true,
            pause_ms: 500,
            handle_ms: 150,
            ..Default::default()
        };
    }

    if let Some(job) = &workspace.review_job {
        let source_changed = app
            .session
            .project
            .preedit
            .maps
            .get(&job.map)
            .and_then(|m| app.session.project.preedit.analyses.get(&m.analysis))
            .is_none_or(|a| Some(a.source.item) != workspace.source);
        if source_changed
            || app.session.revision != job.revision
            || (!job.evidence && !job.pairs && (!workspace.local_review || !workspace.cleanup.cut_bad_takes))
        {
            workspace.review_job = None;
            workspace.message = "Review stopped because the edit or settings changed.".into();
        } else {
            let result = job.result.try_recv();
            let id = job.map;
            let kind = if job.evidence {
                "evidenceReview"
            } else if job.pairs {
                "pairReview"
            } else {
                "localTakeReview"
            };
            match result {
                Ok(result) => {
                    workspace.review_job = None;
                    match result {
                        Ok((response, status)) => {
                            workspace.review(app, id, json!({"kind":kind,"response":response}));
                            if workspace.message.is_empty() {
                                workspace.message = status
                            }
                        }
                        Err(e) => workspace.message = e,
                    }
                }
                Err(mpsc::TryRecvError::Empty) => ui.ctx().request_repaint_after(std::time::Duration::from_millis(200)),
                Err(_) => {
                    workspace.review_job = None;
                    workspace.message = "Local review stopped; kept the rule cut.".into();
                }
            }
        }
    }
    let bounds = rect.shrink2(vec2(10.0, 8.0));
    if let Some(result) = app.session.preedit_apply_result.take() {
        match result {
            Ok(reply) => {
                if let Some(sequence) = reply["sequenceId"].as_str().and_then(|value| value.parse::<u64>().ok()) {
                    workspace.command(app, "sequence.open", json!({"item":sequence}));
                    app.set_workspace("Editing");
                }
            }
            Err(error) => workspace.message = error,
        }
    }
    if workspace.source.is_none() {
        workspace.source = app.session.project.items.values().find(|item| item.as_media().is_some_and(|m| m.info.has_audio())).map(|item| item.id);
    }
    let project = app.session.project.clone();
    let current =
        project.preedit.maps.iter().rev().find(|(_, map)| project.preedit.analyses.get(&map.analysis).is_some_and(|a| Some(a.source.item) == workspace.source));
    let map = current.map(|(&id, map)| (id, &**map));
    let analysis = map.map(|(_, map)| &*project.preedit.analyses[&map.analysis]);
    if let Some(((id, map), analysis)) = map.zip(analysis) {
        if workspace.segments.as_ref().is_none_or(|(cached, _)| *cached != id) {
            workspace.segments = Some((id, map.segments(analysis.source.range).unwrap_or_default().into()));
        }
    } else {
        workspace.segments = None;
    }
    if app.session.preedit_job.is_none()
        && let Some((source, previous)) = workspace.pending_rule_cut.take()
        && let Some((id, map)) = map
        && id > previous
        && map.automatic.is_some()
        && workspace.source == Some(source)
        && workspace.local_review
        && map.cleanup.is_some_and(|c| c.cut_bad_takes)
    {
        workspace.begin_review(app, id, false);
    }
    let segments = workspace.segments.as_ref().map(|(_, segments)| segments.clone()).unwrap_or_default();
    let duration = segments.last().map_or(Tick::ZERO, |segment| segment.output.end());
    let tokens = app.tokens;
    let can_open = map.is_some_and(|(_, map)| (map.automatic.is_some() || map.reviewed) && map.conflicts.is_empty())
        && duration > Tick::ZERO
        && app.session.preedit_apply.is_none()
        && app.session.preedit_job.is_none();
    let previous = project
        .preedit
        .applications
        .iter()
        .rev()
        .find(|(_, application)| {
            project.sequence(application.sequence).is_some()
                && project
                    .preedit
                    .maps
                    .get(&application.map)
                    .and_then(|old| project.preedit.analyses.get(&old.analysis))
                    .is_some_and(|old| Some(old.source.item) == workspace.source)
        })
        .map(|(&id, _)| id);
    let preview_actions = |ui: &mut Ui, app: &mut ShotClutchApp, workspace: &mut Workspace| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let open = ui
                .add_enabled(can_open, primary_button(&tokens, "Open in editor", true))
                .on_disabled_hover_text("Generate a cut with retained material first.");
            app.auto.add_response("roughCut.openInEditor", &open, "Open in editor");
            if open.clicked()
                && let Some((id, _)) = map
            {
                workspace.open_in_editor(app, id, None);
            }
            ui.menu_button("More", |ui| {
                if ui.button("Back to editing workspace").clicked() {
                    workspace.stop(app.audio.as_deref_mut());
                    app.stop_source();
                    app.set_workspace("Editing");
                    ui.close();
                }
                if let (Some(previous), Some((id, _))) = (previous, map)
                    && ui.add_enabled(can_open, egui::Button::new("Replace previous sequence")).clicked()
                {
                    workspace.open_in_editor(app, id, Some(previous));
                    ui.close();
                }
            })
            .response
            .on_hover_text("Workspace actions");
            ui.separator();
            for (icon, label, command, enabled) in
                [(Icon::Redo, "Redo", "edit.redo", app.session.history.can_redo()), (Icon::Undo, "Undo", "edit.undo", app.session.history.can_undo())]
            {
                let response =
                    ui.add_enabled_ui(enabled && app.session.preedit_apply.is_none(), |ui| icons::button(ui, icon, 24.0, false, &tokens, label)).inner;
                if response.clicked() {
                    workspace.stop(app.audio.as_deref_mut());
                    app.stop_source();
                    workspace.command(app, command, json!({}));
                }
            }
        });
    };
    let transport_h = (bounds.height() * 0.24).clamp(170.0, 220.0);
    let body = Rect::from_min_max(bounds.min, pos2(bounds.right(), (bounds.bottom() - transport_h - 8.0).max(bounds.top() + 80.0)));
    let split = body.left() + (body.width() * 0.38).clamp(280.0_f32.min(body.width() * 0.47), 650.0);
    let left = Rect::from_min_max(body.min, pos2(split - 3.0, body.bottom()));
    let right = Rect::from_min_max(pos2(split + 3.0, body.top()), body.max);
    let bottom = Rect::from_min_max(pos2(bounds.left(), body.bottom() + 6.0), bounds.max);
    app.auto.add("roughCut.scriptPane", left, "Script pane");
    app.auto.add("roughCut.viewer", right, "Video preview");
    app.auto.add("roughCut.transport", bottom, "Review transport");
    for panel in [left, right, bottom] {
        ui.painter().rect(panel, 2.0, tokens.panel_bg, (1.0, tokens.separator), egui::StrokeKind::Inside);
    }
    let mut left_ui = ui.new_child(egui::UiBuilder::new().max_rect(left.shrink(12.0)));
    left_ui.set_clip_rect(left);
    left_ui.horizontal(|ui| {
        let has_analysis = analysis.is_some();
        for (tab, id, label) in [(ReviewTab::Script, "script", "Transcript"), (ReviewTab::Cuts, "cuts", "Review cuts"), (ReviewTab::Files, "files", "Setup")] {
            let selected = if has_analysis { workspace.tab == tab } else { tab == ReviewTab::Files };
            let response = ui.add_enabled(has_analysis || tab == ReviewTab::Files, egui::Button::selectable(selected, label));
            if response.clicked() {
                workspace.tab = tab;
            }
            app.auto.add_response(&format!("roughCut.tab.{id}"), &response, label);
        }
    });
    left_ui.separator();
    let mut viewer = ui.new_child(egui::UiBuilder::new().max_rect(right.shrink2(vec2(10.0, 8.0))));
    viewer.set_clip_rect(right);
    let mut transport = ui.new_child(egui::UiBuilder::new().max_rect(bottom.shrink2(vec2(10.0, 8.0))));
    transport.set_clip_rect(bottom);
    // Status consumes its actual height before the timeline takes the remaining space.
    if app.session.preedit_apply.is_some() {
        transport.horizontal(|ui| {
            ui.spinner();
            ui.label("Verifying original before applying…");
            if ui.button("Cancel apply").clicked() {
                workspace.command(app, "preedit.cancelApply", json!({}));
            }
        });
    }
    if !workspace.message.is_empty() {
        transport.label(&workspace.message);
    }

    if let Some((id, map)) = map {
        if workspace.map != Some(id) {
            workspace.timeline.trim = None;
        }
        let settings = &analysis.unwrap().settings;
        let milliseconds = |tick: Tick| (tick.0 / Tick::from_units(1, 1000).0).max(0) as u32;
        workspace.sync_cleanup(map.cleanup, (milliseconds(settings.min_pause), milliseconds(settings.handle)));
        workspace.map = Some(id);
        let analysis_mode = (map.analysis, map.automatic.is_some());
        if workspace.seen_analysis != Some(analysis_mode) {
            workspace.assemblyai = analysis.unwrap().settings.model.as_deref() == Some("assemblyai");
            if let Some(model) = analysis.unwrap().settings.model.as_ref().filter(|m| m.as_str() != "assemblyai") {
                workspace.local_model = model.clone();
            }
            workspace.cleanup_initialized = true;
            workspace.seen_analysis = Some(analysis_mode);
            workspace.original = duration == Tick::ZERO || map.automatic.is_none();
            workspace.cursor = analysis.unwrap().source.range.start;
            workspace.mark_in = None;
            workspace.mark_out = None;
            workspace.timeline = TimelineSelection::default();
            workspace.text_selection = None;
            workspace.text_anchor = None;
            workspace.text_dragging = false;
            workspace.correction = None;
            workspace.window_start = workspace.cursor;
            workspace.stop(app.audio.as_deref_mut());
            app.stop_source();
        }
    } else {
        workspace.original = true;
        workspace.seen_analysis = None;
    }
    if duration == Tick::ZERO && !workspace.original {
        workspace.original = true;
        workspace.stop(app.audio.as_deref_mut());
    }

    if let (ReviewTab::Script, Some((id, map)), Some(analysis)) = (&workspace.tab, map, analysis) {
        if map.automatic.is_none() && analysis.status == AnalysisStatus::Complete && left_ui.button("Generate smart cut from transcript").clicked() {
            workspace.review(app, id, json!({"kind":"autoCut"}));
            if workspace.map != Some(id) {
                workspace.original = false;
            }
        }
        if left_ui.small_button("Cleanup settings…").clicked() {
            workspace.tab = ReviewTab::Files;
        }
        workspace.script(app, &mut left_ui, id, map, analysis, &segments);
    } else if let (ReviewTab::Cuts, Some((id, map)), Some(analysis)) = (&workspace.tab, map, analysis) {
        workspace.cut_review(app, &mut left_ui, id, map, analysis, &segments);
    } else {
        let primary_above_settings = workspace.source.is_some() && app.session.preedit_job.is_none();
        if primary_above_settings {
            let saved = workspace.saved_analysis_map(app);
            let (id, label) = if saved.is_some() {
                ("roughCut.cleanUpSaved", "Update cut")
            } else if workspace.silence_only {
                ("roughCut.transcribeAndCut", "Analyze & cut pauses")
            } else {
                ("roughCut.transcribeAndCut", "Transcribe & cut")
            };
            let response = left_ui.add(primary_button(&tokens, label, true));
            app.auto.add_response(id, &response, label);
            if response.clicked() {
                if let Some(id) = saved {
                    workspace.update_cut(app, id);
                } else {
                    workspace.start(app, None, true);
                }
            }
            left_ui.add_space(5.0);
        }
        egui::ScrollArea::vertical().id_salt("rough-cut-files").show(&mut left_ui, |ui| {
            workspace.inputs(app, ui, primary_above_settings);
        });
    }

    if let Some(item) = workspace.source
        && let Some(media) = project.item(item).and_then(|item| item.as_media())
    {
        let source_range = analysis.filter(|a| a.source.item == item).map_or_else(|| TimeRange::from_bounds(Tick::ZERO, media.duration()), |a| a.source.range);
        let stream = analysis.filter(|a| a.source.item == item).map_or(workspace.stream, |a| a.source.audio_stream);
        if source_range.duration > Tick::ZERO {
            let changed = map.is_some_and(|(id, _)| workspace.map != Some(id));
            let inline_actions = viewer.available_width() >= 620.0;
            viewer.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("Preview").strong());
                ui.separator();
                let original = ui.selectable_label(workspace.original, "Original");
                let result = ui.add_enabled(duration > Tick::ZERO, egui::Button::selectable(!workspace.original, "Result"));
                app.auto.add_response("roughCut.preview.original", &original, "Original preview");
                app.auto.add_response("roughCut.preview.result", &result, "Result preview");
                if (original.clicked() && !workspace.original) || (result.clicked() && workspace.original) {
                    workspace.stop(app.audio.as_deref_mut());
                    app.stop_source();
                    workspace.original = original.clicked();
                    workspace.seek(app, item, &segments, workspace.cursor);
                }
                if map.is_some() {
                    let cuts = cut_count(&segments, source_range);
                    ui.label(egui::RichText::new(format!("{cuts} cuts · {} saved", time_label(source_range.duration - duration))).color(tokens.text_dim));
                }
                if app.session.preedit_job.is_some() {
                    ui.spinner();
                    let status = project
                        .preedit
                        .analyses
                        .values()
                        .rev()
                        .find(|a| a.source.item == item && a.status == AnalysisStatus::Running)
                        .map(|a| {
                            let covered = a.chunks.last().map_or(a.source.range.start, |chunk| chunk.coverage.end()) - a.source.range.start;
                            let phase = if covered >= a.source.range.duration {
                                "Finishing cut"
                            } else if a.settings.model.is_some() {
                                "Transcribing"
                            } else {
                                "Analyzing audio"
                            };
                            format!("{phase} · {} / {}", time_label(covered), time_label(a.source.range.duration))
                        })
                        .unwrap_or_else(|| "Preparing analysis…".into());
                    let status = app
                        .session
                        .preedit_job
                        .as_ref()
                        .and_then(|j| j.progress.as_ref())
                        .map(|p| format!("{} · {} / {}", p.phase, time_label(p.processed), time_label(p.total)))
                        .unwrap_or(status);
                    ui.label(status);
                    let cancel = ui.small_button("Cancel");
                    app.auto.add_response("roughCut.cancelAnalysis", &cancel, "Cancel analysis");
                    if cancel.clicked() {
                        workspace.command(app, "preedit.cancel", json!({}));
                    }
                }
                if inline_actions {
                    preview_actions(ui, app, &mut workspace);
                }
            });
            if !inline_actions {
                viewer.horizontal(|ui| preview_actions(ui, app, &mut workspace));
            }
            viewer.separator();
            let mut picture = viewer.available_rect_before_wrap();
            picture.max.y = (picture.max.y - 36.0).max(picture.min.y);
            viewer.scope_builder(egui::UiBuilder::new().max_rect(picture), |ui| {
                ui.painter().rect_filled(picture, 1.0, tokens.field_bg);
                if workspace.original {
                    if app.session.state.source_item != Some(item) {
                        workspace.seek_source(app, item, workspace.cursor.clamp(source_range.start, source_range.end()));
                    }
                    workspace.cursor = app.session.state.source_playhead.clamp(source_range.start, source_range.end());
                    workspace.original_view(app, ui, item, source_range);
                } else if !changed && let Some((id, _)) = map {
                    workspace.result_surface(app, ui, id, duration, &segments);
                }
            });
            viewer.add_space(4.0);

            let playing = if workspace.original { app.source_playback.clock.playing } else { workspace.is_playing() };
            let mut toggle = std::mem::take(&mut workspace.toggle_requested);
            viewer.horizontal_wrapped(|ui| {
                let frame = media.info.video.as_ref().map_or(Tick::from_units(1, 30), |video| video.frame_rate.frame_duration());
                let play_icon = if playing { Icon::Pause } else { Icon::Play };
                let play_label = if playing { "Pause" } else { "Play" };
                for (icon, id, label, step) in [
                    (Icon::StepBack, "roughCut.stepBack", "Previous frame", -frame),
                    (play_icon, "roughCut.play", play_label, Tick::ZERO),
                    (Icon::StepFwd, "roughCut.stepForward", "Next frame", frame),
                ] {
                    let tooltip = if step == Tick::ZERO { "Play / pause (Space)" } else { label };
                    let response = icons::button(ui, icon, 28.0, false, &app.tokens, tooltip);
                    app.auto.add_response(id, &response, label);
                    if response.clicked() {
                        if step == Tick::ZERO {
                            toggle = true;
                        } else if workspace.original {
                            workspace.seek(app, item, &segments, (workspace.cursor + step).clamp(source_range.start, source_range.end()));
                        } else {
                            workspace.stop(app.audio.as_deref_mut());
                            if let Some(view) = &mut workspace.result {
                                view.time = (view.time + step).clamp(Tick::ZERO, duration);
                            }
                        }
                    }
                }
                let elapsed =
                    if workspace.original { workspace.cursor - source_range.start } else { workspace.result.as_ref().map_or(Tick::ZERO, |view| view.time) };
                ui.monospace(format!("{} / {}", time_label(elapsed), time_label(if workspace.original { source_range.duration } else { duration })));
            });
            transport.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("Source timeline").strong());
                ui.separator();
                let cuts = ui.checkbox(&mut workspace.show_cuts, "Show cuts");
                app.auto.add_response("roughCut.showCuts", &cuts, "Show cuts");
                let mut skip = !workspace.original;
                let response = ui
                    .add_enabled(duration > Tick::ZERO, egui::Checkbox::new(&mut skip, "Skip cuts"))
                    .on_hover_text("Play the edited result, skipping removed source ranges");
                app.auto.add_response("roughCut.skipCuts", &response, "Skip cuts");
                if response.changed() {
                    workspace.stop(app.audio.as_deref_mut());
                    app.stop_source();
                    workspace.original = !skip;
                    workspace.seek(app, item, &segments, workspace.cursor);
                    // Resume once the newly selected preview has been prepared.
                    workspace.toggle_requested = playing;
                    ui.ctx().request_repaint();
                }
            });
            if toggle && !changed {
                workspace.toggle(app, &viewer, item, stream, source_range, duration);
            }
            if let Some((id, map)) = map
                && !changed
            {
                let selected =
                    workspace.mark_in.zip(workspace.mark_out).filter(|(start, end)| start != end).map(|(a, b)| TimeRange::from_bounds(a.min(b), a.max(b)));
                let cut = cut_at(&segments, source_range, workspace.cursor);
                let mut action = None;
                transport.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Marked range").color(tokens.text_dim));
                    if ui.button("Set in").clicked() {
                        workspace.mark_in = Some(workspace.cursor);
                    }
                    if ui.button("Set out").clicked() {
                        workspace.mark_out = Some(workspace.cursor);
                    }
                    let selected =
                        workspace.mark_in.zip(workspace.mark_out).filter(|(start, end)| start != end).map(|(a, b)| TimeRange::from_bounds(a.min(b), a.max(b)));
                    if let Some(range) = selected {
                        ui.monospace(format!("{}–{}", time_label(range.start), time_label(range.end())));
                    }
                    if ui.add_enabled(selected.is_some(), egui::Button::new("Cut range")).clicked() {
                        action = selected.map(|range| range_action(range, "Remove"));
                    }
                    let restore = selected.or(cut);
                    if ui.add_enabled(restore.is_some(), egui::Button::new("Restore range")).clicked() {
                        action = restore.map(|range| range_action(range, "Keep"));
                    }
                    if ui.small_button("Clear range").clicked() {
                        workspace.mark_in = None;
                        workspace.mark_out = None;
                    }
                    if !map.conflicts.is_empty()
                        && ui
                            .button("Discard edits from previous source")
                            .on_hover_text("The source binding changed. Clear the old edits to open this cut in the editor.")
                            .clicked()
                    {
                        action = Some(json!({"kind":"clearConflicts"}));
                    }
                });
                if let Some(action) = action {
                    workspace.review(app, id, action);
                }
                if let Some(analysis) = analysis {
                    let span = Tick((source_range.duration.0 as f64 / f64::from(workspace.zoom.max(1.0))) as i64).max(Tick(1));
                    let start = workspace.window_start.clamp(source_range.start, source_range.end() - span);
                    let follow = (playing || std::mem::take(&mut workspace.follow_cursor)) && workspace.zoom_drag.is_none();
                    workspace.window_start = if follow && (workspace.cursor < start || workspace.cursor > start + span) {
                        (workspace.cursor - Tick(span.0 / 2)).clamp(source_range.start, source_range.end() - span)
                    } else {
                        start
                    };
                    let window = TimeRange::from_bounds(workspace.window_start, workspace.window_start + span);
                    let area = transport.available_rect_before_wrap();
                    let wave_area = Rect::from_min_max(area.min, pos2(area.right(), (area.bottom() - 18.0).max(area.top())));
                    let mut wave_ui = transport.new_child(egui::UiBuilder::new().max_rect(wave_area));
                    wave_ui.set_clip_rect(wave_area);
                    let (seek, edit) = waveform(
                        &mut wave_ui,
                        app,
                        map.analysis,
                        analysis,
                        &segments,
                        window,
                        workspace.cursor,
                        selected,
                        workspace.show_cuts,
                        &mut workspace.timeline,
                    );
                    if workspace.timeline.trim.is_some() {
                        workspace.stop(app.audio.as_deref_mut());
                        app.stop_source();
                    }
                    if let Some(time) = seek {
                        workspace.seek(app, item, &segments, time);
                    }
                    if let Some(edit) = edit {
                        workspace.review(app, id, edit);
                    }
                    let bar = Rect::from_min_max(pos2(area.left() + 72.0_f32.min(area.width() * 0.2), area.bottom() - 12.0), area.max);
                    zoom_bar(&mut transport, app, bar, source_range, &mut workspace);
                }
            } else if map.is_none() {
                let elapsed = ((workspace.cursor - source_range.start).0 as f64 / TICKS_PER_SECOND as f64).max(0.0);
                let mut seek = elapsed;
                if transport.add(egui::Slider::new(&mut seek, 0.0..=source_range.duration.0 as f64 / TICKS_PER_SECOND as f64).text("Source position")).changed()
                {
                    workspace.seek(app, item, &[], source_range.start + Tick::from_seconds_f64(seek));
                }
            }
        }
    } else {
        preview_actions(&mut viewer, app, &mut workspace);
        viewer.centered_and_justified(|ui| {
            ui.label("Import media to preview it here.");
        });
    }
    app.rough_cut = workspace;
}

fn review_reason(reason: &str) -> String {
    let detail = reason.strip_prefix("Context-aware v").and_then(|s| s.split_once(": ")).map_or(reason, |(_, detail)| detail);
    match detail {
        "ordered-prefix-restart"
        | "short-restart-continuation"
        | "screened-restart"
        | "screened-quiet-restart"
        | "completed-restart-chain"
        | "indexed-continuation" => {
            return "Incomplete repeated attempt".into();
        }
        "ordered-equivalent-take" | "aligned-retake" => return "Repeated statement".into(),
        "isolated-retry-cue" => return "Retry cue between attempts".into(),
        "filler-lexicon" => return "Recognized hesitation".into(),
        "Retained alternative (context-aware v1)" | "Retained alternative (context-aware)" => return "Selected alternative".into(),
        _ => {}
    }
    // Old saved analyses retain their original metadata; keep visible copy product-neutral.
    reason.replace(" (Gling-compatible)", "").replace("Gling-compatible", "Transcript").replace("OpenGling", "Smart Cut")
}

#[cfg(test)]
mod cleanup_form_tests {
    use super::*;

    fn workspace() -> Workspace {
        let mut w = Workspace { pause_ms: 500, handle_ms: 150, ..Default::default() };
        w.sync_cleanup(Some(CleanupSettings { mode: CleanupMode::GlingCompatible, ..Default::default() }), (500, 150));
        w.cleanup_initialized = true;
        w
    }

    #[test]
    fn smart_cut_cleanup_form_follows_saved_updates_and_undo() {
        let mut w = workspace();
        let old = w.cleanup;
        let saved = CleanupSettings { take_matching: TakeMatching::OrderedV1, preserve_pauses_ms: Some(800), ..old };
        w.sync_cleanup(Some(saved), (500, 150));
        assert_eq!(w.cleanup, saved);
        w.sync_cleanup(Some(old), (500, 150));
        assert_eq!(w.cleanup, old);
    }

    #[test]
    fn smart_cut_cleanup_form_preserves_dirty_options_and_acknowledges_update() {
        let mut w = workspace();
        let old = w.cleanup;
        w.cleanup.take_matching = TakeMatching::OrderedV1;
        w.pause_ms = 900;
        w.sync_cleanup(Some(old), (500, 150));
        assert_eq!(w.cleanup.take_matching, TakeMatching::OrderedV1);
        assert_eq!(w.pause_ms, 900);
        let updated = w.cleanup_options(true);
        w.sync_cleanup(Some(updated), (500, 150));
        w.sync_cleanup(Some(old), (500, 150));
        assert_eq!(w.cleanup, old);
        assert_eq!(w.pause_ms, 500);
    }

    #[test]
    fn smart_cut_cleanup_form_does_not_undo_pending_pacing_changes() {
        let mut w = workspace();
        let old = w.cleanup;
        w.handle_ms = 400;
        w.sync_cleanup(Some(old), (500, 150));
        assert_eq!(w.handle_ms, 400);
    }
}

struct ZoomDrag {
    pointer: f32,
    start: f64,
    end: f64,
    per_point: f64,
    mode: u8,
}

fn zoom_bar(ui: &mut Ui, app: &mut ShotClutchApp, bar: Rect, source: TimeRange, workspace: &mut Workspace) {
    let total = source.duration.seconds();
    if bar.width() <= 0.0 || total <= 0.0 {
        return;
    }
    let visible = (total / f64::from(workspace.zoom.max(1.0))).min(total);
    let mut start = (workspace.window_start - source.start).seconds().clamp(0.0, (total - visible).max(0.0));
    let width = (bar.width() * (visible / total) as f32).clamp(34.0_f32.min(bar.width()), bar.width());
    let travel = (bar.width() - width).max(0.0);
    let range = (total - visible).max(0.0);
    let per_point = if travel > 0.5 { range / f64::from(travel) } else { 0.0 };
    let left = bar.left() + if range > 0.0 { (start / range) as f32 * travel } else { 0.0 };
    let thumb = Rect::from_min_max(pos2(left, bar.top()), pos2(left + width, bar.bottom()));
    let t = app.tokens;
    ui.painter().rect_filled(bar, bar.height() / 2.0, t.separator);
    ui.painter().rect_filled(thumb, thumb.height() / 2.0, if ui.rect_contains_pointer(thumb) { t.scroll_thumb_hover } else { t.scroll_thumb });
    for (name, x) in [("start", thumb.left() + thumb.height() / 2.0), ("end", thumb.right() - thumb.height() / 2.0)] {
        ui.painter().circle_filled(pos2(x, thumb.center().y), 4.5, t.panel_bg);
        ui.painter().circle_stroke(pos2(x, thumb.center().y), 4.5, (1.5, t.icon_active));
        app.auto.add(&format!("roughCut.zoomBar.{name}"), Rect::from_center_size(pos2(x, thumb.center().y), vec2(12.0, bar.height())), "Drag to zoom");
    }
    app.auto.add("roughCut.zoomBar", thumb, "Drag to scroll; drag either end to zoom");
    app.auto.add("roughCut.zoomBar.track", bar, "Timeline zoom bar");
    let response = ui.interact(bar, ui.make_persistent_id("rough-cut-zoom-bar"), egui::Sense::click_and_drag());
    let jump = |x: f32| ((f64::from(x - bar.left() - width / 2.0)) * per_point).clamp(0.0, range);
    let mut next = None;
    if response.double_clicked() {
        next = Some((0.0, total));
        workspace.zoom_drag = None;
    } else if response.clicked()
        && let Some(pointer) = response.interact_pointer_pos().filter(|p| !thumb.contains(*p))
    {
        next = Some((jump(pointer.x), visible));
    }
    if response.drag_started()
        && let Some(pointer) = ui.input(|i| i.pointer.press_origin()).or(response.interact_pointer_pos())
    {
        let mode = if !thumb.expand2(vec2(0.0, 4.0)).contains(pointer) {
            start = jump(pointer.x);
            0
        } else if pointer.x - thumb.left() < 12.0 {
            1
        } else if thumb.right() - pointer.x < 12.0 {
            2
        } else {
            0
        };
        workspace.zoom_drag = Some(ZoomDrag { pointer: pointer.x, start, end: start + visible, per_point, mode });
    }
    if let Some(drag) = &workspace.zoom_drag {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            next = Some((drag.start, drag.end - drag.start));
            workspace.zoom_drag = None;
        } else if response.dragged()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let dx = f64::from(pointer.x - drag.pointer);
            let minimum = (f64::from(bar.width()) / 24_000.0).max(0.05).min(total).min((drag.end - drag.start).max(0.000001));
            next = Some(match drag.mode {
                1 => {
                    let left = (drag.start + dx / f64::from(bar.width()) * total).clamp(0.0, drag.end - minimum);
                    (left, drag.end - left)
                }
                2 => {
                    let end = (drag.end + dx / f64::from(bar.width()) * total).clamp(drag.start + minimum, total);
                    (drag.start, end - drag.start)
                }
                _ => ((drag.start + dx * drag.per_point).clamp(0.0, (total - visible).max(0.0)), visible),
            });
        }
    }
    if let Some((start, span)) = next {
        workspace.window_start = source.start + Tick::from_seconds_f64(start);
        workspace.zoom = (total / span.max(0.000001)) as f32;
        workspace.follow_cursor = false;
        ui.ctx().request_repaint();
    }
    if response.drag_stopped() || !ui.input(|i| i.pointer.primary_down()) {
        workspace.zoom_drag = None;
    }
}

fn section_title(ui: &mut Ui, tokens: &crate::theme::Tokens, number: &str, label: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(number).monospace().color(tokens.text_dim));
        ui.label(egui::RichText::new(label).size(14.0).strong());
    });
}

fn cleanup_option(ui: &mut Ui, tokens: &crate::theme::Tokens, enabled: &mut bool, label: &str, description: &str) {
    ui.checkbox(enabled, label);
    ui.horizontal(|ui| {
        ui.add_space(ui.spacing().icon_width + ui.spacing().icon_spacing);
        ui.label(egui::RichText::new(description).small().color(tokens.text_dim));
    });
    ui.add_space(3.0);
}

fn primary_button<'a>(tokens: &crate::theme::Tokens, label: &'a str, primary: bool) -> egui::Button<'a> {
    if primary {
        egui::Button::new(egui::RichText::new(label).strong().color(egui::Color32::WHITE)).fill(tokens.accent).min_size(vec2(0.0, 26.0))
    } else {
        egui::Button::new(label)
    }
}

fn review_frame_grace(rate: shotclutch_time::FrameRate) -> i64 {
    rate.frame_at(Tick::from_units(1, 2)).max(1)
}

fn paint_picture(ui: &mut Ui, texture: egui::TextureId, picture: Rect) {
    ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
    ui.painter().image(texture, picture, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
}

fn update_picture(app: &mut ShotClutchApp, ui: &Ui, name: &str, key: FrameKey, late_frames: i64) {
    let shown = app.texture_key(name);
    for offset in 0..=late_frames.min(key.frame.max(0)) {
        let candidate = FrameKey { frame: key.frame - offset, ..key };
        // Late pictures can move the preview forward, but should never roll it back.
        if offset > 0 && shown.is_some_and(|old| old == FrameKey { frame: old.frame, ..key } && old.frame >= candidate.frame) {
            break;
        }
        if let Some(img) = app.frames.get(&candidate) {
            app.texture_for(ui.ctx(), name, candidate, &img);
            break;
        }
    }
}

fn result_time(segments: &[Segment], source: Tick) -> Tick {
    segments
        .iter()
        .find(|segment| segment.source.end() > source)
        .map(|segment| segment.output.start + (source - segment.source.start).max(Tick::ZERO))
        .or_else(|| segments.last().map(|segment| segment.output.end()))
        .unwrap_or(Tick::ZERO)
}

fn retained_at(segments: &[Segment], time: Tick) -> bool {
    let index = segments.partition_point(|segment| segment.source.end() <= time);
    segments.get(index).is_some_and(|segment| segment.source.contains(time))
}

fn cut_at(segments: &[Segment], source: TimeRange, time: Tick) -> Option<TimeRange> {
    let index = segments.partition_point(|segment| segment.source.end() <= time);
    if segments.get(index).is_some_and(|segment| segment.source.contains(time)) || !source.contains(time) {
        return None;
    }
    let start = index.checked_sub(1).map_or(source.start, |i| segments[i].source.end());
    let end = segments.get(index).map_or(source.end(), |segment| segment.source.start);
    (end > start).then(|| TimeRange::from_bounds(start, end))
}

fn cut_count(segments: &[Segment], source: TimeRange) -> usize {
    let Some(first) = segments.first() else { return 1 };
    segments.len() - 1 + usize::from(first.source.start > source.start) + usize::from(segments.last().is_some_and(|last| last.source.end() < source.end()))
}

fn range_action(range: TimeRange, decision: &str) -> Value {
    json!({"kind":"range","range":range_json(range),"decision":decision})
}

fn range_json(range: TimeRange) -> Value {
    json!({"start":range.start.0.to_string(),"end":range.end().0.to_string()})
}

fn time_label(time: Tick) -> String {
    let seconds = (time.0 as f64 / TICKS_PER_SECOND as f64).max(0.0);
    format!("{:02}:{:05.2}", (seconds / 60.0) as u64, seconds % 60.0)
}

fn waveform(
    ui: &mut Ui,
    app: &mut ShotClutchApp,
    analysis_id: u64,
    analysis: &PreEditAnalysis,
    segments: &[Segment],
    window: TimeRange,
    playhead: Tick,
    selected: Option<TimeRange>,
    show_cuts: bool,
    selection: &mut TimelineSelection,
) -> (Option<Tick>, Option<Value>) {
    let (strip, _) = ui.allocate_exact_size(ui.available_size_before_wrap().max(vec2(0.0, 0.0)), egui::Sense::hover());
    let tokens = app.tokens;
    let label_width = 72.0_f32.min(strip.width() * 0.2);
    let rect = Rect::from_min_max(pos2(strip.left() + label_width, strip.top()), strip.max);
    let response = ui.interact(rect, ui.make_persistent_id("rough-cut-waveform"), egui::Sense::click_and_drag());
    app.auto.add_response("roughCut.waveform", &response, "Source waveform");
    let painter = ui.painter().with_clip_rect(strip);
    painter.rect_filled(strip, 2.0, tokens.field_bg);
    painter.line_segment([rect.left_top(), rect.left_bottom()], (1.0, tokens.separator));
    let painter = painter.with_clip_rect(rect);
    let x = |t: Tick| rect.left() + rect.width() * (t - window.start).0 as f32 / window.duration.0 as f32;
    let time_at =
        |pointer: egui::Pos2| window.start + Tick((window.duration.0 as f64 * f64::from(((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0))) as i64);
    let lane_top = rect.top() + 25.0_f32.min(rect.height() * 0.35);
    let lane_height = (rect.bottom() - lane_top).max(0.0);
    let cuts_height = (lane_height * 0.28).clamp(12.0, 36.0).min(lane_height);
    let audio_top = (lane_top + cuts_height + 3.0).min(rect.bottom());
    let cuts_lane = Rect::from_min_max(pos2(rect.left(), lane_top), pos2(rect.right(), lane_top + cuts_height));
    let audio_lane = Rect::from_min_max(pos2(rect.left(), audio_top), rect.right_bottom());
    for (label, y) in [("Cuts", cuts_lane.center().y), ("Audio", audio_lane.center().y)] {
        ui.painter().with_clip_rect(strip).text(
            pos2(strip.left() + 8.0, y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(12.0),
            tokens.text_dim,
        );
    }
    // The gaps between retained segments are removed time, including gaps with no audio peaks.
    painter.rect_filled(cuts_lane, 1.0, if show_cuts { tokens.control_border } else { tokens.control_well });
    if show_cuts {
        painter.rect_filled(audio_lane, 0.0, tokens.field_border);
    }
    for segment in segments.iter().filter(|segment| segment.source.start < window.end() && segment.source.end() > window.start) {
        let left = x(segment.source.start.max(window.start));
        let right = x(segment.source.end().min(window.end()));
        let kept_cuts = Rect::from_min_max(pos2(left, lane_top), pos2(right, cuts_lane.bottom()));
        painter.rect_filled(kept_cuts, 0.0, tokens.accent.gamma_multiply(0.8));
        if show_cuts {
            let kept_audio = Rect::from_min_max(pos2(left, audio_top), pos2(right, rect.bottom()));
            painter.rect_filled(kept_audio, 0.0, tokens.field_bg);
        }
    }
    if let Some(range) = selected {
        let highlight = Rect::from_min_max(pos2(x(range.start), lane_top), pos2(x(range.end()), rect.bottom()));
        painter.rect_filled(highlight, 0.0, tokens.accent.gamma_multiply(0.24));
    }
    painter.line_segment([pos2(rect.left(), audio_top), pos2(rect.right(), audio_top)], (1.0, tokens.separator));
    let steps = (rect.width() / 115.0).clamp(2.0, 12.0) as i64;
    for step in 0..=steps {
        let time = window.start + Tick(window.duration.0 * step / steps);
        let major = x(time);
        for minor in 0..if step == steps { 1 } else { 5 } {
            let px = major + rect.width() * minor as f32 / (steps * 5) as f32;
            painter.line_segment([pos2(px, rect.top() + if minor == 0 { 16.0 } else { 20.0 }), pos2(px, lane_top)], (1.0, tokens.separator));
        }
        painter.text(
            pos2(major, rect.top() + 3.0),
            if step == steps { egui::Align2::RIGHT_TOP } else { egui::Align2::LEFT_TOP },
            time_label(time),
            egui::FontId::monospace(11.0),
            tokens.text_dim,
        );
    }
    let width = rect.width().clamp(1.0, 2048.0) as usize;
    if selection
        .peaks
        .as_ref()
        .is_none_or(|peaks| peaks.analysis != analysis_id || peaks.chunks != analysis.chunks.len() || peaks.window != window || peaks.bins.len() != width)
    {
        let mut bins = vec![0u8; width];
        for chunk in analysis.chunks.iter().filter(|chunk| chunk.coverage.start < window.end() && chunk.coverage.end() > window.start) {
            for (i, peak) in chunk.peaks.iter().enumerate() {
                let tick = chunk.coverage.start + Tick(chunk.coverage.duration.0 * i as i64 / chunk.peaks.len() as i64);
                if window.contains(tick) {
                    let column = ((tick - window.start).0 as f64 / window.duration.0 as f64 * width as f64) as usize;
                    if let Some(bin) = bins.get_mut(column) {
                        *bin = (*bin).max(*peak);
                    }
                }
            }
        }
        selection.peaks = Some(WaveformPeaks { analysis: analysis_id, chunks: analysis.chunks.len(), window, bins });
    }
    let mid = (audio_top + rect.bottom()) / 2.0;
    for (i, peak) in selection.peaks.iter().flat_map(|peaks| peaks.bins.iter().copied()).enumerate().filter(|(_, peak)| *peak != 0) {
        let tick = window.start + Tick(window.duration.0 * i as i64 / width as i64);
        let retained = retained_at(segments, tick);
        if !show_cuts && !retained {
            continue;
        }
        let px = rect.left() + i as f32 * rect.width() / width as f32;
        let y = ((rect.bottom() - audio_top) / 2.0 - 2.0) * (f32::from(peak) / 255.0).sqrt();
        painter.line_segment([pos2(px, mid - y), pos2(px, mid + y)], (1.0, if retained { tokens.accent_hover } else { tokens.text_dim }));
    }
    let px = x(playhead);
    painter.line_segment([pos2(px, rect.top()), pos2(px, rect.bottom())], (2.0, tokens.accent_hover));
    let source = analysis.source.range;
    let region_at = |time| {
        cut_at(segments, source, time).or_else(|| {
            let index = segments.partition_point(|s| s.source.end() <= time);
            segments.get(index).filter(|s| s.source.contains(time)).map(|s| s.source)
        })
    };
    if let Some(time) = selection.select_at.take() {
        selection.range = region_at(time);
    }
    // A quick press and release can arrive in the same frame.
    let completed_drag = ui
        .input(|input| {
            let mut start = None;
            for event in &input.events {
                if let egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, .. } = event {
                    if *pressed {
                        start = Some(*pos);
                    } else if let Some(start) = start
                        && (pos.x - start.x).abs() >= 6.0
                    {
                        return Some(start);
                    }
                }
            }
            None
        })
        .filter(|pos| ui.ctx().layer_id_at(*pos) == Some(ui.layer_id()));
    if completed_drag.is_none()
        && response.clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        selection.range = if pointer.y >= lane_top { region_at(time_at(pointer)) } else { None };
    }
    if let Some(range) = selection.range.filter(|r| region_at(r.start) == Some(*r)) {
        let removed = cut_at(segments, source, range.start).is_some();
        let highlight = Rect::from_min_max(pos2(x(range.start), lane_top), pos2(x(range.end()), rect.bottom()));
        painter.rect_stroke(highlight, 1.0, (1.5, tokens.accent_hover), egui::StrokeKind::Inside);
        let edges = [(true, range.start), (false, range.end())];
        let edges = edges.into_iter().filter(|(_, edge)| (!removed || show_cuts) && *edge >= window.start && *edge <= window.end());
        for (left, edge) in edges.clone() {
            let handle = Rect::from_center_size(pos2(x(edge), cuts_lane.center().y), vec2(14.0, cuts_lane.height()));
            let visible = handle.intersect(rect);
            app.auto.add(if left { "roughCut.trim.start" } else { "roughCut.trim.end" }, visible, "Drag region edge");
            painter.rect_filled(Rect::from_center_size(handle.center(), vec2(5.0, 13.0)), 1.0, tokens.accent_hover);
        }
        let find_edge = |pointer: egui::Pos2| {
            if pointer.y < lane_top || !rect.contains(pointer) {
                return None;
            }
            edges
                .clone()
                .filter(|(_, edge)| (x(*edge) - pointer.x).abs() <= 8.0)
                .min_by(|(_, a), (_, b)| (x(*a) - pointer.x).abs().total_cmp(&(x(*b) - pointer.x).abs()))
        };
        if response.hovered() && ui.input(|i| i.pointer.hover_pos()).and_then(find_edge).is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        let origin =
            completed_drag.or_else(|| response.drag_started_by(egui::PointerButton::Primary).then(|| ui.input(|i| i.pointer.press_origin())).flatten());
        if let Some(origin) = origin
            && let Some((left, edge)) = find_edge(origin)
        {
            selection.trim = Some(RegionTrim { range, removed, left, target: edge, pointer_offset: origin.x - x(edge) });
        }
    }
    let trimming = selection.trim.is_some();
    let mut edit = None;
    if let Some(trim) = &mut selection.trim {
        if let Some(pointer) = ui.input(|i| i.pointer.interact_pos()) {
            let target = time_at(pos2(pointer.x - trim.pointer_offset, pointer.y));
            trim.target = if trim.left { target.min(trim.range.end()) } else { target.max(trim.range.start) };
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        let edge = x(trim.target);
        painter.line_segment([pos2(edge, lane_top), pos2(edge, rect.bottom())], (2.0, tokens.accent_hover));
        painter.text(pos2(edge, lane_top - 2.0), egui::Align2::CENTER_BOTTOM, time_label(trim.target), egui::FontId::monospace(12.0), tokens.accent_hover);
        if completed_drag.is_some() || response.drag_stopped_by(egui::PointerButton::Primary) {
            let old = if trim.left { trim.range.start } else { trim.range.end() };
            if trim.target != old {
                let contracts = if trim.left { trim.target > old } else { trim.target < old };
                let keep = contracts == trim.removed;
                let difference = TimeRange::from_bounds(old.min(trim.target), old.max(trim.target));
                edit = Some(range_action(difference, if keep { "Keep" } else { "Remove" }));
                // Resolve the selected range from the next map's snapped boundaries.
                let (start, end) = if trim.left { (trim.target, trim.range.end()) } else { (trim.range.start, trim.target) };
                selection.select_at = (start < end).then(|| start + Tick((end - start).0 / 2));
                selection.range = None;
            }
            selection.trim = None;
        } else if !ui.input(|i| i.pointer.primary_down()) {
            selection.trim = None;
        }
    }
    let seek = (!trimming && (response.clicked() || response.dragged())).then(|| response.interact_pointer_pos().map(time_at)).flatten();
    let anchor = ui.make_persistent_id("rough-cut-waveform-menu");
    if response.secondary_clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        ui.data_mut(|data| data.insert_temp(anchor, time_at(pointer)));
        selection.range = region_at(time_at(pointer));
    }
    response.context_menu(|ui| {
        let Some(time) = ui.data(|data| data.get_temp::<Tick>(anchor)) else { return };
        if let Some(range) = selected.filter(|r| r.contains(time)).or_else(|| region_at(time)) {
            let marked = selected == Some(range);
            let is_cut = cut_at(segments, source, time).is_some();
            for (label, decision, show) in [("Cut", "Remove", marked || !is_cut), ("Uncut", "Keep", marked || is_cut)] {
                if show {
                    let button = ui.button(label);
                    app.auto.add_response(&format!("roughCut.timeline.{}", label.to_lowercase()), &button, label);
                    if button.clicked() {
                        edit = Some(range_action(range, decision));
                        ui.close();
                    }
                }
            }
        }
    });
    (seek, edit)
}
