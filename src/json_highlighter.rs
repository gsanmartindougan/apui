use iced_core::{Color, text::Highlighter};
use std::ops::Range;

pub struct JsonHighlighter {
    line: usize,
}

impl Highlighter for JsonHighlighter {
    type Settings = ();
    type Highlight = Color;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Color)>;

    fn new(_: &Self::Settings) -> Self {
        Self { line: 0 }
    }

    fn update(&mut self, _: &Self::Settings) {}

    fn change_line(&mut self, line: usize) {
        self.line = line;
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let tokens = tokens(line);
        self.line += 1;
        tokens.into_iter()
    }

    fn current_line(&self) -> usize {
        self.line
    }
}

fn tokens(line: &str) -> Vec<(Range<usize>, Color)> {
    let bytes = line.as_bytes();
    let mut spans = Vec::new();
    let mut cursor = 0;

    while cursor < bytes.len() {
        if bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
            continue;
        }
        let start = cursor;
        let color = match bytes[cursor] {
            b'"' => {
                cursor += 1;
                while cursor < bytes.len() {
                    if bytes[cursor] == b'\\' {
                        cursor = (cursor + 2).min(bytes.len());
                    } else if bytes[cursor] == b'"' {
                        cursor += 1;
                        break;
                    } else {
                        cursor += 1;
                    }
                }
                let mut next = cursor;
                while next < bytes.len() && bytes[next].is_ascii_whitespace() {
                    next += 1;
                }
                if bytes.get(next) == Some(&b':') {
                    Color::from_rgb8(107, 46, 99)
                } else {
                    Color::from_rgb8(27, 54, 68)
                }
            }
            b'{' | b'}' | b'[' | b']' | b',' | b':' => {
                cursor += 1;
                Color::from_rgb8(108, 124, 130)
            }
            b'-' | b'0'..=b'9' => {
                cursor += 1;
                while cursor < bytes.len()
                    && (bytes[cursor].is_ascii_digit()
                        || matches!(bytes[cursor], b'.' | b'e' | b'E' | b'+' | b'-'))
                {
                    cursor += 1;
                }
                Color::from_rgb8(47, 122, 86)
            }
            _ => {
                if !bytes[cursor].is_ascii_alphabetic() {
                    cursor += 1;
                    continue;
                }
                while cursor < bytes.len() && bytes[cursor].is_ascii_alphabetic() {
                    cursor += 1;
                }
                let color = match &line[start..cursor] {
                    "true" | "false" | "null" => Color::from_rgb8(157, 46, 40),
                    _ => continue,
                };
                color
            }
        };
        spans.push((start..cursor, color));
    }

    spans
}
