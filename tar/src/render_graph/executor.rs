use std::collections::HashMap;

use uuid::Uuid;

use crate::{
    render_graph::{
        compiled::CompiledRenderGraph,
        shader::Shader,
    },
    wgpu_util::{blit_pass, PipelineDatabase},
};

pub struct RenderGraphExecutor {
    /// Allocated GPU textures, indexed by TextureHandle.
    allocated_textures: Vec<wgpu::Texture>,
    allocated_views: Vec<wgpu::TextureView>,

    /// Cached render pipelines per shader Uuid.
    pipelines: HashMap<Uuid, wgpu::RenderPipeline>,

    /// A default sampler shared across all passes.
    default_sampler: Option<wgpu::Sampler>,
}

impl RenderGraphExecutor {
    pub fn new() -> Self {
        Self {
            allocated_textures: Vec::new(),
            allocated_views: Vec::new(),
            pipelines: HashMap::new(),
            default_sampler: None,
        }
    }

    /// Allocate GPU textures based on the compiled graph.
    pub fn allocate(&mut self, compiled: &CompiledRenderGraph, device: &wgpu::Device) {
        self.allocated_textures.clear();
        self.allocated_views.clear();

        if self.default_sampler.is_none() {
            self.default_sampler = Some(device.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }));
        }

        for tex_alloc in &compiled.textures {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("rg_tex_{}", tex_alloc.handle.0)),
                size: wgpu::Extent3d {
                    width: tex_alloc.width.max(1),
                    height: tex_alloc.height.max(1),
                    depth_or_array_layers: tex_alloc.depth_or_layers.max(1),
                },
                mip_level_count: tex_alloc.mip_levels.max(1),
                sample_count: 1,
                dimension: tex_alloc.dimension,
                format: tex_alloc.format,
                usage: tex_alloc.usage,
                view_formats: &[tex_alloc.format],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.allocated_textures.push(texture);
            self.allocated_views.push(view);
        }
    }

    /// Build render pipelines for each pass using layout: None (auto-derive from shader).
    pub fn build_pipelines(
        &mut self,
        compiled: &CompiledRenderGraph,
        shader_cache: &HashMap<Uuid, Shader>,
        device: &wgpu::Device,
    ) {
        self.pipelines.clear();

        for pass in &compiled.passes {
            if self.pipelines.contains_key(&pass.shader_id) {
                continue;
            }

            let shader = match shader_cache.get(&pass.shader_id) {
                Some(s) => s,
                None => continue,
            };

            let shader_module = match shader.shader_module() {
                Some(m) => m,
                None => continue,
            };

            let output_format = compiled.textures[pass.output_texture.0].format;

            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&format!("rg_pass_{}", pass.shader_id)),
                layout: None, // Auto-derive from shader
                vertex: wgpu::VertexState {
                    module: shader_module,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: shader_module,
                    entry_point: Some("main"),
                    compilation_options: Default::default(),
                    targets: &[Some(output_format.into())],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

            self.pipelines.insert(pass.shader_id, pipeline);
        }
    }

    /// Execute all passes, then blit the display output to the target view.
    pub fn execute(
        &self,
        compiled: &CompiledRenderGraph,
        shader_cache: &HashMap<Uuid, Shader>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_view: &wgpu::TextureView,
        target_format: wgpu::TextureFormat,
        pipeline_database: &mut PipelineDatabase,
    ) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render_graph_execute"),
        });

        let sampler = match &self.default_sampler {
            Some(s) => s,
            None => return,
        };

        for pass in &compiled.passes {
            let pipeline = match self.pipelines.get(&pass.shader_id) {
                Some(p) => p,
                None => continue,
            };

            let shader = match shader_cache.get(&pass.shader_id) {
                Some(s) => s,
                None => continue,
            };

            // Build bind group entries
            let mut entries: Vec<wgpu::BindGroupEntry> = Vec::new();

            for &(_set, binding, tex_handle) in &pass.input_bindings {
                entries.push(wgpu::BindGroupEntry {
                    binding,
                    resource: wgpu::BindingResource::TextureView(
                        &self.allocated_views[tex_handle.0],
                    ),
                });
            }

            // Add sampler entries (auto-inject for any Sampler bindings in the shader)
            for b in shader.get_bindings() {
                if b.resource_type == "Sampler" && b.set == 0 {
                    entries.push(wgpu::BindGroupEntry {
                        binding: b.binding,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    });
                }
            }

            // Only create bind group if there are entries
            let bind_group = if !entries.is_empty() {
                let bind_group_layout = pipeline.get_bind_group_layout(0);
                Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &bind_group_layout,
                    entries: &entries,
                }))
            } else {
                None
            };

            let output_view = &self.allocated_views[pass.output_texture.0];

            {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("rg_render_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: output_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                rpass.set_pipeline(pipeline);
                if let Some(bg) = &bind_group {
                    rpass.set_bind_group(0, bg, &[]);
                }
                rpass.draw(0..3, 0..1); // fullscreen triangle
            }
        }

        // Blit display output to target
        if let Some(display) = &compiled.display_out {
            let src_view = &self.allocated_views[display.input_texture.0];
            blit_pass::encode_blit(
                &blit_pass::BlitPassParameters {
                    src_view,
                    dst_view: target_view,
                    target_format,
                    blending: None,
                },
                device,
                &mut encoder,
                pipeline_database,
            );
        }

        queue.submit(Some(encoder.finish()));
    }
}
