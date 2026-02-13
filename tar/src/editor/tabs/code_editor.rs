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
    has_focus: bool,
    saved_source_code_hash: u64,
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

        let saved_source_code_hash = code_editor.doc_hash();

        Self {
            id: code_file.id(),
            title,
            code_editor,
            has_focus: false,
            saved_source_code_hash,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn title(&self) -> String {
        if self.source_code_changed() {
            format!("{}*", self.title)
        } else {
            self.title.clone()
        }
    }

    pub fn has_focus(&self) -> bool {
        self.has_focus
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, project: &mut Project, key_modifiers: &KeyModifiers) {
        if let Some(code_file) = project.code_files.get_file(self.id) {
            self.title = code_file
                .relative_path()
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            self.has_focus = self.code_editor.ui(ui, key_modifiers);
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("Unable to locate file, it was probably removed.");
            });
        }
    }

    pub fn source_code_changed(&self) -> bool {
        self.saved_source_code_hash != self.code_editor.doc_hash()
    }

    pub fn save_to_project(&mut self, project: &mut Project) {
        if let Err(e) = project
            .code_files
            .set_source(self.id, self.code_editor.doc.to_string())
        {
            log::warn!("Failed to save file: {e}");
        }

        self.saved_source_code_hash = self.code_editor.doc_hash();
    }
}
