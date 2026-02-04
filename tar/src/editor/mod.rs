use std::{any::TypeId, collections::HashMap, path::PathBuf};

use egui_tiles::{Tiles, Tree};
use uuid::Uuid;

use crate::{
    editor::{
        popup::{create_project::CreateProject, open_project::OpenProject, Popup},
        tabs::{
            code_editor::CodeEditorTab, console::ConsoleTab, file_explorer::FileExplorerTab,
            render_graph::RenderGraphTab, viewport::ViewportTab, Tab, TabViewer,
        },
    },
    egui_util::KeyModifiers,
    project::Project,
};

pub mod code_editor;
pub mod node_graph;
pub mod popup;
pub mod tabs;

pub enum EditorDragPayloadType {}

pub enum EditorDragPayload {
    CodeFile(Uuid, PathBuf),
    Folder(PathBuf),
}

pub struct Editor {
    tree: Option<Tree<Tab>>,
    popups: HashMap<TypeId, Box<dyn Popup>>,
    drag_payload: Option<EditorDragPayload>,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    pub fn new() -> Self {
        Self {
            tree: None,
            popups: HashMap::new(),
            drag_payload: None,
        }
    }

    fn build_tree(project: &Project) -> Tree<Tab> {
        let mut tiles = Tiles::default();

        let viewport_id = tiles.insert_pane(Tab::Viewport(ViewportTab::default()));
        let render_graph_id = tiles.insert_pane(Tab::RenderGraph(RenderGraphTab::default()));
        let console_id = tiles.insert_pane(Tab::Console(ConsoleTab::default()));
        let file_explorer_id = tiles.insert_pane(Tab::FileExplorer(FileExplorerTab::default()));

        let first_code_file = project.code_files.files_iter().next();

        let main_tabs = if let Some(first_code_file) = first_code_file {
            let code_editor_id =
                tiles.insert_pane(Tab::CodeEditor(CodeEditorTab::new(first_code_file.1)));
            egui_tiles::Tabs {
                children: vec![code_editor_id, render_graph_id],
                active: Some(code_editor_id),
            }
        } else {
            egui_tiles::Tabs {
                children: vec![render_graph_id],
                active: Some(render_graph_id),
            }
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
        tree
    }

    fn open_popup<T: Popup + 'static>(&mut self, popup: T) -> bool {
        let type_id = TypeId::of::<T>();

        #[allow(clippy::map_entry)]
        if !self.popups.contains_key(&type_id) {
            self.popups.insert(type_id, Box::new(popup));
            true
        } else {
            false
        }
    }

    fn popup_ui(&mut self, ctx: &egui::Context, project: &mut Option<Project>) {
        let mut to_remove = Vec::new();
        for popup in self.popups.values_mut() {
            if !popup.ui(ctx, project) {
                to_remove.push(popup.as_ref().type_id());
            }
        }

        for type_id in to_remove {
            self.popups.remove(&type_id);
        }
    }

    pub fn ui(
        &mut self,
        egui_ctx: &mut egui::Context,
        project: &mut Option<Project>,
        key_modifiers: &KeyModifiers,
    ) {
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

                            self.open_popup(CreateProject::default());
                        }

                        if ui
                            .button("Open Project")
                            .on_hover_text("Open an existing project")
                            .clicked()
                        {
                            ui.close();

                            self.open_popup(OpenProject::default());
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
                if let Some(project) = project {
                    if let Some(tree) = &mut self.tree {
                        tree.ui(
                            &mut TabViewer::new(key_modifiers, project, &mut self.drag_payload),
                            ui,
                        );
                    } else {
                        self.tree = Some(Self::build_tree(project));
                    }
                }
            });

        self.popup_ui(egui_ctx, project);

        if let (Some(ty), Some(project)) = (&self.drag_payload, &project) {
            let pointer_pos = egui_ctx.pointer_interact_pos();

            let painter = egui_ctx.layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("drag_preview"),
            ));

            let font_id = egui::FontId::proportional(14.0);

            if let Some(pos) = pointer_pos {
                match ty {
                    EditorDragPayload::CodeFile(id, path) => {
                        let icon = project.get_file_icon(path, *id);
                        let name = path
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();

                        painter.text(
                            pos + egui::vec2(16.0, 16.0),
                            egui::Align2::CENTER_CENTER,
                            format!("{} {}", icon, name),
                            font_id,
                            egui::Color32::WHITE,
                        );
                    }
                    EditorDragPayload::Folder(path) => {
                        let name = path
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();

                        painter.text(
                            pos + egui::vec2(16.0, 16.0),
                            egui::Align2::CENTER_CENTER,
                            format!("{} {}", egui_phosphor::regular::FOLDER, name),
                            font_id,
                            egui::Color32::WHITE,
                        );
                    }
                }
            }
        }

        // Make sure to clear the drag payload when the primary pointer is released
        if egui_ctx.input(|i| i.pointer.primary_released()) {
            self.drag_payload = None;
        }
    }
}
