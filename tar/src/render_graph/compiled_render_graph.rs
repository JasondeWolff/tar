use std::collections::HashMap;

use anyhow::bail;
use uuid::Uuid;

use crate::{
    editor::node_graph::NodeId,
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
    ) -> anyhow::Result<Self> {
        let mut buffers = Vec::new();
        let mut textures = Vec::new();
        let mut texture_views = Vec::new();
        let mut graphics_passes = Vec::new();

        let nodes = topological_sort(graph)?;

        for &node_id in &nodes {
            let template = graph[node_id].user_data.0;

            match template {
                RgNodeTemplate::ScreenTex => {
                    let resolution = read_input_value(graph, node_id, "resolution")?
                        .as_screen_tex_resolution()?;
                    let mips = read_input_value(graph, node_id, "mips")?;
                    let format = match read_input_value(graph, node_id, "format") {
                        Some(RgValueType::TextureFormat(f)) => f,
                        _ => BasicColorTextureFormat::default(),
                    };
                    let persistent = match read_input_value(graph, node_id, "persistent") {
                        Some(RgValueType::Bool(b)) => b,
                        _ => false,
                    };

                    let [w, h] = resolve_screen_resolution(resolution, screen_size);
                    let handle = TextureHandle(textures.len());
                    textures.push(TextureAllocation {
                        handle,
                        width: w,
                        height: h,
                        depth_or_layers: 1,
                        format: format.into(),
                        mip_levels: mips,
                        usage: tex_usage,
                        dimension: wgpu::TextureDimension::D2,
                        persistent,
                    });

                    if let Ok(output_id) = graph[node_id].get_output("tex") {
                        output_textures.insert(output_id, handle);
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
