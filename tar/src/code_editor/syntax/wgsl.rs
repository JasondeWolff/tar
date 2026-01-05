use crate::code_editor::syntax::SyntaxFormatter;

use super::Syntax;
use std::collections::BTreeSet;

struct Formatter;
impl SyntaxFormatter for Formatter {
    fn format(&self, code: String) -> String {
        const INDENT_SIZE: usize = 4;

        let mut result = String::new();
        let mut indent_level: usize = 0;
        let mut chars = code.chars().peekable();
        let mut line_buffer = String::new();
        let mut needs_indent = true;

        fn trim_before_trim_after(
            character: char,
            line_buffer: &mut String,
            chars: &mut std::iter::Peekable<std::str::Chars>,
        ) {
            *line_buffer = line_buffer.trim_end().to_string();
            line_buffer.push(character);
            while chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                chars.next();
            }
        }

        fn trim_before_space_after(
            character: char,
            line_buffer: &mut String,
            chars: &mut std::iter::Peekable<std::str::Chars>,
        ) {
            *line_buffer = line_buffer.trim_end().to_string();
            line_buffer.push(character);
            while chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                chars.next();
            }
            line_buffer.push(' ');
        }

        fn trim_before_newline_after(
            character: char,
            line_buffer: &mut String,
            chars: &mut std::iter::Peekable<std::str::Chars>,
            indent_level: usize,
        ) {
            *line_buffer = line_buffer.trim_end().to_string();
            line_buffer.push(character);
            while chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                chars.next();
            }

            if chars.peek() != Some(&'\n') {
                line_buffer.push('\n');
                line_buffer.push_str(&" ".repeat(indent_level * INDENT_SIZE));
            }
        }

        fn space_before_space_after(
            character: char,
            line_buffer: &mut String,
            chars: &mut std::iter::Peekable<std::str::Chars>,
        ) {
            if !line_buffer.ends_with(' ') && !line_buffer.is_empty() {
                line_buffer.push(' ');
            }
            line_buffer.push(character);
            while chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                chars.next();
            }
            line_buffer.push(' ');
        }

        fn space_before_space_after_str(
            text: &str,
            line_buffer: &mut String,
            chars: &mut std::iter::Peekable<std::str::Chars>,
        ) {
            if !line_buffer.ends_with(' ') && !line_buffer.is_empty() {
                line_buffer.push(' ');
            }
            line_buffer.push_str(text);
            while chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                chars.next();
            }
            line_buffer.push(' ');
        }

        while let Some(ch) = chars.next() {
            // Check for @include directive
            if ch == '@' && line_buffer.trim().is_empty() {
                line_buffer.push(ch);
                // Peek ahead to see if this is @include
                let mut lookahead = String::new();
                let mut peek_chars = Vec::new();

                while let Some(&next_ch) = chars.peek() {
                    if next_ch.is_alphanumeric() || next_ch == '_' {
                        peek_chars.push(chars.next().unwrap());
                        lookahead.push(*peek_chars.last().unwrap());
                    } else {
                        break;
                    }
                }

                line_buffer.push_str(&lookahead);

                if lookahead == "include" {
                    // This is @include - add rest of line without formatting
                    while let Some(&next_ch) = chars.peek() {
                        if next_ch == '\n' {
                            break;
                        }
                        line_buffer.push(chars.next().unwrap());
                    }
                    continue;
                }
                // Not @include, continue normal processing
                continue;
            }

            match ch {
                '{' => {
                    if line_buffer.trim().is_empty() {
                        if result.ends_with('\n') {
                            result.pop();
                            while result.ends_with(' ') || result.ends_with('\t') {
                                result.pop();
                            }
                        }
                        result.push_str(" {");
                        line_buffer.clear();
                    } else {
                        line_buffer = line_buffer.trim_end().to_string();
                        line_buffer.push_str(" {");
                    }
                    indent_level += 1;
                }
                '}' => {
                    if !line_buffer.trim().is_empty() {
                        result.push_str(&line_buffer);
                        result.push('\n');
                        line_buffer.clear();
                    }
                    indent_level = indent_level.saturating_sub(1);
                    result.push_str(&" ".repeat(indent_level * INDENT_SIZE));
                    result.push('}');
                    result.push('\n');
                    needs_indent = true;
                }
                // Check for multi-character operators
                '-' => {
                    if chars.peek() == Some(&'>') {
                        chars.next(); // consume '>'
                        space_before_space_after_str("->", &mut line_buffer, &mut chars);
                    } else {
                        space_before_space_after('-', &mut line_buffer, &mut chars);
                    }
                }
                '/' => {
                    if chars.peek() == Some(&'/') {
                        // Comment - add everything as-is until end of line
                        line_buffer.push('/');
                        line_buffer.push('/');
                        chars.next(); // consume second '/'

                        // Add rest of line without formatting
                        while let Some(&next_ch) = chars.peek() {
                            if next_ch == '\n' {
                                break;
                            }
                            line_buffer.push(chars.next().unwrap());
                        }
                    } else if chars.peek() == Some(&'*') {
                        // Multi-line comment - add everything until */
                        line_buffer.push('/');
                        line_buffer.push('*');
                        chars.next(); // consume '*'

                        while let Some(next_ch) = chars.next() {
                            line_buffer.push(next_ch);
                            if next_ch == '*' && chars.peek() == Some(&'/') {
                                line_buffer.push(chars.next().unwrap());
                                break;
                            }
                        }
                    } else {
                        space_before_space_after('/', &mut line_buffer, &mut chars);
                    }
                }
                c @ ('+' | '*' | '%') => {
                    space_before_space_after(c, &mut line_buffer, &mut chars);
                }
                c @ (':' | ',') => {
                    trim_before_space_after(c, &mut line_buffer, &mut chars);
                }
                '.' => {
                    trim_before_trim_after('.', &mut line_buffer, &mut chars);
                }
                ';' => {
                    trim_before_newline_after(';', &mut line_buffer, &mut chars, indent_level);
                }
                '\n' => {
                    let trimmed = line_buffer.trim_end();
                    if !trimmed.is_empty() {
                        result.push_str(trimmed);
                    }
                    result.push('\n');
                    line_buffer.clear();
                    needs_indent = true;
                }
                '\r' => {}
                _ => {
                    if needs_indent && !ch.is_whitespace() {
                        line_buffer.push_str(&" ".repeat(indent_level * INDENT_SIZE));
                        needs_indent = false;
                    }

                    if needs_indent && ch.is_whitespace() {
                        continue;
                    }

                    line_buffer.push(ch);
                }
            }
        }

        if !line_buffer.trim().is_empty() {
            result.push_str(&line_buffer);
            result.push('\n');
        }

        while result.ends_with("\n\n") {
            result.pop();
        }
        if !result.ends_with('\n') && !result.is_empty() {
            result.push('\n');
        }

        result
    }
}

impl Syntax {
    pub fn wgsl() -> Self {
        Syntax {
            language: "Wgsl",
            case_sensitive: true,
            comment: "//",
            comment_multiline: ["/*", "*/"],
            hyperlinks: BTreeSet::from(["http"]),
            keywords: BTreeSet::from([
                "@align",
                "@binding",
                "@builtin",
                "@compute",
                "@const",
                "@diagnostic",
                "@fragment",
                "@group",
                "@id",
                "@include",
                "@interpolate",
                "@invariant",
                "@location",
                "@blend_src",
                "@must_use",
                "@size",
                "@vertex",
                "@workgroup_size",
                "alias",
                "break",
                "case",
                "const",
                "const_assert",
                "continue",
                "continuing",
                "default",
                "diagnostic",
                "discard",
                "else",
                "enable",
                "fn",
                "for",
                "if",
                "let",
                "loop",
                "override",
                "requires",
                "return",
                "struct",
                "switch",
                "var",
                "while",
            ]),
            types: BTreeSet::from([
                "atomic", "array", "bool", "i32", "u32", "f16", "f32", "vec2", "vec2i", "vec2f",
                "vec2h", "vec3", "vec3i", "vec3f", "vec3h", "vec4", "vec4i", "vec4f", "vec4h",
                "mat2x2", "mat2x2f", "mat2x2h", "mat2x3", "mat2x3f", "mat2x3h", "mat2x4",
                "mat2x4f", "mat2x4h", "mat3x2", "mat3x2f", "mat3x2h", "mat3x3", "mat3x3f",
                "mat3x3h", "mat3x4", "mat3x4f", "mat3x4h", "mat4x2", "mat4x2f", "mat4x2h",
                "mat4x3", "mat4x3f", "mat4x3h", "mat4x4", "mat4x4f", "mat4x4h",
            ]),
            special: BTreeSet::from(["true", "false"]),
            formatter: Box::new(Formatter),
        }
    }
}
