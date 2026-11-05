//! Native review surface. Only the editor commands can change a rough-cut map.

use std::sync::Arc;

use egui::{Rect, Ui, pos2, vec2};
use serde_json::{Value, json};
use shotclutch_project::preedit::{AnalysisStatus, PreEditAnalysis, Segment};
use shotclutch_project::{ItemId, Project};
use shotclutch_time::{TICKS_PER_SECOND, Tick, TimeRange};

use crate::frames::{FrameKey, Target};
use crate::icons::{self, Icon};
use crate::{AudioOut, ShotClutchApp};

#[derive(Default)]
pub struct Workspace {
    tab: ReviewTab,
    show_cuts: bool,
    search: String,
    original: bool,
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
    seen_analysis: Option<(u64, bool)>,
    cursor: Tick,
    zoom: f32,
    window_start: Tick,
    mark_in: Option<Tick>,
    mark_out: Option<Tick>,
    pub(crate) toggle_requested: bool,
    message: String,
    result: Option<ResultView>,
    preview_attempt: Option<(u64, u64)>,
    download: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    confirm_download: bool,
    document: Option<String>,
}

#[derive(Default, PartialEq)]
enum ReviewTab {
    #[default]
    Script,
    Files,
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
        app.stop_source();
        let revision = app.session.revision.to_string();
        if let Some(reply) = self.command(app, "preedit.review", json!({"map":map.to_string(),"expectedRevision":revision,"action":action})) {
            self.map = reply["map"].as_str().and_then(|value| value.parse().ok());
        }
    }

    fn audition(&mut self, app: &mut ShotClutchApp, source: ItemId, stream: usize, range: TimeRange) {
        self.stop(app.audio.as_deref_mut());
        self.original = true;
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
        if self
            .command(
                app,
                "preedit.analyze",
                json!({
                    "item":item.0.to_string(),"audioStream":self.stream,"channel":self.channel,"model":model,"language":self.language,"range":range,
                    "minPauseMs":self.pause_ms,"handleMs":self.handle_ms,"autoCut":true,
                    "previousMap":self.map.map(|id| id.to_string()),"resume":resume.map(|id| id.to_string())
                }),
            )
            .is_some()
        {
            self.tab = ReviewTab::Script;
        }
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
            self.cursor = Tick::ZERO;
            self.window_start = Tick::ZERO;
            self.mark_in = None;
            self.mark_out = None;
            app.stop_source();
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
                if ui.button(if self.map.is_some() { "Run cleanup again" } else { "Generate smart cut" }).clicked() {
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

    fn seek_source(&mut self, app: &mut ShotClutchApp, item: ItemId, time: Tick) {
        self.stop(app.audio.as_deref_mut());
        app.stop_source();
        if app.session.state.source_item != Some(item) {
            self.command(app, "source.open", json!({"item":item.0}));
        }
        self.command(app, "source.setPlayhead", json!({"time":time.0}));
    }

    fn script(&mut self, app: &mut ShotClutchApp, ui: &mut Ui, id: u64, analysis: &PreEditAnalysis, segments: &[Segment]) {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Find in script").desired_width(ui.available_width()));
        });
        ui.add_space(10.0);
        let query = self.search.to_lowercase();
        let mut seek = None;
        let mut action = None;
        let mut any = false;
        egui::ScrollArea::vertical().id_salt("rough-cut-script").auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 7.0);
            for chunk in &analysis.chunks {
                let Some(text) = &chunk.transcript else { continue };
                for words in text.words.chunks(32) {
                    if !query.is_empty() && !words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ").to_lowercase().contains(&query) {
                        continue;
                    }
                    let Some(first) = words.first() else { continue };
                    any = true;
                    ui.add_space(10.0);
                    if ui.small_button(time_label(first.start)).on_hover_text("Seek original").clicked() {
                        seek = Some(first.start);
                    }
                    ui.horizontal_wrapped(|ui| {
                        for word in words {
                            let midpoint = word.start + Tick(word.range().duration.0 / 2);
                            let removed = !segments.iter().any(|segment| segment.source.contains(midpoint));
                            if removed && !self.show_cuts { continue; }
                            let active = word.range().contains(self.cursor);
                            let mut label = egui::RichText::new(&word.text).size(16.0).color(if removed { app.tokens.text_dim } else { app.tokens.text });
                            if removed { label = label.strikethrough(); }
                            if active { label = label.background_color(app.tokens.row_selected); }
                            let response = ui.add(egui::Label::new(label).sense(egui::Sense::click()));
                            if response.clicked() {
                                seek = Some(word.start);

                            }
                            response.on_hover_text("Click to seek · right-click to remove or restore").context_menu(|ui| {
                                for (label, decision) in [("Keep / restore word", "Keep"), ("Remove word", "Remove")] {
                                    if ui.button(label).clicked() {
                                        action = Some(json!({"kind":"range","range":{"start":word.start.0.to_string(),"end":word.range().end().0.to_string()},"decision":decision}));
                                        ui.close();
                                    }
                                }
                            });
                        }
                    });
                }
            }
            if !any {
                ui.label(if query.is_empty() { "No speech transcript. Review the cuts in the timeline." } else { "No matching phrases." });
            }
        });
        if let Some(time) = seek {
            self.seek(app, analysis.source.item, segments, time);
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
        let scale = (ui.available_width() / video.width as f32).clamp(0.05, 0.5);
        let time = app.session.state.source_playhead.clamp(range.start, range.end() - Tick(1));
        let key = FrameKey {
            target: Target::Item(item),
            frame: view.rate.frame_at(time),
            size: (scale * 1000.0) as u32,
            revision: app.item_revision(item),
            draft: false,
        };
        if app.source_playback.clock.playing {
            app.frames.schedule_playback(key, view.rate, scale, &app.session.project, 1.0, false);
        } else {
            app.frames.request(key, time, scale, &app.session.project, 0);
        }
        if let Some(img) = app.frames.get(&key) {
            app.texture_for(ui.ctx(), "rough-cut-original", key, &img);
        }
        if let Some((texture, size)) =
            app.texture_existing("rough-cut-original").filter(|_| app.texture_key("rough-cut-original").is_some_and(|shown| shown.target == key.target))
        {
            ui.add(egui::Image::new((texture, size)).max_size(ui.available_size()));
        } else {
            ui.label("Loading original frame…");
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(if app.source_playback.clock.playing { 16 } else { 100 }));
    }

    fn result_surface(&mut self, app: &mut ShotClutchApp, ui: &mut Ui, map: u64, duration: Tick, segments: &[Segment]) {
        ui.separator();
        if self.result.as_ref().is_some_and(|view| view.map != map || view.revision != app.session.revision) {
            self.stop(app.audio.as_deref_mut());
            self.result = None;
        }
        let attempt = (map, app.session.revision);
        let retry = self.result.is_none() && self.preview_attempt == Some(attempt) && ui.button("Retry result preview").clicked();
        if duration > Tick::ZERO && self.result.is_none() && (self.preview_attempt != Some(attempt) || retry) {
            self.preview_attempt = Some(attempt);
            self.stop(app.audio.as_deref_mut());
            match shotclutch_engine::preedit::preview(&app.session, map) {
                Ok((project, sequence)) => {
                    self.result =
                        Some(ResultView { map, revision: app.session.revision, project, sequence, time: result_time(segments, self.cursor), playing: None })
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
        let scale = (ui.available_width() / sequence.settings.width as f32).clamp(0.05, 0.5);
        if !sequence.video_tracks.is_empty() {
            let tick = rate.snap(view.time.min(duration - Tick(1)));
            let key = FrameKey {
                target: Target::SequenceLive(view.sequence),
                frame: rate.frame_at(tick),
                size: (scale * 1000.0) as u32,
                revision: u64::MAX - view.revision,
                draft: false,
            };
            if view.playing.is_some() {
                app.frames.schedule_playback(key, rate, scale, &view.project, 1.0, false);
            } else {
                app.frames.request(key, tick, scale, &view.project, 0);
            }
            if let Some(img) = app.frames.get(&key) {
                app.texture_for(ui.ctx(), "rough-cut-result", key, &img);
            }
            if let Some((texture, size)) =
                app.texture_existing("rough-cut-result").filter(|_| app.texture_key("rough-cut-result").is_some_and(|shown| shown.revision == key.revision))
            {
                let available = ui.available_size();
                ui.add(egui::Image::new((texture, size)).max_size(available));
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

pub fn show(app: &mut ShotClutchApp, ui: &mut Ui, rect: Rect) {
    if cfg!(target_arch = "wasm32") {
        ui.put(rect, egui::Label::new("Smart Rough Cut requires the native desktop app."));
        return;
    }
    let mut workspace = std::mem::take(&mut app.rough_cut);
    let document = app.session.document_id.to_string();
    if workspace.document.as_ref() != Some(&document) {
        workspace.stop(app.audio.as_deref_mut());
        workspace = Workspace {
            document: Some(document),
            language: "en".into(),
            show_cuts: true,
            zoom: 1.0,
            original: true,
            pause_ms: 1000,
            handle_ms: 150,
            ..Default::default()
        };
    }

    let bounds = rect.shrink2(vec2(16.0, 10.0));
    let mut header = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(bounds.min, pos2(bounds.right(), bounds.top() + 40.0))));
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
    header.horizontal(|ui| {
        ui.label(egui::RichText::new("Smart Rough Cut").size(18.0).strong());
        if let Some(item) = workspace.source.and_then(|id| project.item(id)) {
            ui.separator();
            ui.add_sized(vec2((bounds.width() * 0.4).max(80.0), 22.0), egui::Label::new(&item.name).truncate());
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Editing workspace").clicked() {
                workspace.stop(app.audio.as_deref_mut());
                app.stop_source();
                app.set_workspace("Editing");
            }
            for (label, command, enabled) in [("Redo", "edit.redo", app.session.history.can_redo()), ("Undo", "edit.undo", app.session.history.can_undo())] {
                if ui.add_enabled(enabled && app.session.preedit_apply.is_none(), egui::Button::new(label)).clicked() {
                    workspace.stop(app.audio.as_deref_mut());
                    app.stop_source();
                    workspace.command(app, command, json!({}));
                }
            }
        });
    });
    let transport_h = (bounds.height() * 0.24).clamp(180.0, 220.0);
    let body =
        Rect::from_min_max(pos2(bounds.left(), bounds.top() + 44.0), pos2(bounds.right(), (bounds.bottom() - transport_h - 8.0).max(bounds.top() + 80.0)));
    let split = body.left() + body.width() * 0.49;
    let left = Rect::from_min_max(body.min, pos2(split - 10.0, body.bottom()));
    let right = Rect::from_min_max(pos2(split + 10.0, body.top()), body.max);
    let bottom = Rect::from_min_max(pos2(bounds.left(), body.bottom() + 12.0), bounds.max);
    app.auto.add("roughCut.scriptPane", left, "Script pane");
    app.auto.add("roughCut.viewer", right, "Video preview");
    app.auto.add("roughCut.transport", bottom, "Review transport");
    ui.painter().rect_filled(left, 8.0, app.tokens.field_bg);
    ui.painter().line_segment([bottom.left_top(), bottom.right_top()], (1.0, app.tokens.separator));
    let mut left_ui = ui.new_child(egui::UiBuilder::new().max_rect(left.shrink(12.0)));
    left_ui.set_clip_rect(left);
    left_ui.horizontal(|ui| {
        for (tab, id, label) in [(ReviewTab::Script, "script", "Script"), (ReviewTab::Files, "files", "Files & analysis")] {
            let response = ui.selectable_value(&mut workspace.tab, tab, label);
            app.auto.add_response(&format!("roughCut.tab.{id}"), &response, label);
        }
    });
    left_ui.separator();
    let mut viewer = ui.new_child(egui::UiBuilder::new().max_rect(right));
    viewer.set_clip_rect(right);
    let mut transport = ui.new_child(egui::UiBuilder::new().max_rect(bottom.shrink2(vec2(0.0, 8.0))));
    transport.set_clip_rect(bottom);

    let map = current.map(|(&id, map)| (id, &**map));
    let analysis = map.map(|(_, map)| &*project.preedit.analyses[&map.analysis]);
    let segments = map.zip(analysis).and_then(|((_, map), analysis)| map.segments(analysis.source.range).ok()).unwrap_or_default();
    let duration = segments.last().map_or(Tick::ZERO, |segment| segment.output.end());
    if let Some((id, map)) = map {
        workspace.map = Some(id);
        let analysis_mode = (map.analysis, map.automatic.is_some());
        if workspace.seen_analysis != Some(analysis_mode) {
            workspace.seen_analysis = Some(analysis_mode);
            workspace.original = duration == Tick::ZERO || map.automatic.is_none();
            workspace.cursor = analysis.unwrap().source.range.start;
            workspace.mark_in = None;
            workspace.mark_out = None;
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
        workspace.script(app, &mut left_ui, id, analysis, &segments);
    } else {
        egui::ScrollArea::vertical().id_salt("rough-cut-files").show(&mut left_ui, |ui| {
            if map.is_none() {
                ui.heading("Start a smart cut");
                ui.label("Choose your source. Cleanup opens the cut automatically when processing finishes.");
                ui.add_space(8.0);
            }
            if ui.button("Import media…").clicked() {
                if let Err(error) = crate::menus::invoke(app, ui.ctx(), "file.import", json!({})) {
                    workspace.message = error;
                }
            }
            workspace.inputs(app, ui);
        });
    }

    if let Some(item) = workspace.source
        && let Some(media) = project.item(item).and_then(|item| item.as_media())
    {
        let source_range = analysis.filter(|a| a.source.item == item).map_or_else(|| TimeRange::from_bounds(Tick::ZERO, media.duration()), |a| a.source.range);
        let stream = analysis.filter(|a| a.source.item == item).map_or(workspace.stream, |a| a.source.audio_stream);
        if source_range.duration > Tick::ZERO {
            let changed = map.is_some_and(|(id, _)| workspace.map != Some(id));
            viewer.horizontal_wrapped(|ui| {
                ui.label(if workspace.original { "Original" } else { "Smart cut" });
                ui.separator();
                let total = if workspace.original { source_range.duration } else { duration };
                ui.label(time_label(total));
                if map.is_some() {
                    let cuts = cut_count(&segments, source_range);
                    ui.label(format!("{cuts} cuts · {} saved", time_label(source_range.duration - duration)));
                }
                if app.session.preedit_job.is_some() {
                    ui.spinner();
                    ui.label("Generating smart cut…");
                    if ui.small_button("Cancel").clicked() {
                        workspace.command(app, "preedit.cancel", json!({}));
                    }
                }
            });
            viewer.add_space(8.0);
            let mut picture = viewer.available_rect_before_wrap();
            picture.max.y = (picture.max.y - 36.0).max(picture.min.y);
            viewer.scope_builder(egui::UiBuilder::new().max_rect(picture), |ui| {
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
                let cuts = ui.checkbox(&mut workspace.show_cuts, "Show cuts");
                app.auto.add_response("roughCut.showCuts", &cuts, "Show cuts");
                let mut skip = !workspace.original;
                let response = ui.add_enabled(duration > Tick::ZERO, egui::Checkbox::new(&mut skip, "Skip cuts"));
                app.auto.add_response("roughCut.skipCuts", &response, "Skip cuts");
                if response.changed() {
                    workspace.stop(app.audio.as_deref_mut());
                    app.stop_source();
                    workspace.original = !skip;
                    workspace.seek(app, item, &segments, workspace.cursor);
                }
                ui.add(egui::Slider::new(&mut workspace.zoom, 1.0..=64.0).logarithmic(true).show_value(false).text("Zoom"));
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
                    if ui.small_button("Clear marks").clicked() {
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
                    let can_open =
                        (map.automatic.is_some() || map.reviewed) && map.conflicts.is_empty() && duration > Tick::ZERO && app.session.preedit_apply.is_none();
                    let response = ui.add_enabled(can_open, egui::Button::new("Open in editor"));
                    app.auto.add_response("roughCut.openInEditor", &response, "Open in editor");
                    if response.clicked() {
                        workspace.open_in_editor(app, id, None);
                    }
                    if let Some((&previous, _)) = project.preedit.applications.iter().rev().find(|(_, application)| {
                        project.sequence(application.sequence).is_some()
                            && project
                                .preedit
                                .maps
                                .get(&application.map)
                                .and_then(|old| project.preedit.analyses.get(&old.analysis))
                                .is_some_and(|old| old.source.item == item)
                    }) && ui.add_enabled(can_open, egui::Button::new("Replace previous sequence")).clicked()
                    {
                        workspace.open_in_editor(app, id, Some(previous));
                    }
                });
                if let Some(action) = action {
                    workspace.review(app, id, action);
                }
                if let Some(analysis) = analysis {
                    let span = Tick((source_range.duration.0 as f64 / f64::from(workspace.zoom.max(1.0))) as i64).max(Tick(1));
                    let start = workspace.window_start.clamp(source_range.start, source_range.end() - span);
                    workspace.window_start = if workspace.cursor < start || workspace.cursor > start + span {
                        (workspace.cursor - Tick(span.0 / 2)).clamp(source_range.start, source_range.end() - span)
                    } else {
                        start
                    };
                    let window = TimeRange::from_bounds(workspace.window_start, workspace.window_start + span);
                    let (seek, restore) = waveform(&mut transport, app, analysis, &segments, window, workspace.cursor, selected, workspace.show_cuts);
                    if let Some(time) = seek {
                        workspace.seek(app, item, &segments, time);
                    }
                    if let Some(range) = restore {
                        workspace.review(app, id, range_action(range, "Keep"));
                    }
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
        viewer.centered_and_justified(|ui| {
            ui.label("Import media to preview it here.");
        });
    }
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
    app.rough_cut = workspace;
}

fn result_time(segments: &[Segment], source: Tick) -> Tick {
    segments
        .iter()
        .find(|segment| segment.source.end() > source)
        .map(|segment| segment.output.start + (source - segment.source.start).max(Tick::ZERO))
        .or_else(|| segments.last().map(|segment| segment.output.end()))
        .unwrap_or(Tick::ZERO)
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
    json!({"kind":"range","range":{"start":range.start.0.to_string(),"end":range.end().0.to_string()},"decision":decision})
}

fn time_label(time: Tick) -> String {
    let seconds = (time.0 as f64 / TICKS_PER_SECOND as f64).max(0.0);
    format!("{:02}:{:05.2}", (seconds / 60.0) as u64, seconds % 60.0)
}

fn waveform(
    ui: &mut Ui,
    app: &mut ShotClutchApp,
    analysis: &PreEditAnalysis,
    segments: &[Segment],
    window: TimeRange,
    playhead: Tick,
    selected: Option<TimeRange>,
    show_cuts: bool,
) -> (Option<Tick>, Option<TimeRange>) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 76.0), egui::Sense::click_and_drag());
    app.auto.add_response("roughCut.waveform", &response, "Source waveform");
    let painter = ui.painter().with_clip_rect(rect);
    let tokens = app.tokens;
    painter.rect_filled(rect, 4.0, tokens.field_bg);
    let x = |t: Tick| rect.left() + rect.width() * (t - window.start).0 as f32 / window.duration.0 as f32;
    let time_at =
        |pointer: egui::Pos2| window.start + Tick((window.duration.0 as f64 * f64::from(((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0))) as i64);
    let lane = |range: TimeRange| Rect::from_min_max(pos2(x(range.start), rect.top() + 19.0), pos2(x(range.end()), rect.bottom()));
    if show_cuts {
        painter.rect_filled(rect, 4.0, tokens.field_border);
    }
    for segment in segments.iter().filter(|segment| segment.source.start < window.end() && segment.source.end() > window.start) {
        painter.rect_filled(lane(segment.source), 3.0, tokens.control_surface);
        painter.line_segment([pos2(x(segment.source.start), rect.top() + 19.0), pos2(x(segment.source.start), rect.bottom())], (1.0, tokens.separator));
    }
    if let Some(range) = selected {
        painter.rect_filled(lane(range), 0.0, tokens.row_selected);
    }
    let steps = (rect.width() / 115.0).clamp(2.0, 12.0) as i64;
    for step in 0..=steps {
        let time = window.start + Tick(window.duration.0 * step / steps);
        painter.text(
            pos2(x(time), rect.top() + 3.0),
            if step == steps { egui::Align2::RIGHT_TOP } else { egui::Align2::LEFT_TOP },
            time_label(time),
            egui::FontId::monospace(11.0),
            tokens.text_dim,
        );
    }
    let width = rect.width().clamp(1.0, 2048.0) as usize;
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
    let mid = rect.center().y + 9.0;
    for (i, peak) in bins.into_iter().enumerate().filter(|(_, peak)| *peak != 0) {
        let tick = window.start + Tick(window.duration.0 * i as i64 / width as i64);
        let retained = segments.iter().any(|segment| segment.source.contains(tick));
        if !show_cuts && !retained {
            continue;
        }
        let px = rect.left() + i as f32 * rect.width() / width as f32;
        let y = 25.0 * (f32::from(peak) / 255.0).sqrt();
        painter.line_segment([pos2(px, mid - y), pos2(px, mid + y)], (1.0, if retained { tokens.accent_hover } else { tokens.text_dim }));
    }
    let px = x(playhead);
    painter.line_segment([pos2(px, rect.top()), pos2(px, rect.bottom())], (2.0, tokens.accent_hover));
    let seek = (response.clicked() || response.dragged()).then(|| response.interact_pointer_pos().map(time_at)).flatten();
    let mut restored = None;
    let anchor = ui.make_persistent_id("rough-cut-waveform-menu");
    if response.secondary_clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        ui.data_mut(|data| data.insert_temp(anchor, time_at(pointer)));
    }
    response.on_hover_text("Drag to scrub. Right-click a removed range to restore it.").context_menu(|ui| {
        let cut = ui.data(|data| data.get_temp::<Tick>(anchor)).and_then(|t| cut_at(segments, analysis.source.range, t));
        if ui.add_enabled(cut.is_some(), egui::Button::new("Restore cut")).clicked() {
            restored = cut;
            ui.close();
        }
    });
    (seek, restored)
}
