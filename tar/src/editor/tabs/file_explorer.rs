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

impl ExplorerItem {
    fn path(&self) -> &PathBuf {
        match self {
            ExplorerItem::Folder { path, .. } => path,
            ExplorerItem::File { path, .. } => path,
        }
    }

    fn name(&self) -> String {
        self.path()
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    }
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
    expanded_folders: HashMap<PathBuf, bool>,
    selected: Option<PathBuf>,
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

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show_viewport(ui, |ui, viewport| {
                self.draw_explorer(ui, viewport, project, drag_payload);
            });
    }

    fn selected_parent_dir(&self, project: &Project) -> PathBuf {
        match &self.selected {
            Some(selected) if !project.code_files.contains_file(selected) => selected.clone(),
            Some(selected) => selected.parent().unwrap().to_path_buf(),
            None => PathBuf::new(),
        }
    }

    fn file_id_for_path(&self, project: &Project, path: &Path) -> Option<Uuid> {
        project
            .code_files
            .files_iter()
            .find(|(_, f)| f.relative_path() == path)
            .map(|(id, _)| *id)
    }

    fn start_renaming(&mut self, path: PathBuf, name: String) {
        self.selected = Some(path.clone());
        self.renaming = Some(RenamingState {
            path,
            new_name: name,
            request_focus: true,
        });
    }

    fn expand_parent(&mut self, path: &Path) {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                self.expanded_folders.insert(parent.to_path_buf(), true);
            }
        }
    }

    fn draw_toolbar(&mut self, ui: &mut egui::Ui, project: &mut Project) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);

            self.draw_create_file_menu(ui, project);
            self.draw_create_folder_button(ui, project);

            if self.selected.is_some() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    self.draw_delete_button(ui, project);
                    self.draw_rename_button(ui);
                });
            }
        });
    }

    fn draw_create_file_menu(&mut self, ui: &mut egui::Ui, project: &mut Project) {
        ui.menu_button(icons::FILE_PLUS, |ui| {
            for file_type in CodeFileType::iter() {
                if ui.button(file_type.labeled_icon()).clicked() {
                    self.create_new_file(project, file_type);
                    ui.close();
                }
            }
        });
    }

    fn create_new_file(&mut self, project: &mut Project, file_type: CodeFileType) {
        let parent_dir = self.selected_parent_dir(project);
        let extension = file_type.file_extension();

        let mut path = parent_dir.join("new_shader").with_extension(extension);
        for i in 1..10000 {
            if !project.code_files.contains_file(&path) {
                break;
            }
            path = parent_dir
                .join(format!("new_shader{}", i))
                .with_extension(extension);
        }

        match project.code_files.create_file(path.clone(), file_type) {
            Ok(_) => {
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                self.start_renaming(path.clone(), name);
                self.expand_parent(&path);
            }
            Err(e) => log::error!("Failed to create file: {}", e),
        }
    }

    fn draw_create_folder_button(&mut self, ui: &mut egui::Ui, project: &mut Project) {
        if ui
            .button(icons::FOLDER_PLUS)
            .on_hover_text("New Folder")
            .clicked()
        {
            self.create_new_folder(project);
        }
    }

    fn create_new_folder(&mut self, project: &mut Project) {
        let parent_dir = self.selected_parent_dir(project);

        let mut path = parent_dir.join("new_folder");
        for i in 1..10000 {
            if !project.code_files.contains_folder(&path) {
                break;
            }
            path = parent_dir.join(format!("new_folder{}", i));
        }

        match project.code_files.create_folder(path.clone()) {
            Ok(()) => {
                let name = path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                self.start_renaming(path.clone(), name);
                self.expand_parent(&path);
            }
            Err(e) => log::error!("Failed to create folder: {}", e),
        }
    }

    fn draw_delete_button(&mut self, ui: &mut egui::Ui, project: &mut Project) {
        if ui.button(icons::TRASH).on_hover_text("Delete").clicked() {
            if let Some(selected) = self.selected.clone() {
                let result = match self.file_id_for_path(project, &selected) {
                    Some(id) => project.code_files.delete_file(id),
                    None => project.code_files.delete_folder(&selected),
                };

                if let Err(e) = result {
                    log::error!("Failed to delete: {}", e);
                }
                self.selected = None;
            }
        }
    }

    fn draw_rename_button(&mut self, ui: &mut egui::Ui) {
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

        let items = self.build_item_list(project, Path::new(""), 0);
        let total_rows = items.len();

        let content_height = total_rows as f32 * row_height + Self::TOP_PADDING;
        let content_width = ui.available_width();

        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(content_width, content_height.max(ui.available_height())),
            egui::Sense::click_and_drag(),
        );

        let painter = ui.painter_at(rect);

        let colors = ExplorerColors {
            selection: egui::Color32::from_rgb(40, 40, 70),
            hover: egui::Color32::from_rgb(50, 50, 55),
            text: ui.visuals().text_color(),
            selected_text: egui::Color32::WHITE,
            indent_line: ui.visuals().widgets.noninteractive.bg_stroke.color,
        };

        let hovered_row = self.get_hovered_row(ui, rect, row_height, items.len());

        let first_visible = ((viewport.min.y - Self::TOP_PADDING) / row_height)
            .floor()
            .max(0.0) as usize;
        let last_visible = ((viewport.max.y - Self::TOP_PADDING) / row_height).ceil() as usize + 1;

        let mut rename_action: Option<RenameAction> = None;

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

            let is_selected = self.selected.as_ref() == Some(item.path());
            let is_hovered = hovered_row == Some(row_idx);

            self.draw_row_background(&painter, row_rect, is_selected, is_hovered, &colors);
            self.draw_indent_guides(&painter, rect, row_rect, *indent_level, &colors);

            let text_x =
                rect.min.x + Self::LEFT_PADDING + *indent_level as f32 * Self::INDENT_WIDTH;
            let text_y = y + Self::ROW_SPACING / 2.0;

            let icon = self.get_item_icon(item, project);
            let is_renaming = self
                .renaming
                .as_ref()
                .map(|r| &r.path == item.path())
                .unwrap_or(false);

            if is_renaming {
                rename_action = self.draw_rename_row(
                    ui,
                    &painter,
                    &font_id,
                    item,
                    icon,
                    text_x,
                    text_y,
                    row_rect,
                    rect,
                    row_height,
                    is_selected,
                    &colors,
                );
            } else {
                self.draw_normal_row(
                    &painter,
                    &font_id,
                    icon,
                    &item.name(),
                    text_x,
                    text_y,
                    is_selected,
                    &colors,
                );
            }
        }

        self.handle_rename_action(rename_action, project);
        self.handle_interactions(
            ui,
            &response,
            rect,
            row_height,
            &items,
            project,
            drag_payload,
        );
    }

    fn get_hovered_row(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        row_height: f32,
        item_count: usize,
    ) -> Option<usize> {
        ui.input(|i| i.pointer.hover_pos()).and_then(|pos| {
            if rect.contains(pos) {
                let row_idx =
                    ((pos.y - rect.min.y - Self::TOP_PADDING) / row_height).floor() as usize;
                if row_idx < item_count {
                    return Some(row_idx);
                }
            }
            None
        })
    }

    fn draw_row_background(
        &self,
        painter: &egui::Painter,
        row_rect: egui::Rect,
        is_selected: bool,
        is_hovered: bool,
        colors: &ExplorerColors,
    ) {
        if is_selected {
            painter.rect_filled(row_rect, 0.0, colors.selection);
        } else if is_hovered {
            painter.rect_filled(row_rect, 0.0, colors.hover);
        }
    }

    fn draw_indent_guides(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        row_rect: egui::Rect,
        indent_level: usize,
        colors: &ExplorerColors,
    ) {
        for level in 0..indent_level {
            let line_x = rect.min.x
                + Self::LEFT_PADDING
                + level as f32 * Self::INDENT_WIDTH
                + Self::INDENT_WIDTH * 0.5;
            painter.line_segment(
                [
                    egui::pos2(line_x, row_rect.top()),
                    egui::pos2(line_x, row_rect.bottom()),
                ],
                egui::Stroke::new(1.0, colors.indent_line),
            );
        }
    }

    fn get_item_icon(&self, item: &ExplorerItem, project: &Project) -> &'static str {
        match item {
            ExplorerItem::Folder { is_expanded, .. } => {
                if *is_expanded {
                    icons::FOLDER_OPEN
                } else {
                    icons::FOLDER
                }
            }
            ExplorerItem::File { id, path } => project.get_file_icon(path, *id),
        }
    }

    fn draw_normal_row(
        &self,
        painter: &egui::Painter,
        font_id: &egui::FontId,
        icon: &str,
        name: &str,
        text_x: f32,
        text_y: f32,
        is_selected: bool,
        colors: &ExplorerColors,
    ) {
        let color = if is_selected {
            colors.selected_text
        } else {
            colors.text
        };

        painter.text(
            egui::pos2(text_x, text_y),
            egui::Align2::LEFT_TOP,
            format!("{} {}", icon, name),
            font_id.clone(),
            color,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_rename_row(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        font_id: &egui::FontId,
        item: &ExplorerItem,
        icon: &str,
        text_x: f32,
        text_y: f32,
        row_rect: egui::Rect,
        rect: egui::Rect,
        row_height: f32,
        is_selected: bool,
        colors: &ExplorerColors,
    ) -> Option<RenameAction> {
        let color = if is_selected {
            colors.selected_text
        } else {
            colors.text
        };

        let icon_galley = painter.layout_no_wrap(icon.to_string(), font_id.clone(), color);
        let icon_width = icon_galley.rect.width();
        painter.galley(egui::pos2(text_x, text_y), icon_galley, color);

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

        let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
        let escape_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));
        let lost_focus = response.lost_focus();

        if escape_pressed {
            Some(RenameAction::Cancel)
        } else if enter_pressed || (lost_focus && !escape_pressed) {
            Some(RenameAction::Confirm {
                old_path: item.path().clone(),
                new_name: renaming.new_name.clone(),
            })
        } else {
            None
        }
    }

    fn handle_rename_action(&mut self, action: Option<RenameAction>, project: &mut Project) {
        let Some(action) = action else { return };

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

                    let result = match self.file_id_for_path(project, &old_path) {
                        Some(id) => {
                            let new_path = match old_path.extension() {
                                Some(ext) => new_path.with_extension(ext),
                                None => new_path,
                            };
                            project
                                .code_files
                                .move_file(id, &new_path)
                                .map(|_| new_path)
                        }
                        None => project
                            .code_files
                            .move_folder(old_path, &new_path)
                            .map(|_| new_path),
                    };

                    match result {
                        Ok(new_path) => self.selected = Some(new_path),
                        Err(e) => log::warn!("Failed to rename: {}", e),
                    }
                }
                self.renaming = None;
            }
        }
    }

    fn handle_interactions(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: egui::Rect,
        row_height: f32,
        items: &[(ExplorerItem, usize)],
        project: &mut Project,
        drag_payload: &mut Option<EditorDragPayload>,
    ) {
        if response.clicked() {
            response.request_focus();
        }

        if let Some(pos) = response.interact_pointer_pos() {
            let row_idx = ((pos.y - rect.min.y - Self::TOP_PADDING) / row_height).floor() as usize;
            let item = items.get(row_idx).map(|(item, _)| item);

            if let Some(item) = item {
                if response.clicked() {
                    self.handle_item_click(item);
                } else if response.drag_started() {
                    self.handle_drag_start(item, drag_payload);
                }
            }

            if response.clicked() && item.is_none() {
                self.selected = None;
            }

            if ui.input(|i| i.pointer.primary_released()) {
                self.handle_drop(item, project, drag_payload);
            }
        }

        if !response.has_focus() && self.renaming.is_none() {
            self.selected = None;
        }
    }

    fn handle_item_click(&mut self, item: &ExplorerItem) {
        match item {
            ExplorerItem::Folder { path, is_expanded } => {
                self.expanded_folders.insert(path.clone(), !is_expanded);
                self.selected = Some(path.clone());
            }
            ExplorerItem::File { path, .. } => {
                self.selected = Some(path.clone());

                // TODO: open and focus on code editor
            }
        }
    }

    fn handle_drag_start(
        &mut self,
        item: &ExplorerItem,
        drag_payload: &mut Option<EditorDragPayload>,
    ) {
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

    fn handle_drop(
        &mut self,
        target_item: Option<&ExplorerItem>,
        project: &mut Project,
        drag_payload: &mut Option<EditorDragPayload>,
    ) {
        let Some(payload) = drag_payload.take() else {
            return;
        };

        let target_dir = match target_item {
            Some(ExplorerItem::Folder { path, .. }) => Some(path.clone()),
            Some(ExplorerItem::File { .. }) => None,
            None => Some(PathBuf::new()),
        };

        let Some(target_dir) = target_dir else { return };

        let result = match payload {
            EditorDragPayload::CodeFile(id, path) => {
                let name = path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let new_path = target_dir.join(name);
                project
                    .code_files
                    .move_file(id, &new_path)
                    .map(|_| new_path)
            }
            EditorDragPayload::Folder(path) => {
                let name = path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let new_path = target_dir.join(name);
                project
                    .code_files
                    .move_folder(path, &new_path)
                    .map(|_| new_path)
            }
        };

        match result {
            Ok(new_path) => self.selected = Some(new_path),
            Err(e) => log::warn!("Failed to move: {}", e),
        }
    }

    fn build_item_list(
        &self,
        project: &Project,
        current_path: &Path,
        indent_level: usize,
    ) -> Vec<(ExplorerItem, usize)> {
        let mut folders = self.collect_folders(project, current_path);
        let mut files = self.collect_files(project, current_path);

        folders.sort();
        files.sort_by(|a, b| a.1.cmp(&b.1));

        let mut result = Vec::new();

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
                result.extend(self.build_item_list(project, &folder, indent_level + 1));
            }
        }

        for (id, path) in files {
            result.push((ExplorerItem::File { id, path }, indent_level));
        }

        result
    }

    fn collect_folders(&self, project: &Project, current_path: &Path) -> Vec<PathBuf> {
        let mut folders = Vec::new();
        let is_root = current_path.as_os_str().is_empty();

        for (_, file) in project.code_files.files_iter() {
            if let Some(folder) =
                self.extract_child_folder(file.relative_path(), current_path, is_root)
            {
                if !folders.contains(&folder) {
                    folders.push(folder);
                }
            }
        }

        for extra_dir in project.code_files.extra_dirs_iter() {
            if let Some(folder) = self.extract_child_folder(extra_dir, current_path, is_root) {
                if !folders.contains(&folder) {
                    folders.push(folder);
                }
            }

            if let Some(parent) = extra_dir.parent() {
                if parent == current_path && !folders.contains(extra_dir) {
                    folders.push(extra_dir.clone());
                }
            } else if is_root && !folders.contains(extra_dir) {
                folders.push(extra_dir.clone());
            }
        }

        folders
    }

    fn extract_child_folder(
        &self,
        path: &Path,
        current_path: &Path,
        is_root: bool,
    ) -> Option<PathBuf> {
        let parent = path.parent()?;

        if parent.starts_with(current_path) || is_root {
            let components: Vec<_> = if is_root {
                path.components().collect()
            } else {
                path.strip_prefix(current_path).ok()?.components().collect()
            };

            if components.len() > 1 {
                return Some(current_path.join(components[0].as_os_str()));
            }
        }

        None
    }

    fn collect_files(&self, project: &Project, current_path: &Path) -> Vec<(Uuid, PathBuf)> {
        let is_root = current_path.as_os_str().is_empty();

        project
            .code_files
            .files_iter()
            .filter_map(|(id, file)| {
                let rel_path = file.relative_path();
                let parent = rel_path.parent();

                let is_direct_child = match parent {
                    Some(p) => p == current_path,
                    None => is_root,
                };

                if is_direct_child {
                    Some((*id, rel_path.clone()))
                } else {
                    None
                }
            })
            .collect()
    }
}

struct ExplorerColors {
    selection: egui::Color32,
    hover: egui::Color32,
    text: egui::Color32,
    selected_text: egui::Color32,
    indent_line: egui::Color32,
}
