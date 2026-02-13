use egui_tiles::TileId;
use uuid::Uuid;

use crate::{
    editor::{
        tabs::{
            code_editor::CodeEditorTab, console::ConsoleTab, file_explorer::FileExplorerTab,
            render_graph::RenderGraphTab, viewport::ViewportTab,
        },
        EditorDragPayload,
    },
    egui_util::KeyModifiers,
    project::Project,
};

pub mod code_editor;
pub mod console;
pub mod file_explorer;
pub mod render_graph;
pub mod viewport;

#[allow(clippy::large_enum_variant)]
pub enum Tab {
    CodeEditor(CodeEditorTab),
    Console(ConsoleTab),
    FileExplorer(FileExplorerTab),
    RenderGraph(RenderGraphTab),
    Viewport(ViewportTab),
}

impl Tab {
    pub fn variant_eq(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

impl std::fmt::Display for Tab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CodeEditor(code_editor) => {
                write!(
                    f,
                    "{} {}",
                    egui_phosphor::regular::CODE,
                    code_editor.title()
                )
            }
            Self::Console(_) => {
                write!(f, "{} Console", egui_phosphor::regular::TEXT_ALIGN_LEFT)
            }
            Self::FileExplorer(_) => {
                write!(f, "{} File Explorer", egui_phosphor::regular::FOLDER)
            }
            Self::RenderGraph(_) => {
                write!(f, "{} Render Graph", egui_phosphor::regular::BLUEPRINT)
            }
            Self::Viewport(_) => {
                write!(f, "{} Viewport", egui_phosphor::regular::MONITOR_PLAY)
            }
        }
    }
}

pub struct TabViewer<'a> {
    key_modifiers: &'a KeyModifiers,
    project: &'a mut Project,
    drag_payload: &'a mut Option<EditorDragPayload>,
    file_to_open: &'a mut Option<Uuid>,
    last_focussed_code_editor: &'a mut Option<TileId>,
}

impl<'a> TabViewer<'a> {
    pub fn new(
        key_modifiers: &'a KeyModifiers,
        project: &'a mut Project,
        drag_payload: &'a mut Option<EditorDragPayload>,
        file_to_open: &'a mut Option<Uuid>,
        last_focussed_code_editor: &'a mut Option<TileId>,
    ) -> Self {
        Self {
            key_modifiers,
            project,
            drag_payload,
            file_to_open,
            last_focussed_code_editor,
        }
    }
}

impl<'a> egui_tiles::Behavior<Tab> for TabViewer<'a> {
    fn tab_title_for_pane(&mut self, tab: &Tab) -> egui::WidgetText {
        tab.to_string().into()
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        tile_id: egui_tiles::TileId,
        tab: &mut Tab,
    ) -> egui_tiles::UiResponse {
        match tab {
            Tab::Viewport(tab) => {
                tab.ui(ui);
            }
            Tab::Console(tab) => {
                tab.ui(ui);
            }
            Tab::FileExplorer(tab) => {
                tab.ui(ui, self.project, self.drag_payload, self.file_to_open);
            }
            Tab::RenderGraph(tab) => {
                tab.ui(ui, self.project);
            }
            Tab::CodeEditor(tab) => {
                tab.ui(ui, self.project, self.key_modifiers);

                if tab.has_focus() {
                    *self.last_focussed_code_editor = Some(tile_id);
                }
            }
        }

        Default::default()
    }

    fn is_tab_closable(&self, tiles: &egui_tiles::Tiles<Tab>, tile_id: egui_tiles::TileId) -> bool {
        if let Some(egui_tiles::Tile::Pane(tab)) = tiles.get(tile_id) {
            matches!(tab, Tab::CodeEditor(_))
        } else {
            false
        }
    }

    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            all_panes_must_have_tabs: true,
            ..Default::default()
        }
    }
}
