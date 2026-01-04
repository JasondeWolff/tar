use ropey::Rope;
use std::ops::Range;
use tree_sitter::Query;

#[derive(Clone, Copy)]
struct Highlight {
    color: egui::Color32,
}

fn highlight_for_kind(kind: &str) -> Highlight {
    use egui::Color32;

    match kind {
        "fn" | "let" | "var" | "struct" | "return" => Highlight {
            color: Color32::from_rgb(220, 160, 100),
        },

        "ident" => Highlight {
            color: Color32::from_rgb(220, 220, 220),
        },

        "number_literal" => Highlight {
            color: Color32::from_rgb(180, 220, 180),
        },

        "type" => Highlight {
            color: Color32::from_rgb(160, 200, 255),
        },

        _ => Highlight {
            color: Color32::from_rgb(200, 200, 200),
        },
    }
}

const WGSL_HIGHLIGHT_QUERY: &str = r#"
; Identifiers (includes keywords like 'let')
(ident)            @ident

; Types
(vec2)             @type
(vec3)             @type
(vec4)             @type
(mat4x4)           @type

; Punctuation
(colon)            @punct
(equal)            @punct
(semicolon)        @punct
(paren_left)       @punct
(paren_right)      @punct
"#;

struct Token<'a> {
    start_byte: usize,
    end_byte: usize,
    text: &'a str,
    kind: &'a str,
}

fn collect_line_tokens<'a>(
    root: tree_sitter::Node,
    source: &'a str,
    line_idx: usize,
    query: &'a Query,
) -> Vec<Token<'a>> {
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut tokens = Vec::new();

    for m in cursor.matches(&query, root, source.as_bytes()) {
        for cap in m.captures {
            let node = cap.node;
            let start = node.start_position();
            let end = node.end_position();

            // Only tokens fully on this line
            if start.row == line_idx && end.row == line_idx {
                tokens.push(Token {
                    start_byte: node.start_byte(),
                    end_byte: node.end_byte(),
                    text: &source[node.start_byte()..node.end_byte()],
                    kind: query.capture_names()[cap.index as usize].as_str(),
                });
            }
        }
    }

    // CRUCIAL: sort left → right
    tokens.sort_by_key(|t| t.start_byte);
    tokens
}

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
    parser: tree_sitter::Parser,
}

impl CodeEditor {
    pub fn new(text: &str) -> Self {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_wgsl::language())
            .expect("WGSL grammar");

        Self {
            doc: Rope::from_str(text),
            cursor: 0,
            cursor_blink_offset: 0.0,
            selection: None,
            parser,
        }
    }

    fn draw_highlighted_line(
        painter: &egui::Painter,
        root: tree_sitter::Node,
        source: &str,
        line_idx: usize,
        mut x: f32,
        y: f32,
        font_id: &egui::FontId,
        ui: &egui::Ui,
    ) {
        let language = tree_sitter_wgsl::language();
        let query = tree_sitter::Query::new(language, WGSL_HIGHLIGHT_QUERY)
            .expect("invalid WGSL highlight query");
        let tokens = collect_line_tokens(root, source, line_idx, &query);

        let line_start_byte = source
            .lines()
            .take(line_idx)
            .map(|l| l.len() + 1)
            .sum::<usize>();

        let mut cursor_byte = line_start_byte;

        for token in tokens {
            // Draw gap (whitespace or un-highlighted text)
            if token.start_byte > cursor_byte {
                let gap = &source[cursor_byte..token.start_byte];
                if gap.trim().is_empty() {
                    // draw whitespace
                    x += ui.fonts_mut(|f| {
                        f.layout_no_wrap(gap.to_string(), font_id.clone(), egui::Color32::WHITE)
                            .size()
                            .x
                    });
                } else {
                    // draw unhighlighted text
                    painter.text(
                        egui::pos2(x, y),
                        egui::Align2::LEFT_TOP,
                        gap,
                        font_id.clone(),
                        egui::Color32::LIGHT_GRAY,
                    );
                    x += ui.fonts_mut(|f| {
                        f.layout_no_wrap(
                            gap.to_string(),
                            font_id.clone(),
                            egui::Color32::LIGHT_GRAY,
                        )
                        .size()
                        .x
                    });
                }
            }

            let hl = highlight_for_kind(token.kind);

            painter.text(
                egui::pos2(x, y),
                egui::Align2::LEFT_TOP,
                token.text,
                font_id.clone(),
                hl.color,
            );

            x += ui.fonts_mut(|f| {
                f.layout_no_wrap(token.text.to_string(), font_id.clone(), hl.color)
                    .size()
                    .x
            });

            cursor_byte = token.end_byte;
        }
    }

    fn dump_tree(root: tree_sitter::Node, source: &str) {
        let mut cursor = root.walk();
        loop {
            let node = cursor.node();
            println!(
                "{} {:?}",
                node.kind(),
                &source[node.start_byte()..node.end_byte()]
            );

            if cursor.goto_first_child() {
                continue;
            }
            while !cursor.goto_next_sibling() {
                if !cursor.goto_parent() {
                    return;
                }
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let font_id = egui::FontId::monospace(14.0);
        let line_height = ui.fonts_mut(|f| f.row_height(&font_id));

        println!("\n\n\n");
        let desired_size = egui::vec2(
            ui.available_width(),
            line_height * self.doc.len_lines() as f32,
        );

        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());

        let painter = ui.painter_at(rect);

        // Background
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(25, 25, 25));

        // Parse WGSL
        let source = self.doc.to_string();
        let tree = self.parser.parse(&source, None);

        if let Some(tree) = &tree {
            Self::dump_tree(tree.root_node(), &source);
        }

        // --- Render text line by line ---
        let mut y = rect.min.y;

        for line_idx in 0..self.doc.len_lines() {
            let line = self.doc.line(line_idx);
            let line_str = line.to_string();

            let x = rect.min.x + 6.0;

            if let Some(tree) = &tree {
                let root = tree.root_node();
                Self::draw_highlighted_line(&painter, root, &source, line_idx, x, y, &font_id, ui);
            } else {
                painter.text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_TOP,
                    line_str,
                    font_id.clone(),
                    egui::Color32::LIGHT_GRAY,
                );
            }

            y += line_height;
        }

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
}
