use std::sync::Arc;

use egui_file_dialog::FileDialog;

use crate::project::{default_project_path, Project};

use super::Popup;

pub struct OpenProject {
    file_dialog: FileDialog,
}

impl Default for OpenProject {
    fn default() -> Self {
        let mut file_dialog = FileDialog::new()
            .add_file_filter(
                "Tar Project",
                Arc::new(|path| path == path.with_extension("tarproj")),
            )
            .default_file_filter("Tar Project");

        if let Some(default_project_path) = default_project_path() {
            file_dialog = file_dialog.initial_directory(default_project_path);
        }

        file_dialog.pick_file();

        Self { file_dialog }
    }
}

impl Popup for OpenProject {
    fn ui(&mut self, ctx: &egui::Context, project: &mut Option<Project>) -> bool {
        self.file_dialog.update(ctx);
        if let Some(picked) = self.file_dialog.take_picked() {
            match Project::load(&picked) {
                Ok(new_project) => {
                    *project = Some(new_project);
                }
                Err(e) => {
                    log::warn!("Failed to open project: {}", e);
                }
            }

            false
        } else {
            true
        }
    }
}
