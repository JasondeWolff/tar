use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct ConsoleTab {
    id: Uuid,
}

impl Default for ConsoleTab {
    fn default() -> Self {
        Self { id: Uuid::new_v4() }
    }
}

impl ConsoleTab {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {}
}
