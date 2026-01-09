use uuid::Uuid;

use crate::{
    editor::code_editor::{syntax::Syntax, themes::ColorTheme, CodeEditor},
    egui_util::KeyModifiers,
};

const DEFAULT_CODE: &str = r#"struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) tex_coords: vec2f,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var result: VertexOutput;
    let x = i32(vertex_index) / 2;
    let y = i32(vertex_index) & 1;
    let tc = vec2f(
        f32(x) * 2.0,
        f32(y) * 2.0
    );
    result.position = vec4f(
        tc.x * 2.0 - 1.0,
        1.0 - tc.y * 2.0,
        0.0, 1.0
    );
    result.tex_coords = tc;
    return result;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4f {
    return vec4f(vertex.tex_coord, 0.0, 1.0);
}
"#;

pub struct CodeEditorTab {
    id: Uuid,
    code_editor: CodeEditor,
}

impl PartialEq for CodeEditorTab {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Default for CodeEditorTab {
    fn default() -> Self {
        let code_editor = CodeEditor::new(DEFAULT_CODE, ColorTheme::GITHUB_DARK, Syntax::wgsl());
        Self {
            id: Uuid::new_v4(),
            code_editor,
        }
    }
}

impl CodeEditorTab {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, key_modifiers: &KeyModifiers) {
        self.code_editor.ui(ui, key_modifiers);
    }
}
