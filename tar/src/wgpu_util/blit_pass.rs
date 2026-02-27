use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

pub struct BlitPassParameters<'a> {
    pub src_view: &'a wgpu::TextureView,
    pub dst_view: &'a wgpu::TextureView,
    pub target_format: wgpu::TextureFormat,
    pub blending: Option<f32>,
}

thread_local! {
    static BLIT_PIPELINES: RefCell<HashMap<wgpu::TextureFormat, Arc<wgpu::RenderPipeline>>> = RefCell::new(HashMap::new());
}

pub fn encode_blit(
    parameters: &BlitPassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
) {
    let pipeline = BLIT_PIPELINES.with(|v| {
        let mut map = v.borrow_mut();
        map.entry(parameters.target_format)
            .or_insert_with(|| {
                let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("blit"),
                    source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                        "../../assets/shaders/blit.frag"
                    ))),
                });

                Arc::new(
                    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some(&format!("blit {:?}", parameters.target_format)),
                        layout: None,
                        vertex: wgpu::VertexState {
                            module: &module,
                            entry_point: Some("vs_main"),
                            buffers: &[],
                            compilation_options: Default::default(),
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: &module,
                            entry_point: Some("fs_main"),
                            compilation_options: Default::default(),
                            targets: &[Some(parameters.target_format.into())],
                        }),
                        primitive: wgpu::PrimitiveState::default(),
                        depth_stencil: None,
                        multisample: wgpu::MultisampleState::default(),
                        multiview: None,
                        cache: None,
                    }),
                )
            })
            .clone()
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(parameters.src_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });

    {
        let mut rpass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: parameters.dst_view,
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
        rpass.set_pipeline(&pipeline);
        rpass.set_bind_group(0, &bind_group, &[]);
        rpass.draw(0..3, 0..1);
    }
}
