use std::{any::TypeId, collections::HashMap, path::PathBuf};

use egui_tiles::{TileId, Tiles, Tree};
use uuid::Uuid;

use crate::{
    editor::{
        popup::{create_project::CreateProject, open_project::OpenProject, Popup},
        tabs::{
            code_editor::CodeEditorTab, console::ConsoleTab, file_explorer::FileExplorerTab,
            render_graph::RenderGraphTab, viewport::ViewportTab, Tab, TabViewer,
        },
    },
    egui_util::{EguiPass, KeyModifiers},
    project::Project,
    render_graph::RenderGraphInfo,
};

pub mod code_editor;
pub mod node_graph;
pub mod popup;
pub mod tabs;

pub enum EditorDragPayloadType {}

#[derive(Clone)]
pub enum EditorDragPayload {
    CodeFile(Uuid, PathBuf),
    Folder(PathBuf),
}

struct Tabs {
    tree: Tree<Tab>,
    last_focussed_code_editor: Option<TileId>,
}

impl Tabs {
    fn new(project: &Project, device: &wgpu::Device) -> Self {
        let mut tiles = Tiles::default();

        let viewport_id = tiles.insert_pane(Tab::Viewport(ViewportTab::new(device)));
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

        Self {
            tree,
            last_focussed_code_editor: None,
        }
    }

    pub fn get_container_and_tile_id(&self, target_tab: &Tab) -> Option<(TileId, TileId)> {
        for (tile_id, tile) in self.tree.tiles.iter() {
            if let egui_tiles::Tile::Container(egui_tiles::Container::Tabs(tabs)) = tile {
                for &child_id in &tabs.children {
                    if let Some(egui_tiles::Tile::Pane(tab)) = self.tree.tiles.get(child_id) {
                        if target_tab.variant_eq(tab) {
                            return Some((*tile_id, child_id));
                        }
                    }
                }
            }
        }

        None
    }

    pub fn get_focussed_code_editor(&mut self) -> Option<&mut CodeEditorTab> {
        let mut target_id = None;

        for (_tile_id, tile) in self.tree.tiles.iter() {
            if let egui_tiles::Tile::Container(egui_tiles::Container::Tabs(tabs)) = tile {
                for &child_id in &tabs.children {
                    if let Some(egui_tiles::Tile::Pane(Tab::CodeEditor(_))) =
                        self.tree.tiles.get(child_id)
                    {
                        if tabs.active == Some(child_id)
                            && self.last_focussed_code_editor == Some(child_id)
                        {
                            target_id = Some(child_id);
                            break;
                        }
                    }
                }
                if target_id.is_some() {
                    break;
                }
            }
        }

        if let Some(id) = target_id {
            if let Some(egui_tiles::Tile::Pane(Tab::CodeEditor(code_editor))) =
                self.tree.tiles.get_mut(id)
            {
                return Some(code_editor);
            }
        }

        None
    }
}

pub struct Editor {
    tabs: Option<Tabs>,
    popups: HashMap<TypeId, Box<dyn Popup>>,
    drag_payload: Option<EditorDragPayload>,
    viewport_texture: Option<(wgpu::TextureView, [u32; 2])>,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    pub fn new() -> Self {
        Self {
            tabs: None,
            popups: HashMap::new(),
            drag_payload: None,
            viewport_texture: None,
        }
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
        egui_pass: &mut EguiPass,
        project: &mut Option<Project>,
        key_modifiers: &KeyModifiers,
        rg_info: &mut RenderGraphInfo,
        device: &wgpu::Device,
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

                        if let (Some(tabs), Some(project)) = (&mut self.tabs, project.as_mut()) {
                            if let Some((file_explorer_container, file_explorer)) = tabs
                                .get_container_and_tile_id(&Tab::FileExplorer(
                                    FileExplorerTab::default(), // TODO: unclean overhead?
                                ))
                            {
                                ui.menu_button("New File", |ui| {
                                    let mut file_explorer_requires_focus = false;

                                    // Show options to create different file types
                                    if let Some(file_explorer_tab) =
                                        tabs.tree.tiles.get_mut(file_explorer).and_then(|tile| {
                                            if let egui_tiles::Tile::Pane(Tab::FileExplorer(
                                                explorer,
                                            )) = tile
                                            {
                                                Some(explorer)
                                            } else {
                                                None
                                            }
                                        })
                                    {
                                        file_explorer_requires_focus = file_explorer_tab
                                            .draw_create_file_menu_options(ui, project);
                                    }

                                    if file_explorer_requires_focus {
                                        // Focus on file explorer tab
                                        if let Some(file_explorer_container) = tabs
                                            .tree
                                            .tiles
                                            .get_mut(file_explorer_container)
                                            .and_then(|tile| {
                                                if let egui_tiles::Tile::Container(
                                                    egui_tiles::Container::Tabs(tabs),
                                                ) = tile
                                                {
                                                    Some(tabs)
                                                } else {
                                                    None
                                                }
                                            })
                                        {
                                            file_explorer_container.active = Some(file_explorer);
                                        }
                                    }
                                });
                            }
                        } else {
                            ui.add_enabled_ui(false, |ui| {
                                ui.menu_button("New File", |_| {});
                            });
                        }

                        let mut allow_save_file = false;
                        if let (Some(tabs), Some(project)) = (&mut self.tabs, project.as_mut()) {
                            if let Some(code_editor) = tabs.get_focussed_code_editor() {
                                allow_save_file = true;
                                if ui
                                    .button("Save File")
                                    .on_hover_text("Save the currently focussed code file")
                                    .clicked()
                                {
                                    ui.close();

                                    code_editor.save_to_project(project);
                                    // TODO: save
                                }
                            }
                        }
                        if !allow_save_file {
                            ui.add_enabled_ui(false, |ui| {
                                ui.button("Save File")
                                    .on_hover_text("Save the currently focussed code file");
                            });
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
                if let Some(project) = project {
                    if let Some(tabs) = &mut self.tabs {
                        let mut file_to_open = None;

                        tabs.tree.ui(
                            &mut TabViewer::new(
                                egui_pass,
                                key_modifiers,
                                project,
                                &mut self.drag_payload,
                                &mut file_to_open,
                                &mut tabs.last_focussed_code_editor,
                                rg_info,
                                &mut self.viewport_texture,
                                device,
                            ),
                            ui,
                        );

                        let tree = &mut tabs.tree;

                        if let Some(file_to_open) = file_to_open {
                            if project.code_files.get_file(file_to_open).is_some() {
                                // First, check if a code editor for this file already exists
                                let mut existing_tile = None;
                                let mut code_editor_container = None;
                                let mut first_tabs_container = None;

                                for (tile_id, tile) in tree.tiles.iter() {
                                    if let egui_tiles::Tile::Container(
                                        egui_tiles::Container::Tabs(tabs),
                                    ) = tile
                                    {
                                        if first_tabs_container.is_none() {
                                            first_tabs_container = Some(tile_id);
                                        }

                                        for &child_id in &tabs.children {
                                            if let Some(egui_tiles::Tile::Pane(Tab::CodeEditor(
                                                editor,
                                            ))) = tree.tiles.get(child_id)
                                            {
                                                if code_editor_container.is_none() {
                                                    code_editor_container = Some(tile_id);
                                                }

                                                if editor.id() == file_to_open {
                                                    existing_tile = Some((tile_id, child_id));
                                                    break;
                                                }
                                            }
                                        }

                                        if existing_tile.is_some() {
                                            break;
                                        }
                                    }
                                }

                                // Prefer container with code editors, fallback to any tabs container
                                let target_container =
                                    code_editor_container.or(first_tabs_container);

                                if let Some((&container_id, existing_id)) = existing_tile {
                                    // Focus the existing tab
                                    if let Some(egui_tiles::Tile::Container(
                                        egui_tiles::Container::Tabs(tabs),
                                    )) = tree.tiles.get_mut(container_id)
                                    {
                                        tabs.active = Some(existing_id);
                                    }
                                } else if let Some(&container_id) = target_container {
                                    // Create a new tab and add it to the container
                                    let code_file =
                                        project.code_files.get_file(file_to_open).unwrap();
                                    let new_tab = Tab::CodeEditor(CodeEditorTab::new(code_file));
                                    let new_tile_id = tree.tiles.insert_pane(new_tab);

                                    if let Some(egui_tiles::Tile::Container(
                                        egui_tiles::Container::Tabs(tabs),
                                    )) = tree.tiles.get_mut(container_id)
                                    {
                                        tabs.children.push(new_tile_id);
                                        tabs.active = Some(new_tile_id);
                                    }
                                }
                            } else {
                                log::warn!("Failed to open {} in code editor.", file_to_open);
                            }
                        }
                    } else {
                        self.tabs = Some(Tabs::new(project, device));
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

    pub fn viewport_texture(&self) -> &Option<(wgpu::TextureView, [u32; 2])> {
        &self.viewport_texture
    }
}
