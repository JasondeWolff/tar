use std::sync::Arc;

use crate::{
    editor::Editor,
    egui_util::KeyModifiers,
    project::{CodeFileType, Project},
    render_graph::executor::RenderGraphExecutor,
    runtime::{Runtime, Static},
    time::FpsCounter,
    wgpu_util::PipelineDatabase,
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
    executor: RenderGraphExecutor,
    pipeline_database: PipelineDatabase,
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
        config: wgpu::SurfaceConfiguration,
        _adapter: &wgpu::Adapter,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _window: Arc<winit::window::Window>,
    ) -> Self {
        Self {
            executor: RenderGraphExecutor::new(),
            pipeline_database: PipelineDatabase::new(),
            surface_config: config,
        }
    }

    fn resize(
        &mut self,
        config: wgpu::SurfaceConfiguration,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) {
        self.surface_config = config;
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

        app.editor.ui(egui_ctx, &mut app.project, key_modifiers);

        if let Some(project) = &mut app.project {
            // Sync render graph shaders and dynamic inputs
            let code_sources: Vec<(uuid::Uuid, String)> = project
                .code_files
                .files_iter()
                .filter(|(_, f)| f.ty() == CodeFileType::Fragment)
                .map(|(id, f)| (*id, f.source.clone()))
                .collect();
            let rg = project.render_graph_mut();
            rg.sync_shaders(&code_sources, device);
            rg.sync_dynamic_inputs();

            // Compile and execute the render graph
            let screen_size = [self.surface_config.width, self.surface_config.height];
            match render_graph::compiled::compile(
                rg.graph(),
                &rg.graph_state().shader_cache,
                screen_size,
            ) {
                Ok(compiled) => {
                    self.executor.allocate(&compiled, device);
                    self.executor.build_pipelines(
                        &compiled,
                        &rg.graph_state().shader_cache,
                        device,
                    );
                    self.executor.execute(
                        &compiled,
                        &rg.graph_state().shader_cache,
                        device,
                        queue,
                        target_view,
                        target_format,
                        &mut self.pipeline_database,
                    );
                }
                Err(e) => {
                    log::warn!("Render graph compile error: {}", e);
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
