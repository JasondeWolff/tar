// Modified version built on-top of Roman Chumaks egui_code_editor (https://github.com/p4ymak/egui_code_editor/)

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
use crate::egui_util::KeyModifiers;

pub mod highlighting;
pub mod syntax;
pub mod themes;

const INDENT: &str = "    ";
const INDENT_WIDTH: usize = 4;

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

fn y_to_row_index(y: f32, galley: &egui::Galley) -> usize {
    for (i, row) in galley.rows.iter().enumerate() {
        if y >= row.min_y() && y < row.max_y() {
            return i;
        }
    }

    galley.rows.len().saturating_sub(1)
}

fn selection_line_range(
    doc: &ropey::Rope,
    sel: &std::ops::Range<usize>,
) -> std::ops::RangeInclusive<usize> {
    let start_line = doc.char_to_line(sel.start);
    let mut end_line = doc.char_to_line(sel.end);

    // If selection ends exactly at start of a line, don't include that line
    if sel.end > sel.start && doc.line_to_char(end_line) == sel.end {
        end_line = end_line.saturating_sub(1);
    }

    start_line..=end_line
}

fn safe_char_to_line(doc: &Rope, char_idx: usize) -> usize {
    if char_idx >= doc.len_chars() {
        doc.len_lines().saturating_sub(1)
    } else {
        doc.char_to_line(char_idx)
    }
}

pub struct CodeEditor {
    pub doc: Rope,
    pub cursor: usize,
    cursor_blink_offset: f64,
    desired_column: Option<usize>,
    pub selection: Option<Range<usize>>,
    selection_anchor: Option<usize>,

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
            selection_anchor: None,
            theme,
            syntax,
            fontsize: 14.0,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, key_modifiers: &KeyModifiers) {
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.draw_editor(ui, key_modifiers);
            });
    }

    pub fn draw_editor(&mut self, ui: &mut egui::Ui, key_modifiers: &KeyModifiers) {
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
        let (rect, mut response) =
            ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());
        response.flags -= egui::response::Flags::FAKE_PRIMARY_CLICKED;

        let painter = ui.painter_at(rect);

        // --- Render background ---
        painter.rect_filled(rect, 0.0, self.theme.bg());

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

        // --- Render selection background ---
        let text_x = rect.min.x + gutter_width + 6.0;

        if let Some(selection) = &self.selection {
            let (start_line, start_col) = char_to_line_col(&self.doc, selection.start);
            let (end_line, end_col) = char_to_line_col(&self.doc, selection.end);

            for row_idx in start_line..=end_line {
                let row = &galley.rows[row_idx];
                let row_start_char = self.doc.line_to_char(row_idx);
                let row_end_char =
                    row_start_char + line_len_without_newline(self.doc.line(row_idx));

                // Compute selection range within this row
                let sel_start_col = if row_idx == start_line { start_col } else { 0 };
                let sel_end_col = if row_idx == end_line {
                    end_col
                } else {
                    row_end_char - row_start_char
                };

                if sel_start_col >= sel_end_col {
                    continue;
                }

                // Compute x coordinates using the font layout
                let x_start = text_x
                    + ui.fonts_mut(|f| {
                        f.layout_no_wrap(
                            self.doc.line(row_idx).slice(..sel_start_col).to_string(),
                            font_id.clone(),
                            Color32::WHITE,
                        )
                        .size()
                        .x
                    });
                let x_end = text_x
                    + ui.fonts_mut(|f| {
                        f.layout_no_wrap(
                            self.doc.line(row_idx).slice(..sel_end_col).to_string(),
                            font_id.clone(),
                            Color32::WHITE,
                        )
                        .size()
                        .x
                    });

                let selection_rect = egui::Rect::from_min_max(
                    egui::pos2(x_start, rect.min.y + row.min_y()),
                    egui::pos2(x_end, rect.min.y + row.max_y()),
                );

                painter.rect_filled(selection_rect, 0.0, self.theme.selection());
            }
        }

        // --- Render text ---
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

        // --- Mouse input ---
        if ui.input(|i| i.pointer.any_pressed()) {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                // --- Convert pointer position to char index ---
                let y = pos.y - rect.min.y;
                let line = y_to_row_index(y, &galley);

                let line_text = self.doc.line(line);
                let max_col = line_len_without_newline(line_text);
                let mut x = 0.0;
                let mut col = 0;

                for (i, c) in line_text.chars().take(max_col).enumerate() {
                    let cw = ui.fonts_mut(|f| {
                        f.layout_no_wrap(c.to_string(), font_id.clone(), Color32::WHITE)
                            .size()
                            .x
                    });
                    if text_x + x + cw / 2.0 >= pos.x {
                        col = i;
                        break;
                    }
                    x += cw;
                    col = i + 1;
                }
                col = col.min(max_col);

                let char_idx = self.doc.line_to_char(line) + col;

                // --- Update editor state ---
                self.cursor = char_idx;
                self.desired_column = Some(col);
                self.selection = None; // clear any selection
                self.selection_anchor = None; // clear drag anchor
                self.cursor_blink_offset = time;

                ui.memory_mut(|m| m.request_focus(response.id));
            }
        }

        if response.dragged() {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                let (drag_line, drag_col) = {
                    let y = pos.y - rect.min.y;
                    let line = y_to_row_index(y, &galley);

                    let line_text = self.doc.line(line);
                    let max_col = line_len_without_newline(line_text);
                    let mut x = 0.0;
                    let mut col = 0;

                    for (i, c) in line_text.chars().take(max_col).enumerate() {
                        let cw = ui.fonts_mut(|f| {
                            f.layout_no_wrap(c.to_string(), font_id.clone(), Color32::WHITE)
                                .size()
                                .x
                        });
                        if text_x + x + cw / 2.0 >= pos.x {
                            col = i;
                            break;
                        }
                        x += cw;
                        col = i + 1;
                    }
                    col = col.min(max_col);
                    (line, col)
                };
                let char_idx = self.doc.line_to_char(drag_line) + drag_col;

                // Set anchor on first drag
                if self.selection_anchor.is_none() {
                    self.selection_anchor = Some(self.cursor);
                }

                let anchor = self.selection_anchor.unwrap();
                self.selection = Some(anchor.min(char_idx)..anchor.max(char_idx));

                // Update cursor to follow mouse
                self.cursor = char_idx;
                self.desired_column = Some(drag_col);
                self.cursor_blink_offset = time;
            }
        } else {
            // Clear anchor when not dragging
            self.selection_anchor = None;
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
                    egui::Stroke::new(1.0, self.theme.cursor()),
                );
            }

            // --- Input ---
            let events = ui.input(|i| i.filtered_events(&event_filter));
            for event in events {
                match event {
                    egui::Event::Text(text) => {
                        if key_modifiers.ctrl {
                            #[cfg(target_os = "android")]
                            match text.as_str() {
                                "c" | "C" => self.copy(ui),
                                "v" | "V" => {
                                    if let Ok(text) = android_clipboard::get_text() {
                                        self.paste(ui, text);
                                    }
                                }
                                "x" | "X" => self.cut(ui),
                                _ => {}
                            }
                        } else {
                            if let Some(selection) = &self.selection {
                                if selection.start != selection.end {
                                    self.doc.remove(selection.start..selection.end);
                                    self.cursor = selection.start;
                                }
                            }

                            self.doc.insert(self.cursor, &text);
                            self.cursor += text.chars().count();

                            self.selection = None;
                            self.desired_column = None;
                            self.cursor_blink_offset = time;
                        }
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
                        if let Some(selection) = &self.selection {
                            if selection.start != selection.end {
                                self.doc.remove(selection.start..selection.end);
                                self.cursor = selection.start;
                            }
                        } else if self.cursor > 0 {
                            self.doc.remove((self.cursor - 1)..self.cursor);
                            self.cursor = self.cursor.saturating_sub(1);
                        }

                        self.selection = None;
                        self.desired_column = None;
                        self.cursor_blink_offset = time;
                    }
                    egui::Event::Key {
                        key: egui::Key::Delete,
                        pressed: true,
                        ..
                    } => {
                        if let Some(selection) = &self.selection {
                            if selection.start != selection.end {
                                self.doc.remove(selection.start..selection.end);
                                self.cursor = selection.start;
                            }
                        } else if self.cursor < self.doc.len_chars() {
                            self.doc.remove(self.cursor..self.cursor + 1);
                        }

                        self.selection = None;
                        self.desired_column = None;
                        self.cursor_blink_offset = time;
                    }
                    egui::Event::Key {
                        key: egui::Key::Tab,
                        pressed: true,
                        modifiers,
                        ..
                    } => {
                        let shift = modifiers.shift;

                        if let Some(selection) = self.selection.clone() {
                            let line_range = selection_line_range(&self.doc, &selection);

                            if shift {
                                // ---- SHIFT+TAB : UNINDENT ----
                                let mut removed_total = 0;

                                for line in line_range.clone() {
                                    let line_start = self.doc.line_to_char(line);
                                    let line_text = self.doc.line(line);

                                    let remove_count = line_text
                                        .chars()
                                        .take_while(|c| *c == ' ')
                                        .take(INDENT_WIDTH)
                                        .count();

                                    if remove_count > 0 {
                                        self.doc.remove(line_start..line_start + remove_count);
                                        removed_total += remove_count;
                                    }
                                }

                                self.cursor = self.cursor.saturating_sub(removed_total);
                            } else {
                                // ---- TAB : INDENT ----
                                let mut added_total = 0;

                                for line in line_range.clone() {
                                    let line_start = self.doc.line_to_char(line);
                                    self.doc.insert(line_start, INDENT);
                                    added_total += INDENT_WIDTH;
                                }

                                self.cursor += added_total;
                            }

                            // Update selection to stay covering same lines
                            let start_line = self.doc.char_to_line(selection.start);
                            let end_line = safe_char_to_line(&self.doc, selection.end);

                            let new_start = self.doc.line_to_char(start_line);
                            let new_end = self
                                .doc
                                .line_to_char(end_line + 1)
                                .min(self.doc.len_chars());

                            self.selection = Some(new_start..new_end);
                        } else {
                            // ---- No selection ----
                            if shift {
                                // Unindent current line
                                let line = self.doc.char_to_line(self.cursor);
                                let line_start = self.doc.line_to_char(line);
                                let line_text = self.doc.line(line);

                                let remove_count = line_text
                                    .chars()
                                    .take_while(|c| *c == ' ')
                                    .take(INDENT_WIDTH)
                                    .count();

                                if remove_count > 0 {
                                    self.doc.remove(line_start..line_start + remove_count);
                                    self.cursor = self.cursor.saturating_sub(remove_count);
                                }
                            } else {
                                self.doc.insert(self.cursor, INDENT);
                                self.cursor += INDENT_WIDTH;
                            }
                        }

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
                    egui::Event::Copy => {
                        self.copy(ui);
                    }
                    egui::Event::Cut => {
                        self.cut(ui);
                    }
                    egui::Event::Paste(text) => {
                        self.paste(ui, text);
                    }
                    _ => {}
                }
            }
        }
    }

    fn copy(&self, ui: &mut egui::Ui) {
        if let Some(text) = self.selected_text() {
            cfg_if::cfg_if! {
                if #[cfg(target_os = "android")] {
                    android_clipboard::set_text(text);
                } else {
                    ui.ctx().copy_text(text.clone());
                }
            }
        }
    }

    fn cut(&mut self, ui: &mut egui::Ui) {
        if let Some(selection) = &self.selection {
            if selection.start != selection.end {
                if let Some(text) = self.selected_text() {
                    cfg_if::cfg_if! {
                        if #[cfg(target_os = "android")] {
                            android_clipboard::set_text(text);
                        } else {
                            ui.ctx().copy_text(text.clone());
                        }
                    }
                }

                self.doc.remove(selection.start..selection.end);
                self.cursor = selection.start;
            }
        }

        self.selection = None;
        self.desired_column = None;
        self.cursor_blink_offset = ui.input(|i| i.time);
    }

    fn paste(&mut self, ui: &mut egui::Ui, text: String) {
        if let Some(selection) = &self.selection {
            if selection.start != selection.end {
                self.doc.remove(selection.start..selection.end);
                self.cursor = selection.start;
            }
        }

        self.doc.insert(self.cursor, &text);
        self.cursor += text.chars().count();

        self.selection = None;
        self.desired_column = None;
        self.cursor_blink_offset = ui.input(|i| i.time);
    }

    fn selected_text(&self) -> Option<String> {
        self.selection
            .as_ref()
            .map(|range| self.doc.slice(range.clone()).to_string())
    }

    fn format_token(&self, ty: TokenType) -> egui::text::TextFormat {
        let font_id = egui::FontId::monospace(self.fontsize);
        let color = self.theme.type_color(ty);
        egui::text::TextFormat::simple(font_id, color)
    }

    fn append(&self, job: &mut egui::text::LayoutJob, token: &Token) {
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
