use std::sync::Arc;

use egui_code_editor::{CodeEditor, ColorTheme, Completer, Syntax};

use crate::app::{Runtime, Static};

pub mod app;
pub mod egui_util;
pub mod wgpu_util;

pub struct App {
    code: String,
    completer: Completer,
}

impl App {
    fn new() -> Self {
        Self {
            code: String::new(),
            completer: Completer::new_with_syntax(&Syntax::rust()),
        }
    }
}

pub struct RenderPipeline {}

impl app::RenderPipeline<App> for RenderPipeline {
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
        app: &mut App,
    ) {
        egui::CentralPanel::default().show(egui_ctx, |ui| {
            // ui.add(egui::TextEdit::multiline(&mut self.code).desired_rows(30));

            CodeEditor::default()
                .id_source("code editor")
                .with_rows(12)
                .with_fontsize(14.0)
                .with_theme(ColorTheme::GITHUB_DARK)
                .with_syntax(Syntax::rust())
                .with_numlines(true)
                .show_with_completer(ui, &mut app.code, &mut app.completer);
        });
    }
}

pub fn internal_main(#[cfg(target_os = "android")] android_app: android_activity::AndroidApp) {
    Static::init();

    let app = App::new();

    Runtime::new(app).run::<RenderPipeline>(
        #[cfg(target_os = "android")]
        android_app,
    );
}
