use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct FileExplorerTab {
    id: Uuid,
}

impl Default for FileExplorerTab {
    fn default() -> Self {
        Self { id: Uuid::new_v4() }
    }
}

impl FileExplorerTab {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {}
}
