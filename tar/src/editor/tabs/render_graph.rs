use strum::IntoEnumIterator;
use uuid::Uuid;

use egui::{self};
use std::borrow::Cow;

use crate::{
    editor::{node_graph::*, EditorDragPayload},
    project::{CodeFileType, Project},
    render_graph::{
        RgDataType, RgGraph, RgGraphState, RgNodeData, RgNodeTemplate, RgValueType,
        ScreenTexResolution, Tex2D, Tex2DArray, Tex3D, TextureUsage,
    },
    wgpu_util::BasicColorTextureFormat,
};

// // ========= First, define your user data types =============

// /// `DataType`s are what defines the possible range of connections when
// /// attaching two ports together. The graph UI will make sure to not allow
// /// attaching incompatible datatypes.
// #[derive(PartialEq, Eq, serde::Serialize, serde::Deserialize)]
// pub enum MyDataType {
//     Scalar,
//     Vec2,
// }

// /// In the graph, input parameters can optionally have a constant value. This
// /// value can be directly edited in a widget inside the node itself.
// ///
// /// There will usually be a correspondence between DataTypes and ValueTypes. But
// /// this library makes no attempt to check this consistency. For instance, it is
// /// up to the user code in this example to make sure no parameter is created
// /// with a DataType of Scalar and a ValueType of Vec2.
// #[derive(Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
// pub enum MyValueType {
//     Vec2 { value: egui::Vec2 },
//     Scalar { value: f32 },
// }

// impl Default for MyValueType {
//     fn default() -> Self {
//         // NOTE: This is just a dummy `Default` implementation. The library
//         // requires it to circumvent some internal borrow checker issues.
//         Self::Scalar { value: 0.0 }
//     }
// }

// impl MyValueType {
//     /// Tries to downcast this value type to a vector
//     pub fn try_to_vec2(self) -> anyhow::Result<egui::Vec2> {
//         if let MyValueType::Vec2 { value } = self {

//             Ok(value)
//         } else {
//             anyhow::bail!("Invalid cast from {:?} to vec2", self)
//         }
//     }

//     /// Tries to downcast this value type to a scalar
//     pub fn try_to_scalar(self) -> anyhow::Result<f32> {
//         if let MyValueType::Scalar { value } = self {
//             Ok(value)
//         } else {
//             anyhow::bail!("Invalid cast from {:?} to scalar", self)
//         }
//     }
// }

// /// NodeTemplate is a mechanism to define node templates. It's what the graph
// /// will display in the "new node" popup. The user code needs to tell the
// /// library how to convert a NodeTemplate into a Node.
// #[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
// pub enum MyNodeTemplate {
//     MakeScalar,
//     AddScalar,
//     SubtractScalar,
//     MakeVector,
//     AddVector,
//     SubtractVector,
//     VectorTimesScalar,
// }

/// The response type is used to encode side-effects produced when drawing a
/// node in the graph. Most side-effects (creating new nodes, deleting existing
/// nodes, handling connections...) are already handled by the library, but this
/// mechanism allows creating additional side effects from user code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MyResponse {
    SetInspectNode(NodeId),
    ClearInspectNode,
}

// =========== Then, you need to implement some traits ============

// A trait for the data types, to tell the library how to display them
impl DataTypeTrait<RgGraphState> for RgDataType {
    fn data_type_color(&self, _user_state: &mut RgGraphState) -> egui::Color32 {
        match self {
            Self::UInt => egui::Color32::from_rgb(38, 109, 211),
            Self::UInt2 => egui::Color32::from_rgb(38, 109, 211),
            Self::UInt3 => egui::Color32::from_rgb(38, 109, 211),
            Self::Float => egui::Color32::from_rgb(238, 207, 109),
            Self::Bool => egui::Color32::from_rgb(238, 207, 109),
            Self::ScreenTexResolution => egui::Color32::from_rgb(238, 207, 109),
            Self::TextureFormat => egui::Color32::from_rgb(238, 207, 109),
            Self::TextureUsage => egui::Color32::from_rgb(238, 207, 109),
            Self::Tex2D => egui::Color32::from_rgb(109, 238, 182),
            Self::HistoryTex2D => egui::Color32::from_rgb(238, 109, 182),
            Self::Tex2DArray => egui::Color32::from_rgb(109, 182, 238),
            Self::Tex3D => egui::Color32::from_rgb(211, 182, 38),
            Self::HistoryTex3D => egui::Color32::from_rgb(182, 38, 211),
            Self::Buffer => egui::Color32::from_rgb(38, 211, 182),
            Self::HistoryBuffer => egui::Color32::from_rgb(38, 211, 182),
            Self::CodeFile => egui::Color32::from_rgb(38, 211, 182),
        }
    }

    fn name(&self) -> Cow<'_, str> {
        match self {
            Self::UInt => Cow::Borrowed("uint"),
            Self::UInt2 => Cow::Borrowed("uint2"),
            Self::UInt3 => Cow::Borrowed("uint3"),
            Self::Float => Cow::Borrowed("float"),
            Self::Bool => Cow::Borrowed("bool"),
            Self::ScreenTexResolution => Cow::Borrowed("screen texture resolution"),
            Self::TextureFormat => Cow::Borrowed("texture format"),
            Self::TextureUsage => Cow::Borrowed("texture usage"),
            Self::Tex2D => Cow::Borrowed("2D texture"),
            Self::HistoryTex2D => Cow::Borrowed("history 2D texture"),
            Self::Tex2DArray => Cow::Borrowed("2D texture array"),
            Self::Tex3D => Cow::Borrowed("3D texture"),
            Self::HistoryTex3D => Cow::Borrowed("history 3D texture"),
            Self::Buffer => Cow::Borrowed("buffer"),
            Self::HistoryBuffer => Cow::Borrowed("history buffer"),
            Self::CodeFile => Cow::Borrowed("code file"),
        }
    }
}

// A trait for the node kinds, which tells the library how to build new nodes
// from the templates in the node finder
impl NodeTemplateTrait for RgNodeTemplate {
    type NodeData = RgNodeData;
    type DataType = RgDataType;
    type ValueType = RgValueType;
    type UserState = RgGraphState;
    type CategoryType = String;

    fn node_finder_label(&self, _user_state: &mut Self::UserState) -> Cow<'_, str> {
        Cow::Borrowed(match self {
            Self::ScreenTex => "Screen Tex",
            Self::HistoryScreenTex => "History Screen Tex",
            Self::Tex2D => "Tex 2D",
            Self::HistoryTex2D => "History Tex 2D",
            Self::Tex2DArray => "Tex 2D Array",
            Self::Tex3D => "Tex 3D",
            Self::HistoryTex3D => "History Tex 3D",
            Self::Buffer => "Buffer",
            Self::HistoryBuffer => "History Buffer",

            Self::GraphicsPass => "Graphics Pass",

            Self::DisplayOut => "Display Out",
        })
    }

    // this is what allows the library to show collapsible lists in the node finder.
    fn node_finder_categories(&self, _user_state: &mut Self::UserState) -> Vec<String> {
        match self {
            Self::ScreenTex
            | Self::HistoryScreenTex
            | Self::Tex2D
            | Self::HistoryTex2D
            | Self::Tex2DArray
            | Self::Tex3D
            | Self::HistoryTex3D => {
                vec![format!("{} Texture", egui_phosphor::regular::CHECKERBOARD)]
            }

            Self::Buffer | Self::HistoryBuffer => {
                vec![format!("{} Buffer", egui_phosphor::regular::BINARY)]
            }

            Self::GraphicsPass => vec![format!("{} Render", egui_phosphor::regular::GRAPHICS_CARD)],

            Self::DisplayOut => vec![format!("{} Display", egui_phosphor::regular::MONITOR)],
        }
    }

    fn node_graph_label(&self, user_state: &mut Self::UserState) -> String {
        // It's okay to delegate this to node_finder_label if you don't want to
        // show different names in the node finder and the node itself.
        self.node_finder_label(user_state).into()
    }

    fn user_data(&self, _user_state: &mut Self::UserState) -> Self::NodeData {
        RgNodeData(*self)
    }

    fn build_node(
        &self,
        graph: &mut Graph<Self::NodeData, Self::DataType, Self::ValueType>,
        _user_state: &mut Self::UserState,
        node_id: NodeId,
    ) {
        let input_uint = |graph: &mut RgGraph, name: &str| {
            graph.add_input_param(
                node_id,
                name.to_string(),
                RgDataType::UInt,
                RgValueType::UInt(0),
                InputParamKind::ConstantOnly,
                true,
            );
        };
        let input_uint2 = |graph: &mut RgGraph, name: &str| {
            graph.add_input_param(
                node_id,
                name.to_string(),
                RgDataType::UInt2,
                RgValueType::UInt2([0; 2]),
                InputParamKind::ConstantOnly,
                true,
            );
        };
        let input_uint3 = |graph: &mut RgGraph, name: &str| {
            graph.add_input_param(
                node_id,
                name.to_string(),
                RgDataType::UInt3,
                RgValueType::UInt3([0; 3]),
                InputParamKind::ConstantOnly,
                true,
            );
        };
        let input_float = |graph: &mut RgGraph, name: &str| {
            graph.add_input_param(
                node_id,
                name.to_string(),
                RgDataType::Float,
                RgValueType::Float(0.0),
                InputParamKind::ConstantOnly,
                true,
            );
        };
        let input_bool = |graph: &mut RgGraph, name: &str| {
            graph.add_input_param(
                node_id,
                name.to_string(),
                RgDataType::Bool,
                RgValueType::Bool(true),
                InputParamKind::ConstantOnly,
                true,
            );
        };

        let input_screen_tex_resolution = |graph: &mut RgGraph, name: &str| {
            graph.add_input_param(
                node_id,
                name.to_string(),
                RgDataType::ScreenTexResolution,
                RgValueType::ScreenTexResolution(ScreenTexResolution::default()),
                InputParamKind::ConstantOnly,
                true,
            );
        };

        let input_tex_format = |graph: &mut RgGraph, name: &str| {
            graph.add_input_param(
                node_id,
                name.to_string(),
                RgDataType::TextureFormat,
                RgValueType::TextureFormat(BasicColorTextureFormat::default()),
                InputParamKind::ConstantOnly,
                true,
            );
        };

        let input_tex_usage = |graph: &mut RgGraph, name: &str| {
            graph.add_input_param(
                node_id,
                name.to_string(),
                RgDataType::TextureUsage,
                RgValueType::TextureUsage(TextureUsage::default()),
                InputParamKind::ConstantOnly,
                true,
            );
        };

        let input_tex_2d = |graph: &mut RgGraph, name: &str| {
            graph.add_input_param(
                node_id,
                name.to_string(),
                RgDataType::Tex2D,
                RgValueType::Tex2D(Tex2D::default()),
                InputParamKind::ConnectionOnly,
                true,
            );
        };
        let input_tex_2d_array = |graph: &mut RgGraph, name: &str| {
            graph.add_input_param(
                node_id,
                name.to_string(),
                RgDataType::Tex2DArray,
                RgValueType::Tex2DArray(Tex2DArray::default()),
                InputParamKind::ConnectionOnly,
                true,
            );
        };
        let input_tex_3d = |graph: &mut RgGraph, name: &str| {
            graph.add_input_param(
                node_id,
                name.to_string(),
                RgDataType::Tex3D,
                RgValueType::Tex3D(Tex3D::default()),
                InputParamKind::ConnectionOnly,
                true,
            );
        };
        let input_code_file = |graph: &mut RgGraph, name: &str| {
            graph.add_input_param(
                node_id,
                name.to_string(),
                RgDataType::CodeFile,
                RgValueType::CodeFile(None),
                InputParamKind::ConstantOnly,
                true,
            );
        };

        let output_tex_2d = |graph: &mut RgGraph, name: &str| {
            graph.add_output_param(node_id, name.to_string(), RgDataType::Tex2D);
        };
        let output_tex_2d_array = |graph: &mut RgGraph, name: &str| {
            graph.add_output_param(node_id, name.to_string(), RgDataType::Tex2DArray);
        };
        let output_tex_3d = |graph: &mut RgGraph, name: &str| {
            graph.add_output_param(node_id, name.to_string(), RgDataType::Tex3D);
        };
        let output_buffer = |graph: &mut RgGraph, name: &str| {
            graph.add_output_param(node_id, name.to_string(), RgDataType::Buffer);
        };

        match self {
            RgNodeTemplate::ScreenTex => {
                input_screen_tex_resolution(graph, "resolution");
                input_uint(graph, "mips");
                input_tex_format(graph, "format");
                input_tex_usage(graph, "usage");
                input_bool(graph, "persistent");
                output_tex_2d(graph, "tex");
            }
            RgNodeTemplate::HistoryScreenTex => {
                input_screen_tex_resolution(graph, "resolution");
                input_uint(graph, "mips");
                input_tex_format(graph, "format");
                input_tex_usage(graph, "usage");
                output_tex_2d(graph, "current tex");
                output_tex_2d(graph, "previous tex");
            }
            RgNodeTemplate::Tex2D => {
                input_uint2(graph, "resolution");
                input_uint(graph, "mips");
                input_tex_format(graph, "format");
                input_tex_usage(graph, "usage");
                input_bool(graph, "persistent");
                output_tex_2d(graph, "tex");
            }
            RgNodeTemplate::HistoryTex2D => {
                input_uint2(graph, "resolution");
                input_uint(graph, "mips");
                input_tex_format(graph, "format");
                input_tex_usage(graph, "usage");
                output_tex_2d(graph, "current tex");
                output_tex_2d(graph, "previous tex");
            }
            RgNodeTemplate::Tex2DArray => {
                input_uint2(graph, "resolution");
                input_uint(graph, "count");
                input_uint(graph, "mips");
                input_tex_format(graph, "format");
                input_bool(graph, "persistent");
                output_tex_2d_array(graph, "tex");
            }
            RgNodeTemplate::Tex3D => {
                input_uint3(graph, "resolution");
                input_uint(graph, "mips");
                input_tex_format(graph, "format");
                input_bool(graph, "persistent");
                output_tex_3d(graph, "tex");
            }
            RgNodeTemplate::HistoryTex3D => {
                input_uint3(graph, "resolution");
                input_uint(graph, "mips");
                input_tex_format(graph, "format");
                output_tex_3d(graph, "current tex");
                output_tex_3d(graph, "previous tex");
            }
            RgNodeTemplate::Buffer => {
                input_uint(graph, "size");
                input_bool(graph, "persistent");
                output_buffer(graph, "buf");
            }
            RgNodeTemplate::HistoryBuffer => {
                input_uint(graph, "size");
                output_buffer(graph, "current buf");
                output_buffer(graph, "previous buf");
            }
            RgNodeTemplate::GraphicsPass => {
                input_code_file(graph, "code");
                input_tex_2d(graph, "in");
                output_tex_2d(graph, "out");
            }
            RgNodeTemplate::DisplayOut => {
                input_tex_2d(graph, "in");
            }
        }
    }
}

pub struct AllMyNodeTemplates;
impl NodeTemplateIter for AllMyNodeTemplates {
    type Item = RgNodeTemplate;

    fn all_kinds(&self) -> Vec<Self::Item> {
        Self::Item::iter().collect()
    }
}

impl WidgetValueTrait for RgValueType {
    type Response = MyResponse;
    type UserState = RgGraphState;
    type NodeData = RgNodeData;
    fn value_widget(
        &mut self,
        param_name: &str,
        _node_id: NodeId,
        ui: &mut egui::Ui,
        user_state: &mut RgGraphState,
        _node_data: &RgNodeData,
    ) -> Vec<MyResponse> {
        // This trait is used to tell the library which UI to display for the
        // inline parameter widgets.
        match self {
            Self::UInt(value) => {
                ui.horizontal(|ui| {
                    ui.label(param_name);
                    ui.add(egui::DragValue::new(value));
                });
            }
            Self::UInt2(value) => {
                ui.label(param_name);
                ui.horizontal(|ui| {
                    ui.label("x");
                    ui.add(egui::DragValue::new(&mut value[0]));
                    ui.label("y");
                    ui.add(egui::DragValue::new(&mut value[1]));
                });
            }
            Self::UInt3(value) => {
                ui.label(param_name);
                ui.horizontal(|ui| {
                    ui.label("x");
                    ui.add(egui::DragValue::new(&mut value[0]));
                    ui.label("y");
                    ui.add(egui::DragValue::new(&mut value[1]));
                    ui.label("z");
                    ui.add(egui::DragValue::new(&mut value[2]));
                });
            }
            Self::Float(value) => {
                ui.horizontal(|ui| {
                    ui.label(param_name);
                    ui.add(egui::DragValue::new(value));
                });
            }
            Self::Bool(value) => {
                ui.horizontal(|ui| {
                    ui.label(param_name);
                    ui.add(egui::Checkbox::new(value, ""));
                });
            }
            Self::ScreenTexResolution(value) => {
                ui.horizontal(|ui| {
                    ui.label(param_name);
                    egui::ComboBox::from_id_salt(param_name)
                        .selected_text(value.to_string())
                        .show_ui(ui, |ui| {
                            for variant in ScreenTexResolution::iter() {
                                ui.selectable_value(value, variant, variant.to_string());
                            }
                        });
                });
            }
            Self::TextureFormat(value) => {
                ui.horizontal(|ui| {
                    ui.label(param_name);
                    egui::ComboBox::from_id_salt(param_name)
                        .selected_text(value.to_string())
                        .show_ui(ui, |ui| {
                            for variant in BasicColorTextureFormat::iter() {
                                ui.selectable_value(value, variant, variant.to_string());
                            }
                        });
                });
            }
            Self::TextureUsage(value) => {
                ui.horizontal(|ui| {
                    ui.label(param_name);
                    egui::ComboBox::from_id_salt(param_name)
                        .selected_text(value.to_string())
                        .show_ui(ui, |ui| {
                            for variant in TextureUsage::iter() {
                                ui.selectable_value(value, variant, variant.to_string());
                            }
                        });
                });
            }
            Self::CodeFile(value) => {
                let editor = user_state.editor.as_mut().unwrap();
                let code_file_names = &editor.code_file_names;
                let drag_payload = &mut editor.drag_payload;

                ui.horizontal(|ui| {
                    ui.label(param_name);

                    let drop_target = ui.group(|ui| {
                        let code_file_name = if let Some(value) = value {
                            if let Some((_ty, name)) = code_file_names.get(value) {
                                name.file_name()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_default()
                            } else {
                                "Missing".to_string()
                            }
                        } else {
                            "None".to_string()
                        };

                        ui.label(code_file_name)
                    });

                    let pointer_pos = ui.ctx().pointer_hover_pos();
                    let is_hovered = match pointer_pos {
                        Some(pos) => drop_target.response.rect.contains(pos),
                        None => false,
                    };

                    if let Some(EditorDragPayload::CodeFile(id, ..)) = drag_payload {
                        if let Some((ty, _)) = code_file_names.get(id) {
                            if *ty == CodeFileType::Fragment {
                                ui.painter().rect_stroke(
                                    drop_target.response.rect,
                                    0.0,
                                    egui::Stroke::new(1.0, egui::Color32::LIGHT_BLUE),
                                    egui::StrokeKind::Middle,
                                );

                                if is_hovered {
                                    ui.painter().rect_stroke(
                                        drop_target.response.rect,
                                        0.0,
                                        egui::Stroke::new(1.0, egui::Color32::DARK_BLUE),
                                        egui::StrokeKind::Middle,
                                    );

                                    if ui.input(|i| i.pointer.primary_released()) {
                                        *value = Some(*id);
                                        *drag_payload = None;
                                    }
                                }
                            }
                        }
                    }
                });
            }
            Self::Tex2D(_) | Self::Tex2DArray(_) | Self::Tex3D(_) | Self::Buffer(_) => {
                ui.label(param_name);
            }
        }
        // This allows you to return your responses from the inline widgets.
        Vec::new()
    }
}

impl UserResponseTrait for MyResponse {}
impl NodeDataTrait for RgNodeData {
    type Response = MyResponse;
    type UserState = RgGraphState;
    type DataType = RgDataType;
    type ValueType = RgValueType;

    // This method will be called when drawing each node. This allows adding
    // extra ui elements inside the nodes. In this case, we create an "active"
    // button which introduces the concept of having an active node in the
    // graph. This is done entirely from user code with no modifications to the
    // node graph library.
    fn bottom_ui(
        &self,
        ui: &mut egui::Ui,
        node_id: NodeId,
        _graph: &Graph<RgNodeData, RgDataType, RgValueType>,
        user_state: &mut Self::UserState,
    ) -> Vec<NodeResponse<MyResponse, RgNodeData>>
    where
        MyResponse: UserResponseTrait,
    {
        // This logic is entirely up to the user. In this case, we check if the
        // current node we're drawing is the active one, by comparing against
        // the value stored in the global user state, and draw different button
        // UIs based on that.

        let mut responses = vec![];
        let is_active = user_state
            .inspect_node
            .map(|id| id == node_id)
            .unwrap_or(false);

        // Pressing the button will emit a custom user response to either set,
        // or clear the active node. These responses do nothing by themselves,
        // the library only makes the responses available to you after the graph
        // has been drawn. See below at the update method for an example.
        if !is_active {
            if ui
                .button(egui::RichText::new(format!(
                    "{} Inspect",
                    egui_phosphor::regular::EYE
                )))
                .clicked()
            {
                responses.push(NodeResponse::User(MyResponse::SetInspectNode(node_id)));
            }
        } else {
            let button = egui::Button::new(
                egui::RichText::new(format!("{} Inspect", egui_phosphor::regular::EYE_SLASH))
                    .color(egui::Color32::BLACK),
            )
            .fill(egui::Color32::GOLD);
            if ui.add(button).clicked() {
                responses.push(NodeResponse::User(MyResponse::ClearInspectNode));
            }
        }

        responses
    }
}

pub type RgEditorState =
    GraphEditorState<RgNodeData, RgDataType, RgValueType, RgNodeTemplate, RgGraphState>;

pub struct RenderGraphTab {
    id: Uuid,
}

impl Default for RenderGraphTab {
    fn default() -> Self {
        Self { id: Uuid::new_v4() }
    }
}

impl RenderGraphTab {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        project: &mut Project,
        drag_payload: &mut Option<EditorDragPayload>,
    ) -> bool {
        let code_file_names: std::collections::HashMap<Uuid, (CodeFileType, std::path::PathBuf)> =
            project
                .code_files
                .files_iter()
                .map(|(id, file)| (*id, (file.ty(), file.relative_path().clone())))
                .collect();

        project
            .render_graph_mut()
            .ui(ui, code_file_names, drag_payload)
    }
}
