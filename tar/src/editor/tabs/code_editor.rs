use uuid::Uuid;

use crate::{
    editor::code_editor::{syntax::Syntax, themes::ColorTheme, CodeEditor},
    egui_util::KeyModifiers,
    project::{CodeFile, Project},
};

pub struct CodeEditorTab {
    id: Uuid,
    title: String,
    code_editor: CodeEditor,
}

impl PartialEq for CodeEditorTab {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl CodeEditorTab {
    pub fn new(code_file: &CodeFile) -> Self {
        let code_editor =
            CodeEditor::new(&code_file.source, ColorTheme::GITHUB_DARK, Syntax::wgsl());

        let title = code_file
            .relative_path()
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        Self {
            id: code_file.id(),
            title,
            code_editor,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, project: &mut Project, key_modifiers: &KeyModifiers) {
        if let Some(code_file) = project.code_files.get_file(self.id) {
            self.title = code_file
                .relative_path()
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            self.code_editor.ui(ui, key_modifiers);
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("Unable to locate file, it was probably removed.");
            });
        }
    }
}
