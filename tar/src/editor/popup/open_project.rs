use crate::project::Project;

use super::Popup;

pub struct OpenProject;

impl Popup for OpenProject {
    fn ui(&mut self, _ctx: &egui::Context, project: &mut Option<Project>) -> bool {
        //let default_project_path = default_project_path();

        // if let Some(file_path) = rfd::FileDialog::new()
        //     .set_title("Select a Project")
        //     .set_directory(default_project_path)
        //     .add_filter("Box Project", &["box"])
        //     .pick_file()
        // {
        //     let project_file_path = file_path.to_string_lossy().to_string();
        //     if let Ok(project) = Project::load(project_file_path) {
        //         resources.insert::<Project>(project);
        //     }
        // }

        false
    }
}
