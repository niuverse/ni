//! Width-aware rendering for structured change previews.
//!
//! This module is pure: it turns a `ChangePreview` into terminal rows. The
//! approval pager and the headless fallback both render those rows, so layout
//! and line-number alignment are tested without a terminal.

use crate::text_layout::{clean_text, wrap};
use niucode_tools::{ChangePreview, FilePreview, LineKind, PreviewBody, PreviewLine};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const MIN_WIDTH: usize = 40;
const MIN_BODY_WIDTH: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    File,
    Hunk,
    Context,
    Add,
    Remove,
    Note,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub kind: RowKind,
    pub text: String,
}

pub fn layout(preview: &ChangePreview, width: usize) -> Vec<Row> {
    let width = width.max(MIN_WIDTH);
    let mut rows = Vec::new();

    if let Some(warning) = &preview.warning {
        rows.extend(wrap_prefixed("! ", warning, width, RowKind::Warning));
    }

    match &preview.body {
        PreviewBody::Diff { files } => {
            for file in files {
                rows.extend(file_rows(file, width));
            }
        }
        PreviewBody::Command { command } => {
            if command.is_empty() {
                rows.push(note("(empty command)"));
            } else {
                for (index, line) in command.lines().enumerate() {
                    let prefix = if index == 0 { "$ " } else { "  " };
                    rows.extend(wrap_prefixed(prefix, line, width, RowKind::Note));
                }
            }
        }
        PreviewBody::Note { message } => {
            rows.extend(wrap_prefixed("! ", message, width, RowKind::Note));
        }
    }

    if rows.is_empty() {
        rows.push(note("(nothing to preview)"));
    }
    rows
}

/// Render a preview without color, for headless approval and non-TTY input.
pub fn render_plain(preview: &ChangePreview, width: usize) -> String {
    let mut rendered = String::new();
    for row in layout(preview, width) {
        rendered.push_str(&row.text);
        rendered.push('\n');
    }
    rendered
}

/// Starting row of each file header, in display order.
pub fn file_starts(rows: &[Row]) -> Vec<usize> {
    rows.iter()
        .enumerate()
        .filter_map(|(index, row)| (row.kind == RowKind::File).then_some(index))
        .collect()
}

pub fn display_width(text: &str) -> usize {
    text.width()
}

fn file_rows(file: &FilePreview, width: usize) -> Vec<Row> {
    let stats = format!("+{} -{}", file.added, file.removed);
    let path = clean_text(&file.path);
    let label = if file.is_new {
        format!("{path} (new file)")
    } else {
        path
    };
    let label_budget = width
        .saturating_sub(2 + display_width(&stats) + 2)
        .max(MIN_BODY_WIDTH);
    let label = fit_tail(&label, label_budget);
    let padding = width.saturating_sub(2 + display_width(&label) + display_width(&stats));
    let header = format!("▎ {label}{}{stats}", " ".repeat(padding));

    let mut rows = vec![Row {
        kind: RowKind::File,
        text: header,
    }];

    if file.hunks.is_empty() {
        rows.push(Row {
            kind: RowKind::Note,
            text: "  (no textual changes)".to_string(),
        });
        return rows;
    }

    let (old_width, new_width) = line_number_widths(file);
    for hunk in &file.hunks {
        rows.push(Row {
            kind: RowKind::Hunk,
            text: format!(
                "@@ -{} +{} @@",
                range_text(hunk.old_start, hunk.old_len),
                range_text(hunk.new_start, hunk.new_len)
            ),
        });
        for line in &hunk.lines {
            rows.extend(render_line(line, old_width, new_width, width));
        }
    }

    rows
}

fn render_line(line: &PreviewLine, old_width: usize, new_width: usize, width: usize) -> Vec<Row> {
    let sign = match line.kind {
        LineKind::Context => ' ',
        LineKind::Add => '+',
        LineKind::Remove => '-',
    };
    let old = line
        .old_line
        .map(|line| line.to_string())
        .unwrap_or_default();
    let new = line
        .new_line
        .map(|line| line.to_string())
        .unwrap_or_default();
    let prefix = format!(
        "{old:>old_width$} {new:>new_width$} │ {sign} ",
        old_width = old_width,
        new_width = new_width
    );
    let continuation = format!(
        "{:>old_width$} {:>new_width$} │   ",
        "",
        "",
        old_width = old_width,
        new_width = new_width
    );
    let body_width = width
        .saturating_sub(display_width(&prefix))
        .max(MIN_BODY_WIDTH);
    let cleaned = clean_text(&line.text);
    let segments = wrap(&cleaned, body_width);
    let kind = match line.kind {
        LineKind::Context => RowKind::Context,
        LineKind::Add => RowKind::Add,
        LineKind::Remove => RowKind::Remove,
    };

    segments
        .into_iter()
        .enumerate()
        .map(|(index, segment)| Row {
            kind,
            text: if index == 0 {
                format!("{prefix}{segment}")
            } else {
                format!("{continuation}{segment}")
            },
        })
        .collect()
}

fn line_number_widths(file: &FilePreview) -> (usize, usize) {
    let max_old = file
        .hunks
        .iter()
        .flat_map(|hunk| &hunk.lines)
        .filter_map(|line| line.old_line)
        .max()
        .unwrap_or(0);
    let max_new = file
        .hunks
        .iter()
        .flat_map(|hunk| &hunk.lines)
        .filter_map(|line| line.new_line)
        .max()
        .unwrap_or(0);
    (
        max_old.to_string().len().max(2),
        max_new.to_string().len().max(2),
    )
}

fn range_text(start: usize, len: usize) -> String {
    if len == 1 {
        start.to_string()
    } else {
        format!("{start},{len}")
    }
}

fn wrap_prefixed(prefix: &str, text: &str, width: usize, kind: RowKind) -> Vec<Row> {
    let cleaned = clean_text(text);
    let prefix_width = display_width(prefix);
    let body_width = width.saturating_sub(prefix_width).max(1);
    wrap(&cleaned, body_width)
        .into_iter()
        .enumerate()
        .map(|(index, segment)| Row {
            kind,
            text: if index == 0 {
                format!("{prefix}{segment}")
            } else {
                format!("{}{segment}", " ".repeat(prefix_width))
            },
        })
        .collect()
}

fn fit_tail(text: &str, budget: usize) -> String {
    if display_width(text) <= budget {
        return text.to_string();
    }
    if budget == 0 {
        return String::new();
    }

    let mut tail = Vec::new();
    let mut used = 0;
    for grapheme in text.graphemes(true).rev() {
        let width = grapheme.width();
        if used + width > budget.saturating_sub(1) {
            break;
        }
        tail.push(grapheme);
        used += width;
    }
    tail.reverse();
    format!("…{}", tail.concat())
}

fn note(message: impl Into<String>) -> Row {
    Row {
        kind: RowKind::Note,
        text: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use niucode_tools::{ChangePreview, FilePreview};

    use super::*;

    fn preview(old: &str, new: &str, width: usize) -> Vec<Row> {
        let file = FilePreview::from_text("src/main.rs", old, new, false);
        layout(&ChangePreview::diff(vec![file]), width)
    }

    #[test]
    fn rows_never_exceed_the_requested_width() {
        let old = "short\n".to_string() + &"a".repeat(300) + "\nend\n";
        let new = "short\n".to_string() + &"b".repeat(300) + "\nend\n";
        for width in [40, 60, 80, 120] {
            for row in preview(&old, &new, width) {
                assert!(
                    display_width(&row.text) <= width,
                    "row wider than {width}: {:?} ({})",
                    row.text,
                    display_width(&row.text)
                );
            }
        }
    }

    #[test]
    fn line_numbers_and_change_markers_share_columns() {
        let rows = preview("a\nb\nc\n", "a\nB\nc\n", 80);
        let removed = rows
            .iter()
            .find(|row| row.kind == RowKind::Remove && row.text.contains("b"))
            .expect("removed row");
        let added = rows
            .iter()
            .find(|row| row.kind == RowKind::Add && row.text.contains("B"))
            .expect("added row");
        assert!(removed.text.contains("│ - b"));
        assert!(added.text.contains("│ + B"));
        let removed_marker = removed.text.find("│ - ").expect("marker");
        let added_marker = added.text.find("│ + ").expect("marker");
        assert_eq!(removed_marker, added_marker);
        assert!(removed.text[..removed_marker].trim().ends_with('2'));
        assert!(added.text[..added_marker].trim().ends_with('2'));
    }

    #[test]
    fn hunk_headers_are_rendered() {
        let rows = preview("a\nb\nc\n", "a\nB\nc\n", 80);
        assert!(rows
            .iter()
            .any(|row| row.kind == RowKind::Hunk && row.text == "@@ -1,3 +1,3 @@"));
        assert!(rows
            .iter()
            .any(|row| row.kind == RowKind::File && row.text.contains("src/main.rs")));
    }

    #[test]
    fn long_lines_wrap_under_the_gutter() {
        let long = "x".repeat(200);
        let rows = preview("old\n", &format!("{long}\n"), 60);
        let body: Vec<&Row> = rows
            .iter()
            .filter(|row| matches!(row.kind, RowKind::Add | RowKind::Remove))
            .collect();
        assert!(body.len() > 2, "expected wrapped rows");
        assert!(body.iter().all(|row| row.text.contains('│')));
        for row in body {
            assert!(display_width(&row.text) <= 60);
        }
    }

    #[test]
    fn tabs_expand_and_escape_sequences_stay_visible() {
        let file = FilePreview::from_text("a.rs", "", "\tif x {\u{1b}[31m}\n", true);
        let rows = layout(&ChangePreview::diff(vec![file]), 80);
        let text = rows
            .iter()
            .map(|row| row.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !text.contains('\u{1b}'),
            "raw escape leaked to the terminal"
        );
        assert!(text.contains("if x {␛[31m}"));
        assert!(text.contains("    if x"));
    }

    #[test]
    fn wide_characters_fit_in_columns() {
        let text = "中文注释".repeat(10);
        let file = FilePreview::from_text("a.rs", "", &format!("{text}\n"), true);
        let rows = layout(&ChangePreview::diff(vec![file]), 48);
        for row in &rows {
            assert!(
                display_width(&row.text) <= 48,
                "wide row too long: {:?}",
                row.text
            );
        }
        assert!(rows
            .iter()
            .any(|row| row.kind == RowKind::Add && row.text.contains('中')));
    }

    #[test]
    fn new_files_are_labelled() {
        let file = FilePreview::from_text("new.txt", "", "one\ntwo\n", true);
        let rows = layout(&ChangePreview::diff(vec![file]), 80);
        assert!(rows[0].text.contains("new.txt (new file)"));
        assert!(rows[0].text.contains("+2 -0"));
    }

    #[test]
    fn warnings_and_commands_render_as_notes() {
        let command = ChangePreview::command("cargo test\n--all");
        let rows = layout(&command, 80);
        assert!(rows[0].text.starts_with("$ cargo test"));
        assert!(rows[1].text.trim_start().starts_with("--all"));

        let warning =
            ChangePreview::note("old string not found").with_warning("sandbox is read-only");
        let rows = layout(&warning, 80);
        assert!(rows.iter().any(|row| row.kind == RowKind::Warning));
        assert!(rows.iter().any(|row| row.kind == RowKind::Note));
    }

    #[test]
    fn file_starts_track_each_file() {
        let first = FilePreview::from_text("a.rs", "", "a\n", true);
        let second = FilePreview::from_text("b.rs", "", "b\n", true);
        let rows = layout(&ChangePreview::diff(vec![first, second]), 80);
        let starts = file_starts(&rows);
        assert_eq!(starts.len(), 2);
        assert_eq!(rows[starts[0]].text.split_whitespace().nth(1), Some("a.rs"));
        assert_eq!(rows[starts[1]].text.split_whitespace().nth(1), Some("b.rs"));
    }

    #[test]
    fn preview_wraps_whole_emoji_and_combining_graphemes_under_the_gutter() {
        let source = "👩‍💻e\u{301}中".repeat(40);
        for width in [40, 41, 48, 79] {
            let rows = preview("", &source, width);
            let bodies: Vec<_> = rows
                .iter()
                .filter(|r| r.kind == RowKind::Add)
                .map(|r| {
                    r.text
                        .split_once("│ ")
                        .unwrap()
                        .1
                        .chars()
                        .skip(2)
                        .collect::<String>()
                })
                .collect();
            assert_eq!(bodies.concat(), source);
            assert!(rows.iter().all(|r| r.text.width() <= width));
            for body in bodies {
                assert!(body
                    .graphemes(true)
                    .all(|g| matches!(g, "👩‍💻" | "e\u{301}" | "中")));
            }
        }
        assert_eq!(fit_tail("prefix👩‍💻e\u{301}", 4), "…👩‍💻e\u{301}");
    }
}
