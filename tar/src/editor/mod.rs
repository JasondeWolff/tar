use crate::{
    editor::tabs::{code_editor::CodeEditorTab, viewport::ViewportTab, Tab, TabViewer},
    egui_util::KeyModifiers,
};

pub mod code_editor;
pub mod tabs;

pub struct Editor {
    dock_state: egui_dock::DockState<Tab>,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    pub fn new() -> Self {
        let mut dock_state = egui_dock::DockState::new(vec![Tab::Viewport(ViewportTab::default())]);

        let [_viewport, _code_editor] = dock_state.main_surface_mut().split_right(
            egui_dock::NodeIndex::root(),
            0.35,
            vec![Tab::CodeEditor(CodeEditorTab::default())],
        );

        Self { dock_state }
    }

    pub fn ui(&mut self, egui_ctx: &mut egui::Context, key_modifiers: &KeyModifiers) {
        egui::TopBottomPanel::top("top_bar").show(egui_ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.menu_button("File", |ui| {
                        if ui
                            .button("New Project")
                            .on_hover_text("Create a new project")
                            .clicked()
                        {
                            ui.close();
                        }

                        if ui
                            .button("Open Project")
                            .on_hover_text("Open an existing project")
                            .clicked()
                        {
                            ui.close();
                        }
                    });

                    ui.menu_button("Window", |ui| {
                        if ui
                            .button("Code Editor")
                            .on_hover_text("Open the Code Editor")
                            .clicked()
                        {
                            self.dock_state
                                .add_window(vec![Tab::CodeEditor(CodeEditorTab::default())]);
                            ui.close();
                        }

                        if ui
                            .button("Viewport")
                            .on_hover_text("Open the Viewport")
                            .clicked()
                        {
                            self.dock_state
                                .add_window(vec![Tab::Viewport(ViewportTab::default())]);
                            ui.close();
                        }
                    });
                });
            });
        });

        let dock_style = egui_dock::Style::from_egui(egui_ctx.style().as_ref());
        let dock_style = egui_dock::Style {
            main_surface_border_stroke: egui::Stroke::new(0.5, egui::Color32::from_rgb(50, 50, 50)),
            tab: egui_dock::TabStyle {
                tab_body: egui_dock::TabBodyStyle {
                    stroke: egui::Stroke::new(0.5, egui::Color32::from_rgb(50, 50, 50)),
                    ..dock_style.tab.tab_body
                },
                ..dock_style.tab
            },
            ..dock_style
        };

        // Draw panels
        egui_dock::DockArea::new(&mut self.dock_state)
            .style(dock_style)
            .show_leaf_collapse_buttons(false)
            .show_leaf_close_all_buttons(false)
            .show(egui_ctx, &mut TabViewer::new(key_modifiers));
    }
}
