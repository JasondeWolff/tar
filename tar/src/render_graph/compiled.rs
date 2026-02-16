use std::collections::HashMap;

use uuid::Uuid;

use crate::{
    editor::node_graph::{NodeId, OutputId},
    render_graph::{RgGraph, RgNodeTemplate, RgValueType, ScreenTexResolution},
    wgpu_util::BasicColorTextureFormat,
};

/// A handle to an allocated texture in the compiled graph.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct TextureHandle(pub usize);

/// Describes a texture allocation.
pub struct TextureAllocation {
    pub handle: TextureHandle,
    pub width: u32,
    pub height: u32,
    pub depth_or_layers: u32,
    pub format: wgpu::TextureFormat,
    pub mip_levels: u32,
    pub usage: wgpu::TextureUsages,
    pub dimension: wgpu::TextureDimension,
    pub persistent: bool,
}

/// A compiled render pass (one per GraphicsPass node).
pub struct CompiledPass {
    pub node_id: NodeId,
    pub shader_id: Uuid,
    /// The output texture this pass renders into.
    pub output_texture: TextureHandle,
    /// Map from binding (set, binding) -> TextureHandle for input textures.
    pub input_bindings: Vec<(u32, u32, TextureHandle)>,
}

/// The display output configuration.
pub struct CompiledDisplayOut {
    pub input_texture: TextureHandle,
}

/// The fully compiled render graph, ready for execution.
pub struct CompiledRenderGraph {
    pub textures: Vec<TextureAllocation>,
    pub passes: Vec<CompiledPass>,
    pub display_out: Option<CompiledDisplayOut>,
}

/// Topologically sort the nodes in a render graph using Kahn's algorithm.
/// Returns nodes in dependency order (sources first, sinks last).
pub fn topological_sort(graph: &RgGraph) -> Result<Vec<NodeId>, String> {
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
            dependents.entry(producer_node).or_default().push(consumer_node);
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
        return Err("Cycle detected in render graph".to_string());
    }

    Ok(sorted)
}

/// Helper to read a value from an input parameter on a node.
fn read_input_value(graph: &RgGraph, node_id: NodeId, name: &str) -> Option<RgValueType> {
    graph[node_id]
        .get_input(name)
        .ok()
        .map(|input_id| graph.get_input(input_id).value)
}

fn resolve_screen_resolution(res: ScreenTexResolution, screen_size: [u32; 2]) -> [u32; 2] {
    match res {
        ScreenTexResolution::Full => screen_size,
        ScreenTexResolution::Half => [screen_size[0] / 2, screen_size[1] / 2],
        ScreenTexResolution::Quarter => [screen_size[0] / 4, screen_size[1] / 4],
    }
}

/// Compile the render graph into a `CompiledRenderGraph`.
pub fn compile(
    graph: &RgGraph,
    shader_cache: &HashMap<Uuid, crate::render_graph::shader::Shader>,
    screen_size: [u32; 2],
) -> Result<CompiledRenderGraph, String> {
    let sorted = topological_sort(graph)?;

    let mut textures: Vec<TextureAllocation> = Vec::new();
    let mut passes: Vec<CompiledPass> = Vec::new();
    let mut display_out: Option<CompiledDisplayOut> = None;

    // Map from OutputId -> TextureHandle
    let mut output_textures: HashMap<OutputId, TextureHandle> = HashMap::new();

    let tex_usage =
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT;

    for &node_id in &sorted {
        let template = graph[node_id].user_data.0;

        match template {
            RgNodeTemplate::ScreenTex => {
                let resolution = match read_input_value(graph, node_id, "resolution") {
                    Some(RgValueType::ScreenTexResolution(r)) => r,
                    _ => ScreenTexResolution::Full,
                };
                let mips = match read_input_value(graph, node_id, "mips") {
                    Some(RgValueType::UInt(v)) => v.max(1),
                    _ => 1,
                };
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

            RgNodeTemplate::Tex2D => {
                let res = match read_input_value(graph, node_id, "resolution") {
                    Some(RgValueType::UInt2(r)) => r,
                    _ => [256, 256],
                };
                let mips = match read_input_value(graph, node_id, "mips") {
                    Some(RgValueType::UInt(v)) => v.max(1),
                    _ => 1,
                };
                let format = match read_input_value(graph, node_id, "format") {
                    Some(RgValueType::TextureFormat(f)) => f,
                    _ => BasicColorTextureFormat::default(),
                };
                let persistent = match read_input_value(graph, node_id, "persistent") {
                    Some(RgValueType::Bool(b)) => b,
                    _ => false,
                };

                let handle = TextureHandle(textures.len());
                textures.push(TextureAllocation {
                    handle,
                    width: res[0].max(1),
                    height: res[1].max(1),
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

            RgNodeTemplate::Tex2DArray => {
                let res = match read_input_value(graph, node_id, "resolution") {
                    Some(RgValueType::UInt2(r)) => r,
                    _ => [256, 256],
                };
                let count = match read_input_value(graph, node_id, "count") {
                    Some(RgValueType::UInt(v)) => v.max(1),
                    _ => 1,
                };
                let mips = match read_input_value(graph, node_id, "mips") {
                    Some(RgValueType::UInt(v)) => v.max(1),
                    _ => 1,
                };
                let format = match read_input_value(graph, node_id, "format") {
                    Some(RgValueType::TextureFormat(f)) => f,
                    _ => BasicColorTextureFormat::default(),
                };
                let persistent = match read_input_value(graph, node_id, "persistent") {
                    Some(RgValueType::Bool(b)) => b,
                    _ => false,
                };

                let handle = TextureHandle(textures.len());
                textures.push(TextureAllocation {
                    handle,
                    width: res[0].max(1),
                    height: res[1].max(1),
                    depth_or_layers: count,
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

            RgNodeTemplate::Tex3D => {
                let res = match read_input_value(graph, node_id, "resolution") {
                    Some(RgValueType::UInt3(r)) => r,
                    _ => [64, 64, 64],
                };
                let mips = match read_input_value(graph, node_id, "mips") {
                    Some(RgValueType::UInt(v)) => v.max(1),
                    _ => 1,
                };
                let format = match read_input_value(graph, node_id, "format") {
                    Some(RgValueType::TextureFormat(f)) => f,
                    _ => BasicColorTextureFormat::default(),
                };
                let persistent = match read_input_value(graph, node_id, "persistent") {
                    Some(RgValueType::Bool(b)) => b,
                    _ => false,
                };

                let handle = TextureHandle(textures.len());
                textures.push(TextureAllocation {
                    handle,
                    width: res[0].max(1),
                    height: res[1].max(1),
                    depth_or_layers: res[2].max(1),
                    format: format.into(),
                    mip_levels: mips,
                    usage: tex_usage,
                    dimension: wgpu::TextureDimension::D3,
                    persistent,
                });

                if let Ok(output_id) = graph[node_id].get_output("tex") {
                    output_textures.insert(output_id, handle);
                }
            }

            RgNodeTemplate::Tex3DArray => {
                let res = match read_input_value(graph, node_id, "resolution") {
                    Some(RgValueType::UInt3(r)) => r,
                    _ => [64, 64, 64],
                };
                let count = match read_input_value(graph, node_id, "count") {
                    Some(RgValueType::UInt(v)) => v.max(1),
                    _ => 1,
                };
                let mips = match read_input_value(graph, node_id, "mips") {
                    Some(RgValueType::UInt(v)) => v.max(1),
                    _ => 1,
                };
                let format = match read_input_value(graph, node_id, "format") {
                    Some(RgValueType::TextureFormat(f)) => f,
                    _ => BasicColorTextureFormat::default(),
                };
                let persistent = match read_input_value(graph, node_id, "persistent") {
                    Some(RgValueType::Bool(b)) => b,
                    _ => false,
                };

                let handle = TextureHandle(textures.len());
                textures.push(TextureAllocation {
                    handle,
                    width: res[0].max(1),
                    height: res[1].max(1),
                    depth_or_layers: res[2].max(1) * count,
                    format: format.into(),
                    mip_levels: mips,
                    usage: tex_usage,
                    dimension: wgpu::TextureDimension::D3,
                    persistent,
                });

                if let Ok(output_id) = graph[node_id].get_output("tex") {
                    output_textures.insert(output_id, handle);
                }
            }

            RgNodeTemplate::HistoryScreenTex => {
                let resolution = match read_input_value(graph, node_id, "resolution") {
                    Some(RgValueType::ScreenTexResolution(r)) => r,
                    _ => ScreenTexResolution::Full,
                };
                let mips = match read_input_value(graph, node_id, "mips") {
                    Some(RgValueType::UInt(v)) => v.max(1),
                    _ => 1,
                };
                let format = match read_input_value(graph, node_id, "format") {
                    Some(RgValueType::TextureFormat(f)) => f,
                    _ => BasicColorTextureFormat::default(),
                };

                let [w, h] = resolve_screen_resolution(resolution, screen_size);

                // Current texture
                let current_handle = TextureHandle(textures.len());
                textures.push(TextureAllocation {
                    handle: current_handle,
                    width: w,
                    height: h,
                    depth_or_layers: 1,
                    format: format.into(),
                    mip_levels: mips,
                    usage: tex_usage,
                    dimension: wgpu::TextureDimension::D2,
                    persistent: true,
                });

                // Previous texture
                let prev_handle = TextureHandle(textures.len());
                textures.push(TextureAllocation {
                    handle: prev_handle,
                    width: w,
                    height: h,
                    depth_or_layers: 1,
                    format: format.into(),
                    mip_levels: mips,
                    usage: tex_usage,
                    dimension: wgpu::TextureDimension::D2,
                    persistent: true,
                });

                if let Ok(output_id) = graph[node_id].get_output("current tex") {
                    output_textures.insert(output_id, current_handle);
                }
                if let Ok(output_id) = graph[node_id].get_output("previous tex") {
                    output_textures.insert(output_id, prev_handle);
                }
            }

            RgNodeTemplate::HistoryTex2D => {
                let res = match read_input_value(graph, node_id, "resolution") {
                    Some(RgValueType::UInt2(r)) => r,
                    _ => [256, 256],
                };
                let mips = match read_input_value(graph, node_id, "mips") {
                    Some(RgValueType::UInt(v)) => v.max(1),
                    _ => 1,
                };
                let format = match read_input_value(graph, node_id, "format") {
                    Some(RgValueType::TextureFormat(f)) => f,
                    _ => BasicColorTextureFormat::default(),
                };

                let current_handle = TextureHandle(textures.len());
                textures.push(TextureAllocation {
                    handle: current_handle,
                    width: res[0].max(1),
                    height: res[1].max(1),
                    depth_or_layers: 1,
                    format: format.into(),
                    mip_levels: mips,
                    usage: tex_usage,
                    dimension: wgpu::TextureDimension::D2,
                    persistent: true,
                });

                let prev_handle = TextureHandle(textures.len());
                textures.push(TextureAllocation {
                    handle: prev_handle,
                    width: res[0].max(1),
                    height: res[1].max(1),
                    depth_or_layers: 1,
                    format: format.into(),
                    mip_levels: mips,
                    usage: tex_usage,
                    dimension: wgpu::TextureDimension::D2,
                    persistent: true,
                });

                if let Ok(output_id) = graph[node_id].get_output("current tex") {
                    output_textures.insert(output_id, current_handle);
                }
                if let Ok(output_id) = graph[node_id].get_output("previous tex") {
                    output_textures.insert(output_id, prev_handle);
                }
            }

            RgNodeTemplate::HistoryTex3D => {
                let res = match read_input_value(graph, node_id, "resolution") {
                    Some(RgValueType::UInt3(r)) => r,
                    _ => [64, 64, 64],
                };
                let mips = match read_input_value(graph, node_id, "mips") {
                    Some(RgValueType::UInt(v)) => v.max(1),
                    _ => 1,
                };
                let format = match read_input_value(graph, node_id, "format") {
                    Some(RgValueType::TextureFormat(f)) => f,
                    _ => BasicColorTextureFormat::default(),
                };

                let current_handle = TextureHandle(textures.len());
                textures.push(TextureAllocation {
                    handle: current_handle,
                    width: res[0].max(1),
                    height: res[1].max(1),
                    depth_or_layers: res[2].max(1),
                    format: format.into(),
                    mip_levels: mips,
                    usage: tex_usage,
                    dimension: wgpu::TextureDimension::D3,
                    persistent: true,
                });

                let prev_handle = TextureHandle(textures.len());
                textures.push(TextureAllocation {
                    handle: prev_handle,
                    width: res[0].max(1),
                    height: res[1].max(1),
                    depth_or_layers: res[2].max(1),
                    format: format.into(),
                    mip_levels: mips,
                    usage: tex_usage,
                    dimension: wgpu::TextureDimension::D3,
                    persistent: true,
                });

                if let Ok(output_id) = graph[node_id].get_output("current tex") {
                    output_textures.insert(output_id, current_handle);
                }
                if let Ok(output_id) = graph[node_id].get_output("previous tex") {
                    output_textures.insert(output_id, prev_handle);
                }
            }

            RgNodeTemplate::GraphicsPass => {
                // Read the code file id
                let shader_id = match read_input_value(graph, node_id, "code") {
                    Some(RgValueType::CodeFile(Some(id))) => id,
                    _ => continue, // No shader assigned, skip this pass
                };

                let shader = match shader_cache.get(&shader_id) {
                    Some(s) if s.shader_module().is_some() => s,
                    _ => continue, // Shader not compiled or has errors
                };

                // Output texture (screen-sized Rgba16Float by default)
                let out_handle = TextureHandle(textures.len());
                textures.push(TextureAllocation {
                    handle: out_handle,
                    width: screen_size[0],
                    height: screen_size[1],
                    depth_or_layers: 1,
                    format: wgpu::TextureFormat::Rgba16Float,
                    mip_levels: 1,
                    usage: tex_usage,
                    dimension: wgpu::TextureDimension::D2,
                    persistent: false,
                });

                if let Ok(output_id) = graph[node_id].get_output("out") {
                    output_textures.insert(output_id, out_handle);
                }

                // Resolve input bindings from dynamic ports
                let mut input_bindings = Vec::new();
                for binding in shader.get_bindings() {
                    if binding.resource_type == "Sampler" {
                        continue;
                    }

                    if let Ok(input_id) = graph[node_id].get_input(&binding.name) {
                        if let Some(connected_output) = graph.connection(input_id) {
                            if let Some(&tex_handle) = output_textures.get(&connected_output) {
                                input_bindings.push((binding.set, binding.binding, tex_handle));
                            }
                        }
                    }
                }

                passes.push(CompiledPass {
                    node_id,
                    shader_id,
                    output_texture: out_handle,
                    input_bindings,
                });
            }

            RgNodeTemplate::DisplayOut => {
                if let Ok(input_id) = graph[node_id].get_input("in") {
                    if let Some(connected_output) = graph.connection(input_id) {
                        if let Some(&tex_handle) = output_textures.get(&connected_output) {
                            display_out = Some(CompiledDisplayOut {
                                input_texture: tex_handle,
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(CompiledRenderGraph {
        textures,
        passes,
        display_out,
    })
}
