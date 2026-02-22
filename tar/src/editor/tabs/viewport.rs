use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct ViewportTab {
    id: Uuid,
}

impl Default for ViewportTab {
    fn default() -> Self {
        Self { id: Uuid::new_v4() }
    }
}

impl ViewportTab {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        viewport_texture: &mut Option<wgpu::Texture>,
        device: &wgpu::Device,
    ) {
        // Get available size
        // Create viewport texture if not present
        // If present, recreate if available size doesn't match current resolution
    }
}
