use std::sync::Arc;

use crate::{
    app::{Runtime, Static},
    code_editor::{syntax::Syntax, themes::ColorTheme, CodeEditor},
    egui_util::KeyModifiers,
    time::FpsCounter,
};

pub mod app;
pub mod code_editor;
pub mod egui_util;
pub mod time;
pub mod wgpu_util;

pub struct App {
    fps_counter: FpsCounter,
    code: CodeEditor,
}

const DEFAULT_CODE: &str = r#"@include tar/common.wgsl

fn main(tex_coords: vec2f) -> vec4f {
    let color = vec3f(tex_coords, 0.0);
    
    return vec4f(color, 1.0);
}
"#;

impl App {
    fn new() -> Self {
        Self {
            fps_counter: FpsCounter::new(),
            code: CodeEditor::new(DEFAULT_CODE, ColorTheme::GITHUB_DARK, Syntax::wgsl()),
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
        key_modifiers: &KeyModifiers,
        app: &mut App,
    ) {
        if app.fps_counter.update() {
            log::info!(
                "FPS {} (ms {:.2})",
                app.fps_counter.fps(),
                app.fps_counter.ms()
            );
        }

        egui::CentralPanel::default().show(egui_ctx, |ui| {
            app.code.ui(ui, key_modifiers);
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
