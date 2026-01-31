use egui_phosphor::regular as icons;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use strum::IntoEnumIterator;
use uuid::Uuid;

use crate::editor::EditorDragPayload;
use crate::project::{CodeFileType, Project};

#[derive(Clone)]
enum ExplorerItem {
    Folder { path: PathBuf, is_expanded: bool },
    File { id: Uuid, path: PathBuf },
}

#[derive(Debug, Clone, PartialEq)]
struct RenamingState {
    path: PathBuf,
    new_name: String,
    request_focus: bool,
}

enum RenameAction {
    Cancel,
    Confirm { old_path: PathBuf, new_name: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileExplorerTab {
    id: Uuid,

    /// Tracks which folders are expanded (by their path)
    expanded_folders: HashMap<PathBuf, bool>,

    selected: Option<PathBuf>,

    /// State for renaming a file or folder
    renaming: Option<RenamingState>,
}

impl Default for FileExplorerTab {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            expanded_folders: HashMap::new(),
            selected: None,
            renaming: None,
        }
    }
}

impl FileExplorerTab {
    const INDENT_WIDTH: f32 = 16.0;
    const LEFT_PADDING: f32 = 10.0;
    const TOP_PADDING: f32 = 5.0;
    const ROW_SPACING: f32 = 4.0;

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        project: &mut Project,
        drag_payload: &mut Option<EditorDragPayload>,
    ) {
        ui.add_space(4.0);
        self.draw_toolbar(ui, project);
        ui.separator();

        // File explorer with scrolling
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show_viewport(ui, |ui, viewport| {
                self.draw_explorer(ui, viewport, project, drag_payload);
            });
    }

    fn draw_toolbar(&mut self, ui: &mut egui::Ui, project: &mut Project) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);

            ui.menu_button(icons::FILE_PLUS, |ui| {
                for code_file_type in CodeFileType::iter() {
                    if ui.button(code_file_type.labeled_icon()).clicked() {
                        let new_relative_file_dir = if let Some(selected) = &self.selected {
                            let is_dir = !project.code_files.contains_file(selected);

                            if is_dir {
                                selected.clone()
                            } else {
                                selected.parent().unwrap().to_path_buf()
                            }
                        } else {
                            PathBuf::new()
                        };

                        let mut new_relative_file_path = new_relative_file_dir
                            .join("new_shader")
                            .with_extension(code_file_type.file_extension());
                        for i in 1..10000 {
                            if !project.code_files.contains_file(&new_relative_file_path) {
                                break;
                            }

                            new_relative_file_path = new_relative_file_dir
                                .join(format!("new_shader{}", i))
                                .with_extension(code_file_type.file_extension());
                        }

                        match project
                            .code_files
                            .create_file(new_relative_file_path.clone(), code_file_type)
                        {
                            Ok(_id) => {
                                // Select the new file and start renaming
                                let name = new_relative_file_path
                                    .file_stem()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_default();
                                self.selected = Some(new_relative_file_path.clone());
                                self.renaming = Some(RenamingState {
                                    path: new_relative_file_path,
                                    new_name: name,
                                    request_focus: true,
                                });
                                // Expand parent folder if needed
                                if let Some(parent) =
                                    self.selected.as_ref().and_then(|p| p.parent())
                                {
                                    if !parent.as_os_str().is_empty() {
                                        self.expanded_folders.insert(parent.to_path_buf(), true);
                                    }
                                }
                            }
                            Err(e) => log::error!("Failed to create file: {}", e),
                        };

                        ui.close();
                    }
                }
            });

            // Create folder button (always visible)
            if ui
                .button(icons::FOLDER_PLUS)
                .on_hover_text("New Folder")
                .clicked()
            {
                // TODO: Hook up create folder logic
            }

            if self.selected.is_some() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);

                    if ui.button(icons::TRASH).on_hover_text("Delete").clicked() {
                        // TODO: Hook up delete logic
                    }

                    if ui
                        .button(icons::PENCIL_SIMPLE)
                        .on_hover_text("Rename")
                        .clicked()
                    {
                        if let Some(selected) = &self.selected {
                            let name = selected
                                .file_name()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_default();
                            self.renaming = Some(RenamingState {
                                path: selected.clone(),
                                new_name: name,
                                request_focus: true,
                            });
                        }
                    }
                });
            }
        });
    }

    fn draw_explorer(
        &mut self,
        ui: &mut egui::Ui,
        viewport: egui::Rect,
        project: &mut Project,
        drag_payload: &mut Option<EditorDragPayload>,
    ) {
        let font_id = egui::FontId::proportional(14.0);
        let text_height = ui.text_style_height(&egui::TextStyle::Body);
        let row_height = text_height + Self::ROW_SPACING;

        // Build flattened list of visible items
        let items = self.build_item_list(project, Path::new(""), 0);
        let total_rows = items.len();

        // Calculate content size
        let content_height = total_rows as f32 * row_height + Self::TOP_PADDING;
        let content_width = ui.available_width();

        // Allocate the full content rect
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(content_width, content_height.max(ui.available_height())),
            egui::Sense::click_and_drag(),
        );

        let painter = ui.painter_at(rect);

        // Colors
        let selection_color = egui::Color32::from_rgb(40, 40, 70);
        let hover_color = egui::Color32::from_rgb(50, 50, 55);
        let text_color = ui.visuals().text_color();
        let selected_text_color = egui::Color32::WHITE;
        let line_color = ui.visuals().widgets.noninteractive.bg_stroke.color;

        // Get hover position
        let hover_pos = ui.input(|i| i.pointer.hover_pos());
        let hovered_row = hover_pos.and_then(|pos| {
            if rect.contains(pos) {
                let row_idx =
                    ((pos.y - rect.min.y - Self::TOP_PADDING) / row_height).floor() as usize;
                if row_idx < items.len() {
                    return Some(row_idx);
                }
            }
            None
        });

        // Calculate visible rows
        let first_visible = ((viewport.min.y - Self::TOP_PADDING) / row_height)
            .floor()
            .max(0.0) as usize;
        let last_visible = ((viewport.max.y - Self::TOP_PADDING) / row_height).ceil() as usize + 1;

        // Track rename action to process after the loop
        let mut rename_action: Option<RenameAction> = None;

        // Draw visible rows
        for (row_idx, (item, indent_level)) in items
            .iter()
            .enumerate()
            .skip(first_visible)
            .take(last_visible - first_visible)
        {
            let y = rect.min.y + Self::TOP_PADDING + row_idx as f32 * row_height;
            let row_rect = egui::Rect::from_min_size(
                egui::pos2(rect.min.x, y),
                egui::vec2(rect.width(), row_height),
            );

            let item_path = match item {
                ExplorerItem::Folder { path, .. } => path,
                ExplorerItem::File { path, .. } => path,
            };
            let is_selected = self.selected.as_ref() == Some(item_path);
            let is_hovered = hovered_row == Some(row_idx);

            // Draw hover or selection background (full width)
            if is_selected {
                painter.rect_filled(row_rect, 0.0, selection_color);
            } else if is_hovered {
                painter.rect_filled(row_rect, 0.0, hover_color);
            }

            // Draw indent guides
            for level in 0..*indent_level {
                let line_x = rect.min.x
                    + Self::LEFT_PADDING
                    + level as f32 * Self::INDENT_WIDTH
                    + Self::INDENT_WIDTH * 0.5;
                painter.line_segment(
                    [
                        egui::pos2(line_x, row_rect.top()),
                        egui::pos2(line_x, row_rect.bottom()),
                    ],
                    egui::Stroke::new(1.0, line_color),
                );
            }

            // Draw icon and text (vertically centered in row)
            let text_x =
                rect.min.x + Self::LEFT_PADDING + *indent_level as f32 * Self::INDENT_WIDTH;
            let text_y = y + Self::ROW_SPACING / 2.0;
            let (icon, name) = match item {
                ExplorerItem::Folder { path, is_expanded } => {
                    let icon = if *is_expanded {
                        icons::FOLDER_OPEN
                    } else {
                        icons::FOLDER
                    };
                    let name = path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    (icon, name)
                }
                ExplorerItem::File { id, path } => {
                    let icon = project.get_file_icon(path, *id);
                    let name = path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    (icon, name)
                }
            };

            let is_renaming = self
                .renaming
                .as_ref()
                .map(|r| &r.path == item_path)
                .unwrap_or(false);

            if is_renaming {
                // Draw only the icon
                let color = if is_selected {
                    selected_text_color
                } else {
                    text_color
                };

                let icon_galley = painter.layout_no_wrap(icon.to_string(), font_id.clone(), color);
                let icon_width = icon_galley.rect.width();
                painter.galley(egui::pos2(text_x, text_y), icon_galley, color);

                // Draw TextEdit for the name
                let text_edit_x = text_x + icon_width + 4.0;
                let text_edit_rect = egui::Rect::from_min_size(
                    egui::pos2(text_edit_x, row_rect.top() + 1.0),
                    egui::vec2(rect.right() - text_edit_x - 8.0, row_height - 2.0),
                );

                let renaming = self.renaming.as_mut().unwrap();

                let text_edit = egui::TextEdit::singleline(&mut renaming.new_name)
                    .font(font_id.clone())
                    .frame(true)
                    .margin(egui::vec2(4.0, 2.0));

                let response = ui.put(text_edit_rect, text_edit);

                if renaming.request_focus {
                    response.request_focus();
                    // Select all text
                    if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), response.id) {
                        state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::two(
                                egui::text::CCursor::new(0),
                                egui::text::CCursor::new(renaming.new_name.len()),
                            )));
                        state.store(ui.ctx(), response.id);
                    }
                    renaming.request_focus = false;
                }

                // Check for Enter (confirm) or Escape (cancel)
                let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let escape_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));
                let lost_focus = response.lost_focus();

                if escape_pressed {
                    // Cancel renaming
                    rename_action = Some(RenameAction::Cancel);
                } else if enter_pressed || (lost_focus && !escape_pressed) {
                    // Confirm renaming
                    rename_action = Some(RenameAction::Confirm {
                        old_path: item_path.clone(),
                        new_name: renaming.new_name.clone(),
                    });
                }
            } else {
                let label = format!("{} {}", icon, name);
                let color = if is_selected {
                    selected_text_color
                } else {
                    text_color
                };

                painter.text(
                    egui::pos2(text_x, text_y),
                    egui::Align2::LEFT_TOP,
                    label,
                    font_id.clone(),
                    color,
                );
            }
        }

        // Handle rename action after the loop
        if let Some(action) = rename_action {
            match action {
                RenameAction::Cancel => {
                    self.renaming = None;
                }
                RenameAction::Confirm { old_path, new_name } => {
                    let new_name = new_name.trim();
                    if !new_name.is_empty() {
                        let new_path = old_path
                            .parent()
                            .map(|p| p.join(new_name))
                            .unwrap_or_else(|| PathBuf::from(new_name));

                        // Check if it's a file or folder and get file id if it's a file
                        let file_id = project
                            .code_files
                            .files_iter()
                            .find(|(_, f)| f.relative_path() == &old_path)
                            .map(|(id, _)| *id);

                        if let Some(id) = file_id {
                            // It's a file - preserve extension
                            let extension = old_path
                                .extension()
                                .map(|e| e.to_string_lossy().to_string());
                            let new_path = if let Some(ext) = extension {
                                new_path.with_extension(ext)
                            } else {
                                new_path
                            };

                            if let Err(e) = project.code_files.move_file(id, &new_path) {
                                log::warn!("Failed to rename file: {}", e);
                            } else {
                                self.selected = Some(new_path);
                            }
                        } else {
                            // It's a folder
                            if let Err(e) = project.code_files.move_folder(old_path, &new_path) {
                                log::warn!("Failed to rename folder: {}", e);
                            } else {
                                self.selected = Some(new_path);
                            }
                        }
                    }
                    self.renaming = None;
                }
            }
        }

        if response.clicked() {
            response.request_focus();
        }

        if let Some(pos) = response.interact_pointer_pos() {
            let row_idx = ((pos.y - rect.min.y - Self::TOP_PADDING) / row_height).floor() as usize;

            let item = if row_idx < items.len() {
                Some(&items[row_idx].0)
            } else {
                None
            };

            if let Some(item) = item {
                if response.clicked() {
                    match item {
                        ExplorerItem::Folder { path, is_expanded } => {
                            self.expanded_folders.insert(path.clone(), !is_expanded);
                            self.selected = Some(path.clone());
                        }
                        ExplorerItem::File { path, .. } => {
                            self.selected = Some(path.clone());
                        }
                    }
                } else if response.drag_started() {
                    match item {
                        ExplorerItem::Folder { path, .. } => {
                            *drag_payload = Some(EditorDragPayload::Folder(path.clone()));
                            self.selected = Some(path.clone());
                        }
                        ExplorerItem::File { id, path } => {
                            *drag_payload = Some(EditorDragPayload::CodeFile(*id, path.clone()));
                            self.selected = Some(path.clone());
                        }
                    }
                }
            }

            if response.clicked() && item.is_none() {
                self.selected = None;
            }

            if ui.input(|i| i.pointer.primary_released()) {
                if let Some(drag_payload) = drag_payload.take() {
                    // Get new relative dir, a payload cannot be dropped onto a file, returning None
                    let new_relative_dir = match item {
                        Some(item) => match item {
                            ExplorerItem::File { .. } => None,
                            ExplorerItem::Folder { path, .. } => Some(path.clone()),
                        },
                        None => Some(PathBuf::new()),
                    };

                    if let Some(new_relative_dir) = new_relative_dir {
                        match drag_payload {
                            EditorDragPayload::CodeFile(id, path) => {
                                let name = path
                                    .file_name()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_default();

                                let new_relative_path = new_relative_dir.join(name);

                                if let Err(e) = project.code_files.move_file(id, &new_relative_path)
                                {
                                    log::warn!("Failed to move file: {}", e);
                                } else {
                                    self.selected = Some(new_relative_path);
                                }
                            }
                            EditorDragPayload::Folder(path) => {
                                let name = path
                                    .file_name()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_default();

                                let new_relative_path = new_relative_dir.join(name);

                                if let Err(e) =
                                    project.code_files.move_folder(path, &new_relative_path)
                                {
                                    log::warn!("Failed to move folder: {}", e);
                                } else {
                                    self.selected = Some(new_relative_path);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Don't clear selection if we're renaming (TextEdit has focus instead)
        if !response.has_focus() && self.renaming.is_none() {
            self.selected = None;
        }
    }

    fn build_item_list(
        &self,
        project: &Project,
        current_path: &Path,
        indent_level: usize,
    ) -> Vec<(ExplorerItem, usize)> {
        let mut result = Vec::new();

        // Collect folders and files at this level
        let mut folders: Vec<PathBuf> = Vec::new();
        let mut files_here: Vec<(Uuid, PathBuf)> = Vec::new();

        for (id, file) in project.code_files.files_iter() {
            let rel_path = file.relative_path();

            if let Some(parent) = rel_path.parent() {
                if parent == current_path {
                    files_here.push((*id, rel_path.clone()));
                } else if parent.starts_with(current_path) || current_path.as_os_str().is_empty() {
                    let components: Vec<_> = if current_path.as_os_str().is_empty() {
                        rel_path.components().collect()
                    } else {
                        rel_path
                            .strip_prefix(current_path)
                            .unwrap()
                            .components()
                            .collect()
                    };

                    if components.len() > 1 {
                        let folder_name = current_path.join(components[0].as_os_str());
                        if !folders.contains(&folder_name) {
                            folders.push(folder_name);
                        }
                    }
                }
            } else if current_path.as_os_str().is_empty() {
                files_here.push((*id, rel_path.clone()));
            }
        }

        folders.sort();
        files_here.sort_by(|a, b| a.1.cmp(&b.1));

        // Add folders
        for folder in folders {
            let is_expanded = *self.expanded_folders.get(&folder).unwrap_or(&false);
            result.push((
                ExplorerItem::Folder {
                    path: folder.clone(),
                    is_expanded,
                },
                indent_level,
            ));

            if is_expanded {
                let children = self.build_item_list(project, &folder, indent_level + 1);
                result.extend(children);
            }
        }

        // Add files
        for (id, path) in files_here {
            result.push((ExplorerItem::File { id, path }, indent_level));
        }

        result
    }
}
