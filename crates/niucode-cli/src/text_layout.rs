//! Shared terminal text layout. No editor, terminal, or session state.
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const MAX_DISPLAY_LINE_BYTES: usize = 1_048_576;
const TRUNCATED: &str = "[display truncated]";

pub struct Layout {
    pub rows: Vec<String>,
    pub cursor: (usize, usize),
    pub positions: Vec<(usize, usize, usize)>,
}

/// Input reserves an insertion cell after a full final row. Output does not.
pub fn input(text: &str, cursor: usize, width: usize, prefix: &str) -> Layout {
    layout(text, width, prefix, Some(cursor))
}

/// Wrap untrusted output, preserving hard breaks and whole graphemes. Limit
/// each source line, so an oversized line cannot hide subsequent diagnostics.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut rows = Vec::new();
    for line in text.split('\n') {
        let end = if line.len() <= MAX_DISPLAY_LINE_BYTES {
            line.len()
        } else {
            line.grapheme_indices(true)
                .find(|(offset, g)| offset + g.len() > MAX_DISPLAY_LINE_BYTES)
                .map_or(line.len(), |(offset, _)| offset)
        };
        rows.extend(layout(&line[..end], width, "", None).rows);
        if end < line.len() {
            rows.extend(layout(TRUNCATED, width, "", None).rows);
        }
    }
    rows
}

fn layout(text: &str, width: usize, prefix: &str, input_cursor: Option<usize>) -> Layout {
    let width = width.max(1);
    let prefix = clip(prefix, width.saturating_sub(1));
    let mut col = prefix.width();
    let mut logical_col = col;
    let mut rows = vec![prefix];
    let mut positions = Vec::new();
    let mut cursor = (0, col);
    for (offset, grapheme) in text.grapheme_indices(true) {
        let mut displayed = display_grapheme(grapheme, logical_col);
        let size = displayed.width();
        // Tabs are individual spaces; a wide grapheme stays indivisible.
        if grapheme != "\t" && size > width {
            displayed = "�".into();
        }
        let next_width = if grapheme == "\t" {
            1
        } else {
            displayed.width()
        };
        if grapheme != "\n" && col + next_width > width {
            rows.push(String::new());
            col = 0;
        }
        let insertion = (rows.len() - 1, col.min(width - 1));
        if input_cursor.is_some() {
            positions.push((offset, insertion.0, insertion.1));
            if input_cursor == Some(offset) {
                cursor = insertion;
            }
        }
        if grapheme == "\n" {
            rows.push(String::new());
            col = 0;
            logical_col = 0;
        } else {
            for g in displayed.graphemes(true) {
                let w = g.width();
                if col + w > width {
                    rows.push(String::new());
                    col = 0;
                }
                rows.last_mut().expect("at least one row").push_str(g);
                col += w;
            }
            logical_col += size;
        }
    }
    if input_cursor.is_some() {
        if col == width {
            rows.push(String::new());
            col = 0;
        }
        positions.push((text.len(), rows.len() - 1, col));
        if input_cursor == Some(text.len()) {
            cursor = (rows.len() - 1, col);
        }
    }
    Layout {
        rows,
        cursor,
        positions,
    }
}

fn display_grapheme(grapheme: &str, column: usize) -> String {
    if grapheme == "\t" {
        return " ".repeat(4 - column % 4);
    }
    grapheme
        .chars()
        .map(|c| match c {
            '\u{1b}' => '␛',
            c if c.is_control() => '·',
            c => c,
        })
        .collect()
}

/// Neutralize terminal controls and expand tabs without changing line layout.
pub fn clean_text(text: &str) -> String {
    let mut cleaned = String::with_capacity(text.len());
    let mut column = 0;
    for grapheme in text.graphemes(true) {
        let displayed = display_grapheme(grapheme, column);
        column += displayed.width();
        cleaned.push_str(&displayed);
    }
    cleaned
}

/// Native scrollback keeps its own soft wrapping and explicit document breaks.
pub fn clean_multiline(text: &str) -> String {
    text.split('\n')
        .map(clean_text)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn clip(text: &str, width: usize) -> String {
    let mut result = String::new();
    let mut used = 0;
    for g in text.graphemes(true) {
        let size = g.width();
        if used + size > width {
            break;
        }
        result.push_str(g);
        used += size;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_breaks_and_full_rows_do_not_invent_blank_output_lines() {
        for (text, expected) in [
            ("", vec![""]),
            ("abc", vec!["abc"]),
            ("abc\nx", vec!["abc", "x"]),
            ("abc\n\nx\n", vec!["abc", "", "x", ""]),
            ("abcdef", vec!["abc", "def"]),
        ] {
            assert_eq!(wrap(text, 3), expected, "{text:?}");
        }
        let input = input("abc", 3, 3, "");
        assert_eq!(input.rows, ["abc", ""]);
        assert_eq!(input.cursor, (1, 0));
    }

    #[test]
    fn graphemes_and_controls_are_safe_even_in_one_column() {
        let text = "中e\u{301}👩‍💻\t\x1b]52;bad\x07\r";
        for width in 0..12 {
            let rows = wrap(text, width);
            assert!(rows.iter().all(|r| r.width() <= width.max(1)));
            assert!(rows.iter().all(|r| !r.chars().any(char::is_control)));
            let joined = rows.concat();
            assert!(joined.contains("e\u{301}"));
            assert!(joined.contains("␛]52;bad··"));
            if width >= 2 {
                assert!(joined.contains("👩‍💻"));
                assert!(joined.contains('中'));
            } else {
                assert!(joined.starts_with("�e\u{301}�"));
            }
        }
        assert_eq!(clip("👩‍💻e\u{301}", 2), "👩‍💻");
        assert_eq!(clip("👩‍💻", 1), "");
    }

    #[test]
    fn tabs_use_grapheme_columns_and_do_not_split_into_empty_rows() {
        assert_eq!(clean_text("👩‍💻\tx"), "👩‍💻  x");
        assert_eq!(clean_text("e\u{301}\tx"), "e\u{301}   x");
        assert_eq!(wrap("\t", 1), [" ", " ", " ", " "]);
        assert_eq!(wrap("abcd\tx", 3), ["abc", "d  ", "  x"]);
        for width in 1..9 {
            let text = "👩‍💻\tabcde\tx";
            let rows = wrap(text, width);
            let mut editing = input(text, text.len(), width, "");
            if editing.rows.last().is_some_and(String::is_empty) {
                editing.rows.pop(); // Only the editor reserves a final cursor cell.
            }
            assert_eq!(rows, editing.rows);
        }
    }

    #[test]
    fn cursor_positions_follow_hard_breaks_soft_wraps_and_prefixes() {
        let text = "中e\u{301}\n👩‍💻x";
        for width in 1..10 {
            for offset in text
                .grapheme_indices(true)
                .map(|(i, _)| i)
                .chain([text.len()])
            {
                let result = input(text, offset, width, "> ");
                assert!(result.cursor.0 < result.rows.len());
                assert!(result.cursor.1 < width);
                assert!(result.rows.iter().all(|r| r.width() <= width));
                assert!(result
                    .positions
                    .contains(&(offset, result.cursor.0, result.cursor.1)));
            }
        }
        let newline = input("abc\nx", 3, 3, "");
        assert_eq!(newline.rows, ["abc", "x"]);
        assert_eq!(newline.cursor, (0, 2));
        let tab = input("abc\tx", 3, 3, "");
        assert_eq!(tab.rows, ["abc", " x"]);
        assert_eq!(tab.cursor, (1, 0));
    }

    #[test]
    fn oversized_line_keeps_a_grapheme_safe_prefix_marker_and_next_line() {
        let prefix = "x".repeat(MAX_DISPLAY_LINE_BYTES - 1);
        let text = format!("{prefix}e\u{301}lost\nnext diagnostic");
        let rows = wrap(&text, 79);
        assert!(rows.iter().all(|r| r.width() <= 79));
        assert_eq!(rows.concat(), format!("{prefix}{TRUNCATED}next diagnostic"));
        assert_eq!(rows.last().unwrap(), "next diagnostic");
    }
}
