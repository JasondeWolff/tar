use std::collections::HashMap;

use anyhow::bail;
use uuid::Uuid;

use crate::{
    editor::node_graph::{NodeId, OutputId},
    render_graph::{shader::Shader, RgGraph, RgNodeTemplate, RgValueType, ScreenTexResolution},
};

struct BufferHandle(usize);
struct TextureHandle(usize);

enum InputBindingData {
    Buffer(BufferHandle),
    Texture(TextureHandle),
}

struct InputBinding {
    set: u32,
    binding: u32,
    data: InputBindingData,
}

struct CompiledGraphicsPass {
    pub node_id: NodeId,
    pub shader_id: Uuid,

    pub render_target_texture: TextureHandle,
    pub input_bindings: Vec<InputBinding>,
}

pub struct CompiledRenderGraph {
    buffers: Vec<wgpu::Buffer>,
    textures: Vec<wgpu::Texture>,
    texture_views: Vec<wgpu::TextureView>,

    graphics_passes: Vec<CompiledGraphicsPass>,
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

        let nodes = topological_sort(graph)?;

        let mut output_buffer_handles: HashMap<OutputId, BufferHandle> = HashMap::new();
        let mut output_texture_handles: HashMap<OutputId, TextureHandle> = HashMap::new();

        let mut build_tex =
            |width, height, array_layers, dimension, mip_level_count, format, usage| {
                let handle = TextureHandle(textures.len());

                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(&format!("rg texture {}", handle.0)),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: array_layers,
                    },
                    mip_level_count,
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

        let mut build_buffer = |size| {
            let handle = BufferHandle(buffers.len());

            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("rg buffer {}", handle.0)),
                size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::UNIFORM,
                mapped_at_creation: false,
            });

            buffers.push(buffer);
            handle
        };

        for &node_id in &nodes {
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

                _ => {}
            }
        }

        Ok(Self {
            buffers,
            textures,
            texture_views,
            graphics_passes,
        })
    }
}
