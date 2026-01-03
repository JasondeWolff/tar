use egui::{FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

pub fn init(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::empty();

    fonts.font_data.insert(
        "cascadia".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/CascadiaCode-Regular.ttf"
        ))),
    );

    fonts
        .families
        .insert(FontFamily::Proportional, vec!["cascadia".to_owned()]);
    fonts
        .families
        .insert(FontFamily::Monospace, vec!["cascadia".to_owned()]);

    ctx.set_fonts(fonts);
}
