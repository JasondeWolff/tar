use std::collections::HashMap;

use anyhow::{anyhow, bail};
use uuid::Uuid;

use crate::{
    editor::node_graph::{NodeId, OutputId},
    render_graph::{shader::Shader, RgDataType, RgGraph, RgNodeTemplate, RgValueType},
    wgpu_util::blit_pass,
};

#[derive(Clone, Copy)]
struct BufferHandle(usize);
#[derive(Clone, Copy)]
struct TextureHandle(usize);

// enum InputBindingData {
//     Buffer(BufferHandle),
//     Texture(TextureHandle),
// }

// struct InputBinding {
//     set: u32,
//     binding: u32,
//     data: InputBindingData,
// }

struct CompiledGraphicsPass {
    pub node_id: NodeId,
    pub shader_id: Uuid,

    pub pipeline: wgpu::RenderPipeline,
    pub bind_group: Option<wgpu::BindGroup>,
    pub render_target_texture: TextureHandle,
}

pub struct CompiledRenderGraph {
    screen_size: [u32; 2],

    buffers: Vec<wgpu::Buffer>,
    textures: Vec<wgpu::Texture>,
    texture_views: Vec<wgpu::TextureView>,

    graphics_passes: Vec<CompiledGraphicsPass>,
    display_output: TextureHandle,
}

/// Topologically sort the nodes in a render graph using Kahn's algorithm.
/// Returns nodes in dependency order (sources first, sinks last).
pub fn topological_sort(graph: &RgGraph) -> anyhow::Result<Vec<NodeId>> {
    let mut in_degree: HashMap<NodeId, usize> = HashMap::new();
    let mut dependents: HashMap<NodeId, Vec<NodeId>> = HashMap::new();

    for node_id in graph.iter_nodes() {
        in_degree.entry(node_id).or_insert(0);
    }

    for (input_id, output_id) in graph.iter_connections() {
        let consumer_node = graph.get_input(input_id).node;
        let producer_node = graph.get_output(output_id).node;

        if producer_node != consumer_node {
            *in_degree.entry(consumer_node).or_insert(0) += 1;
            dependents
                .entry(producer_node)
                .or_default()
                .push(consumer_node);
        }
    }

    // Kahn's algorithm
    let mut queue: Vec<NodeId> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&id, _)| id)
        .collect();
    let mut sorted = Vec::new();

    while let Some(node) = queue.pop() {
        sorted.push(node);
        if let Some(deps) = dependents.get(&node) {
            for &dep in deps {
                let deg = in_degree.get_mut(&dep).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push(dep);
                }
            }
        }
    }

    if sorted.len() != in_degree.len() {
        bail!("Cycle detected in render graph".to_string());
    }

    Ok(sorted)
}

/// Helper to read a value from an input parameter on a node.
fn read_input_value(graph: &RgGraph, node_id: NodeId, name: &str) -> anyhow::Result<RgValueType> {
    Ok(graph.get_input(graph[node_id].get_input(name)?).value)
}

impl CompiledRenderGraph {
    pub fn new(
        graph: &RgGraph,
        shader_cache: &HashMap<Uuid, Shader>,
        screen_size: [u32; 2],
        device: &wgpu::Device,
    ) -> anyhow::Result<Self> {
        let mut buffers = Vec::new();
        let mut textures = Vec::new();
        let mut texture_views = Vec::new();
        let mut graphics_passes = Vec::new();
        let mut display_output = None;

        let nodes = topological_sort(graph)?;

        let mut output_buffer_handles: HashMap<OutputId, BufferHandle> = HashMap::new();
        let mut output_texture_handles: HashMap<OutputId, TextureHandle> = HashMap::new();

        for &node_id in &nodes {
            let mut build_tex = |width: u32,
                                 height: u32,
                                 array_layers: u32,
                                 dimension,
                                 mip_level_count: u32,
                                 format,
                                 usage| {
                let handle = TextureHandle(textures.len());

                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(&format!("rg texture {}", handle.0)),
                    size: wgpu::Extent3d {
                        width: width.max(1),
                        height: height.max(1),
                        depth_or_array_layers: array_layers.max(1),
                    },
                    mip_level_count: mip_level_count.max(1),
                    format,
                    sample_count: 1,
                    dimension,
                    view_formats: &[],
                    usage,
                });
                let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

                textures.push(texture);
                texture_views.push(texture_view);
                handle
            };

            let mut build_buffer = |size: u64| {
                let handle = BufferHandle(buffers.len());

                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("rg buffer {}", handle.0)),
                    size: size.max(1),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::UNIFORM,
                    mapped_at_creation: false,
                });

                buffers.push(buffer);
                handle
            };

            let template = graph[node_id].user_data.0;

            match template {
                RgNodeTemplate::ScreenTex => {
                    let resolution = *read_input_value(graph, node_id, "resolution")?
                        .as_screen_tex_resolution()?;
                    let mip_level_count = *read_input_value(graph, node_id, "mips")?.as_uint()?;
                    let format =
                        *read_input_value(graph, node_id, "format")?.as_texture_format()?;
                    let usage = *read_input_value(graph, node_id, "usage")?.as_texture_usage()?;
                    let _persistent = read_input_value(graph, node_id, "persistent")?.as_bool()?;

                    let [width, height] = resolution.resolve(screen_size);

                    let handle = build_tex(
                        width,
                        height,
                        1,
                        wgpu::TextureDimension::D2,
                        mip_level_count,
                        format.into(),
                        usage.into(),
                    );

                    if let Ok(output_id) = graph[node_id].get_output("tex") {
                        output_texture_handles.insert(output_id, handle);
                    }
                }
                RgNodeTemplate::HistoryScreenTex => {
                    let resolution = *read_input_value(graph, node_id, "resolution")?
                        .as_screen_tex_resolution()?;
                    let mip_level_count = *read_input_value(graph, node_id, "mips")?.as_uint()?;
                    let format =
                        *read_input_value(graph, node_id, "format")?.as_texture_format()?;
                    let usage = *read_input_value(graph, node_id, "usage")?.as_texture_usage()?;
                    let _persistent = read_input_value(graph, node_id, "persistent")?.as_bool()?;

                    let [width, height] = resolution.resolve(screen_size);

                    let current_handle = build_tex(
                        width,
                        height,
                        1,
                        wgpu::TextureDimension::D2,
                        mip_level_count,
                        format.into(),
                        usage.into(),
                    );
                    let previous_handle = build_tex(
                        width,
                        height,
                        1,
                        wgpu::TextureDimension::D2,
                        mip_level_count,
                        format.into(),
                        usage.into(),
                    );

                    if let Ok(output_id) = graph[node_id].get_output("current tex") {
                        output_texture_handles.insert(output_id, current_handle);
                    }
                    if let Ok(output_id) = graph[node_id].get_output("previous tex") {
                        output_texture_handles.insert(output_id, previous_handle);
                    }
                }
                RgNodeTemplate::Tex2D => {
                    let [width, height] =
                        *read_input_value(graph, node_id, "resolution")?.as_uint2()?;
                    let mip_level_count = *read_input_value(graph, node_id, "mips")?.as_uint()?;
                    let format =
                        *read_input_value(graph, node_id, "format")?.as_texture_format()?;
                    let usage = *read_input_value(graph, node_id, "usage")?.as_texture_usage()?;
                    let _persistent = read_input_value(graph, node_id, "persistent")?.as_bool()?;

                    let handle = build_tex(
                        width,
                        height,
                        1,
                        wgpu::TextureDimension::D2,
                        mip_level_count,
                        format.into(),
                        usage.into(),
                    );

                    if let Ok(output_id) = graph[node_id].get_output("tex") {
                        output_texture_handles.insert(output_id, handle);
                    }
                }
                RgNodeTemplate::HistoryTex2D => {
                    let [width, height] =
                        *read_input_value(graph, node_id, "resolution")?.as_uint2()?;
                    let mip_level_count = *read_input_value(graph, node_id, "mips")?.as_uint()?;
                    let format =
                        *read_input_value(graph, node_id, "format")?.as_texture_format()?;
                    let usage = *read_input_value(graph, node_id, "usage")?.as_texture_usage()?;

                    let current_handle = build_tex(
                        width,
                        height,
                        1,
                        wgpu::TextureDimension::D2,
                        mip_level_count,
                        format.into(),
                        usage.into(),
                    );
                    let previous_handle = build_tex(
                        width,
                        height,
                        1,
                        wgpu::TextureDimension::D2,
                        mip_level_count,
                        format.into(),
                        usage.into(),
                    );

                    if let Ok(output_id) = graph[node_id].get_output("current tex") {
                        output_texture_handles.insert(output_id, current_handle);
                    }
                    if let Ok(output_id) = graph[node_id].get_output("previous tex") {
                        output_texture_handles.insert(output_id, previous_handle);
                    }
                }
                RgNodeTemplate::Tex2DArray => {
                    let [width, height] =
                        *read_input_value(graph, node_id, "resolution")?.as_uint2()?;
                    let array_count = *read_input_value(graph, node_id, "count")?.as_uint()?;
                    let mip_level_count = *read_input_value(graph, node_id, "mips")?.as_uint()?;
                    let format =
                        *read_input_value(graph, node_id, "format")?.as_texture_format()?;
                    let usage = *read_input_value(graph, node_id, "usage")?.as_texture_usage()?;
                    let _persistent = read_input_value(graph, node_id, "persistent")?.as_bool()?;

                    let handle = build_tex(
                        width,
                        height,
                        array_count,
                        wgpu::TextureDimension::D2,
                        mip_level_count,
                        format.into(),
                        usage.into(),
                    );

                    if let Ok(output_id) = graph[node_id].get_output("tex") {
                        output_texture_handles.insert(output_id, handle);
                    }
                }
                RgNodeTemplate::Tex3D => {
                    let [width, height, depth] =
                        *read_input_value(graph, node_id, "resolution")?.as_uint3()?;
                    let mip_level_count = *read_input_value(graph, node_id, "mips")?.as_uint()?;
                    let format =
                        *read_input_value(graph, node_id, "format")?.as_texture_format()?;
                    let usage = *read_input_value(graph, node_id, "usage")?.as_texture_usage()?;
                    let _persistent = read_input_value(graph, node_id, "persistent")?.as_bool()?;

                    let handle = build_tex(
                        width,
                        height,
                        depth,
                        wgpu::TextureDimension::D3,
                        mip_level_count,
                        format.into(),
                        usage.into(),
                    );

                    if let Ok(output_id) = graph[node_id].get_output("tex") {
                        output_texture_handles.insert(output_id, handle);
                    }
                }
                RgNodeTemplate::HistoryTex3D => {
                    let [width, height, depth] =
                        *read_input_value(graph, node_id, "resolution")?.as_uint3()?;
                    let mip_level_count = *read_input_value(graph, node_id, "mips")?.as_uint()?;
                    let format =
                        *read_input_value(graph, node_id, "format")?.as_texture_format()?;
                    let usage = *read_input_value(graph, node_id, "usage")?.as_texture_usage()?;

                    let current_handle = build_tex(
                        width,
                        height,
                        depth,
                        wgpu::TextureDimension::D3,
                        mip_level_count,
                        format.into(),
                        usage.into(),
                    );
                    let previous_handle = build_tex(
                        width,
                        height,
                        depth,
                        wgpu::TextureDimension::D3,
                        mip_level_count,
                        format.into(),
                        usage.into(),
                    );

                    if let Ok(output_id) = graph[node_id].get_output("current tex") {
                        output_texture_handles.insert(output_id, current_handle);
                    }
                    if let Ok(output_id) = graph[node_id].get_output("previous tex") {
                        output_texture_handles.insert(output_id, previous_handle);
                    }
                }
                RgNodeTemplate::Buffer => {
                    let size = *read_input_value(graph, node_id, "size")?.as_uint()?;
                    let _persistent = read_input_value(graph, node_id, "persistent")?.as_bool()?;

                    let handle = build_buffer(size as u64);

                    if let Ok(output_id) = graph[node_id].get_output("buf") {
                        output_buffer_handles.insert(output_id, handle);
                    }
                }
                RgNodeTemplate::HistoryBuffer => {
                    let size = *read_input_value(graph, node_id, "size")?.as_uint()?;

                    let current_handle = build_buffer(size as u64);
                    let previous_handle = build_buffer(size as u64);

                    if let Ok(output_id) = graph[node_id].get_output("current buf") {
                        output_buffer_handles.insert(output_id, current_handle);
                    }
                    if let Ok(output_id) = graph[node_id].get_output("previous buf") {
                        output_buffer_handles.insert(output_id, previous_handle);
                    }
                }

                RgNodeTemplate::GraphicsPass => {
                    let shader_id = *read_input_value(graph, node_id, "code")?.as_code_file()?;
                    let shader_id = shader_id.ok_or(anyhow!("Unassigned code file"))?;

                    // Resolve the render target from the "in" connection.
                    let in_tex = *read_input_value(graph, node_id, "render target")?.as_tex2d()?;
                    let in_input_id = graph[node_id].get_input("render target")?;
                    let render_target_handle = graph
                        .connection(in_input_id)
                        .and_then(|out| output_texture_handles.get(&out).copied())
                        .ok_or(anyhow!(
                            "GraphicsPass 'render target' not connected to a texture"
                        ))?;

                    let shader = match shader_cache.get(&shader_id) {
                        Some(s) if s.shader_module().is_some() => s,
                        _ => bail!("Invalid shader"), // Shader not compiled or has errors
                    };

                    let render_target_format: wgpu::TextureFormat = in_tex.format.into();

                    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some(&format!("rg pipeline {}", shader_id)),
                        layout: None,
                        vertex: wgpu::VertexState {
                            module: shader.shader_module().as_ref().unwrap(),
                            entry_point: Some("vs_main"),
                            buffers: &[],
                            compilation_options: Default::default(),
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: shader.shader_module().as_ref().unwrap(),
                            entry_point: Some("fs_main"),
                            compilation_options: Default::default(),
                            targets: &[Some(render_target_format.into())],
                        }),
                        primitive: wgpu::PrimitiveState::default(),
                        depth_stencil: None,
                        multisample: wgpu::MultisampleState::default(),
                        multiview: None,
                        cache: None,
                    });

                    // Separate the index lookups so we can borrow texture_views / buffers
                    // after the closures are no longer needed.
                    let mut tex_entries: Vec<(u32, usize)> = Vec::new(); // (binding, tex_idx)
                    let mut buf_entries: Vec<(u32, usize)> = Vec::new(); // (binding, buf_idx)

                    for binding in shader.get_bindings() {
                        match binding.resource_type {
                            RgDataType::Tex2D | RgDataType::Tex2DArray | RgDataType::Tex3D => {
                                if let Ok(input_id) = graph[node_id].get_input(&binding.name) {
                                    if let Some(connected_output) = graph.connection(input_id) {
                                        if let Some(tex_handle) =
                                            output_texture_handles.get(&connected_output)
                                        {
                                            tex_entries.push((binding.binding, tex_handle.0));
                                        }
                                    }
                                }
                            }
                            RgDataType::Buffer => {
                                if let Ok(input_id) = graph[node_id].get_input(&binding.name) {
                                    if let Some(connected_output) = graph.connection(input_id) {
                                        if let Some(buf_handle) =
                                            output_buffer_handles.get(&connected_output)
                                        {
                                            buf_entries.push((binding.binding, buf_handle.0));
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }

                    // Now build the actual wgpu entries (borrows texture_views / buffers directly).
                    let mut entries: Vec<wgpu::BindGroupEntry> = Vec::new();
                    for (binding, idx) in &tex_entries {
                        entries.push(wgpu::BindGroupEntry {
                            binding: *binding,
                            resource: wgpu::BindingResource::TextureView(&texture_views[*idx]),
                        });
                    }
                    for (binding, idx) in &buf_entries {
                        entries.push(wgpu::BindGroupEntry {
                            binding: *binding,
                            resource: buffers[*idx].as_entire_binding(),
                        });
                    }

                    let bind_group = if !entries.is_empty() {
                        Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some(&format!("rg bind group {}", shader_id)),
                            layout: &pipeline.get_bind_group_layout(0),
                            entries: &entries,
                        }))
                    } else {
                        None
                    };

                    graphics_passes.push(CompiledGraphicsPass {
                        node_id,
                        shader_id,
                        pipeline,
                        bind_group,
                        render_target_texture: render_target_handle,
                    });

                    if let Ok(output_id) = graph[node_id].get_output("render target") {
                        output_texture_handles.insert(output_id, render_target_handle);
                    }
                }
                RgNodeTemplate::DisplayOut => {
                    if let Ok(input_id) = graph[node_id].get_input("in") {
                        if let Some(connected_output) = graph.connection(input_id) {
                            if let Some(&tex_handle) = output_texture_handles.get(&connected_output)
                            {
                                display_output = Some(tex_handle);
                            } else {
                                bail!("DisplayOut in texture is invalid");
                            }
                        } else {
                            bail!("DisplayOut in is not connected");
                        }
                    } else {
                        bail!("No input 'in' found for DisplayOut")
                    }
                }
            }
        }

        let display_output = display_output.ok_or(anyhow!("No display output"))?;

        Ok(Self {
            screen_size,
            buffers,
            textures,
            texture_views,
            graphics_passes,
            display_output,
        })
    }

    pub fn screen_size(&self) -> &[u32; 2] {
        &self.screen_size
    }

    pub fn record_command_encoder(
        &self,
        device: &wgpu::Device,
        target_view: &wgpu::TextureView,
        target_format: wgpu::TextureFormat,
    ) -> wgpu::CommandEncoder {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("rg cmd encoder"),
        });

        for (i, pass) in self.graphics_passes.iter().enumerate() {
            let output_view = &self.texture_views[pass.render_target_texture.0];

            {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some(&format!("rg render pass {}", i)),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: output_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                rpass.set_pipeline(&pass.pipeline);

                if let Some(bind_group) = &pass.bind_group {
                    rpass.set_bind_group(0, bind_group, &[]);
                }

                rpass.draw(0..3, 0..1);
            }
        }

        let src_view = &self.texture_views[self.display_output.0];
        blit_pass::encode_blit(
            &blit_pass::BlitPassParameters {
                src_view,
                dst_view: target_view,
                target_format,
                blending: None,
            },
            device,
            &mut encoder,
        );

        encoder
    }
}
