//! Terminal table renderer for Markdown pipe tables.
//!
//! Rows are buffered by the transcript until the block ends, because column
//! widths need the whole table. Cell text carries inline-Markdown styles as
//! byte ranges into the final line; the terminal applies them without changing
//! cell width.

use unicode_width::UnicodeWidthStr;

use crate::markdown::Style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableLineKind {
    Border,
    Header,
    Row,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledRange {
    pub start: usize,
    pub end: usize,
    pub style: Style,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableLine {
    pub text: String,
    pub kind: TableLineKind,
    pub styles: Vec<StyledRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Cell {
    text: String,
    styles: Vec<StyledRange>,
}

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

fn cell(raw: &str) -> Cell {
    let mut text = String::new();
    let mut styles = Vec::new();
    for span in crate::markdown::inline(raw.trim()) {
        let start = text.len();
        text.push_str(&span.text);
        if span.style != Style::default() {
            styles.push(StyledRange {
                start,
                end: text.len(),
                style: span.style,
            });
        }
    }
    Cell { text, styles }
}

fn cells(line: &str) -> Vec<Cell> {
    let trimmed = line.trim();
    let body = trimmed
        .strip_prefix('|')
        .unwrap_or(trimmed)
        .strip_suffix('|')
        .unwrap_or(trimmed);
    body.split('|').map(cell).collect()
}

fn separator(cells: &[Cell]) -> Option<Vec<Align>> {
    if cells.is_empty() {
        return None;
    }
    let mut aligns = Vec::new();
    for cell in cells {
        let trimmed = cell.text.trim();
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

fn border_line(widths: &[usize], left: &str, middle: &str, right: &str) -> TableLine {
    let mut text = String::from(left);
    for (index, width) in widths.iter().enumerate() {
        text.push_str(&"─".repeat(*width + 2));
        text.push_str(if index + 1 == widths.len() {
            right
        } else {
            middle
        });
    }
    TableLine {
        text,
        kind: TableLineKind::Border,
        styles: Vec::new(),
    }
}

/// Render a table block. The returned lines contain no ANSI styling; the
/// terminal maps `styles` to spans, so width stays stable.
pub fn render(lines: &[String], max_width: usize) -> Vec<TableLine> {
    let mut rows: Vec<Vec<Cell>> = lines
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
            widths[index] = widths[index].max(cell.text.width());
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

    let mut out = vec![border_line(&widths, "┌", "┬", "┐")];
    for (row_index, row) in rows.iter().enumerate() {
        let mut text = String::from("│");
        let mut styles = Vec::new();
        for (index, width) in widths.iter().enumerate() {
            let cell = row.get(index);
            let plain = cell.map(|cell| cell.text.as_str()).unwrap_or("");
            let available = *width;
            let (cell_text, keep_styles) = if plain.width() > available {
                (truncate(plain, available), false)
            } else {
                (plain.to_string(), true)
            };
            let pad = available.saturating_sub(cell_text.width());
            let (left, right) = match aligns.get(index).copied().unwrap_or(Align::Left) {
                Align::Left => (0, pad),
                Align::Right => (pad, 0),
                Align::Center => (pad / 2, pad - pad / 2),
            };
            text.push(' ');
            text.push_str(&" ".repeat(left));
            if keep_styles {
                if let Some(cell) = cell {
                    let offset = text.len();
                    for style in &cell.styles {
                        styles.push(StyledRange {
                            start: offset + style.start,
                            end: offset + style.end,
                            style: style.style,
                        });
                    }
                }
            }
            text.push_str(&cell_text);
            text.push_str(&" ".repeat(right));
            text.push(' ');
            text.push('│');
        }
        out.push(TableLine {
            text,
            kind: if row_index == 0 {
                TableLineKind::Header
            } else {
                TableLineKind::Row
            },
            styles,
        });
        if row_index == 0 {
            out.push(border_line(&widths, "├", "┼", "┤"));
        }
    }
    out.push(border_line(&widths, "└", "┴", "┘"));
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
        assert!(lines[0].text.starts_with('┌'));
        assert!(lines[1].text.contains("name"));
        assert!(lines[2].text.starts_with('├'));
        assert!(lines.last().unwrap().text.starts_with('└'));
        // Right-aligned value column keeps 1 and 22 flush right.
        assert!(lines[3].text.contains("1 │"));
        assert!(lines[4].text.contains("22 │"));
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[0].kind, TableLineKind::Border);
        assert_eq!(lines[1].kind, TableLineKind::Header);
        assert_eq!(lines[3].kind, TableLineKind::Row);
    }

    #[test]
    fn inline_styles_become_ranges_without_markers() {
        let lines = render(&["| **bold** | `code` |".into()], 80);
        assert!(lines[1].text.contains("bold"));
        assert!(lines[1].text.contains("code"));
        assert!(!lines[1].text.contains('*'));
        assert!(!lines[1].text.contains('`'));
        assert!(lines[1].styles.iter().any(|range| range.style.bold));
        assert!(lines[1].styles.iter().any(|range| range.style.code));
        for range in &lines[1].styles {
            assert!(range.start < range.end);
            assert!(range.end <= lines[1].text.len());
        }
    }

    #[test]
    fn narrow_width_truncates_with_ellipsis() {
        let lines = render(&["| a very long cell | b |".into()], 15);
        assert!(lines.iter().any(|line| line.text.contains('…')));
        for line in &lines {
            assert!(line.text.width() <= 16, "{}", line.text.width());
        }
    }
}
