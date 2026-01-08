use egui_tiles::{Tiles, Tree};

use crate::{
    editor::tabs::{
        code_editor::CodeEditorTab, console::ConsoleTab, file_explorer::FileExplorerTab,
        viewport::ViewportTab, Tab, TabViewer,
    },
    egui_util::KeyModifiers,
};

pub mod code_editor;
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
        let console_id = tiles.insert_pane(Tab::Console(ConsoleTab::default()));
        let file_explorer_id = tiles.insert_pane(Tab::FileExplorer(FileExplorerTab::default()));

        // Left side: viewport (top) and console (bottom) - vertical split
        let mut left_shares = egui_tiles::Shares::default();
        left_shares[viewport_id] = 0.7;
        left_shares[console_id] = 0.3;

        let left_linear = egui_tiles::Linear {
            children: vec![viewport_id, console_id],
            dir: egui_tiles::LinearDir::Vertical,
            shares: left_shares,
        };
        let left_id = tiles.insert_container(egui_tiles::Container::Linear(left_linear));

        // Right side: code_editor (left) and file_explorer (right) - horizontal split
        let mut right_shares = egui_tiles::Shares::default();
        right_shares[code_editor_id] = 0.8;
        right_shares[file_explorer_id] = 0.2;

        let right_linear = egui_tiles::Linear {
            children: vec![code_editor_id, file_explorer_id],
            dir: egui_tiles::LinearDir::Horizontal,
            shares: right_shares,
        };
        let right_id = tiles.insert_container(egui_tiles::Container::Linear(right_linear));

        // Root: left and right side by side - horizontal split
        let mut root_shares = egui_tiles::Shares::default();
        root_shares[left_id] = 0.35;
        root_shares[right_id] = 0.65;

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
