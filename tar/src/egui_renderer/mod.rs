use egui::epaint;
use egui_winit::State;
use wgpu::{Device, TextureFormat, TextureView};
use winit::event::WindowEvent;
use winit::window::Window;

mod renderer;
pub use renderer::{Renderer, ScreenDescriptor};

pub struct EguiPass {
    context: Option<egui::Context>,
    state: State,
    renderer: Renderer,
}

impl EguiPass {
    pub fn new(
        output_color_format: TextureFormat,
        msaa_samples: u32,
        window: &Window,
        device: &Device,
    ) -> Self {
        let egui_context = egui::Context::default();

        let egui_state = egui_winit::State::new(
            egui_context.clone(),
            egui_context.viewport_id(),
            &window,
            None,
            None,
            None,
        );

        let egui_renderer = Renderer::new(device, output_color_format, None, msaa_samples, true);

        Self {
            context: Some(egui_context),
            state: egui_state,
            renderer: egui_renderer,
        }
    }

    pub fn handle_window_event(&mut self, window: &Window, event: &WindowEvent) {
        let _ = self.state.on_window_event(window, event);
    }

    pub fn begin_frame(&mut self, window: &Window) -> egui::Context {
        let raw_input = self.state.take_egui_input(window);

        let context = self.context.take().expect("Frame was not ended.");
        context.begin_pass(raw_input);
        context
    }

    pub fn end_frame(&mut self, context: egui::Context) {
        assert!(self.context.is_none());
        self.context = Some(context);
    }

    pub fn encode(
        &mut self,
        window: &Window,
        window_surface_view: &TextureView,
        screen_descriptor: ScreenDescriptor,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        command_encoder: &mut wgpu::CommandEncoder,
    ) {
        let context = self.context.as_ref().expect("Frame was not ended.");

        let full_output = context.end_pass();

        self.state
            .handle_platform_output(window, full_output.platform_output);

        let tris = context.tessellate(full_output.shapes, full_output.pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }

        {
            self.renderer
                .update_buffers(device, queue, command_encoder, &tris, &screen_descriptor);

            let rpass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: window_surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                label: Some("egui"),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.renderer
                .render(&mut rpass.forget_lifetime(), &tris, &screen_descriptor);
        }

        for texture_id in &full_output.textures_delta.free {
            self.renderer.free_texture(texture_id);
        }
    }

    pub fn register_native_texture(
        &mut self,
        device: &wgpu::Device,
        texture: &wgpu::TextureView,
        texture_filter: wgpu::FilterMode,
    ) -> epaint::TextureId {
        self.renderer
            .register_native_texture(device, texture, texture_filter)
    }

    pub fn free_texture(&mut self, id: &epaint::TextureId) {
        self.renderer.free_texture(id)
    }
}
