//! Native ShotClutch wordmark; no upstream trademark assets are embedded.

/// Draw the product name using the editor's bundled typography.
pub fn paint_wordmark(ui: &egui::Ui, left_center: egui::Pos2, height: f32, dark: bool) -> Option<egui::Rect> {
    let color = if dark { egui::Color32::WHITE } else { egui::Color32::BLACK };
    let galley = ui.painter().layout_no_wrap("ShotClutch".into(), egui::FontId::proportional(height), color);
    let pos = egui::pos2(left_center.x, left_center.y - galley.size().y / 2.0);
    let rect = egui::Rect::from_min_size(pos, galley.size());
    ui.painter().galley(pos, galley, color);
    Some(rect)
}
