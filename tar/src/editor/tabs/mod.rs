use crate::{
    editor::tabs::{code_editor::CodeEditorTab, viewport::ViewportTab},
    egui_util::KeyModifiers,
};

pub mod code_editor;
pub mod viewport;

#[allow(clippy::large_enum_variant)]
pub enum Tab {
    CodeEditor(CodeEditorTab),
    Viewport(ViewportTab),
}

impl std::fmt::Display for Tab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CodeEditor(_) => {
                write!(f, "{} Code Editor", egui_phosphor::regular::CODEPEN_LOGO)
            }
            Self::Viewport(_) => {
                write!(f, "{} Viewport", egui_phosphor::regular::EYE)
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

impl egui_dock::TabViewer for TabViewer<'_> {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.to_string().into()
    }

    fn scroll_bars(&self, _tab: &Self::Tab) -> [bool; 2] {
        [false, false]
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            Tab::Viewport(tab) => {
                tab.ui(ui);
            }
            Tab::CodeEditor(tab) => {
                tab.ui(ui, self.key_modifiers);
            }
        }
    }

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        match tab {
            Tab::Viewport(tab) => egui::Id::new(format!("viewport_{}", tab.id())),
            Tab::CodeEditor(tab) => egui::Id::new(format!("code_editor_{}", tab.id())),
        }
    }
}
