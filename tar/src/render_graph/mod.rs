use std::{collections::HashMap, path::PathBuf};

use anyhow::bail;
use serde::{Deserialize, Serialize};
use strum::EnumIter;
use uuid::Uuid;

use crate::{
    editor::{
        node_graph::{self, Graph, InputParamKind, NodeId, NodeResponse, NodeTemplateTrait},
        tabs::render_graph::{AllMyNodeTemplates, MyResponse, RgEditorState},
        EditorDragPayload,
    },
    project::{CodeFileType, CodeFiles},
    render_graph::{compiled_render_graph::CompiledRenderGraph, shader::Shader},
    wgpu_util::BasicColorTextureFormat,
};

pub mod compiled_render_graph;
pub mod shader;

pub type RgGraph = Graph<RgNodeData, RgDataType, RgValueType>;

#[derive(
    Default,
    Copy,
    Clone,
    Debug,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    strum::EnumIter,
    strum::Display,
)]
pub enum ScreenTexResolution {
    #[default]
    Full,
    Half,
    Quarter,
}

impl ScreenTexResolution {
    pub fn resolve(&self, screen_size: [u32; 2]) -> [u32; 2] {
        match self {
            Self::Full => screen_size,
            Self::Half => [screen_size[0].div_ceil(2), screen_size[1].div_ceil(2)],
            Self::Quarter => [screen_size[0].div_ceil(4), screen_size[1].div_ceil(4)],
        }
    }
}

#[derive(
    Default,
    Copy,
    Clone,
    Debug,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    strum::EnumIter,
    strum::Display,
)]
pub enum TextureUsage {
    #[default]
    RenderTarget,
    Storage,
}

impl From<TextureUsage> for wgpu::TextureUsages {
    fn from(usage: TextureUsage) -> Self {
        match usage {
            TextureUsage::RenderTarget => {
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING
            }
            TextureUsage::Storage => {
                wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING
            }
        }
    }
}

#[derive(Default, Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ScreenTex {
    pub resolution: ScreenTexResolution,
    pub mipmaps: u32,
    pub format: BasicColorTextureFormat,
    pub persistent: bool,
}

#[derive(Default, Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct HistoryScreenTex {
    pub resolution: ScreenTexResolution,
    pub mipmaps: u32,
    pub format: BasicColorTextureFormat,
}

#[derive(Default, Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Tex2D {
    pub resolution: [u32; 2],
    pub mipmaps: u32,
    pub format: BasicColorTextureFormat,
    pub persistent: bool,
}

#[derive(Default, Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct HistoryTex2D {
    pub resolution: [u32; 2],
    pub mipmaps: u32,
    pub format: BasicColorTextureFormat,
}

#[derive(Default, Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Tex2DArray {
    pub resolution: [u32; 2],
    pub count: u32,
    pub mipmaps: u32,
    pub format: BasicColorTextureFormat,
    pub persistent: bool,
}

#[derive(Default, Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Tex3D {
    pub resolution: [u32; 3],
    pub mipmaps: u32,
    pub format: BasicColorTextureFormat,
    pub persistent: bool,
}

#[derive(Default, Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct HistoryTex3D {
    pub resolution: [u32; 3],
    pub mipmaps: u32,
    pub format: BasicColorTextureFormat,
}

#[derive(Default, Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Buffer {
    pub size: u32,
    pub persistent: bool,
}

#[derive(Default, Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct HistoryBuffer {
    pub size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RgDataType {
    UInt,
    UInt2,
    UInt3,
    Float,
    Bool,

    ScreenTexResolution,
    TextureFormat,
    TextureUsage,

    Tex2D,
    HistoryTex2D,
    Tex2DArray,
    Tex3D,
    HistoryTex3D,
    Buffer,
    HistoryBuffer,

    CodeFile,
}

#[derive(Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum RgValueType {
    UInt(u32),
    UInt2([u32; 2]),
    UInt3([u32; 3]),
    Float(f32),
    Bool(bool),

    ScreenTexResolution(ScreenTexResolution),
    TextureFormat(BasicColorTextureFormat),
    TextureUsage(TextureUsage),

    Tex2D(Tex2D),
    Tex2DArray(Tex2DArray),
    Tex3D(Tex3D),
    Buffer(Buffer),

    CodeFile(Option<Uuid>),
}

impl Default for RgValueType {
    fn default() -> Self {
        Self::UInt(0)
    }
}

impl RgValueType {
    pub fn as_screen_tex_resolution(&self) -> anyhow::Result<&ScreenTexResolution> {
        match self {
            Self::ScreenTexResolution(result) => Ok(result),
            _ => bail!("{:?} is not of type ScreenTexResolution", self),
        }
    }

    pub fn as_texture_format(&self) -> anyhow::Result<&BasicColorTextureFormat> {
        match self {
            Self::TextureFormat(result) => Ok(result),
            _ => bail!("{:?} is not of type TextureFormat", self),
        }
    }

    pub fn as_texture_usage(&self) -> anyhow::Result<&TextureUsage> {
        match self {
            Self::TextureUsage(result) => Ok(result),
            _ => bail!("{:?} is not of type TextureUsage", self),
        }
    }

    pub fn as_uint(&self) -> anyhow::Result<&u32> {
        match self {
            Self::UInt(result) => Ok(result),
            _ => bail!("{:?} is not of type UInt", self),
        }
    }

    pub fn as_uint2(&self) -> anyhow::Result<&[u32; 2]> {
        match self {
            Self::UInt2(result) => Ok(result),
            _ => bail!("{:?} is not of type UInt2", self),
        }
    }

    pub fn as_uint3(&self) -> anyhow::Result<&[u32; 3]> {
        match self {
            Self::UInt3(result) => Ok(result),
            _ => bail!("{:?} is not of type UInt3", self),
        }
    }

    pub fn as_float(&self) -> anyhow::Result<&f32> {
        match self {
            Self::Float(result) => Ok(result),
            _ => bail!("{:?} is not of type Float", self),
        }
    }

    pub fn as_bool(&self) -> anyhow::Result<&bool> {
        match self {
            Self::Bool(result) => Ok(result),
            _ => bail!("{:?} is not of type Bool", self),
        }
    }

    pub fn as_tex2d(&self) -> anyhow::Result<&Tex2D> {
        match self {
            Self::Tex2D(result) => Ok(result),
            _ => bail!("{:?} is not of type Tex2D", self),
        }
    }

    pub fn as_tex2d_array(&self) -> anyhow::Result<&Tex2DArray> {
        match self {
            Self::Tex2DArray(result) => Ok(result),
            _ => bail!("{:?} is not of type Tex2DArray", self),
        }
    }

    pub fn as_tex3d(&self) -> anyhow::Result<&Tex3D> {
        match self {
            Self::Tex3D(result) => Ok(result),
            _ => bail!("{:?} is not of type Tex3D", self),
        }
    }

    pub fn as_buffer(&self) -> anyhow::Result<&Buffer> {
        match self {
            Self::Buffer(result) => Ok(result),
            _ => bail!("{:?} is not of type Buffer", self),
        }
    }

    pub fn as_code_file(&self) -> anyhow::Result<&Option<Uuid>> {
        match self {
            Self::CodeFile(result) => Ok(result),
            _ => bail!("{:?} is not of type CodeFile", self),
        }
    }
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize, EnumIter)]
pub enum RgNodeTemplate {
    ScreenTex,
    HistoryScreenTex,
    Tex2D,
    HistoryTex2D,
    Tex2DArray,
    Tex3D,
    HistoryTex3D,
    Buffer,
    HistoryBuffer,

    GraphicsPass,

    DisplayOut,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct RgNodeData(pub RgNodeTemplate);

#[derive(Default)]
pub struct RgEditorGraphState {
    pub code_file_names: HashMap<Uuid, (CodeFileType, PathBuf)>,
    pub drag_payload: Option<EditorDragPayload>,
}

/// The graph 'global' state. This state struct is passed around to the node and
/// parameter drawing callbacks. The contents of this struct are entirely up to
/// the user. For this example, we use it to keep track of the 'active' node.
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct RgGraphState {
    #[serde(skip)]
    pub shader_cache: HashMap<Uuid, Shader>,

    // TODO: also editor-only related
    pub inspect_node: Option<NodeId>,

    #[serde(skip)]
    pub editor: Option<RgEditorGraphState>,
}

#[derive(Serialize, Deserialize)]
pub struct RenderGraph {
    node_graph: RgEditorState,
    graph_state: RgGraphState,
}

impl RenderGraph {
    pub fn new(code_files: &CodeFiles) -> Self {
        let mut node_graph = RgEditorState::default();
        let mut graph_state = RgGraphState::default();

        let mut add_node = |template: RgNodeTemplate, pos| -> NodeId {
            let node_id = node_graph.graph.add_node(
                template.node_graph_label(&mut graph_state),
                RgNodeData(template),
                |_, _| {},
            );
            template.build_node(&mut node_graph.graph, &mut graph_state, node_id);
            node_graph.node_positions.insert(node_id, pos);
            node_graph.node_order.push(node_id);

            node_id
        };

        const OFFSET: egui::Vec2 = egui::Vec2::new(100.0, 100.0);
        const SPACING: f32 = 350.0;

        let screen_tex_node = add_node(
            RgNodeTemplate::ScreenTex,
            egui::Pos2::new(SPACING * 0.0, 0.0) + OFFSET,
        );
        let graphics_pass_node = add_node(
            RgNodeTemplate::GraphicsPass,
            egui::Pos2::new(SPACING * 1.0, 0.0) + OFFSET,
        );
        let display_out_node = add_node(
            RgNodeTemplate::DisplayOut,
            egui::Pos2::new(SPACING * 2.0, 0.0) + OFFSET,
        );

        node_graph.graph.add_connection(
            node_graph
                .graph
                .nodes
                .get(screen_tex_node)
                .unwrap()
                .get_output("tex")
                .unwrap(),
            node_graph
                .graph
                .nodes
                .get(graphics_pass_node)
                .unwrap()
                .get_input("in")
                .unwrap(),
            0,
        );

        node_graph.graph.add_connection(
            node_graph
                .graph
                .nodes
                .get(graphics_pass_node)
                .unwrap()
                .get_output("out")
                .unwrap(),
            node_graph
                .graph
                .nodes
                .get(display_out_node)
                .unwrap()
                .get_input("in")
                .unwrap(),
            0,
        );

        let code_file = code_files.files_iter().next().map(|(id, _)| *id);

        let graphics_pass_code = node_graph
            .graph
            .nodes
            .get(graphics_pass_node)
            .unwrap()
            .get_input("code")
            .unwrap();
        node_graph.graph.inputs[graphics_pass_code].value = RgValueType::CodeFile(code_file);

        Self {
            node_graph,
            graph_state,
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        code_file_names: HashMap<Uuid, (CodeFileType, PathBuf)>,
        drag_payload: &mut Option<EditorDragPayload>,
    ) -> bool {
        self.graph_state.editor = Some(RgEditorGraphState {
            code_file_names,
            drag_payload: std::mem::take(drag_payload),
        });

        let graph_response = self.node_graph.draw_graph_editor(
            ui,
            AllMyNodeTemplates,
            &mut self.graph_state,
            Vec::default(),
        );

        if let Some(editor) = self.graph_state.editor.take() {
            *drag_payload = editor.drag_payload;
        }

        let mut dirty = false;

        for node_response in graph_response.node_responses {
            match node_response {
                NodeResponse::User(user_event) => match user_event {
                    MyResponse::SetInspectNode(node) => self.graph_state.inspect_node = Some(node),
                    MyResponse::ClearInspectNode => self.graph_state.inspect_node = None,
                    MyResponse::ValueChanged => dirty = true,
                },
                NodeResponse::ConnectEventEnded { .. }
                | NodeResponse::CreatedNode(_)
                | NodeResponse::DeleteNodeFull { .. }
                | NodeResponse::DisconnectEvent { .. } => {
                    dirty = true;
                }
                _ => {}
            }
        }

        dirty
    }

    /// Synchronize the shader cache with the current code file sources.
    /// Compiles new/changed fragment shaders and removes deleted ones.
    pub fn sync_graphics_shaders(
        &mut self,
        code_sources: &[(Uuid, String)],
        device: &wgpu::Device,
    ) -> bool {
        let valid_ids: std::collections::HashSet<Uuid> =
            code_sources.iter().map(|(id, _)| *id).collect();

        // Remove deleted shaders
        self.graph_state
            .shader_cache
            .retain(|id, _| valid_ids.contains(id));

        let mut dirty = false;

        // Add or update shaders
        for (id, source) in code_sources {
            if let Some(shader) = self.graph_state.shader_cache.get_mut(id) {
                if shader.update_source(source.to_owned(), device) {
                    dirty = true;
                }
            } else {
                self.graph_state
                    .shader_cache
                    .insert(*id, Shader::new(source.to_owned(), device));
                dirty = true;
            }
        }

        dirty
    }

    /// For each GraphicsPass node, sync its dynamic input ports to match the
    pub fn sync_dynamic_node_inputs(&mut self) {
        let graph = &mut self.node_graph.graph;
        let shader_cache = &self.graph_state.shader_cache;

        // Collect all GraphicsPass node ids
        let graphics_pass_nodes: Vec<NodeId> = graph
            .iter_nodes()
            .filter(|&nid| matches!(graph[nid].user_data.0, RgNodeTemplate::GraphicsPass))
            .collect();

        for node_id in graphics_pass_nodes {
            // Read the code file uuid from the "code" input
            let code_file_id = graph[node_id].get_input("code").ok().and_then(|input_id| {
                if let RgValueType::CodeFile(Some(id)) = &graph.get_input(input_id).value {
                    Some(*id)
                } else {
                    None
                }
            });

            // Get expected bindings from shader cache
            let bindings = code_file_id
                .and_then(|id| shader_cache.get(&id))
                .map(|s| s.get_bindings().to_vec())
                .unwrap_or_default();

            // Build desired input ports from bindings (skip Samplers - auto-injected)
            let desired: Vec<(String, RgDataType, bool)> = bindings
                .iter()
                .map(|b| (b.name.clone(), b.resource_type.clone(), b.readonly))
                .collect();

            // Names of static inputs that should never be removed
            let static_names: &[&str] = &["code", "in"];

            // Current dynamic inputs
            let current_dynamic: Vec<(String, node_graph::InputId)> = graph[node_id]
                .inputs
                .iter()
                .filter(|(name, _)| !static_names.contains(&name.as_str()))
                .cloned()
                .collect();

            // Remove ports not in desired set
            for (name, input_id) in &current_dynamic {
                if !desired.iter().any(|(n, _, _)| n == name) {
                    graph.remove_input_param(*input_id);
                }
            }

            // Add ports in desired set not currently present
            for (name, data_type, readonly) in &desired {
                let exists = graph[node_id].inputs.iter().any(|(n, _)| n == name);
                if !exists {
                    let (dt, vt) = match data_type {
                        RgDataType::Tex2D => {
                            (RgDataType::Tex2D, RgValueType::Tex2D(Tex2D::default()))
                        }
                        RgDataType::Tex3D => {
                            (RgDataType::Tex3D, RgValueType::Tex3D(Tex3D::default()))
                        }
                        RgDataType::Buffer => {
                            (RgDataType::Buffer, RgValueType::Buffer(Buffer::default()))
                        }
                        _ => continue,
                    };

                    let consumer = !readonly;

                    graph.add_input_param(
                        node_id,
                        name.clone(),
                        dt,
                        vt,
                        InputParamKind::ConnectionOnly,
                        consumer,
                        true,
                    );
                }
            }
        }
    }

    pub fn compile(
        &self,
        screen_size: [u32; 2],
        device: &wgpu::Device,
    ) -> anyhow::Result<CompiledRenderGraph> {
        CompiledRenderGraph::new(
            &self.node_graph.graph,
            &self.graph_state.shader_cache,
            screen_size,
            device,
        )
    }

    pub fn shaders_iter(&self) -> impl Iterator<Item = (&Uuid, &Shader)> {
        self.graph_state.shader_cache.iter()
    }
}
