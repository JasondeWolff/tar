use egui::{Color32, Stroke};

pub fn init(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    style.visuals.widgets.active.bg_stroke = Stroke::NONE;
    style.visuals.widgets.hovered.bg_stroke = Stroke::NONE;
    style.visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(0, 0, 0, 0);

    style.interaction.selectable_labels = false;

    style.debug.show_unaligned = false;

    ctx.set_style(style);
}
