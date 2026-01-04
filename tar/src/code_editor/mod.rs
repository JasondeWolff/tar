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

fn leading_whitespace(s: &str) -> &str {
    let count = s.chars().take_while(|c| *c == ' ' || *c == '\t').count();
    &s[..count]
}

fn line_len_without_newline(line: ropey::RopeSlice) -> usize {
    let len = line.len_chars();
    if len > 0 && line.char(len - 1) == '\n' {
        len - 1
    } else {
        len
    }
}

pub struct CodeEditor {
    pub doc: Rope,
    pub cursor: usize,
    cursor_blink_offset: f64,
    desired_column: Option<usize>,
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
            desired_column: None,
            selection: None,
            theme,
            syntax,
            fontsize: 14.0,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.draw_editor(ui);
            });
    }

    pub fn draw_editor(&mut self, ui: &mut egui::Ui) {
        // --- Build text layout ---
        let mut source = self.doc.to_string();
        // TODO: better fix
        if source.is_empty() {
            source = "\n".to_owned();
        }

        let layout_job = highlight(ui.ctx(), self, &source);
        let galley = ui.fonts_mut(|f| f.layout_job(layout_job));

        // --- Allocate base rect ---
        let font_id = egui::FontId::monospace(self.fontsize);

        let mut width = ui.available_width();
        // Estimate max line width (cheap but effective)
        let max_line_width = self
            .doc
            .lines()
            .map(|l| {
                ui.fonts_mut(|f| {
                    f.layout_no_wrap(l.to_string(), font_id.clone(), egui::Color32::WHITE)
                        .size()
                        .x
                })
            })
            .fold(0.0, f32::max);
        width = width.max(max_line_width + 200.0);

        let height = galley.rows.last().unwrap().max_y();

        let desired_size = egui::vec2(width, height);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());

        let painter = ui.painter_at(rect);

        // --- Render background ---
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(25, 25, 25));

        // --- Render line highlights ---
        let (cursor_line, _) = char_to_line_col(&self.doc, self.cursor);

        let total_lines = self.doc.len_lines().max(1);
        let digits = total_lines.ilog10() + 1;
        let gutter_padding = 8.0;

        let digit_width = ui.fonts_mut(|f| {
            f.layout_no_wrap("0".to_string(), font_id.clone(), egui::Color32::GRAY)
                .size()
                .x
        });
        let gutter_width = digit_width * digits as f32 + gutter_padding * 2.0;

        for (row_idx, row) in galley.rows.iter().enumerate() {
            let is_current = row_idx == cursor_line;

            if is_current {
                let highlight_rect = egui::Rect::from_min_max(
                    egui::pos2(rect.min.x, rect.min.y + row.min_y()),
                    egui::pos2(rect.max.x, rect.min.y + row.max_y()),
                );
                painter.rect_filled(highlight_rect, 0.0, egui::Color32::from_rgb(35, 35, 35));
            }

            // --- Line numbers ---
            let line_number = (row_idx + 1).to_string();
            let color = if is_current {
                Color32::WHITE
            } else {
                Color32::from_gray(140)
            };

            let text_size = ui.fonts_mut(|f| {
                f.layout_no_wrap(line_number.clone(), font_id.clone(), color)
                    .size()
            });

            let x = rect.min.x + gutter_width - gutter_padding - text_size.x;
            painter.text(
                egui::pos2(x, rect.min.y + row.min_y()),
                egui::Align2::LEFT_TOP,
                line_number,
                font_id.clone(),
                color,
            );
        }

        // --- Render text ---
        let text_x = rect.min.x + gutter_width + 6.0;
        let pos = egui::pos2(text_x, rect.min.y);
        painter.galley(pos, galley.clone(), Color32::WHITE);

        let time = ui.input(|i| i.time);

        let event_filter = egui::EventFilter {
            tab: true,
            vertical_arrows: true,
            horizontal_arrows: true,
            escape: false,
        };
        ui.memory_mut(|mem| mem.set_focus_lock_filter(response.id, event_filter));

        let response = response.on_hover_cursor(egui::CursorIcon::Text);

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
                let cursor_x = text_x
                    + ui.fonts_mut(|f| {
                        f.layout_no_wrap(
                            self.doc.line(cursor_line).slice(..cursor_col).to_string(),
                            font_id.clone(),
                            egui::Color32::WHITE,
                        )
                        .size()
                        .x
                    });

                let cursor_height = galley.rows[cursor_line].height();
                let cursor_y = rect.min.y + galley.rows[cursor_line].min_y();

                painter.line_segment(
                    [
                        egui::pos2(cursor_x, cursor_y),
                        egui::pos2(cursor_x, cursor_y + cursor_height),
                    ],
                    egui::Stroke::new(1.0, egui::Color32::WHITE),
                );
            }

            // --- Input ---
            let events = ui.input(|i| i.filtered_events(&event_filter));
            for event in events {
                match event {
                    egui::Event::Text(text) => {
                        self.doc.insert(self.cursor, &text);
                        self.cursor += text.chars().count();

                        self.desired_column = None;
                        self.cursor_blink_offset = time;
                    }
                    egui::Event::Key {
                        key: egui::Key::Enter,
                        pressed: true,
                        ..
                    } => {
                        let (line, _) = char_to_line_col(&self.doc, self.cursor);
                        let line_text = self.doc.line(line).to_string();
                        let indent = leading_whitespace(&line_text);

                        let insert_text = format!("\n{}", indent);
                        self.doc.insert(self.cursor, &insert_text);
                        self.cursor += insert_text.chars().count();

                        self.selection = None;
                        self.desired_column = None;
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
                            self.desired_column = None;
                            self.cursor_blink_offset = time;
                        }
                    }
                    egui::Event::Key {
                        key: egui::Key::Delete,
                        pressed: true,
                        ..
                    } => {
                        if self.cursor < self.doc.len_chars() {
                            self.doc.remove(self.cursor..self.cursor + 1);

                            self.selection = None;
                            self.desired_column = None;
                            self.cursor_blink_offset = time;
                        }
                    }
                    egui::Event::Key {
                        key: egui::Key::Tab,
                        pressed: true,
                        ..
                    } => {
                        self.doc.insert(self.cursor, "    ");
                        self.cursor += 4;

                        self.selection = None;
                        self.desired_column = None;
                        self.cursor_blink_offset = time;
                    }
                    egui::Event::Key {
                        key: egui::Key::ArrowLeft,
                        pressed: true,
                        ..
                    } => {
                        self.cursor = self.cursor.saturating_sub(1);

                        self.selection = None;
                        self.desired_column = None;
                        self.cursor_blink_offset = time;
                    }
                    egui::Event::Key {
                        key: egui::Key::ArrowRight,
                        pressed: true,
                        ..
                    } => {
                        self.cursor = (self.cursor + 1).min(self.doc.len_chars());

                        self.selection = None;
                        self.desired_column = None;
                        self.cursor_blink_offset = time;
                    }
                    egui::Event::Key {
                        key: egui::Key::ArrowUp,
                        pressed: true,
                        ..
                    } => {
                        let (line, col) = char_to_line_col(&self.doc, self.cursor);

                        if line > 0 {
                            let target_col = self.desired_column.unwrap_or(col);
                            let prev_line = self.doc.line(line - 1);
                            let prev_line_start = self.doc.line_to_char(line - 1);
                            let prev_line_len = line_len_without_newline(prev_line);

                            let new_col = target_col.min(prev_line_len);
                            self.cursor = prev_line_start + new_col;
                            self.desired_column = Some(target_col);
                        }

                        self.selection = None;
                        self.cursor_blink_offset = time;
                    }
                    egui::Event::Key {
                        key: egui::Key::ArrowDown,
                        pressed: true,
                        ..
                    } => {
                        let (line, col) = char_to_line_col(&self.doc, self.cursor);

                        if line + 1 < self.doc.len_lines() {
                            let target_col = self.desired_column.unwrap_or(col);
                            let next_line = self.doc.line(line + 1);
                            let next_line_start = self.doc.line_to_char(line + 1);
                            let next_line_len = line_len_without_newline(next_line);

                            let new_col = target_col.min(next_line_len);
                            self.cursor = next_line_start + new_col;
                            self.desired_column = Some(target_col);
                        }

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
