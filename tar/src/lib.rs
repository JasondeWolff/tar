use std::sync::Arc;

use crate::app::QwrlApp;

pub mod app;
pub mod egui_renderer;
pub mod wgpu_util;

pub struct TarRenderPipeline {}

impl app::RenderPipeline for TarRenderPipeline {
    fn required_limits() -> wgpu::Limits {
        wgpu::Limits {
            max_texture_dimension_2d: 1024 * 8,
            ..wgpu::Limits::downlevel_defaults()
        }
    }

    fn init(
        _config: wgpu::SurfaceConfiguration,
        _adapter: &wgpu::Adapter,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _window: Arc<winit::window::Window>,
    ) -> Self {
        Self {}
    }

    fn resize(
        &mut self,
        _config: wgpu::SurfaceConfiguration,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) {
    }

    fn render(
        &mut self,
        _target_view: &wgpu::TextureView,
        _target_format: wgpu::TextureFormat,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        egui_ctx: &mut egui::Context,
    ) {
        egui::Window::new("My Window").show(egui_ctx, |ui| {
            ui.label("Hello World!");
        });
    }
}

pub fn internal_main(#[cfg(target_os = "android")] android_app: android_activity::AndroidApp) {
    let app = QwrlApp::new();
    app.run::<TarRenderPipeline>(
        #[cfg(target_os = "android")]
        android_app,
    );
}
