// Modified version built on-top of Roman Chumaks egui_code_editor (https://github.com/p4ymak/egui_code_editor/)

use egui::text::LayoutJob;
use egui::Color32;
use ropey::Rope;
use std::hash::{Hash, Hasher};
use std::ops::Range;

use crate::code_editor::highlighting::highlight;
use crate::code_editor::{
    highlighting::Token,
    syntax::{Syntax, TokenType},
    themes::ColorTheme,
};

pub mod highlighting;
pub mod syntax;
pub mod themes;

fn char_to_line_col(doc: &Rope, char_idx: usize) -> (usize, usize) {
    let line = doc.char_to_line(char_idx);
    let line_start = doc.line_to_char(line);
    (line, char_idx - line_start)
}

pub struct CodeEditor {
    pub doc: Rope,
    pub cursor: usize, // char index
    cursor_blink_offset: f64,
    pub selection: Option<Range<usize>>,

    theme: ColorTheme,
    syntax: Syntax,
    fontsize: f32,
}

impl CodeEditor {
    pub fn new(text: &str, theme: ColorTheme, syntax: Syntax) -> Self {
        Self {
            doc: Rope::from_str(text),
            cursor: 0,
            cursor_blink_offset: 0.0,
            selection: None,
            theme,
            syntax,
            fontsize: 14.0,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let font_id = egui::FontId::monospace(self.fontsize);
        let line_height = ui.fonts_mut(|f| f.row_height(&font_id));

        let desired_size = egui::vec2(
            ui.available_width(),
            line_height * self.doc.len_lines() as f32,
        );
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());

        let painter = ui.painter_at(rect);

        // Background
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(25, 25, 25));

        // --- Render text line by line ---
        let source = self.doc.to_string();
        let layout_job = highlight(ui.ctx(), self, &source);
        let galley = ui.fonts_mut(|f| f.layout_job(layout_job));

        let pos = egui::pos2(rect.min.x + 6.0, rect.min.y);
        painter.galley(pos, galley, Color32::WHITE);

        let time = ui.input(|i| i.time);

        if response.clicked() {
            ui.memory_mut(|m| m.request_focus(response.id));
            self.cursor_blink_offset = time;
        }

        if response.has_focus() {
            // --- Cursor ---
            const BLINK_SPEED: f64 = 0.530 * 2.0;
            let cursor_visible =
                ((time - self.cursor_blink_offset) % BLINK_SPEED) < (BLINK_SPEED * 0.5);

            if cursor_visible {
                let (cursor_line, cursor_col) = char_to_line_col(&self.doc, self.cursor);
                let cursor_x = rect.min.x
                    + 6.0
                    + ui.fonts_mut(|f| {
                        f.layout_no_wrap(
                            self.doc.line(cursor_line).slice(..cursor_col).to_string(),
                            font_id.clone(),
                            egui::Color32::WHITE,
                        )
                        .size()
                        .x
                    });

                let cursor_y = rect.min.y + cursor_line as f32 * line_height;

                painter.line_segment(
                    [
                        egui::pos2(cursor_x, cursor_y),
                        egui::pos2(cursor_x, cursor_y + line_height),
                    ],
                    egui::Stroke::new(1.0, egui::Color32::WHITE),
                );
            }

            // --- Input ---
            for event in ui.input(|i| i.events.clone()) {
                match event {
                    egui::Event::Text(text) => {
                        self.doc.insert(self.cursor, &text);
                        self.cursor += text.chars().count();

                        self.cursor_blink_offset = time;
                    }
                    egui::Event::Key {
                        key: egui::Key::Backspace,
                        pressed: true,
                        ..
                    } => {
                        if self.cursor > 0 {
                            self.doc.remove((self.cursor - 1)..self.cursor);
                            self.cursor = self.cursor.saturating_sub(1);
                            self.selection = None;

                            self.cursor_blink_offset = time;
                        }
                    }
                    egui::Event::Key {
                        key: egui::Key::ArrowLeft,
                        pressed: true,
                        ..
                    } => {
                        self.cursor = self.cursor.saturating_sub(1);
                        self.selection = None;

                        self.cursor_blink_offset = time;
                    }
                    egui::Event::Key {
                        key: egui::Key::ArrowRight,
                        pressed: true,
                        ..
                    } => {
                        self.cursor = (self.cursor + 1).min(self.doc.len_chars());
                        self.selection = None;

                        self.cursor_blink_offset = time;
                    }
                    _ => {}
                }
            }
        }
    }

    fn format_token(&self, ty: TokenType) -> egui::text::TextFormat {
        let font_id = egui::FontId::monospace(self.fontsize);
        let color = self.theme.type_color(ty);
        egui::text::TextFormat::simple(font_id, color)
    }

    fn append(&self, job: &mut LayoutJob, token: &Token) {
        if !token.buffer().is_empty() {
            job.append(token.buffer(), 0.0, self.format_token(token.ty()));
        }
    }

    fn syntax(&self) -> &Syntax {
        &self.syntax
    }
}

impl Hash for CodeEditor {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.theme.hash(state);
        (self.fontsize as u32).hash(state);
        self.syntax.hash(state);
    }
}
