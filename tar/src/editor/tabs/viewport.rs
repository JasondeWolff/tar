use egui::epaint;
use uuid::Uuid;

use crate::egui_util::EguiPass;

#[derive(Debug, Clone, PartialEq)]
pub struct ViewportTab {
    id: Uuid,
    viewport_texture: wgpu::Texture,
    viewport_texture_ui_id: epaint::TextureId,
}

impl ViewportTab {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            id: Uuid::new_v4(),
            viewport_texture: Self::rebuild_texture(32, 32, device),
            viewport_texture_ui_id: epaint::TextureId::default(),
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn rebuild_texture(width: u32, height: u32, device: &wgpu::Device) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            format: wgpu::TextureFormat::Rgba16Float,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            view_formats: &[],
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        })
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        egui_pass: &mut EguiPass,
        viewport_texture: &mut Option<wgpu::TextureView>,
        device: &wgpu::Device,
    ) {
        let size = ui.available_size();
        let width = (size.x.ceil() as u32).max(1);
        let height = (size.y.ceil() as u32).max(1);

        let rebuild =
            self.viewport_texture.width() != width || self.viewport_texture.height() != height;
        if rebuild {
            self.viewport_texture = Self::rebuild_texture(width, height, device);
        }

        if rebuild || viewport_texture.is_none() {
            let texture_view = self
                .viewport_texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            if self.viewport_texture_ui_id != epaint::TextureId::default() {
                egui_pass.free_texture(&self.viewport_texture_ui_id);
            }
            self.viewport_texture_ui_id =
                egui_pass.register_native_texture(device, &texture_view, wgpu::FilterMode::Linear);

            *viewport_texture = Some(texture_view);
        }

        ui.image((self.viewport_texture_ui_id, size));
    }
}
