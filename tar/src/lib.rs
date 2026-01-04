use std::sync::Arc;

use crate::{
    app::{Runtime, Static},
    code_editor::{syntax::Syntax, themes::ColorTheme, CodeEditor},
};

pub mod app;
pub mod code_editor;
pub mod egui_util;
pub mod wgpu_util;

pub struct App {
    code: CodeEditor,
}

impl App {
    fn new() -> Self {
        Self {
            code: CodeEditor::new(
                "let pos: vec2 = vec2(0.0);",
                ColorTheme::GITHUB_DARK,
                Syntax::rust(),
            ),
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

            app.code.ui(ui);

            // CodeEditor::default()
            //     .id_source("code editor")
            //     .with_rows(12)
            //     .with_fontsize(14.0)
            //     .with_theme(ColorTheme::GITHUB_DARK)
            //     .with_syntax(Syntax::rust())
            //     .with_numlines(true)
            //     .show_with_completer(ui, &mut app.code, &mut app.completer);
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
