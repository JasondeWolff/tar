use std::sync::Arc;

use crate::{
    editor::Editor,
    egui_util::{EguiPass, KeyModifiers},
    project::{CodeFileType, Project},
    render_graph::{compiled_render_graph::CompiledRenderGraph, RenderGraphInfo},
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
    rg_info: RenderGraphInfo,
}

impl App {
    fn new() -> Self {
        Self {
            fps_counter: FpsCounter::new(),
            editor: Editor::new(),
            project: None,
            rg_info: RenderGraphInfo::default(),
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

        app.rg_info.dirty = false;
        app.editor.ui(
            egui_ctx,
            egui_pass,
            &mut app.project,
            key_modifiers,
            &mut app.rg_info,
            device,
        );

        if let Some(project) = &mut app.project {
            // TODO: cloning all sources here is slow
            let code_sources: Vec<(uuid::Uuid, String)> = project
                .code_files
                .files_iter()
                .filter(|(_, f)| f.ty() == CodeFileType::Fragment)
                .map(|(id, f)| (*id, f.source().to_string()))
                .collect();

            let rg = project.render_graph_mut();

            // Update all shaders and retrieve if there are any dirty shaders
            let shaders_dirty = rg.sync_graphics_shaders(&code_sources, device);
            if shaders_dirty {
                // If there were dirty shaders we need to update the dynamic node inputs
                rg.sync_dynamic_node_inputs();
            }

            // Get the output texture, format and its resolution we want to render to
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

            // Check if the resolution has changed compared to what the graph was compiled with
            let viewport_resolution_dirty = if let Some(compiled_rg) = &self.compiled_rg {
                *compiled_rg.screen_size() != rg_target_resolution
            } else {
                false
            };

            // If any of the shaders are dirty, the graph itself or the target resolution, we recompile
            if shaders_dirty || app.rg_info.dirty || viewport_resolution_dirty {
                log::info!(
                    "RECOMPILE RG shaders={} rg={} resolution={}!",
                    shaders_dirty,
                    app.rg_info.dirty,
                    viewport_resolution_dirty
                );

                match rg.compile(rg_target_resolution, &mut app.rg_info, device) {
                    Ok(compiled_rg) => {
                        // Cache the compiled graph when succesful
                        self.compiled_rg = Some(compiled_rg);
                        app.rg_info.error = None;
                    }
                    Err(e) => {
                        let err_msg = format!("Failed to compile rg: {}", e);

                        log::warn!("{}", err_msg);
                        app.rg_info.error = Some(err_msg);
                    }
                }
            }

            // Execute the render graph
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
