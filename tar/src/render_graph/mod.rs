use std::{collections::HashMap, default, num::NonZeroU32, path::PathBuf};

use serde::{Deserialize, Serialize};
use strum::{EnumIter, EnumString};
use uuid::Uuid;
use wgpu::naga::{
    front::wgsl,
    valid::{Capabilities, ValidationFlags, Validator},
};

use crate::{
    editor::{
        node_graph::{self, Graph, NodeId, NodeResponse, NodeTemplateTrait},
        tabs::render_graph::{AllMyNodeTemplates, MyResponse, RgEditorState},
        EditorDragPayload,
    },
    project::{CodeFileType, Project},
    render_graph::shader::Shader,
    wgpu_util::BasicColorTextureFormat,
};

pub mod shader;

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
pub struct Tex3DArray {
    pub resolution: [u32; 3],
    pub count: u32,
    pub mipmaps: u32,
    pub format: BasicColorTextureFormat,
    pub persistent: bool,
}

#[derive(PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RgDataType {
    UInt,
    UInt2,
    UInt3,
    Float,
    Bool,

    ScreenTexResolution,
    TextureFormat,

    Tex2D,
    HistoryTex2D,
    Tex2DArray,
    Tex3D,
    HistoryTex3D,
    Tex3DArray,

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

    ScreenTex(ScreenTex),
    Tex2D(Tex2D),
    Tex2DArray(Tex2DArray),
    Tex3D(Tex3D),
    Tex3DArray(Tex3DArray),

    CodeFile(Option<Uuid>),
}

impl Default for RgValueType {
    fn default() -> Self {
        Self::UInt(0)
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
    Tex3DArray,

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

impl Default for RenderGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderGraph {
    pub fn new() -> Self {
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
    ) {
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

        for node_response in graph_response.node_responses {
            if let NodeResponse::User(user_event) = node_response {
                match user_event {
                    MyResponse::SetInspectNode(node) => self.graph_state.inspect_node = Some(node),
                    MyResponse::ClearInspectNode => self.graph_state.inspect_node = None,
                }
            }
        }
    }

    /// Synchronize the shader cache with the current code file sources.
    /// Compiles new/changed fragment shaders and removes deleted ones.
    pub fn sync_graphics_shaders(
        &mut self,
        code_sources: &[(Uuid, String)],
        device: &wgpu::Device,
    ) {
        let valid_ids: std::collections::HashSet<Uuid> =
            code_sources.iter().map(|(id, _)| *id).collect();

        // Remove deleted shaders
        self.graph_state
            .shader_cache
            .retain(|id, _| valid_ids.contains(id));

        // Add or update shaders
        for (id, source) in code_sources {
            if let Some(shader) = self.graph_state.shader_cache.get_mut(id) {
                shader.update_source(source.to_owned(), device);
            } else {
                self.graph_state
                    .shader_cache
                    .insert(*id, Shader::new(source.to_owned(), device));
            }
        }
    }

    pub fn shaders_iter(&self) -> impl Iterator<Item = (&Uuid, &Shader)> {
        self.graph_state.shader_cache.iter()
    }
}
