use std::path::PathBuf;

use egui_file_dialog::FileDialog;

use crate::project::{self, Project};

use super::Popup;

pub struct CreateProject {
    pub project_name: String,
    pub project_path: PathBuf,
    is_first_frame: bool,

    file_dialog: Option<FileDialog>,
}

impl Default for CreateProject {
    fn default() -> Self {
        // Retreive the users documents folder, fallback to empty path
        let project_path = project::default_project_path().unwrap_or_default();

        Self {
            project_name: "new-project".to_owned(),
            project_path,
            is_first_frame: true,
            file_dialog: None,
        }
    }
}

impl Popup for CreateProject {
    fn ui(&mut self, ctx: &egui::Context, project: &mut Option<Project>) -> bool {
        let mut should_close = false;
        let mut open = true;

        // Place the window in the center of the screen with a fixed size
        let window_rect =
            egui::Rect::from_center_size(ctx.content_rect().center(), egui::vec2(400.0, 250.0));

        let resp = egui::Window::new("Create Project")
            .default_rect(window_rect)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                // Project name and path input fields
                ui.horizontal(|ui| {
                    ui.label("Project Name:");
                    ui.text_edit_singleline(&mut self.project_name);
                });

                ui.horizontal(|ui| {
                    ui.label("Project Path:");

                    let mut project_path_str = self.project_path.to_str().unwrap().to_string();
                    ui.text_edit_singleline(&mut project_path_str);
                    self.project_path = PathBuf::from(project_path_str);

                    if ui.button(egui_phosphor::regular::FOLDER_OPEN).clicked() {
                        let mut file_dialog =
                            FileDialog::new().initial_directory(PathBuf::from(&self.project_path));
                        file_dialog.pick_directory();

                        self.file_dialog = Some(file_dialog);
                    }
                });

                ui.add_space(10.0);

                let project_name = self.project_name.replace(" ", "_");
                let project_path = PathBuf::from(&self.project_path).join(&project_name);

                let project_file_path = project_path.join(&project_name).with_extension("tarproj");

                // Check if a current project file already exists
                let exists = std::fs::exists(&project_file_path).unwrap_or(true);

                ui.add_enabled_ui(!exists, |ui| {
                    if ui.button("Create").clicked() {
                        // Create new project struct from the input fields
                        let new_project = Project::new(&project_file_path);

                        // Create all necessary directories
                        if std::fs::create_dir_all(&project_path).is_ok() {
                            // Save the project to a file
                            if new_project.save().is_ok() {
                                *project = Some(new_project);
                            }
                        }

                        should_close = true;
                    }
                });

                if exists {
                    ui.label("A project with this name already exists in the specified path.");
                }
            });

        if let Some(file_dialog) = &mut self.file_dialog {
            file_dialog.update(ctx);
            if let Some(picked) = file_dialog.take_picked() {
                self.project_path = picked;
                self.file_dialog = None;
            }

            return true;
        }

        // Close the popup if the user clicks outside of it
        if !self.is_first_frame {
            if let Some(resp) = resp {
                let window_rect = resp.response.rect;

                if ctx.input(|i| i.pointer.any_click())
                    && ctx
                        .input(|i| i.pointer.latest_pos())
                        .is_some_and(|pos| !window_rect.contains(pos))
                {
                    should_close = true;
                }
            }
        }
        self.is_first_frame = false;

        open && !should_close
    }
}
