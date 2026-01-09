use egui_tiles::{Tiles, Tree};

use crate::{
    editor::tabs::{
        code_editor::CodeEditorTab, console::ConsoleTab, file_explorer::FileExplorerTab,
        render_graph::RenderGraphTab, viewport::ViewportTab, Tab, TabViewer,
    },
    egui_util::KeyModifiers,
};

pub mod code_editor;
pub mod node_graph;
pub mod tabs;

pub struct Editor {
    tree: Tree<Tab>,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    pub fn new() -> Self {
        let mut tiles = Tiles::default();

        let viewport_id = tiles.insert_pane(Tab::Viewport(ViewportTab::default()));
        let code_editor_id = tiles.insert_pane(Tab::CodeEditor(CodeEditorTab::default()));
        let render_graph_id = tiles.insert_pane(Tab::RenderGraph(RenderGraphTab::default()));
        let console_id = tiles.insert_pane(Tab::Console(ConsoleTab::default()));
        let file_explorer_id = tiles.insert_pane(Tab::FileExplorer(FileExplorerTab::default()));

        let main_tabs = egui_tiles::Tabs {
            children: vec![code_editor_id, render_graph_id],
            active: Some(code_editor_id),
        };
        let main_id = tiles.insert_container(egui_tiles::Container::Tabs(main_tabs));

        // Left side: viewport (top) and console (bottom) - vertical split
        let left_tabs = egui_tiles::Tabs {
            children: vec![viewport_id, file_explorer_id],
            active: Some(viewport_id),
        };
        let left_id = tiles.insert_container(egui_tiles::Container::Tabs(left_tabs));

        // Right side: code_editor (left) and file_explorer (right) - horizontal split
        let mut right_shares = egui_tiles::Shares::default();
        right_shares[main_id] = 0.8;
        right_shares[console_id] = 0.2;

        let right_linear = egui_tiles::Linear {
            children: vec![main_id, console_id],
            dir: egui_tiles::LinearDir::Vertical,
            shares: right_shares,
        };
        let right_id = tiles.insert_container(egui_tiles::Container::Linear(right_linear));

        // Root: left and right side by side - horizontal split
        let mut root_shares = egui_tiles::Shares::default();
        root_shares[left_id] = 0.25;
        root_shares[right_id] = 0.75;

        let root_linear = egui_tiles::Linear {
            children: vec![left_id, right_id],
            dir: egui_tiles::LinearDir::Horizontal,
            shares: root_shares,
        };
        let root = tiles.insert_container(egui_tiles::Container::Linear(root_linear));

        let tree = Tree::new("my_tree", root, tiles);

        Self { tree }
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
                            // self.dock_state
                            //     .add_window(vec![Tab::CodeEditor(CodeEditorTab::default())]);
                            ui.close();
                        }

                        if ui
                            .button("Viewport")
                            .on_hover_text("Open the Viewport")
                            .clicked()
                        {
                            // self.dock_state
                            //     .add_window(vec![Tab::Viewport(ViewportTab::default())]);
                            ui.close();
                        }
                    });
                });
            });

            // Allocate space for 1 horizontally centered button
            let desired_size = egui::Vec2::new(100.0, 20.0);
            let center = ui.min_rect().center();
            let rect = egui::Rect::from_center_size(center, desired_size);

            ui.add_enabled_ui(true, |ui| {
                ui.allocate_ui_at_rect(rect, |ui| {
                    ui.columns(1, |columns| {
                        columns[0].vertical_centered(|ui| {
                            let is_compiling = false;
                            if ui
                                .add(
                                    egui::Button::new(format!(
                                        "{} Compile",
                                        egui_phosphor::regular::HAMMER
                                    ))
                                    .fill(if is_compiling {
                                        egui::Color32::from_rgb(10, 100, 255) // TODO:"cool animated "breathing" color"
                                    } else {
                                        ui.visuals().widgets.active.bg_fill
                                    }),
                                )
                                .clicked()
                            {
                                println!("COMPILE!");
                            }
                        });
                    });
                });
            });
        });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(&egui_ctx.style())
                    .inner_margin(0.0)
                    .outer_margin(0.0),
            )
            .show(egui_ctx, |ui| {
                self.tree.ui(&mut TabViewer::new(key_modifiers), ui);
            });
    }
}
