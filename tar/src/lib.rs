use std::sync::Arc;

use crate::{
    editor::Editor,
    egui_util::KeyModifiers,
    project::Project,
    runtime::{Runtime, Static},
    time::FpsCounter,
};

pub mod editor;
pub mod egui_util;
pub mod project;
pub mod render_graph;
pub mod runtime;
pub mod time;
pub mod wgpu_util;

pub struct App {
    fps_counter: FpsCounter,
    editor: Editor,
    project: Option<Project>,
}

impl App {
    fn new() -> Self {
        Self {
            fps_counter: FpsCounter::new(),
            editor: Editor::new(),
            project: None,
        }
    }
}

pub struct RenderPipeline {}

impl runtime::RenderPipeline<App> for RenderPipeline {
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

        app.editor.ui(egui_ctx, &mut app.project, key_modifiers);
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
