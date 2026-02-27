// Modified version built on-top of Roman Chumak's egui_code_editor
// (https://github.com/p4ymak/egui_code_editor/)

use egui::epaint::text::PlacedRow;
use egui::Color32;
use ropey::Rope;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;

use crate::editor::code_editor::highlighting::highlight;
use crate::editor::code_editor::{
    highlighting::Token,
    syntax::{Syntax, TokenType},
    themes::ColorTheme,
};
use crate::egui_util::KeyModifiers;

pub mod highlighting;
pub mod syntax;
pub mod themes;

// ============================================================================
// Constants
// ============================================================================

const TAB_WIDTH: usize = 4;
const BUFFER_LINES: usize = 10;
const GUTTER_PADDING: f32 = 8.0;
const TEXT_PADDING: f32 = 6.0;
const CURSOR_REVEAL_V_MARGIN_LINES: f32 = 6.0;
const CURSOR_REVEAL_H_MARGIN: f32 = 40.0;

// Cursor blink
const BLINK_SPEED: f64 = 0.530 * 2.0;

// Touch scrolling
const TOUCH_SCROLL_SENSITIVITY: f32 = 4.5;
const TOUCH_SCROLL_SMOOTHING: f32 = 20.0;
const TOUCH_SCROLL_DAMPING: f32 = 5.0;
const AXIS_LOCK_THRESHOLD: f32 = 6.0;

// ============================================================================
// Helper Functions
// ============================================================================

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

// ============================================================================
// Edit & EditStack
// ============================================================================

#[derive(Clone, Debug)]
pub struct Edit {
    pub range: Range<usize>,
    pub removed: String,
    pub inserted: String,
    pub cursor_before: usize,
    pub cursor_after: usize,
    pub selection_before: Option<Range<usize>>,
    pub selection_after: Option<Range<usize>>,
}

#[derive(Default)]
pub struct EditStack {
    undo: Vec<Edit>,
    redo: Vec<Edit>,
}

// ============================================================================
// Touch Scroll
// ============================================================================

enum TouchScrollAxis {
    Vertical,
    Horizontal,
}

// ============================================================================
// CodeEditor
// ============================================================================

pub struct CodeEditor {
    pub doc: Rope,
    doc_hash: u64,
    readonly: bool,

    edit_stack: EditStack,
    max_line_width: Option<f32>,
    text_layout_job: Option<egui::text::LayoutJob>,
    prev_scroll_offset: f32,

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
    pub fn new(text: &str, readonly: bool, theme: ColorTheme, syntax: Syntax) -> Self {
        let mut code_editor = Self {
            doc: Rope::from_str(text),
            doc_hash: 0,
            readonly,
            edit_stack: EditStack::default(),
            max_line_width: None,
            text_layout_job: None,
            prev_scroll_offset: 0.0,
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
        };

        code_editor.update_doc_hash();

        code_editor
    }

    // ========================================================================
    // Public API
    // ========================================================================

    pub fn ui(&mut self, ui: &mut egui::Ui, key_modifiers: &KeyModifiers) -> bool {
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show_viewport(ui, |ui, viewport| {
                self.draw_editor(ui, viewport, key_modifiers)
            })
            .inner
    }

    pub fn doc_hash(&self) -> u64 {
        self.doc_hash
    }

    // ========================================================================
    // Main Draw Method
    // ========================================================================

    pub fn draw_editor(
        &mut self,
        ui: &mut egui::Ui,
        viewport: egui::Rect,
        key_modifiers: &KeyModifiers,
    ) -> bool {
        let scroll_offset = viewport.min.y;
        self.handle_scroll_change(scroll_offset);

        let font_id = egui::FontId::monospace(self.fontsize);
        let line_height = self.line_height(ui, &font_id);

        let (start_line, end_line) = self.calculate_visible_lines(scroll_offset, line_height, ui);
        let visible_text = self.extract_visible_text(start_line, end_line);

        self.ensure_layout_job(ui, &visible_text);
        let visible_galley = ui.fonts_mut(|f| f.layout_job(self.text_layout_job.clone().unwrap()));

        let visible_offset_y = start_line as f32 * line_height;
        let (rect, response, visible_rect) =
            self.allocate_editor_rect(ui, &font_id, line_height, visible_offset_y);

        let painter = ui.painter_at(visible_rect);
        let gutter_width = self.calculate_gutter_width(ui, &font_id);
        let text_x = visible_rect.min.x + gutter_width + TEXT_PADDING;

        // Render
        self.render_background(&painter, visible_rect);
        self.render_line_highlights(
            &painter,
            ui,
            &font_id,
            visible_rect,
            &visible_galley,
            start_line,
            gutter_width,
        );
        self.render_selection(&painter, ui, &font_id, rect, text_x, line_height);
        self.render_text(&painter, text_x, visible_rect, &visible_galley);
        if self.readonly {
            self.render_readonly_overlay(ui, visible_rect);
        }

        // Input handling
        let time = ui.input(|i| i.time);
        let delta_time = ui.input(|i| i.stable_dt);

        self.setup_event_filter(ui, response.id);
        let response = response.on_hover_cursor(egui::CursorIcon::Text);

        if response.has_focus() {
            self.handle_touch_scroll(ui, time, delta_time);
        } else {
            self.touch_scroll_velocity = egui::Vec2::ZERO;
        }

        self.handle_mouse_input(
            ui,
            &response,
            &font_id,
            visible_rect,
            text_x,
            line_height,
            start_line,
            time,
        );
        self.apply_scroll_velocity(ui, delta_time);

        if response.has_focus() {
            self.render_cursor(&painter, ui, &font_id, rect, text_x, line_height, time);
            self.handle_keyboard_input(ui, key_modifiers, time);
            self.handle_cursor_scroll(ui, rect, line_height);
        }

        response.has_focus()
    }

    // ========================================================================
    // Layout & Measurement Helpers
    // ========================================================================

    fn handle_scroll_change(&mut self, scroll_offset: f32) {
        if scroll_offset != self.prev_scroll_offset {
            self.prev_scroll_offset = scroll_offset;
            self.text_layout_job = None;
        }
    }

    fn calculate_visible_lines(
        &self,
        scroll_offset: f32,
        line_height: f32,
        ui: &egui::Ui,
    ) -> (usize, usize) {
        let visible_start_y = scroll_offset.max(0.0);
        let visible_end_y = visible_start_y + ui.available_height();

        let start_line = ((visible_start_y / line_height) as usize)
            .saturating_sub(BUFFER_LINES)
            .min(self.doc.len_lines() - 1);
        let end_line =
            ((visible_end_y / line_height) as usize + BUFFER_LINES + 1).min(self.doc.len_lines());

        (start_line, end_line)
    }

    fn extract_visible_text(&self, start_line: usize, end_line: usize) -> String {
        self.doc
            .lines()
            .skip(start_line)
            .take(end_line - start_line)
            .flat_map(|slice| slice.chars())
            .collect()
    }

    fn ensure_layout_job(&mut self, ui: &egui::Ui, visible_text: &str) {
        if self.text_layout_job.is_none() {
            self.text_layout_job = Some(highlight(ui.ctx(), self, visible_text));
        }
    }

    fn allocate_editor_rect(
        &mut self,
        ui: &mut egui::Ui,
        font_id: &egui::FontId,
        line_height: f32,
        visible_offset_y: f32,
    ) -> (egui::Rect, egui::Response, egui::Rect) {
        let width = self.calculate_editor_width(ui, font_id);
        let height = ui
            .available_height()
            .max(line_height * self.doc.len_lines() as f32);
        let desired_size = egui::vec2(width, height);

        let (rect, mut response) =
            ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());
        response.flags -= egui::response::Flags::FAKE_PRIMARY_CLICKED;

        let mut visible_rect = rect;
        visible_rect.min.y += visible_offset_y;

        (rect, response, visible_rect)
    }

    fn calculate_editor_width(&mut self, ui: &mut egui::Ui, font_id: &egui::FontId) -> f32 {
        if self.max_line_width.is_none() {
            self.max_line_width = Some(
                self.doc
                    .lines()
                    .map(|l| self.measure_text_width(ui, font_id, &l.to_string()))
                    .fold(0.0, f32::max),
            );
        }
        ui.available_width()
            .max(self.max_line_width.unwrap() + 200.0)
    }

    fn calculate_gutter_width(&self, ui: &mut egui::Ui, font_id: &egui::FontId) -> f32 {
        let total_lines = self.doc.len_lines().max(1);
        let digits = total_lines.ilog10() + 1;
        let digit_width = self.measure_text_width(ui, font_id, "0");
        digit_width * digits as f32 + GUTTER_PADDING * 2.0
    }

    fn measure_text_width(&self, ui: &mut egui::Ui, font_id: &egui::FontId, text: &str) -> f32 {
        ui.fonts_mut(|f| {
            f.layout_no_wrap(text.to_string(), font_id.clone(), Color32::WHITE)
                .size()
                .x
        })
    }

    fn line_height(&self, ui: &mut egui::Ui, font_id: &egui::FontId) -> f32 {
        let mut test_job = egui::text::LayoutJob::default();
        test_job.append("Xg", 0.0, self.format_token(TokenType::Literal));
        let test_galley = ui.fonts_mut(|f| f.layout_job(test_job));

        if !test_galley.rows.is_empty() {
            test_galley.rows[0].height()
        } else {
            ui.fonts_mut(|f| f.row_height(font_id))
        }
    }

    // ========================================================================
    // Rendering
    // ========================================================================

    fn render_readonly_overlay(&self, ui: &egui::Ui, visible_rect: egui::Rect) {
        let clip = ui.clip_rect().intersect(visible_rect);
        let painter = ui
            .ctx()
            .layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("readonly_overlay"),
            ))
            .with_clip_rect(clip);

        let font_id = egui::FontId::proportional(44.0);
        let color = egui::Color32::from_rgba_unmultiplied(180, 180, 180, 45);
        painter.text(
            egui::pos2(clip.max.x - 16.0, clip.max.y - 16.0),
            egui::Align2::RIGHT_BOTTOM,
            "READONLY",
            font_id,
            color,
        );
    }

    fn render_background(&self, painter: &egui::Painter, rect: egui::Rect) {
        painter.rect_filled(rect, 0.0, self.theme.bg());
    }

    #[allow(clippy::too_many_arguments)]
    fn render_line_highlights(
        &self,
        painter: &egui::Painter,
        ui: &mut egui::Ui,
        font_id: &egui::FontId,
        visible_rect: egui::Rect,
        galley: &egui::Galley,
        start_line: usize,
        gutter_width: f32,
    ) {
        let (cursor_line, _) = char_to_line_col(&self.doc, self.cursor);

        for (row_idx, row) in galley.rows.iter().enumerate() {
            let line_num = row_idx + start_line;
            let is_current = line_num == cursor_line;

            // Current line highlight
            if is_current {
                let highlight_rect = egui::Rect::from_min_max(
                    egui::pos2(visible_rect.min.x, visible_rect.min.y + row.min_y()),
                    egui::pos2(visible_rect.max.x, visible_rect.min.y + row.max_y()),
                );
                painter.rect_filled(highlight_rect, 0.0, egui::Color32::from_rgb(35, 35, 35));
            }

            // Line number
            self.render_line_number(
                painter,
                ui,
                font_id,
                visible_rect,
                row,
                line_num,
                is_current,
                gutter_width,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_line_number(
        &self,
        painter: &egui::Painter,
        ui: &mut egui::Ui,
        font_id: &egui::FontId,
        visible_rect: egui::Rect,
        row: &PlacedRow,
        line_num: usize,
        is_current: bool,
        gutter_width: f32,
    ) {
        let line_number = (line_num + 1).to_string();
        let color = if is_current {
            Color32::WHITE
        } else {
            Color32::from_gray(140)
        };
        let text_width = self.measure_text_width(ui, font_id, &line_number);
        let x = visible_rect.min.x + gutter_width - GUTTER_PADDING - text_width;

        painter.text(
            egui::pos2(x, visible_rect.min.y + row.min_y()),
            egui::Align2::LEFT_TOP,
            line_number,
            font_id.clone(),
            color,
        );
    }

    fn render_selection(
        &self,
        painter: &egui::Painter,
        ui: &mut egui::Ui,
        font_id: &egui::FontId,
        rect: egui::Rect,
        text_x: f32,
        line_height: f32,
    ) {
        let Some(selection) = &self.selection else {
            return;
        };

        let (start_line, start_col) = char_to_line_col(&self.doc, selection.start);
        let (end_line, end_col) = char_to_line_col(&self.doc, selection.end);

        for line in start_line..=end_line {
            let line_start_char = self.doc.line_to_char(line);
            let line_end_char = line_start_char + line_len_without_newline(self.doc.line(line));

            let sel_start_col = if line == start_line { start_col } else { 0 };
            let sel_end_col = if line == end_line {
                end_col
            } else {
                line_end_char - line_start_char
            };

            if sel_start_col >= sel_end_col {
                continue;
            }

            let x_start = text_x
                + self.measure_text_width(
                    ui,
                    font_id,
                    &self.doc.line(line).slice(..sel_start_col).to_string(),
                );
            let x_end = text_x
                + self.measure_text_width(
                    ui,
                    font_id,
                    &self.doc.line(line).slice(..sel_end_col).to_string(),
                );
            let y = rect.min.y + line as f32 * line_height;

            let selection_rect = egui::Rect::from_min_max(
                egui::pos2(x_start, y),
                egui::pos2(x_end, y + line_height),
            );
            painter.rect_filled(selection_rect, 0.0, self.theme.selection());
        }
    }

    fn render_text(
        &self,
        painter: &egui::Painter,
        text_x: f32,
        visible_rect: egui::Rect,
        galley: &std::sync::Arc<egui::Galley>,
    ) {
        painter.galley(
            egui::pos2(text_x, visible_rect.min.y),
            galley.clone(),
            Color32::WHITE,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn render_cursor(
        &self,
        painter: &egui::Painter,
        ui: &mut egui::Ui,
        font_id: &egui::FontId,
        rect: egui::Rect,
        text_x: f32,
        line_height: f32,
        time: f64,
    ) {
        let cursor_visible =
            ((time - self.cursor_blink_offset) % BLINK_SPEED) < (BLINK_SPEED * 0.5);
        if !cursor_visible {
            return;
        }

        let (cursor_line, cursor_col) = char_to_line_col(&self.doc, self.cursor);
        let cursor_x = text_x
            + self.measure_text_width(
                ui,
                font_id,
                &self.doc.line(cursor_line).slice(..cursor_col).to_string(),
            );
        let cursor_y = rect.min.y + cursor_line as f32 * line_height;

        painter.line_segment(
            [
                egui::pos2(cursor_x, cursor_y),
                egui::pos2(cursor_x, cursor_y + line_height),
            ],
            egui::Stroke::new(1.0, self.theme.cursor()),
        );
    }

    // ========================================================================
    // Input Handling
    // ========================================================================

    fn setup_event_filter(&self, ui: &mut egui::Ui, id: egui::Id) {
        let event_filter = egui::EventFilter {
            tab: true,
            vertical_arrows: true,
            horizontal_arrows: true,
            escape: false,
        };
        ui.memory_mut(|mem| mem.set_focus_lock_filter(id, event_filter));
    }

    fn handle_touch_scroll(&mut self, ui: &mut egui::Ui, time: f64, delta_time: f32) {
        let double_touch = ui.input(|i| i.multi_touch().is_some_and(|mt| mt.num_touches == 2));

        if !double_touch {
            return;
        }

        if let Some(multi_touch) = ui.input(|i| i.multi_touch()) {
            let raw = multi_touch.translation_delta * TOUCH_SCROLL_SENSITIVITY;

            // Axis lock logic
            if self.touch_scroll_axis_lock.is_none() && raw.length() > AXIS_LOCK_THRESHOLD {
                self.touch_scroll_axis_lock = if raw.y.abs() > raw.x.abs() * 1.3 {
                    Some(TouchScrollAxis::Vertical)
                } else {
                    Some(TouchScrollAxis::Horizontal)
                };
            }

            let filtered = match &self.touch_scroll_axis_lock {
                Some(TouchScrollAxis::Vertical) => egui::vec2(0.0, raw.y),
                Some(TouchScrollAxis::Horizontal) => egui::vec2(raw.x, 0.0),
                None => raw,
            };

            // Apply scroll IMMEDIATELY during touch - no smoothing
            ui.scroll_with_delta(filtered);

            // Track velocity for momentum after release (light smoothing to reduce jitter)
            let t = 1.0 - (-TOUCH_SCROLL_SMOOTHING * delta_time).exp();
            self.touch_scroll_velocity = lerp_vec2(self.touch_scroll_velocity, filtered, t);

            self.touch_scroll_timestamp = time;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_mouse_input(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        font_id: &egui::FontId,
        visible_rect: egui::Rect,
        text_x: f32,
        line_height: f32,
        start_line: usize,
        time: f64,
    ) {
        let double_touch = ui.input(|i| i.multi_touch().is_some_and(|mt| mt.num_touches == 2));
        if double_touch {
            return;
        }

        // Click to position cursor
        if ui.input(|i| i.pointer.any_pressed()) {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                if response.rect.contains(pos) {
                    let (char_idx, col) = self.pos_to_char_index(
                        ui,
                        font_id,
                        pos,
                        visible_rect,
                        text_x,
                        line_height,
                        start_line,
                    );

                    self.cursor = char_idx;
                    self.desired_column = Some(col);
                    self.selection = None;
                    self.selection_anchor = None;
                    self.cursor_blink_offset = time;

                    ui.memory_mut(|m| m.request_focus(response.id));
                }
            }
        }

        // Drag to select
        if response.dragged() && (self.touch_scroll_timestamp + 0.5 < time) {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                let (char_idx, col) = self.pos_to_char_index(
                    ui,
                    font_id,
                    pos,
                    visible_rect,
                    text_x,
                    line_height,
                    start_line,
                );

                if self.selection_anchor.is_none() {
                    self.selection_anchor = Some(self.cursor);
                }

                let anchor = self.selection_anchor.unwrap();
                self.selection = Some(anchor.min(char_idx)..anchor.max(char_idx));
                self.update_cursor(char_idx);
                self.desired_column = Some(col);
                self.cursor_blink_offset = time;
                self.touch_scroll_velocity = egui::Vec2::ZERO;
            }
        } else {
            self.selection_anchor = None;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn pos_to_char_index(
        &self,
        ui: &mut egui::Ui,
        font_id: &egui::FontId,
        pos: egui::Pos2,
        visible_rect: egui::Rect,
        text_x: f32,
        line_height: f32,
        start_line: usize,
    ) -> (usize, usize) {
        let y = pos.y - visible_rect.min.y;
        let line =
            ((y / line_height) as usize + start_line).min(self.doc.len_lines().saturating_sub(1));

        let line_text = self.doc.line(line);
        let max_col = line_len_without_newline(line_text);

        let mut x = 0.0;
        let mut col = 0;

        for (i, c) in line_text.chars().take(max_col).enumerate() {
            let char_width = self.measure_text_width(ui, font_id, &c.to_string());
            if text_x + x + char_width / 2.0 >= pos.x {
                col = i;
                break;
            }
            x += char_width;
            col = i + 1;
        }

        let col = col.min(max_col);
        let char_idx = self.doc.line_to_char(line) + col;

        (char_idx, col)
    }

    fn apply_scroll_velocity(&mut self, ui: &mut egui::Ui, delta_time: f32) {
        // Only apply momentum when NOT actively touching
        if ui.input(|i| i.pointer.any_down()) {
            return;
        }

        self.touch_scroll_axis_lock = None;

        // Apply momentum
        if self.touch_scroll_velocity.length() > 0.5 {
            ui.scroll_with_delta(self.touch_scroll_velocity);

            let decay = (-TOUCH_SCROLL_DAMPING * delta_time).exp();
            self.touch_scroll_velocity *= decay;
        } else {
            self.touch_scroll_velocity = egui::Vec2::ZERO;
        }
    }

    fn handle_cursor_scroll(&mut self, ui: &mut egui::Ui, rect: egui::Rect, line_height: f32) {
        if !self.cursor_request_focus {
            return;
        }

        self.cursor_request_focus = false;

        let (cursor_line, _cursor_col) = char_to_line_col(&self.doc, self.cursor);
        let cursor_x = rect.min.x; // Simplified - actual x calculation would need font_id
        let cursor_y = rect.min.y + cursor_line as f32 * line_height;

        let v_margin = line_height * CURSOR_REVEAL_V_MARGIN_LINES;
        let reveal_rect = egui::Rect::from_min_max(
            egui::pos2(cursor_x - CURSOR_REVEAL_H_MARGIN, cursor_y - v_margin),
            egui::pos2(
                cursor_x + CURSOR_REVEAL_H_MARGIN,
                cursor_y + line_height + v_margin,
            ),
        );

        ui.scroll_to_rect(reveal_rect, None);
    }

    fn handle_keyboard_input(
        &mut self,
        ui: &mut egui::Ui,
        key_modifiers: &KeyModifiers,
        time: f64,
    ) {
        let event_filter = egui::EventFilter {
            tab: true,
            vertical_arrows: true,
            horizontal_arrows: true,
            escape: false,
        };

        let events = ui.input(|i| i.filtered_events(&event_filter));

        for event in events {
            match event {
                egui::Event::Text(text) => self.handle_text_input(ui, key_modifiers, &text, time),
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    self.handle_key_input(ui, key_modifiers, key, modifiers, time);
                }
                egui::Event::Copy => self.copy(ui),
                egui::Event::Cut => self.cut(ui),
                egui::Event::Paste(text) => self.paste(ui, text),
                _ => {}
            }
        }
    }

    fn handle_text_input(
        &mut self,
        #[allow(unused_variables)] ui: &mut egui::Ui,
        key_modifiers: &KeyModifiers,
        text: &str,
        time: f64,
    ) {
        if text.is_empty() || self.readonly {
            return;
        }

        if key_modifiers.ctrl {
            #[cfg(target_os = "android")]
            match text {
                "c" | "C" => self.copy(ui),
                "v" | "V" => {
                    if let Ok(clipboard_text) = android_clipboard::get_text() {
                        self.paste(ui, clipboard_text);
                    }
                }
                "x" | "X" => self.cut(ui),
                _ => {}
            }
            return;
        }

        self.insert_text(text, time);
    }

    fn handle_key_input(
        &mut self,
        ui: &mut egui::Ui,
        key_modifiers: &KeyModifiers,
        key: egui::Key,
        modifiers: egui::Modifiers,
        time: f64,
    ) {
        let is_ctrl = modifiers.ctrl || modifiers.command || key_modifiers.ctrl;

        match key {
            egui::Key::Enter => self.handle_enter(time),
            egui::Key::Backspace => self.handle_backspace(time),
            egui::Key::Delete => self.handle_delete(time),
            egui::Key::Tab => self.handle_tab(time),
            egui::Key::ArrowLeft => self.handle_arrow_left(time),
            egui::Key::ArrowRight => self.handle_arrow_right(time),
            egui::Key::ArrowUp => self.handle_arrow_up(time),
            egui::Key::ArrowDown => self.handle_arrow_down(time),
            egui::Key::Z if is_ctrl => self.undo(ui),
            egui::Key::Y if is_ctrl => self.redo(ui),
            egui::Key::A if is_ctrl => self.select_all(time),
            egui::Key::S if is_ctrl => self.format(),
            _ => {}
        }
    }

    // ========================================================================
    // Edit Operations
    // ========================================================================

    /// Returns the range to edit and the text being removed (if any)
    fn get_edit_range(&self) -> (Range<usize>, String) {
        if let Some(sel) = &self.selection {
            if sel.start != sel.end {
                let removed = self.doc.slice(sel.clone()).to_string();
                return (sel.clone(), removed);
            }
        }
        (self.cursor..self.cursor, String::new())
    }

    fn insert_text(&mut self, text: &str, time: f64) {
        let selection_before = self.selection.clone();
        let cursor_before = self.cursor;
        let (range, removed) = self.get_edit_range();
        let cursor_after = range.start + text.chars().count();

        let edit = Edit {
            range,
            removed,
            inserted: text.to_string(),
            cursor_before,
            cursor_after,
            selection_before,
            selection_after: None,
        };

        self.apply_edit(edit);
        self.desired_column = None;
        self.cursor_blink_offset = time;
    }

    fn handle_enter(&mut self, time: f64) {
        if self.readonly {
            return;
        }
        let selection_before = self.selection.clone();
        let cursor_before = self.cursor;
        let (range, removed) = self.get_edit_range();

        let line = self.doc.char_to_line(range.start);
        let line_text = self.doc.line(line).to_string();
        let indent = leading_whitespace(&line_text);
        let inserted = format!("\n{}", indent);
        let cursor_after = range.start + inserted.chars().count();

        let edit = Edit {
            range,
            removed,
            inserted,
            cursor_before,
            cursor_after,
            selection_before,
            selection_after: None,
        };

        self.apply_edit(edit);
        self.desired_column = None;
        self.cursor_blink_offset = time;
    }

    fn handle_backspace(&mut self, time: f64) {
        if self.readonly {
            return;
        }
        let selection_before = self.selection.clone();
        let cursor_before = self.cursor;

        let (range, removed) = if let Some(sel) = &self.selection {
            if sel.start != sel.end {
                (sel.clone(), self.doc.slice(sel.clone()).to_string())
            } else if self.cursor > 0 {
                let r = (self.cursor - 1)..self.cursor;
                (r.clone(), self.doc.slice(r).to_string())
            } else {
                return;
            }
        } else if self.cursor > 0 {
            let r = (self.cursor - 1)..self.cursor;
            (r.clone(), self.doc.slice(r).to_string())
        } else {
            return;
        };

        let edit = Edit {
            range: range.clone(),
            removed,
            inserted: String::new(),
            cursor_before,
            cursor_after: range.start,
            selection_before,
            selection_after: None,
        };

        self.apply_edit(edit);
        self.desired_column = None;
        self.cursor_blink_offset = time;
    }

    fn handle_delete(&mut self, time: f64) {
        if self.readonly {
            return;
        }
        let selection_before = self.selection.clone();
        let cursor_before = self.cursor;

        let (range, removed) = if let Some(sel) = &self.selection {
            if sel.start != sel.end {
                (sel.clone(), self.doc.slice(sel.clone()).to_string())
            } else if self.cursor < self.doc.len_chars() {
                let r = self.cursor..(self.cursor + 1);
                (r.clone(), self.doc.slice(r).to_string())
            } else {
                return;
            }
        } else if self.cursor < self.doc.len_chars() {
            let r = self.cursor..(self.cursor + 1);
            (r.clone(), self.doc.slice(r).to_string())
        } else {
            return;
        };

        let edit = Edit {
            range: range.clone(),
            removed,
            inserted: String::new(),
            cursor_before,
            cursor_after: range.start,
            selection_before,
            selection_after: None,
        };

        self.apply_edit(edit);
        self.desired_column = None;
        self.cursor_blink_offset = time;
    }

    fn handle_tab(&mut self, time: f64) {
        if self.readonly {
            return;
        }
        let line = self.doc.char_to_line(self.cursor);
        let line_start = self.doc.line_to_char(line);
        let column = self.cursor - line_start;
        let spaces = TAB_WIDTH - (column % TAB_WIDTH);
        let text: String = " ".repeat(spaces);

        self.insert_text(&text, time);
    }

    fn handle_arrow_left(&mut self, time: f64) {
        self.update_cursor(self.cursor.saturating_sub(1));
        self.selection = None;
        self.desired_column = None;
        self.cursor_blink_offset = time;
    }

    fn handle_arrow_right(&mut self, time: f64) {
        self.update_cursor((self.cursor + 1).min(self.doc.len_chars()));
        self.selection = None;
        self.desired_column = None;
        self.cursor_blink_offset = time;
    }

    fn handle_arrow_up(&mut self, time: f64) {
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

    fn handle_arrow_down(&mut self, time: f64) {
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

    fn select_all(&mut self, time: f64) {
        let len = self.doc.len_chars();
        self.selection = Some(0..len);
        self.cursor = len;
        self.selection_anchor = Some(0);
        self.desired_column = None;
        self.cursor_blink_offset = time;
    }

    // ========================================================================
    // Edit Application & Undo/Redo
    // ========================================================================

    fn update_doc_hash(&mut self) {
        let mut hasher = DefaultHasher::new();
        self.doc.hash(&mut hasher);
        self.doc_hash = hasher.finish();
    }

    fn apply_edit(&mut self, edit: Edit) {
        self.doc.remove(edit.range.clone());
        self.doc.insert(edit.range.start, &edit.inserted);
        self.update_doc_hash();

        self.update_cursor(edit.cursor_after);
        self.selection = edit.selection_after.clone();
        self.invalidate_layout();

        self.edit_stack.undo.push(edit);
        self.edit_stack.redo.clear();
    }

    fn update_cursor(&mut self, cursor: usize) {
        self.cursor = cursor;
        self.cursor_request_focus = true;
    }

    fn invalidate_layout(&mut self) {
        self.max_line_width = None;
        self.text_layout_job = None;
    }

    fn copy(&self, ui: &mut egui::Ui) {
        let Some(text) = self.selected_text() else {
            return;
        };

        cfg_if::cfg_if! {
            if #[cfg(target_os = "android")] {
                android_clipboard::set_text(text);
            } else {
                ui.ctx().copy_text(text);
            }
        }
    }

    fn cut(&mut self, ui: &mut egui::Ui) {
        if self.readonly {
            self.copy(ui);
            return;
        }
        let Some(selection) = self.selection.clone() else {
            return;
        };
        if selection.start == selection.end {
            return;
        }

        // Copy to clipboard
        if let Some(text) = self.selected_text() {
            cfg_if::cfg_if! {
                if #[cfg(target_os = "android")] {
                    android_clipboard::set_text(text);
                } else {
                    ui.ctx().copy_text(text);
                }
            }
        }

        let selection_before = self.selection.clone();
        let cursor_before = self.cursor;
        let removed = self.doc.slice(selection.clone()).to_string();

        let edit = Edit {
            range: selection.clone(),
            removed,
            inserted: String::new(),
            cursor_before,
            cursor_after: selection.start,
            selection_before,
            selection_after: None,
        };

        self.apply_edit(edit);
        self.desired_column = None;
        self.cursor_blink_offset = ui.input(|i| i.time);
    }

    fn paste(&mut self, ui: &mut egui::Ui, text: String) {
        if text.is_empty() || self.readonly {
            return;
        }

        let selection_before = self.selection.clone();
        let cursor_before = self.cursor;
        let (range, removed) = self.get_edit_range();
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
        if self.readonly {
            return;
        }
        let Some(edit) = self.edit_stack.undo.pop() else {
            return;
        };

        let revert_range = edit.range.start..(edit.range.start + edit.inserted.chars().count());

        self.doc.remove(revert_range.clone());
        self.doc.insert(revert_range.start, &edit.removed);
        self.update_doc_hash();

        self.update_cursor(edit.cursor_before);
        self.selection = edit.selection_before.clone();
        self.invalidate_layout();

        self.edit_stack.redo.push(edit);
        self.cursor_blink_offset = ui.input(|i| i.time);
    }

    fn redo(&mut self, ui: &mut egui::Ui) {
        if self.readonly {
            return;
        }
        let Some(edit) = self.edit_stack.redo.pop() else {
            return;
        };

        self.doc.remove(edit.range.clone());
        self.doc.insert(edit.range.start, &edit.inserted);
        self.update_doc_hash();

        self.update_cursor(edit.cursor_after);
        self.selection = edit.selection_after.clone();
        self.invalidate_layout();

        self.edit_stack.undo.push(edit);
        self.cursor_blink_offset = ui.input(|i| i.time);
    }

    fn format(&mut self) {
        if self.readonly {
            return;
        }
        let cursor_line = self.doc.char_to_line(self.cursor);
        let line_start = self.doc.line_to_char(cursor_line);
        let cursor_col = self.cursor - line_start;

        let source = self.doc.to_string();
        let formatted = self.syntax.formatter.format(source);

        self.doc = Rope::from_str(&formatted);
        self.update_doc_hash();

        // Restore cursor position as best we can
        let new_line = cursor_line.min(self.doc.len_lines() - 1);
        let line_len = self.doc.line(new_line).len_chars();
        let new_col = cursor_col.min(line_len);

        self.update_cursor(self.doc.line_to_char(new_line) + new_col);
        self.selection = None;
    }

    fn selected_text(&self) -> Option<String> {
        self.selection
            .as_ref()
            .map(|range| self.doc.slice(range.clone()).to_string())
    }

    // ========================================================================
    // External API (do not modify)
    // ========================================================================

    fn format_token(&self, ty: TokenType) -> egui::text::TextFormat {
        let font_id = egui::FontId::monospace(self.fontsize);
        let color = self.theme.type_color(ty);
        egui::text::TextFormat {
            font_id,
            color,
            line_height: Some(self.fontsize * 1.25),
            ..Default::default()
        }
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
