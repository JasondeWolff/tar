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

fn lerp_vec2(a: egui::Vec2, b: egui::Vec2, t: f32) -> egui::Vec2 {
    a + (b - a) * t
}

#[derive(Clone, Debug)]
pub struct Edit {
    pub range: std::ops::Range<usize>, // range BEFORE edit
    pub removed: String,               // text removed
    pub inserted: String,              // text inserted

    pub cursor_before: usize,
    pub cursor_after: usize,

    pub selection_before: Option<std::ops::Range<usize>>,
    pub selection_after: Option<std::ops::Range<usize>>,
}

#[derive(Default)]
pub struct EditStack {
    undo: Vec<Edit>,
    redo: Vec<Edit>,
}

enum TouchScrollAxis {
    Vertical,
    Horizontal,
}

pub struct CodeEditor {
    pub doc: Rope,
    edit_stack: EditStack,

    pub cursor: usize,
    cursor_blink_offset: f64,
    cursor_request_focus: bool,
    desired_column: Option<usize>,
    pub selection: Option<Range<usize>>,
    selection_anchor: Option<usize>,

    touch_scroll_velocity: egui::Vec2,
    touch_scroll_axis_lock: Option<TouchScrollAxis>,
    touch_scroll_timestamp: f64,

    theme: ColorTheme,
    syntax: Syntax,
    fontsize: f32,
}

impl CodeEditor {
    pub fn new(text: &str, theme: ColorTheme, syntax: Syntax) -> Self {
        Self {
            doc: Rope::from_str(text),
            edit_stack: EditStack::default(),
            cursor: 0,
            cursor_blink_offset: 0.0,
            cursor_request_focus: false,
            desired_column: None,
            selection: None,
            selection_anchor: None,
            touch_scroll_velocity: egui::Vec2::ZERO,
            touch_scroll_axis_lock: None,
            touch_scroll_timestamp: 0.0,
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
        let line_height = if !galley.rows.is_empty() {
            galley.rows[0].height()
        } else {
            16.0
        };

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
        let delta_time = ui.input(|i| i.stable_dt);

        let event_filter = egui::EventFilter {
            tab: true,
            vertical_arrows: true,
            horizontal_arrows: true,
            escape: false,
        };
        ui.memory_mut(|mem| mem.set_focus_lock_filter(response.id, event_filter));

        let response = response.on_hover_cursor(egui::CursorIcon::Text);

        // --- Mouse input ---
        const TOUCH_SCROLL_SENSITIVITY: f32 = 4.5;
        const TOUCH_SCROLL_SMOOTHING: f32 = 1.5; // 0 = raw, 1 = no movement
        const TOUCH_SCROLL_DAMPING: f32 = 15.0;
        const AXIS_LOCK_THRESHOLD: f32 = 6.0; // pixels

        let double_touch = ui.input(|i| {
            if let Some(multi_touch) = i.multi_touch() {
                return multi_touch.num_touches == 2;
            }

            false
        });

        if double_touch {
            ui.input(|i| {
                if let Some(multi_touch) = i.multi_touch() {
                    if multi_touch.num_touches == 2 {
                        // && self.selection.is_none() {
                        let raw = multi_touch.translation_delta * TOUCH_SCROLL_SENSITIVITY;

                        // Lock axis once
                        if self.touch_scroll_axis_lock.is_none()
                            && raw.length() > AXIS_LOCK_THRESHOLD
                        {
                            if raw.y.abs() > raw.x.abs() * 1.3 {
                                self.touch_scroll_axis_lock = Some(TouchScrollAxis::Vertical);
                            } else {
                                self.touch_scroll_axis_lock = Some(TouchScrollAxis::Horizontal);
                            }
                        }

                        let mut filtered = raw;

                        if let Some(axis) = &self.touch_scroll_axis_lock {
                            match axis {
                                TouchScrollAxis::Vertical => filtered.x = 0.0,
                                TouchScrollAxis::Horizontal => filtered.y = 0.0,
                            }
                        }

                        // Smooth
                        self.touch_scroll_velocity = lerp_vec2(
                            self.touch_scroll_velocity,
                            filtered,
                            (1.0 / TOUCH_SCROLL_SMOOTHING) * (delta_time * 60.0),
                        );

                        self.touch_scroll_timestamp = time;
                    }
                }
            });
        } else {
            if ui.input(|i| i.pointer.any_pressed()) {
                if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                    // --- Convert pointer position to char index ---
                    let y = pos.y - rect.min.y;
                    let line = (y / line_height) as usize;

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

            if response.dragged() && (self.touch_scroll_timestamp + 0.5 < time) {
                if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                    let (drag_line, drag_col) = {
                        let y = pos.y - rect.min.y;
                        let line = (y / line_height) as usize;

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
                    self.update_cursor(char_idx);
                    self.desired_column = Some(drag_col);
                    self.cursor_blink_offset = time;
                    self.touch_scroll_velocity = egui::Vec2::ZERO;
                }
            } else {
                // Clear anchor when not dragging
                self.selection_anchor = None;
            }
        }

        if !ui.input(|i| i.pointer.any_down()) {
            self.touch_scroll_velocity = lerp_vec2(
                self.touch_scroll_velocity,
                egui::Vec2::ZERO,
                (1.0 / TOUCH_SCROLL_DAMPING) * (delta_time * 60.0),
            );

            self.touch_scroll_axis_lock = None;
        };

        if self.touch_scroll_velocity != egui::Vec2::ZERO {
            ui.scroll_with_delta(self.touch_scroll_velocity);
        }

        if response.has_focus() {
            // --- Cursor ---
            const BLINK_SPEED: f64 = 0.530 * 2.0;
            let cursor_visible =
                ((time - self.cursor_blink_offset) % BLINK_SPEED) < (BLINK_SPEED * 0.5);

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

            let cursor_rect = egui::Rect::from_min_max(
                egui::pos2(cursor_x, cursor_y),
                egui::pos2(cursor_x + 1.0, cursor_y + cursor_height),
            );

            if cursor_visible {
                painter.line_segment(
                    [
                        egui::pos2(cursor_rect.min.x, cursor_rect.min.y),
                        egui::pos2(cursor_rect.min.x, cursor_rect.max.y),
                    ],
                    egui::Stroke::new(1.0, self.theme.cursor()),
                );
            }

            // --- Input ---
            let events = ui.input(|i| i.filtered_events(&event_filter));
            for event in events {
                match event {
                    egui::Event::Text(text) => {
                        if text.is_empty() {
                            continue;
                        }

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
                            let selection_before = self.selection.clone();
                            let cursor_before = self.cursor;

                            let (range, removed) = if let Some(sel) = &self.selection {
                                if sel.start != sel.end {
                                    let removed = self.doc.slice(sel.clone()).to_string();
                                    (sel.clone(), removed)
                                } else {
                                    (self.cursor..self.cursor, String::new())
                                }
                            } else {
                                (self.cursor..self.cursor, String::new())
                            };

                            let cursor_after = range.start + text.chars().count();

                            let edit = Edit {
                                range,
                                removed,
                                inserted: text.clone(),

                                cursor_before,
                                cursor_after,

                                selection_before,
                                selection_after: None,
                            };

                            self.apply_edit(edit);

                            self.desired_column = None;
                            self.cursor_blink_offset = time;
                        }
                    }
                    egui::Event::Key {
                        key: egui::Key::Enter,
                        pressed: true,
                        ..
                    } => {
                        // --- capture state before ---
                        let selection_before = self.selection.clone();
                        let cursor_before = self.cursor;

                        // --- determine replacement range ---
                        let (range, removed) = if let Some(sel) = &self.selection {
                            if sel.start != sel.end {
                                let removed = self.doc.slice(sel.clone()).to_string();
                                (sel.clone(), removed)
                            } else {
                                (self.cursor..self.cursor, String::new())
                            }
                        } else {
                            (self.cursor..self.cursor, String::new())
                        };

                        // --- compute auto-indent ---
                        let line = self.doc.char_to_line(range.start);
                        let line_text = self.doc.line(line).to_string();
                        let indent = leading_whitespace(&line_text);

                        let inserted = format!("\n{}", indent);
                        let cursor_after = range.start + inserted.chars().count();

                        // --- build undoable edit ---
                        let edit = Edit {
                            range,
                            removed,
                            inserted,

                            cursor_before,
                            cursor_after,

                            selection_before,
                            selection_after: None, // Enter clears selection
                        };

                        self.apply_edit(edit);

                        self.desired_column = None;
                        self.cursor_blink_offset = time;
                    }
                    egui::Event::Key {
                        key: egui::Key::Backspace,
                        pressed: true,
                        ..
                    } => {
                        // --- capture state before ---
                        let selection_before = self.selection.clone();
                        let cursor_before = self.cursor;

                        // --- determine deletion range ---
                        let (range, removed) = if let Some(sel) = &self.selection {
                            if sel.start != sel.end {
                                let removed = self.doc.slice(sel.clone()).to_string();
                                (sel.clone(), removed)
                            } else if self.cursor > 0 {
                                let range = (self.cursor - 1)..self.cursor;
                                let removed = self.doc.slice(range.clone()).to_string();
                                (range, removed)
                            } else {
                                return;
                            }
                        } else if self.cursor > 0 {
                            let range = (self.cursor - 1)..self.cursor;
                            let removed = self.doc.slice(range.clone()).to_string();
                            (range, removed)
                        } else {
                            return;
                        };

                        let cursor_after = range.start;

                        let edit = Edit {
                            range,
                            removed,
                            inserted: String::new(),

                            cursor_before,
                            cursor_after,

                            selection_before,
                            selection_after: None,
                        };

                        self.apply_edit(edit);

                        self.desired_column = None;
                        self.cursor_blink_offset = time;
                    }
                    egui::Event::Key {
                        key: egui::Key::Delete,
                        pressed: true,
                        ..
                    } => {
                        // --- capture state before ---
                        let selection_before = self.selection.clone();
                        let cursor_before = self.cursor;

                        // --- determine deletion range ---
                        let (range, removed) = if let Some(sel) = &self.selection {
                            if sel.start != sel.end {
                                let removed = self.doc.slice(sel.clone()).to_string();
                                (sel.clone(), removed)
                            } else if self.cursor < self.doc.len_chars() {
                                let range = self.cursor..(self.cursor + 1);
                                let removed = self.doc.slice(range.clone()).to_string();
                                (range, removed)
                            } else {
                                return;
                            }
                        } else if self.cursor < self.doc.len_chars() {
                            let range = self.cursor..(self.cursor + 1);
                            let removed = self.doc.slice(range.clone()).to_string();
                            (range, removed)
                        } else {
                            return;
                        };

                        let cursor_after = range.start;

                        let edit = Edit {
                            range,
                            removed,
                            inserted: String::new(),

                            cursor_before,
                            cursor_after,

                            selection_before,
                            selection_after: None,
                        };

                        self.apply_edit(edit);

                        self.desired_column = None;
                        self.cursor_blink_offset = time;
                    }
                    egui::Event::Key {
                        key: egui::Key::Tab,
                        pressed: true,
                        ..
                    } => {
                        const TAB_WIDTH: usize = 4;

                        let line = self.doc.char_to_line(self.cursor);
                        let line_start = self.doc.line_to_char(line);
                        let column = self.cursor - line_start;
                        let spaces = TAB_WIDTH - (column % TAB_WIDTH);

                        let mut text = String::new();
                        for _ in 0..spaces {
                            text += " ";
                        }

                        let selection_before = self.selection.clone();
                        let cursor_before = self.cursor;

                        let (range, removed) = if let Some(sel) = &self.selection {
                            if sel.start != sel.end {
                                let removed = self.doc.slice(sel.clone()).to_string();
                                (sel.clone(), removed)
                            } else {
                                (self.cursor..self.cursor, String::new())
                            }
                        } else {
                            (self.cursor..self.cursor, String::new())
                        };

                        let cursor_after = range.start + text.chars().count();

                        let edit = Edit {
                            range,
                            removed,
                            inserted: text.clone(),

                            cursor_before,
                            cursor_after,

                            selection_before,
                            selection_after: None,
                        };

                        self.apply_edit(edit);

                        self.desired_column = None;
                        self.cursor_blink_offset = time;
                    }
                    egui::Event::Key {
                        key: egui::Key::ArrowLeft,
                        pressed: true,
                        ..
                    } => {
                        self.update_cursor(self.cursor.saturating_sub(1));

                        self.selection = None;
                        self.desired_column = None;
                        self.cursor_blink_offset = time;
                    }
                    egui::Event::Key {
                        key: egui::Key::ArrowRight,
                        pressed: true,
                        ..
                    } => {
                        self.update_cursor((self.cursor + 1).min(self.doc.len_chars()));

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
                            self.update_cursor(prev_line_start + new_col);
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
                            self.update_cursor(next_line_start + new_col);
                            self.desired_column = Some(target_col);
                        }

                        self.selection = None;
                        self.cursor_blink_offset = time;
                    }
                    egui::Event::Key {
                        key: egui::Key::Z,
                        pressed: true,
                        modifiers,
                        ..
                    } => {
                        if modifiers.ctrl || modifiers.command || key_modifiers.ctrl {
                            self.undo(ui);
                        }
                    }
                    egui::Event::Key {
                        key: egui::Key::Y,
                        pressed: true,
                        modifiers,
                        ..
                    } => {
                        if modifiers.ctrl || modifiers.command || key_modifiers.ctrl {
                            self.redo(ui);
                        }
                    }
                    egui::Event::Key {
                        key: egui::Key::A,
                        pressed: true,
                        modifiers,
                        ..
                    } => {
                        if modifiers.ctrl || modifiers.command || key_modifiers.ctrl {
                            let len = self.doc.len_chars();

                            self.selection = Some(0..len);
                            self.cursor = len; // cursor at end
                            self.selection_anchor = Some(0);

                            self.desired_column = None;
                            self.cursor_blink_offset = time;
                        }
                    }
                    egui::Event::Key {
                        key: egui::Key::S,
                        pressed: true,
                        modifiers,
                        ..
                    } => {
                        if modifiers.ctrl || modifiers.command || key_modifiers.ctrl {
                            self.format();
                        }
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

            if self.cursor_request_focus {
                self.cursor_request_focus = false;

                let v_margin = line_height * 6.0;
                let h_margin = 40.0;

                let mut reveal_rect = cursor_rect;
                reveal_rect.min.y -= v_margin;
                reveal_rect.max.y += v_margin;
                reveal_rect.min.x -= h_margin;
                reveal_rect.max.x += h_margin;

                ui.scroll_to_rect(reveal_rect, None);
            }
        }
    }

    fn apply_edit(&mut self, edit: Edit) {
        // Apply edit
        self.doc.remove(edit.range.clone());
        self.doc.insert(edit.range.start, &edit.inserted);

        self.update_cursor(edit.cursor_after);
        self.selection = edit.selection_after.clone();

        // Push undo
        self.edit_stack.undo.push(edit);
        self.edit_stack.redo.clear();
    }

    fn update_cursor(&mut self, cursor: usize) {
        self.cursor = cursor;
        self.cursor_request_focus = true;
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
        let Some(selection) = self.selection.clone() else {
            return;
        };

        if selection.start == selection.end {
            return;
        }

        // --- Copy to clipboard ---
        if let Some(text) = self.selected_text() {
            cfg_if::cfg_if! {
                if #[cfg(target_os = "android")] {
                    android_clipboard::set_text(text.clone());
                } else {
                    ui.ctx().copy_text(text.clone());
                }
            }
        }

        // --- Capture state before ---
        let selection_before = self.selection.clone();
        let cursor_before = self.cursor;

        // --- Capture removed text ---
        let removed = self.doc.slice(selection.clone()).to_string();

        let cursor_after = selection.start;

        let edit = Edit {
            range: selection.clone(),
            removed,
            inserted: String::new(),

            cursor_before,
            cursor_after,

            selection_before,
            selection_after: None,
        };

        self.apply_edit(edit);

        self.desired_column = None;
        self.cursor_blink_offset = ui.input(|i| i.time);
    }

    fn paste(&mut self, ui: &mut egui::Ui, text: String) {
        if text.is_empty() {
            return;
        }

        let selection_before = self.selection.clone();
        let cursor_before = self.cursor;

        // --- Determine replacement range ---
        let (range, removed) = if let Some(sel) = &self.selection {
            if sel.start != sel.end {
                let removed = self.doc.slice(sel.clone()).to_string();
                (sel.clone(), removed)
            } else {
                (self.cursor..self.cursor, String::new())
            }
        } else {
            (self.cursor..self.cursor, String::new())
        };

        let cursor_after = range.start + text.chars().count();

        let edit = Edit {
            range,
            removed,
            inserted: text,

            cursor_before,
            cursor_after,

            selection_before,
            selection_after: None,
        };

        self.apply_edit(edit);

        self.desired_column = None;
        self.cursor_blink_offset = ui.input(|i| i.time);
    }

    fn undo(&mut self, ui: &mut egui::Ui) {
        let Some(edit) = self.edit_stack.undo.pop() else {
            return;
        };

        // Revert
        let revert = Edit {
            range: edit.range.start..(edit.range.start + edit.inserted.chars().count()),
            removed: edit.inserted.clone(),
            inserted: edit.removed.clone(),

            cursor_before: edit.cursor_after,
            cursor_after: edit.cursor_before,

            selection_before: edit.selection_after.clone(),
            selection_after: edit.selection_before.clone(),
        };

        self.doc.remove(revert.range.clone());
        self.doc.insert(revert.range.start, &revert.inserted);

        self.update_cursor(revert.cursor_after);
        self.selection = revert.selection_after.clone();

        self.edit_stack.redo.push(edit);

        self.cursor_blink_offset = ui.input(|i| i.time);
    }

    fn redo(&mut self, ui: &mut egui::Ui) {
        let Some(edit) = self.edit_stack.redo.pop() else {
            return;
        };

        self.doc.remove(edit.range.clone());
        self.doc.insert(edit.range.start, &edit.inserted);

        self.update_cursor(edit.cursor_after);
        self.selection = edit.selection_after.clone();

        self.edit_stack.undo.push(edit);

        self.cursor_blink_offset = ui.input(|i| i.time);
    }

    fn format(&mut self) {
        // Save cursor line & column before formatting
        let cursor_line = self.doc.char_to_line(self.cursor);
        let line_start = self.doc.line_to_char(cursor_line);
        let cursor_col = self.cursor - line_start;

        // Format source
        let source = self.doc.to_string();
        let formatted = self.syntax.formatter.format(source);

        // Replace doc
        self.doc = Rope::from_str(&formatted);

        // Clamp line
        let new_line = cursor_line.min(self.doc.len_lines() - 1);

        // Clamp column to line length
        let line_len = self.doc.line(new_line).len_chars();
        let new_col = cursor_col.min(line_len);

        // Set cursor
        self.update_cursor(self.doc.line_to_char(new_line) + new_col);

        // Clear selection if needed
        self.selection = None;
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
