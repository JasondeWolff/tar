use std::sync::Arc;

use crate::{
    editor::Editor,
    egui_util::{EguiPass, KeyModifiers},
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
    compiled_rg: Option<CompiledRenderGraph>,
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
        Self {
            surface_config,
            compiled_rg: None,
        }
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
        egui_pass: &mut EguiPass,
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

        let mut render_graph_dirty = false;
        app.editor.ui(
            egui_ctx,
            egui_pass,
            &mut app.project,
            key_modifiers,
            &mut render_graph_dirty,
            device,
        );

        if let Some(project) = &mut app.project {
            // TODO: cloning all sources here is slow
            let code_sources: Vec<(uuid::Uuid, String)> = project
                .code_files
                .files_iter()
                .filter(|(_, f)| f.ty() == CodeFileType::Fragment)
                .map(|(id, f)| (*id, f.source.clone()))
                .collect();

            let rg = project.render_graph_mut();
            let shaders_dirty = rg.sync_graphics_shaders(&code_sources, device);

            if shaders_dirty {
                rg.sync_dynamic_node_inputs();
            }

            let (rg_target_view, rg_target_format, rg_target_resolution) = if let Some((
                editor_viewport_texture,
                resolution,
            )) =
                app.editor.viewport_texture()
            {
                (
                    editor_viewport_texture,
                    wgpu::TextureFormat::Rgba16Float,
                    *resolution,
                )
            } else {
                (
                    target_view,
                    target_format,
                    [self.surface_config.width, self.surface_config.height],
                )
            };

            let viewport_resolution_dirty = if let Some(compiled_rg) = &self.compiled_rg {
                *compiled_rg.screen_size() != rg_target_resolution
            } else {
                false
            };

            if shaders_dirty || render_graph_dirty || viewport_resolution_dirty {
                log::info!(
                    "RECOMPILE RG shaders={} rg={} resolution={}!",
                    shaders_dirty,
                    render_graph_dirty,
                    viewport_resolution_dirty
                );

                match rg.compile(rg_target_resolution, device) {
                    Ok(compiled_rg) => {
                        self.compiled_rg = Some(compiled_rg);
                    }
                    Err(e) => {
                        // TODO: send to console tab
                        log::warn!("Failed to compile rg: {}", e);
                    }
                }
            }

            if let Some(compiled_rg) = &self.compiled_rg {
                let encoder =
                    compiled_rg.record_command_encoder(device, rg_target_view, rg_target_format);

                queue.submit(Some(encoder.finish()));
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
