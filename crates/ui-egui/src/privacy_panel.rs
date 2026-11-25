use shotclutch_engine::Session;
use shotclutch_engine::privacy::{DELETION_SCOPE, DIAGNOSTIC_ENTRIES, Deletion, DeletionPreview, Inventory};
use shotclutch_project::ItemId;

use crate::{ShotClutchApp, native_dialogs::FileDialog};

pub(crate) struct PrivacyPanel {
    inventory: Inventory,
    asset: Option<ItemId>,
    include_dependencies: bool,
    preview: Option<DeletionPreview>,
    checkpoint_days: u16,
    cache_days: u32,
    message: String,
}

impl PrivacyPanel {
    pub(crate) fn new(session: &Session) -> Self {
        Self {
            inventory: session.privacy_inventory(),
            asset: None,
            include_dependencies: false,
            preview: None,
            checkpoint_days: session.retention_preferences().completed_run_days,
            cache_days: if session.prefs.media_cache.management == "olderThan" { session.prefs.media_cache.older_than_days } else { 0 },
            message: String::new(),
        }
    }
}

fn counts(ui: &mut egui::Ui, inventory: &Inventory) {
    ui.label(format!("{} transcripts · {} observations ({} annotations)", inventory.transcripts, inventory.observations, inventory.annotations));
    ui.label(format!(
        "{} Smart Cut analyses · {} maps ({} reviewed or applied)",
        inventory.rough_cut_analyses, inventory.rough_cut_maps, inventory.reviewed_or_applied_maps
    ));
    ui.label(format!("{} completed run checkpoints · {} active or resumable", inventory.finished_runs, inventory.active_or_resumable_runs));
    ui.label(format!(
        "{} undo/redo states · {} retained receipts · {} recovery copies",
        inventory.undo_redo, inventory.retained_receipts, inventory.recovery_copies
    ));
    ui.label(format!("{} preview cache entries · {} generated narrations", inventory.preview_cache_entries, inventory.generated_narrations));
    if inventory.unverified_snapshot_files != 0 {
        ui.label(format!("{} snapshot files lack a verified project identity and are kept.", inventory.unverified_snapshot_files));
    }
    if inventory.dependent_caption_tracks != 0 {
        ui.label(format!(
            "{} dependent caption tracks · {} in edited sequences are preserved",
            inventory.dependent_caption_tracks, inventory.preserved_caption_tracks
        ));
    }
}

pub(crate) fn show(app: &mut ShotClutchApp, ctx: &egui::Context) {
    let Some(mut panel) = app.privacy_panel.take() else { return };
    let mut open = true;
    egui::Window::new("Privacy & Data").default_width(520.0).open(&mut open).vscroll(true).show(ctx, |ui| {
        ui.heading("Stored data");
        counts(ui, &panel.inventory);
        if ui.button("Refresh inventory").clicked() {
            panel.inventory = app.session.privacy_inventory();
            panel.preview = None;
        }
        ui.separator();
        ui.heading("Retention");
        ui.horizontal(|ui| {
            ui.label("Completed run checkpoints");
            ui.add(egui::DragValue::new(&mut panel.checkpoint_days).range(0..=3650).suffix(" days"));
        });
        ui.horizontal(|ui| {
            ui.label("Disposable media cache");
            ui.add(egui::DragValue::new(&mut panel.cache_days).range(0..=3650).suffix(" days"));
        });
        ui.label("Zero disables age-based cleanup. Removing a run checkpoint ends access to that run's progress and recovery.");
        if ui.button("Save retention settings").clicked() {
            let result = app.session.set_privacy_retention(panel.checkpoint_days, panel.cache_days);
            panel.message = result.map_or_else(|error| error.to_string(), |_| "Retention settings saved.".into());
            panel.inventory = app.session.privacy_inventory();
            panel.preview = None;
        }
        ui.separator();
        ui.heading("Delete local data");
        ui.label(DELETION_SCOPE);
        if app.session.privacy_cleanup_pending() && ui.button("Retry pending cleanup").clicked() {
            panel.message = app
                .session
                .retry_privacy_cleanup()
                .map_or_else(|error| error.to_string(), |files| format!("Cleanup completed. {files} managed files removed."));
            panel.inventory = app.session.privacy_inventory();
            panel.preview = None;
        }
        egui::ComboBox::from_id_salt("privacy_asset")
            .selected_text(panel.asset.and_then(|id| app.session.project.items.get(&id)).map_or("Select an asset", |item| item.name.as_str()))
            .show_ui(ui, |ui| {
                for item in app.session.project.items.values().filter(|item| item.as_media().is_some()) {
                    if ui.selectable_value(&mut panel.asset, Some(item.id), &item.name).changed() {
                        panel.preview = None;
                    }
                }
            });
        if ui.checkbox(&mut panel.include_dependencies, "Include dependent authored analysis, history copies and run checkpoints").changed() {
            panel.preview = None;
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("Preview disposable cleanup").clicked() {
                panel.preview = Some(app.session.preview_privacy_deletion(Deletion::Disposable));
            }
            if let Some(asset) = panel.asset
                && ui.button("Preview analysis deletion").clicked()
            {
                panel.preview = Some(app.session.preview_privacy_deletion(Deletion::Analysis { asset, include_dependencies: panel.include_dependencies }));
            }
            if ui.button("Preview project deletion").clicked() {
                panel.preview = Some(app.session.preview_privacy_deletion(Deletion::Project));
            }
        });
        if let Some(preview) = panel.preview.as_ref() {
            ui.group(|ui| {
                ui.strong(match preview.action {
                    Deletion::Disposable => "Disposable cleanup",
                    Deletion::Analysis { .. } => "Analysis deletion",
                    Deletion::Project => "Managed project deletion",
                });
                counts(ui, &preview.current);
                if preview.history.undo_redo != 0 {
                    ui.label("Analysis in retained undo/redo states");
                    counts(ui, &preview.history);
                }
                for file in &preview.managed_files {
                    ui.label(file.display().to_string());
                }
                if app.rough_cut.review_running() {
                    ui.label("Wait for the Smart Cut review to finish before deleting data.");
                } else if let Some(reason) = preview.blocked {
                    ui.label(reason);
                } else if ui.button("Confirm permanent deletion").clicked() {
                    panel.message = match app.session.apply_privacy_deletion(preview) {
                        Ok(result) => {
                            panel.inventory = result.remaining;
                            format!(
                                "Deletion completed. {} managed files removed. {} caption tracks were kept in edited sequences.",
                                result.removed_files, result.preserved_caption_tracks
                            )
                        }
                        Err(error) => error.to_string(),
                    };
                }
            });
        }
        ui.separator();
        ui.heading("Diagnostics");
        for entry in DIAGNOSTIC_ENTRIES {
            ui.label(*entry);
        }
        if ui.button("Export diagnostics…").clicked() {
            app.pick_ui(FileDialog::save_as("JSON", &["json"], "shotclutch-diagnostics.json"), |app, paths| {
                if let Some(path) = paths.first() {
                    app.ui.status = app
                        .session
                        .export_privacy_diagnostics(std::path::Path::new(path))
                        .map_or_else(|error| error.to_string(), |_| "Diagnostics saved.".into());
                }
            });
        }
        if !panel.message.is_empty() {
            ui.label(&panel.message);
        }
    });
    if open {
        app.privacy_panel = Some(panel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privacy_menu_opens_the_desktop_panel() {
        let mut app = ShotClutchApp::new(Session::default());
        let ctx = egui::Context::default();
        let command = crate::menus::invoke(&mut app, &ctx, "app.privacy", serde_json::json!({})).unwrap();
        assert_eq!(command["dialog"], "privacy");
        let mut frame = ctx.run_ui(egui::RawInput::default(), |ui| show(&mut app, ui.ctx()));
        frame.textures_delta.clear();
        assert!(!frame.shapes.is_empty());
        assert!(app.privacy_panel.is_some());
    }
}
