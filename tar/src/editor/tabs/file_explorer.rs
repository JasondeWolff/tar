use egui_phosphor::regular as icons;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::project::{CodeFileType, Project};

#[derive(Debug, Clone, PartialEq)]
pub struct FileExplorerTab {
    id: Uuid,
    /// Tracks which folders are expanded (by their path)
    expanded_folders: HashMap<PathBuf, bool>,
}

impl Default for FileExplorerTab {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            expanded_folders: HashMap::new(),
        }
    }
}

impl FileExplorerTab {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, project: &mut Project) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(5.0); // Top padding
            ui.horizontal(|ui| {
                ui.add_space(10.0); // Left padding
                ui.vertical(|ui| {
                    self.render_directory(ui, project, Path::new(""), 0);
                });
            });
        });
    }

    fn render_directory(
        &mut self,
        ui: &mut egui::Ui,
        project: &mut Project,
        current_path: &Path,
        indent_level: usize,
    ) {
        // Collect folders and files at this level
        let mut folders: Vec<PathBuf> = Vec::new();
        let mut files_here: Vec<(Uuid, PathBuf)> = Vec::new();

        for (id, file) in project.code_files.files_iter() {
            let rel_path = file.relative_path();

            if let Some(parent) = rel_path.parent() {
                if parent == current_path {
                    // File is directly in this folder
                    files_here.push((*id, rel_path.clone()));
                } else if parent.starts_with(current_path) || current_path.as_os_str().is_empty() {
                    // File is in a subfolder - extract immediate child folder
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
                // File at root level (no parent)
                files_here.push((*id, rel_path.clone()));
            }
        }

        folders.sort();
        files_here.sort_by(|a, b| a.1.cmp(&b.1));

        let indent = indent_level as f32 * 16.0;

        // Render folders first
        for folder in folders {
            let folder_name = folder
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            let is_expanded = *self.expanded_folders.get(&folder).unwrap_or(&false);

            ui.horizontal(|ui| {
                ui.add_space(indent);

                let icon = if is_expanded {
                    icons::FOLDER_OPEN
                } else {
                    icons::FOLDER
                };

                let response = ui.selectable_label(false, format!("{} {}", icon, folder_name));

                if response.clicked() {
                    self.expanded_folders.insert(folder.clone(), !is_expanded);
                }
            });

            if is_expanded {
                self.render_directory(ui, project, &folder, indent_level + 1);
            }
        }

        // Render files
        for (id, path) in files_here {
            let file_name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            let icon = self.get_file_icon(&path, project, id);

            ui.horizontal(|ui| {
                ui.add_space(indent);

                if ui
                    .selectable_label(false, format!("{} {}", icon, file_name))
                    .clicked()
                {
                    // Handle file click - open in editor, etc.
                    // project.open_file(id);
                }
            });
        }
    }

    fn get_file_icon(&self, path: &Path, project: &Project, id: Uuid) -> &'static str {
        // Check by CodeFileType if available
        if let Some(file) = project.code_files.get_file(id) {
            return match file.ty() {
                CodeFileType::Fragment => icons::CUBE,
                CodeFileType::Compute => icons::CPU,
                CodeFileType::Shared => icons::SHARE_NETWORK,
            };
        }

        // Fallback: check by extension
        match path.extension().and_then(|s| s.to_str()) {
            Some("wgsl") => icons::FILE_CODE,
            Some("glsl") => icons::FILE_CODE,
            Some("json") => icons::BRACKETS_CURLY,
            Some("toml") | Some("yaml") | Some("yml") => icons::GEAR,
            _ => icons::FILE,
        }
    }
}
