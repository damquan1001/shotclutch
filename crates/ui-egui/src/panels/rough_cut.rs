//! Native review surface. Only the editor commands can change a rough-cut map.

use std::sync::Arc;

use egui::{Color32, Rect, Ui, pos2, vec2};
use serde_json::{Value, json};
use shotclutch_project::preedit::{AnalysisStatus, Decision, PreEditAnalysis, PreEditMap};
use shotclutch_project::{ItemId, Project};
use shotclutch_time::{TICKS_PER_SECOND, Tick, TimeRange};

use crate::frames::{FrameKey, Target};
use crate::{AudioOut, ShotClutchApp};

#[derive(Default)]
pub struct Workspace {
    source: Option<ItemId>,
    stream: usize,
    channel: Option<u16>,
    silence_only: bool,
    language: String,
    selection_start: String,
    selection_end: String,
    pause_ms: u32,
    handle_ms: u32,
    map: Option<u64>,
    candidate: usize,
    transcript_page: usize,
    filter: Filter,
    range_start: String,
    range_end: String,
    message: String,
    result: Option<ResultView>,
    download: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    confirm_download: bool,
    document: Option<String>,
}

#[derive(Default, PartialEq)]
enum Filter {
    #[default]
    All,
    Unresolved,
    Kept,
    Removed,
}

struct ResultView {
    map: u64,
    revision: u64,
    project: Arc<Project>,
    sequence: ItemId,
    time: Tick,
    playing: Option<(f64, Tick, Option<u32>)>,
}

impl Workspace {
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
        self.result = None;
        let revision = app.session.revision.to_string();
        if let Some(reply) = self.command(app, "preedit.review", json!({"map":map.to_string(),"expectedRevision":revision,"action":action})) {
            self.map = reply["map"].as_str().and_then(|value| value.parse().ok());
        }
    }

    fn audition(&mut self, app: &mut ShotClutchApp, source: ItemId, stream: usize, range: TimeRange) {
        self.stop(app.audio.as_deref_mut());
        if let Err(error) = app.audition_rough_cut_source(source, stream, range.start, range.end()) {
            self.message = error;
        }
    }

    fn start(&mut self, app: &mut ShotClutchApp, resume: Option<u64>) {
        let Some(item) = self.source else { return };
        let model = (!self.silence_only).then_some("whisper-base");
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
        self.command(
            app,
            "preedit.analyze",
            json!({
                "item":item.0.to_string(),"audioStream":self.stream,"channel":self.channel,"model":model,"language":self.language,"range":range,
                "minPauseMs":self.pause_ms,"handleMs":self.handle_ms,
                "previousMap":self.map.map(|id| id.to_string()),"resume":resume.map(|id| id.to_string())
            }),
        );
    }

    fn inputs(&mut self, app: &mut ShotClutchApp, ui: &mut Ui) {
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
        let before = self.source;
        ui.add_enabled_ui(!running, |ui| {
            egui::ComboBox::from_id_salt("rough-cut-source")
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
            self.transcript_page = 0;
            self.stop(app.audio.as_deref_mut());
            self.result = None;
        }
        let Some(item) = self.source else {
            ui.label("Import an audio or video original to get started.");
            return;
        };
        let Some(media) = app.session.project.item(item).and_then(|item| item.as_media()) else { return };
        let duration = media.duration();
        let streams = media.info.audio_streams.clone();
        self.stream = self.stream.min(streams.len().saturating_sub(1));
        ui.add_enabled_ui(!running, |ui| {
            ui.collapsing("Analysis settings", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Source start / end (seconds)");
                    ui.add(egui::TextEdit::singleline(&mut self.selection_start).desired_width(55.0));
                    ui.add(egui::TextEdit::singleline(&mut self.selection_end).desired_width(55.0));
                    ui.label("Blank uses the full source");
                });
                ui.horizontal_wrapped(|ui| {
                    ui.add(egui::DragValue::new(&mut self.pause_ms).range(200..=10_000).prefix("Minimum pause ").suffix(" ms"));
                    ui.add(egui::DragValue::new(&mut self.handle_ms).range(0..=1000).prefix("Retained handles ").suffix(" ms"));
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
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(!running, |ui| {
                egui::ComboBox::from_id_salt("rough-cut-audio").selected_text(format!("Audio stream {}", self.stream + 1)).show_ui(ui, |ui| {
                    for (index, stream) in streams.iter().enumerate() {
                        ui.selectable_value(&mut self.stream, index, format!("{} · {} channels · {} Hz", index + 1, stream.channels, stream.sample_rate));
                    }
                });
                ui.checkbox(&mut self.silence_only, "Silence only");
                if ui.button("Audition stream").clicked() {
                    let start = if app.session.state.source_item == Some(item) { app.session.state.source_playhead } else { Tick::ZERO };
                    let end = (start + Tick(10 * TICKS_PER_SECOND)).min(duration);
                    if end > start {
                        self.audition(app, item, self.stream, TimeRange::from_bounds(start, end));
                    }
                }
            });
            if running {
                if ui.button("Cancel analysis").clicked() {
                    self.command(app, "preedit.cancel", json!({}));
                }
            } else {
                if ui.button(if self.map.is_some() { "Analyze again" } else { "Analyze selected source" }).clicked() {
                    self.start(app, None);
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
                    && ui.button("Resume").clicked()
                {
                    self.channel = record.source.channel;
                    self.language.clone_from(&record.settings.language);
                    self.pause_ms = record.settings.min_pause.to_units_floor(1000) as u32;
                    self.handle_ms = record.settings.handle.to_units_floor(1000) as u32;
                    let range = record.source.range;
                    self.selection_start = (range.start.0 as f64 / TICKS_PER_SECOND as f64).to_string();
                    self.selection_end = (range.end().0 as f64 / TICKS_PER_SECOND as f64).to_string();
                    self.start(app, Some(id));
                }
            }
        });
        if !self.silence_only {
            ui.label(if self.language == "en" {
                "English · Whisper base on this device"
            } else {
                "Whisper base on this device · editorial suggestions qualified for English"
            });
            self.model_install(ui);
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
            ui.add(egui::ProgressBar::new(fraction).text(format!("{state} · {:.1}s covered", covered.0 as f64 / TICKS_PER_SECOND as f64)));
        } else if running {
            ui.label("Verifying the source and opening the selected stream…");
        }
    }

    fn model_install(&mut self, ui: &mut Ui) {
        if let Some(receiver) = &self.download {
            match receiver.try_recv() {
                Ok(result) => {
                    self.message = result.err().unwrap_or_else(|| "Whisper base is installed.".into());
                    self.download = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.download = None;
                    self.message = "The model installer stopped.".into();
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ui.label("Downloading and checking the model…");
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
                    return;
                }
            }
        }
        if ui.button("Install local speech model…").clicked() {
            self.confirm_download = true;
        }
        if self.confirm_download {
            ui.label("Whisper base · 293 MB · OpenAI via Hugging Face · MIT weights, Apache 2.0 conversion");
            ui.horizontal(|ui| {
                if ui.button("Download model").clicked() {
                    let (send, receiver) = std::sync::mpsc::channel();
                    let started = std::thread::Builder::new().name("rough-cut-model".into()).spawn(move || {
                        let result = shotclutch_engine::Session::default()
                            .execute("transcript.downloadModel", json!({"model":"whisper-base"}))
                            .map(|_| ())
                            .map_err(|error| error.to_string());
                        let _ = send.send(result);
                    });
                    if started.is_ok() {
                        self.download = Some(receiver);
                    }
                    self.confirm_download = false;
                }
                if ui.button("Cancel").clicked() {
                    self.confirm_download = false;
                }
            });
        }
    }

    fn review_surface(&mut self, app: &mut ShotClutchApp, ui: &mut Ui, id: u64, map: &PreEditMap, analysis: &PreEditAnalysis) {
        ui.separator();
        let segments = map.segments(analysis.source.range).unwrap_or_default();
        let retained = segments.last().map_or(Tick::ZERO, |segment| segment.output.end());
        let seconds = |time: Tick| time.0 as f64 / TICKS_PER_SECOND as f64;
        ui.label(format!(
            "{} suggestions · {:.2}s retained of {:.2}s · {}",
            map.candidates.len(),
            seconds(retained),
            seconds(analysis.source.range.duration),
            if map.reviewed { "Reviewed" } else { "Review required" }
        ));
        let decision = |index: usize| {
            let range = map.candidates[index].range;
            let midpoint = range.start + Tick(range.duration.0 / 2);
            map.overrides.iter().rev().find(|edit| edit.range.contains(midpoint)).map(|edit| edit.decision)
        };
        let mut action = None;
        let mut audition = None;
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.filter, Filter::All, "All");
            ui.selectable_value(&mut self.filter, Filter::Unresolved, "Unresolved");
            ui.selectable_value(&mut self.filter, Filter::Kept, "Kept");
            ui.selectable_value(&mut self.filter, Filter::Removed, "Removed");
            ui.label("Tab through actions, or use N / K / R / C in the review panel");
        });
        waveform(ui, analysis, map.candidates.get(self.candidate).map(|c| c.range));
        egui::ScrollArea::vertical().id_salt("rough-cut-candidates").max_height(165.0).show(ui, |ui| {
            for (index, candidate) in map.candidates.iter().enumerate().filter(|(index, _)| match self.filter {
                Filter::All => true,
                Filter::Unresolved => decision(*index).is_none(),
                Filter::Kept => decision(*index) == Some(Decision::Keep),
                Filter::Removed => decision(*index) == Some(Decision::Remove),
            }) {
                ui.horizontal_wrapped(|ui| {
                    let state = match decision(index) {
                        None => "Review",
                        Some(Decision::Keep) => "Keep",
                        Some(Decision::Remove) => "Remove",
                    };
                    let label = format!("{state} · {:?} · {:.2}s", candidate.kind, seconds(candidate.range.start));
                    if ui.selectable_label(self.candidate == index, label).on_hover_text(&candidate.reason).clicked() {
                        self.candidate = index;
                        self.range_start = format!("{:.3}", seconds(candidate.range.start));
                        self.range_end = format!("{:.3}", seconds(candidate.range.end()));
                    }
                    if ui.small_button("Listen").clicked() {
                        audition = Some(candidate.range);
                    }
                    if ui.small_button("Keep / restore").clicked() {
                        action = Some(json!({"kind":"candidate","index":index,"decision":"Keep"}));
                    }
                    if ui.small_button("Remove").clicked() {
                        action = Some(json!({"kind":"candidate","index":index,"decision":"Remove"}));
                    }
                });
            }
        });
        if let Some(candidate) = map.candidates.get(self.candidate) {
            ui.label(format!("{} · Confidence: {}", candidate.reason, candidate.confidence.map_or_else(|| "unknown".into(), |c| format!("{:.0}%", c * 100.0))));
            let context = Tick(2 * TICKS_PER_SECOND);
            let start = (candidate.range.start - context).max(analysis.source.range.start);
            let end = (candidate.range.end() + context).min(analysis.source.range.end());
            let text = transcript(analysis, TimeRange::from_bounds(start, end));
            ui.label(if text.is_empty() { "No transcript in this range".into() } else { text });
        }
        if !analysis.chunks.is_empty() {
            self.transcript_page = self.transcript_page.min(analysis.chunks.len() - 1);
            ui.collapsing("Browse transcript", |ui| {
                ui.add(egui::Slider::new(&mut self.transcript_page, 0..=analysis.chunks.len() - 1).text("Analysis chunk"));
                let chunk = &analysis.chunks[self.transcript_page];
                ui.label(format!("{:.2}s–{:.2}s", seconds(chunk.coverage.start), seconds(chunk.coverage.end())));
                if let Some(transcript) = &chunk.transcript {
                    egui::ScrollArea::vertical().id_salt("rough-cut-transcript").max_height(100.0).show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            for word in &transcript.words {
                                if ui.small_button(&word.text).on_hover_text(format!("Listen at {:.2}s", seconds(word.start))).clicked() {
                                    audition = Some(word.range());
                                }
                            }
                        });
                    });
                } else {
                    ui.label("Silence-only analysis has no transcript.");
                }
            });
        }
        for (group, take) in map.takes.iter().enumerate() {
            ui.collapsing(format!("Compare take group {}", group + 1), |ui| {
                ui.label(&take.explanation);
                let choice = ui.make_persistent_id(("take-selection", id, group));
                let mut retained = ui
                    .data(|data| data.get_temp::<Vec<usize>>(choice))
                    .unwrap_or_else(|| take.alternatives.iter().copied().filter(|index| decision(*index) != Some(Decision::Remove)).collect());
                for &index in &take.alternatives {
                    let range = map.candidates[index].range;
                    ui.horizontal_wrapped(|ui| {
                        let mut selected = retained.contains(&index);
                        if ui.checkbox(&mut selected, "Retain").changed() {
                            if selected {
                                retained.push(index);
                            } else {
                                retained.retain(|value| *value != index);
                            }
                        }
                        if ui.button(format!("Listen at {:.2}s", seconds(range.start))).clicked() {
                            audition = Some(range);
                        }
                        if ui.button("Use this take").clicked() {
                            action = Some(json!({"kind":"chooseTake","group":group,"retain":[index]}));
                        }
                        ui.label(transcript(analysis, range));
                    });
                }
                if ui.add_enabled(!retained.is_empty(), egui::Button::new("Keep selected attempts")).clicked() {
                    action = Some(json!({"kind":"chooseTake","group":group,"retain":retained}));
                }
                ui.data_mut(|data| data.insert_temp(choice, retained));
                if ui.button("Keep all attempts").clicked() {
                    action = Some(json!({"kind":"chooseTake","group":group,"retain":take.alternatives}));
                }
                if ui.button("Split group").clicked() {
                    action = Some(json!({"kind":"splitGroup","group":group,"at":take.alternatives.len()/2}));
                }
                if ui.button("Reject grouping").clicked() {
                    action = Some(json!({"kind":"rejectGroup","group":group}));
                }
            });
        }
        for (index, conflict) in map.conflicts.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("Earlier {:?} decision at {:.2}s needs review", conflict.decision, seconds(conflict.range.start)));
                if ui.button("Dismiss conflict").clicked() {
                    action = Some(json!({"kind":"dismissConflict","index":index}));
                }
            });
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("Custom range (seconds)");
            ui.add(egui::TextEdit::singleline(&mut self.range_start).desired_width(60.0));
            ui.add(egui::TextEdit::singleline(&mut self.range_end).desired_width(60.0));
            for (label, choice) in [("Restore range", "Keep"), ("Remove range", "Remove")] {
                if ui.button(label).on_hover_text("A manual boundary may cut through a word. Captions follow the word midpoint.").clicked()
                    && let (Ok(start), Ok(end)) = (self.range_start.parse::<f64>(), self.range_end.parse::<f64>())
                    && start.is_finite() && end.is_finite()
                {
                    action = Some(json!({"kind":"range","range":{"start":Tick::from_seconds_f64(start).0.to_string(),"end":Tick::from_seconds_f64(end).0.to_string()},"decision":choice}));
                }
            }
        });
        if ui.rect_contains_pointer(ui.max_rect()) && !ui.ctx().egui_wants_keyboard_input() && !map.candidates.is_empty() {
            ui.input(|input| {
                if input.key_pressed(egui::Key::N) {
                    self.candidate = (1..=map.candidates.len())
                        .map(|n| (self.candidate + n) % map.candidates.len())
                        .find(|i| decision(*i).is_none())
                        .unwrap_or(self.candidate);
                }
                if self.candidate < map.candidates.len() {
                    if input.key_pressed(egui::Key::K) {
                        action = Some(json!({"kind":"candidate","index":self.candidate,"decision":"Keep"}));
                    }
                    if input.key_pressed(egui::Key::R) {
                        action = Some(json!({"kind":"candidate","index":self.candidate,"decision":"Remove"}));
                    }
                    if input.key_pressed(egui::Key::C) {
                        audition = Some(map.candidates[self.candidate].range);
                    }
                }
            });
        }
        if let Some(range) = audition {
            let context = Tick(TICKS_PER_SECOND);
            self.audition(
                app,
                analysis.source.item,
                analysis.source.audio_stream,
                TimeRange::from_bounds((range.start - context).max(analysis.source.range.start), (range.end() + context).min(analysis.source.range.end())),
            );
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("Reset suggestions").clicked() {
                action = Some(json!({"kind":"resetSuggestions"}));
            }
            if ui.button("Reset all decisions").on_hover_text("Clears all manual overrides and conflicts. This can be undone.").clicked() {
                action = Some(json!({"kind":"resetDecisions"}));
            }
            if ui.add_enabled(!map.reviewed && !segments.is_empty() && map.conflicts.is_empty(), egui::Button::new("Mark reviewed")).clicked() {
                action = Some(json!({"kind":"markReviewed"}));
            }
        });
        if let Some(action) = action {
            self.review(app, id, action);
            return;
        }
        self.result_surface(app, ui, id, map.reviewed, retained);
    }

    fn result_surface(&mut self, app: &mut ShotClutchApp, ui: &mut Ui, map: u64, reviewed: bool, duration: Tick) {
        ui.separator();
        if self.result.as_ref().is_some_and(|view| view.map != map || view.revision != app.session.revision) {
            self.stop(app.audio.as_deref_mut());
            self.result = None;
        }
        if ui.add_enabled(duration > Tick::ZERO, egui::Button::new("Preview result")).clicked() {
            self.stop(app.audio.as_deref_mut());
            match shotclutch_engine::preedit::preview(&app.session, map) {
                Ok((project, sequence)) => {
                    self.result = Some(ResultView { map, revision: app.session.revision, project, sequence, time: Tick::ZERO, playing: None })
                }
                Err(error) => self.message = error.to_string(),
            }
        }
        let Some(view) = self.result.as_mut() else { return };
        ui.label("Result preview · editable clips are created on Apply");
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
        let scale = (ui.available_width() / sequence.settings.width as f32).clamp(0.05, 0.25);
        if !sequence.video_tracks.is_empty() {
            let tick = rate.snap(view.time.min(duration - Tick(1)));
            let key = FrameKey {
                target: Target::SequenceLive(view.sequence),
                frame: rate.frame_at(tick),
                size: (scale * 1000.0) as u32,
                revision: u64::MAX - view.revision,
                draft: false,
            };
            app.frames.request(key, tick, scale, &view.project, 0);
            if let Some(img) = app.frames.get(&key) {
                app.texture_for(ui.ctx(), "rough-cut-result", key, &img);
            }
            if let Some((texture, size)) = app.texture_existing("rough-cut-result") {
                ui.add(egui::Image::new((texture, size)).max_height(200.0));
            }
        }
        let mut seconds = view.time.0 as f64 / TICKS_PER_SECOND as f64;
        let changed = ui.add(egui::Slider::new(&mut seconds, 0.0..=(duration.0 as f64 / TICKS_PER_SECOND as f64)).suffix(" s")).changed();
        if changed {
            if view.playing.take().is_some()
                && let Some(audio) = app.audio.as_mut()
            {
                audio.stop();
            }
            view.time = Tick::from_seconds_f64(seconds);
        }
        let mut apply = None;
        ui.horizontal_wrapped(|ui| {
            if ui.button(if view.playing.is_some() { "Stop preview" } else { "Play preview" }).clicked() {
                if view.playing.take().is_some() {
                    if let Some(audio) = app.audio.as_mut() {
                        audio.stop();
                    }
                } else {
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
                    view.playing = Some((now, view.time, rate));
                }
            }
            let can_apply = reviewed && app.session.preedit_apply.is_none();
            if ui.add_enabled(can_apply, egui::Button::new("Apply to new sequence")).clicked() {
                apply = Some(None);
            }
            let project = &app.session.project;
            let item_for_map = |id| {
                let map = project.preedit.maps.get(&id)?;
                project.preedit.analyses.get(&map.analysis).map(|analysis| analysis.source.item)
            };
            if let Some((&id, _)) = project
                .preedit
                .applications
                .iter()
                .rev()
                .find(|(_, application)| project.sequence(application.sequence).is_some() && item_for_map(application.map) == item_for_map(map))
                && ui.add_enabled(can_apply, egui::Button::new("Replace previous result")).clicked()
            {
                apply = Some(Some(id));
            }
        });
        if let Some(replace) = apply {
            self.stop(app.audio.as_deref_mut());
            let revision = app.session.revision;
            let operation = format!("rough-cut-{map}-{revision}");
            self.command(
                app,
                "preedit.applyBackground",
                json!({"map":map.to_string(),"expectedRevision":revision.to_string(),"operationId":operation,"replace":replace.map(|id| id.to_string())}),
            );
        }
    }
}

pub fn show(app: &mut ShotClutchApp, ui: &mut Ui, rect: Rect) {
    if cfg!(target_arch = "wasm32") {
        ui.put(rect, egui::Label::new("Smart Rough Cut requires the native desktop app."));
        return;
    }
    let mut workspace = std::mem::take(&mut app.rough_cut);
    let document = app.session.document_id.to_string();
    if workspace.document.as_ref() != Some(&document) {
        workspace.stop(app.audio.as_deref_mut());
        workspace = Workspace { document: Some(document), language: "en".into(), pause_ms: 1000, handle_ms: 150, ..Default::default() };
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(10.0, 8.0))));
    egui::ScrollArea::vertical().id_salt("rough-cut-review").show(&mut child, |ui| {
        ui.heading("Smart Rough Cut");
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
        if app.session.preedit_apply.is_some() {
            ui.horizontal(|ui| {
                ui.label("Verifying original before applying…");
                if ui.button("Cancel apply").clicked() {
                    workspace.command(app, "preedit.cancelApply", json!({}));
                }
            });
        }
        workspace.inputs(app, ui);
        let project = app.session.project.clone();
        let current = project
            .preedit
            .maps
            .iter()
            .rev()
            .find(|(_, map)| project.preedit.analyses.get(&map.analysis).is_some_and(|analysis| Some(analysis.source.item) == workspace.source));
        if let Some((&id, map)) = current {
            workspace.map = Some(id);
            let analysis = &project.preedit.analyses[&map.analysis];
            workspace.review_surface(app, ui, id, map, analysis);
        }
        if !workspace.message.is_empty() {
            ui.label(&workspace.message);
        }
    });
    app.rough_cut = workspace;
}

fn transcript(analysis: &PreEditAnalysis, range: TimeRange) -> String {
    analysis
        .chunks
        .iter()
        .filter_map(|c| c.transcript.as_ref())
        .flat_map(|t| t.words[t.words_in(range)].iter())
        .take(140)
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

fn waveform(ui: &mut Ui, analysis: &PreEditAnalysis, selection: Option<TimeRange>) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 35.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, Color32::from_gray(25));
    let range = analysis.source.range;
    let x = |t: Tick| rect.left() + rect.width() * (t - range.start).0 as f32 / range.duration.0 as f32;
    if let Some(selection) = selection {
        ui.painter().rect_filled(
            Rect::from_min_max(pos2(x(selection.start), rect.top()), pos2(x(selection.end()), rect.bottom())),
            0.0,
            Color32::from_rgb(44, 59, 79),
        );
    }
    let width = rect.width().clamp(1.0, 2048.0) as usize;
    let mut bins = vec![0u8; width];
    for chunk in &analysis.chunks {
        let offset = x(chunk.coverage.start) - rect.left();
        let span = x(chunk.coverage.end()) - x(chunk.coverage.start);
        for (i, peak) in chunk.peaks.iter().enumerate() {
            let col = (offset + span * i as f32 / chunk.peaks.len() as f32) as usize;
            if let Some(bin) = bins.get_mut(col) {
                *bin = (*bin).max(*peak);
            }
        }
    }
    for (i, peak) in bins.into_iter().enumerate().filter(|(_, peak)| *peak != 0) {
        let x = rect.left() + i as f32 * rect.width() / width as f32;
        let y = rect.height() * f32::from(peak) / 510.0;
        ui.painter().line_segment([pos2(x, rect.center().y - y), pos2(x, rect.center().y + y)], (1.0, Color32::from_rgb(113, 187, 228)));
    }
}
