use std::sync::Arc;

use egui_code_editor::{Completer, Syntax};

use crate::app::{QwrlApp, Static};

pub mod app;
pub mod egui_util;
pub mod wgpu_util;

pub struct TarRenderPipeline {
    code: String,
    completer: Completer,
}

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
        Self {
            code: String::new(),
            completer: Completer::new_with_syntax(&Syntax::rust()),
        }
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
        egui::CentralPanel::default().show(egui_ctx, |ui| {
            ui.add(egui::TextEdit::multiline(&mut self.code).desired_rows(30));
        });
    }
}

pub fn internal_main(#[cfg(target_os = "android")] android_app: android_activity::AndroidApp) {
    Static::init();

    let app = QwrlApp::new();
    app.run::<TarRenderPipeline>(
        #[cfg(target_os = "android")]
        android_app,
    );
}
