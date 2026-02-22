use std::sync::Arc;

use crate::{
    editor::Editor,
    egui_util::KeyModifiers,
    project::{CodeFileType, Project},
    render_graph::compiled_render_graph::CompiledRenderGraph,
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

pub struct RenderPipeline {
    surface_config: wgpu::SurfaceConfiguration,
}

impl runtime::RenderPipeline<App> for RenderPipeline {
    fn required_limits() -> wgpu::Limits {
        wgpu::Limits {
            max_texture_dimension_2d: 1024 * 8,
            ..wgpu::Limits::downlevel_defaults()
        }
    }

    fn init(
        surface_config: wgpu::SurfaceConfiguration,
        _adapter: &wgpu::Adapter,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _window: Arc<winit::window::Window>,
    ) -> Self {
        Self { surface_config }
    }

    fn resize(
        &mut self,
        surface_config: wgpu::SurfaceConfiguration,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) {
        self.surface_config = surface_config;
    }

    fn render(
        &mut self,
        target_view: &wgpu::TextureView,
        target_format: wgpu::TextureFormat,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
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

        app.editor
            .ui(egui_ctx, &mut app.project, key_modifiers, device);

        if let Some(project) = &mut app.project {
            // TODO: cloning all sources here is slow
            let code_sources: Vec<(uuid::Uuid, String)> = project
                .code_files
                .files_iter()
                .filter(|(_, f)| f.ty() == CodeFileType::Fragment)
                .map(|(id, f)| (*id, f.source.clone()))
                .collect();

            let rg = project.render_graph_mut();
            rg.sync_graphics_shaders(&code_sources, device);
            rg.sync_dynamic_node_inputs();

            let resolution = [self.surface_config.width, self.surface_config.height];
            match rg.compile(resolution, device) {
                Ok(compiled_rg) => {
                    let encoder =
                        compiled_rg.record_command_encoder(device, target_view, target_format);

                    queue.submit(Some(encoder.finish()));
                }
                Err(e) => {
                    // TODO: send to console tab
                    log::warn!("Failed to compile rg: {}", e);
                }
            }
        }
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
