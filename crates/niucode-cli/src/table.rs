//! Terminal table renderer for Markdown pipe tables.
//!
//! Rows are buffered by the transcript until the block ends, because column
//! widths need the whole table. Cell text is inline-Markdown-stripped here;
//! styling inside cells is a later pass.

use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Align {
    Left,
    Center,
    Right,
}

pub fn is_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.len() > 1
}

fn cells(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let body = trimmed
        .strip_prefix('|')
        .unwrap_or(trimmed)
        .strip_suffix('|')
        .unwrap_or(trimmed);
    body.split('|')
        .map(|cell| {
            crate::markdown::inline(cell.trim())
                .into_iter()
                .map(|span| span.text)
                .collect::<String>()
        })
        .collect()
}

fn separator(cells: &[String]) -> Option<Vec<Align>> {
    if cells.is_empty() {
        return None;
    }
    let mut aligns = Vec::new();
    for cell in cells {
        let trimmed = cell.trim();
        if trimmed.is_empty()
            || !trimmed
                .chars()
                .all(|character| matches!(character, '-' | ':'))
        {
            return None;
        }
        if !trimmed.contains('-') {
            return None;
        }
        aligns.push(if trimmed.starts_with(':') && trimmed.ends_with(':') {
            Align::Center
        } else if trimmed.ends_with(':') {
            Align::Right
        } else {
            Align::Left
        });
    }
    Some(aligns)
}

/// Render a table block. The returned lines contain no ANSI styling; the
/// terminal applies the row tone once, so width stays stable.
pub fn render(lines: &[String], max_width: usize) -> Vec<String> {
    let mut rows: Vec<Vec<String>> = lines
        .iter()
        .filter(|line| is_row(line))
        .map(|line| cells(line))
        .collect();
    if rows.is_empty() {
        return Vec::new();
    }
    let mut aligns = vec![Align::Left; rows[0].len()];
    if rows.len() > 1 {
        if let Some(parsed) = separator(&rows[1]) {
            for (index, align) in parsed.into_iter().enumerate() {
                if let Some(slot) = aligns.get_mut(index) {
                    *slot = align;
                }
            }
            rows.remove(1);
        }
    }
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    if columns == 0 {
        return Vec::new();
    }
    let mut widths = vec![1usize; columns];
    for row in &rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.width());
        }
    }
    // Keep the block inside the viewport; truncate the widest columns first.
    let mut total = 1 + columns * 3 + widths.iter().sum::<usize>();
    while total > max_width && widths.iter().any(|width| *width > 3) {
        if let Some((index, _)) = widths.iter().enumerate().max_by_key(|(_, width)| **width) {
            widths[index] -= 1;
            total -= 1;
        }
    }

    let border_line = |left: &str, middle: &str, right: &str| {
        let mut line = String::from(left);
        for (index, width) in widths.iter().enumerate() {
            line.push_str(&"─".repeat(*width + 2));
            line.push_str(if index + 1 == columns { right } else { middle });
        }
        line
    };
    let mut out = vec![border_line("┌", "┬", "┐")];
    for (row_index, row) in rows.iter().enumerate() {
        let mut line = String::from("│");
        for (index, width) in widths.iter().enumerate() {
            let cell = row.get(index).map(String::as_str).unwrap_or("");
            let available = *width;
            let mut text = cell.to_string();
            if text.width() > available {
                text = truncate(&text, available);
            }
            let pad = available.saturating_sub(text.width());
            let (left, right) = match aligns.get(index).copied().unwrap_or(Align::Left) {
                Align::Left => (0, pad),
                Align::Right => (pad, 0),
                Align::Center => (pad / 2, pad - pad / 2),
            };
            line.push(' ');
            line.push_str(&" ".repeat(left));
            line.push_str(&text);
            line.push_str(&" ".repeat(right));
            line.push(' ');
            line.push('│');
        }
        out.push(line);
        if row_index == 0 {
            out.push(border_line("├", "┼", "┤"));
        }
    }
    out.push(border_line("└", "┴", "┘"));
    out
}

fn truncate(text: &str, width: usize) -> String {
    let mut out = String::new();
    for character in text.chars() {
        if out.width() + character.width_guess() > width.saturating_sub(1) {
            out.push('…');
            break;
        }
        out.push(character);
    }
    out
}

trait WidthGuess {
    fn width_guess(self) -> usize;
}
impl WidthGuess for char {
    fn width_guess(self) -> usize {
        unicode_width::UnicodeWidthChar::width(self).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block() -> Vec<String> {
        vec![
            "| name | value |".into(),
            "| :--- | ---: |".into(),
            "| alpha | 1 |".into(),
            "| beta | 22 |".into(),
        ]
    }

    #[test]
    fn detects_pipe_rows() {
        assert!(is_row("| a | b |"));
        assert!(is_row("  | a |  "));
        assert!(!is_row("a | b"));
        assert!(is_row("| code |"));
        assert!(!is_row("code |"));
    }

    #[test]
    fn renders_borders_header_and_alignment() {
        let lines = render(&block(), 80);
        assert!(lines[0].starts_with('┌'));
        assert!(lines[1].contains("name"));
        assert!(lines[2].starts_with('├'));
        assert!(lines.last().unwrap().starts_with('└'));
        // Right-aligned value column keeps 1 and 22 flush right.
        assert!(lines[3].contains("1 │"));
        assert!(lines[4].contains("22 │"));
        assert_eq!(lines.len(), 6);
    }

    #[test]
    fn inline_markers_are_stripped_from_cells() {
        let lines = render(&["| **bold** | `code` |".into()], 80);
        assert!(lines[1].contains("bold"));
        assert!(lines[1].contains("code"));
        assert!(!lines[1].contains('*'));
        assert!(!lines[1].contains('`'));
    }

    #[test]
    fn narrow_width_truncates_with_ellipsis() {
        let lines = render(&["| a very long cell | b |".into()], 15);
        assert!(lines.iter().any(|line| line.contains('…')));
        for line in &lines {
            assert!(line.width() <= 16, "{}", line.width());
        }
    }
}
