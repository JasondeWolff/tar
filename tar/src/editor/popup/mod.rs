use std::any::TypeId;

use crate::project::Project;

pub mod create_project;
pub mod open_project;

pub trait Popup: 'static {
    fn ui(&mut self, ctx: &egui::Context, project: &mut Option<Project>) -> bool;

    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}
