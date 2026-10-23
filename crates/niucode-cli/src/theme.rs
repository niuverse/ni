//! Small terminal theme used by the linear TUI and approval pager.
//!
//! Styling is intentionally dependency-free: ANSI SGR codes only, disabled for
//! non-TTY output, `NO_COLOR`, `CLICOLOR=0`, and `TERM=dumb`.

use std::fmt::Display;
use std::io::IsTerminal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    ansi: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Accent,
    Bright,
    Success,
    Danger,
    Warning,
    Muted,
    Magenta,
}

pub const PROMPT: &str = "▸";
pub const ASSISTANT: &str = "◆";
pub const THINKING: &str = "◇";
pub const TOOL: &str = "»";
pub const OK: &str = "✓";
pub const ERR: &str = "✗";
pub const WARN: &str = "!";
pub const COMPACT: &str = "⇣";
pub const TURN: &str = "╰─";
pub const BRANCH: &str = "│";

impl Theme {
    pub fn detect_stdout() -> Self {
        Self::from_tty(std::io::stdout().is_terminal())
    }

    pub fn detect_stderr() -> Self {
        Self::from_tty(std::io::stderr().is_terminal())
    }

    pub fn from_tty(tty: bool) -> Self {
        let no_color = std::env::var_os("NO_COLOR").is_some();
        let dumb = std::env::var("TERM").is_ok_and(|term| term == "dumb");
        let clicolor_off = std::env::var("CLICOLOR").is_ok_and(|value| value == "0");
        Self {
            ansi: tty && !no_color && !dumb && !clicolor_off,
        }
    }

    pub fn plain() -> Self {
        Self { ansi: false }
    }

    #[cfg(test)]
    fn ansi_forced() -> Self {
        Self { ansi: true }
    }

    pub fn is_ansi(self) -> bool {
        self.ansi
    }

    pub fn paint(self, tone: Tone, text: impl Display) -> String {
        if !self.ansi {
            return text.to_string();
        }
        let code = match tone {
            Tone::Accent => "1;36",
            Tone::Bright => "1;37",
            Tone::Success => "1;32",
            Tone::Danger => "1;31",
            Tone::Warning => "1;33",
            Tone::Muted => "2",
            Tone::Magenta => "1;35",
        };
        format!("\x1b[{code}m{text}\x1b[0m")
    }

    /// Build spans only after sanitization; model-provided ANSI is never trusted.
    pub fn row(self, text: &str, code: bool) -> ratatui::text::Line<'static> {
        use ratatui::{
            style::{Color, Modifier, Style},
            text::Span,
        };
        let text = crate::text_layout::clean_text(text);
        let style = |tone: Tone| {
            if !self.ansi {
                return Style::default();
            }
            match tone {
                Tone::Accent => Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
                Tone::Bright => Style::default().add_modifier(Modifier::BOLD),
                Tone::Success => Style::default().fg(Color::Green),
                Tone::Danger => Style::default().fg(Color::Red),
                Tone::Warning => Style::default().fg(Color::Yellow),
                Tone::Muted => Style::default().add_modifier(Modifier::DIM),
                Tone::Magenta => Style::default().fg(Color::Magenta),
            }
        };
        if !code {
            if let Some((level, heading)) = crate::markdown::heading_with_level(&text) {
                let mut heading_style = style(Tone::Accent);
                if self.ansi {
                    if level == 1 {
                        heading_style = heading_style.add_modifier(Modifier::UNDERLINED);
                    } else if level >= 3 {
                        heading_style = heading_style
                            .fg(Color::Cyan)
                            .remove_modifier(Modifier::BOLD);
                    }
                }
                return vec![Span::styled(heading, heading_style)].into();
            }
        }
        let (text, block_style) = if code {
            (text, None)
        } else if let Some(block) = crate::markdown::block(&text) {
            let tone = match block.kind {
                crate::markdown::BlockKind::Quote => Some(Tone::Muted),
                crate::markdown::BlockKind::Task(true) => Some(Tone::Success),
                crate::markdown::BlockKind::Rule => Some(Tone::Muted),
                _ => None,
            };
            (block.text, tone)
        } else {
            (text, None)
        };
        let trimmed = text.trim_start();
        let tone = if trimmed.contains("│ - ")
            || trimmed.starts_with("✗")
            || trimmed.starts_with('-') && !trimmed.starts_with("- ")
        {
            Some(Tone::Danger)
        } else if trimmed.contains("│ + ") || trimmed.starts_with("✓") || trimmed.starts_with('+')
        {
            Some(Tone::Success)
        } else if trimmed.starts_with('!') {
            Some(Tone::Warning)
        } else if trimmed.starts_with('#')
            || trimmed.starts_with("@@")
            || trimmed.starts_with('▎')
            || trimmed.starts_with('→')
            || trimmed.starts_with('─')
        {
            Some(Tone::Accent)
        } else if trimmed.starts_with("```") {
            Some(Tone::Muted)
        } else {
            None
        };
        if let Some(tone) = tone {
            return vec![Span::styled(text, style(tone))].into();
        }
        if let Some((prefix, rest)) = text.split_once('▸') {
            return vec![
                Span::styled(format!("{prefix}▸"), style(Tone::Accent)),
                Span::raw(rest.to_owned()),
            ]
            .into();
        }
        if code {
            return self.code_tokens(&crate::syntax::highlight(&text));
        }
        let base = block_style.map_or(Style::default(), style);
        let spans: Vec<_> = crate::markdown::inline(&text)
            .into_iter()
            .map(|span| {
                let mut style = base;
                if !self.ansi {
                    return Span::styled(span.text, style);
                }
                if span.style.bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if span.style.italic {
                    style = style.add_modifier(Modifier::ITALIC);
                }
                if span.style.strike {
                    style = style.add_modifier(Modifier::CROSSED_OUT);
                }
                if span.style.code {
                    style = style.fg(Color::Cyan);
                }
                if span.style.link {
                    style = style.fg(Color::Cyan).add_modifier(Modifier::UNDERLINED);
                }
                if span.style.math {
                    style = style.fg(Color::Magenta).add_modifier(Modifier::ITALIC);
                }
                Span::styled(span.text, style)
            })
            .collect();
        spans.into()
    }

    pub fn code_tokens(self, tokens: &[crate::syntax::Token]) -> ratatui::text::Line<'static> {
        use ratatui::{
            style::{Color, Modifier, Style},
            text::Span,
        };
        let spans: Vec<_> = tokens
            .iter()
            .map(|token| {
                let token_style = if !self.ansi {
                    Style::default()
                } else {
                    match token.kind {
                        crate::syntax::TokenKind::Keyword => Style::default().fg(Color::Magenta),
                        crate::syntax::TokenKind::String => Style::default().fg(Color::Green),
                        crate::syntax::TokenKind::Comment => {
                            Style::default().add_modifier(Modifier::DIM)
                        }
                        crate::syntax::TokenKind::Number => Style::default().fg(Color::Yellow),
                        crate::syntax::TokenKind::Function => Style::default().fg(Color::Cyan),
                        crate::syntax::TokenKind::Plain => Style::default(),
                    }
                };
                Span::styled(token.text.clone(), token_style)
            })
            .collect();
        spans.into()
    }

    /// The same spans feed linear ANSI and ratatui, so highlighting cannot
    /// change width or leak terminal control characters between renderers.
    pub fn table_row(
        self,
        text: &str,
        kind: crate::table::TableLineKind,
        styles: &[crate::table::StyledRange],
    ) -> ratatui::text::Line<'static> {
        use ratatui::{
            style::{Color, Modifier, Style},
            text::Span,
        };
        let mut spans = Vec::new();
        if self.ansi && !styles.is_empty() {
            let mut cursor = 0;
            for range in styles {
                if range.start > cursor {
                    spans.push(Span::raw(text[cursor..range.start].to_string()));
                }
                let mut style = Style::default();
                if range.style.bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if range.style.italic {
                    style = style.add_modifier(Modifier::ITALIC);
                }
                if range.style.code {
                    style = style.fg(Color::Cyan);
                }
                if range.style.link {
                    style = style.fg(Color::Cyan).add_modifier(Modifier::UNDERLINED);
                }
                if range.style.strike {
                    style = style.add_modifier(Modifier::CROSSED_OUT);
                }
                if range.style.math {
                    style = style.fg(Color::Magenta).add_modifier(Modifier::ITALIC);
                }
                spans.push(Span::styled(
                    text[range.start..range.end].to_string(),
                    style,
                ));
                cursor = range.end;
            }
            if cursor < text.len() {
                spans.push(Span::raw(text[cursor..].to_string()));
            }
        } else {
            spans.push(Span::raw(text.to_string()));
        }
        let modifier = if !self.ansi {
            None
        } else {
            match kind {
                crate::table::TableLineKind::Header => Some(Modifier::BOLD),
                crate::table::TableLineKind::Border => Some(Modifier::DIM),
                crate::table::TableLineKind::Row => None,
            }
        };
        if let Some(modifier) = modifier {
            for span in &mut spans {
                span.style = span.style.add_modifier(modifier);
            }
        }
        ratatui::text::Line::from(spans)
    }

    pub fn render_table_row(
        self,
        text: &str,
        kind: crate::table::TableLineKind,
        styles: &[crate::table::StyledRange],
    ) -> String {
        if !self.ansi {
            return text.to_string();
        }
        let mut body = String::new();
        let mut cursor = 0;
        for range in styles {
            if range.start > cursor {
                body.push_str(&text[cursor..range.start]);
            }
            let mut attrs = Vec::new();
            if range.style.bold {
                attrs.push("1");
            }
            if range.style.italic {
                attrs.push("3");
            }
            if range.style.code {
                attrs.push("36");
            }
            if range.style.link {
                attrs.push("4");
            }
            if range.style.strike {
                attrs.push("9");
            }
            if range.style.math {
                attrs.push("35");
                attrs.push("3");
            }
            if attrs.is_empty() {
                body.push_str(&text[range.start..range.end]);
            } else {
                body.push_str(&format!(
                    "\x1b[{}m{}\x1b[0m",
                    attrs.join(";"),
                    &text[range.start..range.end]
                ));
            }
            cursor = range.end;
        }
        body.push_str(&text[cursor..]);
        match kind {
            crate::table::TableLineKind::Header => format!("\x1b[1m{body}\x1b[0m"),
            crate::table::TableLineKind::Border => format!("\x1b[2m{body}\x1b[0m"),
            crate::table::TableLineKind::Row => body,
        }
    }

    pub fn render_row(self, text: &str, code: bool) -> String {
        let row = self.row(text, code);
        self.render_spans(row)
    }

    pub fn render_code_tokens(self, tokens: &[crate::syntax::Token]) -> String {
        let row = self.code_tokens(tokens);
        self.render_spans(row)
    }

    fn render_spans(self, row: ratatui::text::Line<'static>) -> String {
        use ratatui::style::{Color, Modifier};
        let mut output = String::new();
        for span in row.spans {
            if self.ansi {
                let mut attrs: Vec<&str> = Vec::new();
                match span.style.fg {
                    Some(Color::Cyan) => attrs.push("36"),
                    Some(Color::Green) => attrs.push("32"),
                    Some(Color::Red) => attrs.push("31"),
                    Some(Color::Yellow) => attrs.push("33"),
                    Some(Color::Magenta) => attrs.push("35"),
                    _ => {}
                }
                if span.style.add_modifier.contains(Modifier::BOLD) {
                    attrs.push("1");
                }
                if span.style.add_modifier.contains(Modifier::DIM) {
                    attrs.push("2");
                }
                if span.style.add_modifier.contains(Modifier::ITALIC) {
                    attrs.push("3");
                }
                if span.style.add_modifier.contains(Modifier::UNDERLINED) {
                    attrs.push("4");
                }
                if span.style.add_modifier.contains(Modifier::CROSSED_OUT) {
                    attrs.push("9");
                }
                if attrs.is_empty() {
                    output.push_str(&span.content);
                } else {
                    output.push_str(&format!("\x1b[{}m{}\x1b[0m", attrs.join(";"), span.content));
                }
            } else {
                output.push_str(&span.content);
            }
        }
        output
    }

    pub fn dim(self, text: impl Display) -> String {
        self.paint(Tone::Muted, text)
    }

    pub fn accent(self, text: impl Display) -> String {
        self.paint(Tone::Accent, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_theme_emits_no_ansi() {
        let theme = Theme { ansi: false };
        assert_eq!(theme.accent("niucode"), "niucode");
        assert_eq!(theme.dim("meta"), "meta");
    }

    #[test]
    fn ansi_theme_wraps_and_resets() {
        let theme = Theme::ansi_forced();
        let painted = theme.paint(Tone::Success, "ok");
        assert!(painted.starts_with("\u{1b}[1;32m"));
        assert!(painted.ends_with("\u{1b}[0m"));
        assert!(painted.contains("ok"));
    }

    #[test]
    fn non_tty_disables_ansi() {
        assert!(!Theme::from_tty(false).is_ansi());
    }

    #[test]
    fn symbols_are_single_line() {
        for symbol in [PROMPT, ASSISTANT, THINKING, TOOL, OK, ERR, WARN, COMPACT] {
            assert!(!symbol.contains('\n'));
            assert!(!symbol.is_empty());
        }
    }
    #[test]
    fn table_header_and_border_get_modifiers() {
        use ratatui::style::Modifier;
        let theme = Theme::ansi_forced();
        let header = theme.table_row("│ name │", crate::table::TableLineKind::Header, &[]);
        assert!(header
            .spans
            .iter()
            .all(|span| span.style.add_modifier.contains(Modifier::BOLD)));
        let border = theme.table_row("┌────┐", crate::table::TableLineKind::Border, &[]);
        assert!(border
            .spans
            .iter()
            .all(|span| span.style.add_modifier == Modifier::DIM));
        assert!(Theme::plain()
            .table_row("│ name │", crate::table::TableLineKind::Header, &[])
            .spans
            .iter()
            .all(|span| span.style.add_modifier.is_empty()));
    }

    #[test]
    fn table_cell_inline_styles_are_applied() {
        use ratatui::style::Modifier;
        let lines =
            crate::table::render(&["| head | value |".into(), "| **bold** | 1 |".into()], 80);
        let row = Theme::ansi_forced().table_row(&lines[3].text, lines[3].kind, &lines[3].styles);
        assert!(row
            .spans
            .iter()
            .any(|span| span.style.add_modifier.contains(Modifier::BOLD)));
        assert!(row
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
            .contains("bold"));
    }

    #[test]
    fn heading_levels_get_distinct_emphasis() {
        use ratatui::style::Modifier;
        let theme = Theme::ansi_forced();
        let h1 = theme.row("# One", false);
        assert!(h1.spans[0]
            .style
            .add_modifier
            .contains(Modifier::UNDERLINED));
        let h3 = theme.row("### Three", false);
        assert!(!h3.spans[0].style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(h1.spans[0].content, "One");
        assert_eq!(h3.spans[0].content, "Three");
    }

    #[test]
    fn code_rows_classify_keywords_strings_and_comments() {
        use ratatui::style::{Color, Modifier};
        let row = Theme::ansi_forced().row("let x = \"hi\"; // note", true);
        let text: String = row.spans.iter().map(|span| span.content.as_ref()).collect();
        assert_eq!(text, "let x = \"hi\"; // note");
        assert!(row
            .spans
            .iter()
            .any(|span| span.style.fg == Some(Color::Magenta)));
        assert!(row
            .spans
            .iter()
            .any(|span| span.style.fg == Some(Color::Green)));
        assert!(row
            .spans
            .iter()
            .any(|span| span.style.add_modifier.contains(Modifier::DIM)));
        assert!(Theme::plain()
            .row("let x = 1;", true)
            .spans
            .iter()
            .all(|span| span.style == ratatui::style::Style::default()));
    }

    #[test]
    fn highlighted_code_and_markdown_preserve_text_and_sanitize_controls() {
        let theme = Theme::ansi_forced();
        for (text, code, expected) in [
            ("# Heading", false, "Heading"),
            ("Use `cargo test` now", false, "Use cargo test now"),
            (
                "fn main() { let x = 42; }",
                true,
                "fn main() { let x = 42; }",
            ),
            ("+ new line", false, "+ new line"),
        ] {
            let row = theme.row(text, code);
            assert_eq!(
                row.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>(),
                expected
            );
            assert_eq!(Theme::plain().render_row(text, code), expected);
            assert!(theme.render_row(text, code).contains("\x1b["));
        }
        assert_eq!(
            theme.row("fn main()", true).spans[0].style.fg,
            Some(ratatui::style::Color::Magenta)
        );
        let malicious = "\x1b]52;c;data\x07";
        assert!(!theme.render_row(malicious, false).contains("\x1b]52"));
        assert!(!Theme::plain().render_row(malicious, false).contains('\x1b'));
    }
}
