use egui_phosphor::regular as icons;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::editor::EditorDragPayload;
use crate::project::Project;

#[derive(Clone)]
enum ExplorerItem {
    Folder { path: PathBuf, is_expanded: bool },
    File { id: Uuid, path: PathBuf },
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileExplorerTab {
    id: Uuid,

    /// Tracks which folders are expanded (by their path)
    expanded_folders: HashMap<PathBuf, bool>,

    selected: Option<PathBuf>,
}

impl Default for FileExplorerTab {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            expanded_folders: HashMap::new(),
            selected: None,
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
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show_viewport(ui, |ui, viewport| {
                self.draw_explorer(ui, viewport, project, drag_payload);
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

        // Handle clicks & drags
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

                                if let Err(e) = project.code_files.move_file(id, new_relative_path)
                                {
                                    log::warn!("Failed to move file: {}", e);
                                }
                            }
                            EditorDragPayload::Folder(path) => {
                                let name = path
                                    .file_name()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_default();

                                let new_relative_path = new_relative_dir.join(name);

                                if let Err(e) =
                                    project.code_files.move_folder(path, new_relative_path)
                                {
                                    log::warn!("Failed to move folder: {}", e);
                                }
                            }
                        }
                    }
                }
            }
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
