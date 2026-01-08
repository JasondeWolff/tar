use crate::{
    editor::tabs::{
        code_editor::CodeEditorTab, console::ConsoleTab, file_explorer::FileExplorerTab,
        viewport::ViewportTab,
    },
    egui_util::KeyModifiers,
};

pub mod code_editor;
pub mod console;
pub mod file_explorer;
pub mod viewport;

#[allow(clippy::large_enum_variant)]
pub enum Tab {
    CodeEditor(CodeEditorTab),
    Console(ConsoleTab),
    FileExplorer(FileExplorerTab),
    Viewport(ViewportTab),
}

impl std::fmt::Display for Tab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CodeEditor(_) => {
                write!(f, "{} Code Editor", egui_phosphor::regular::CODE)
            }
            Self::Console(_) => {
                write!(f, "{} Console", egui_phosphor::regular::TERMINAL)
            }
            Self::FileExplorer(_) => {
                write!(f, "{} File Explorer", egui_phosphor::regular::FOLDER)
            }
            Self::Viewport(_) => {
                write!(f, "{} Viewport", egui_phosphor::regular::MONITOR_PLAY)
            }
        }
    }
}

pub struct TabViewer<'a> {
    key_modifiers: &'a KeyModifiers,
}

impl<'a> TabViewer<'a> {
    pub fn new(key_modifiers: &'a KeyModifiers) -> Self {
        Self { key_modifiers }
    }
}

impl<'a> egui_tiles::Behavior<Tab> for TabViewer<'a> {
    fn tab_title_for_pane(&mut self, tab: &Tab) -> egui::WidgetText {
        tab.to_string().into()
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
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
                tab.ui(ui);
            }
            Tab::CodeEditor(tab) => {
                tab.ui(ui, self.key_modifiers);
            }
        }

        Default::default()
    }

    fn is_tab_closable(
        &self,
        _tiles: &egui_tiles::Tiles<Tab>,
        _tile_id: egui_tiles::TileId,
    ) -> bool {
        true
    }

    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            all_panes_must_have_tabs: true,
            ..Default::default()
        }
    }
}
