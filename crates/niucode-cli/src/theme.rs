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
        let trimmed = text.trim_start();
        if !code {
            if let Some(heading) = crate::markdown::heading(&text) {
                return vec![Span::styled(heading, style(Tone::Accent))].into();
            }
        }
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
        } else if trimmed.starts_with('#') || trimmed.starts_with("@@") || trimmed.starts_with('▎')
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
            let spans: Vec<_> = text
                .split_inclusive(|c: char| !c.is_alphanumeric() && c != '_')
                .map(|token| {
                    let word = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
                    let tone = if matches!(
                        word,
                        "fn" | "pub"
                            | "let"
                            | "mut"
                            | "impl"
                            | "use"
                            | "struct"
                            | "enum"
                            | "match"
                            | "if"
                            | "else"
                            | "return"
                            | "async"
                            | "await"
                            | "def"
                            | "class"
                            | "import"
                            | "from"
                            | "const"
                            | "function"
                            | "for"
                            | "while"
                    ) {
                        Tone::Magenta
                    } else if !word.is_empty() && word.chars().all(|c| c.is_ascii_digit()) {
                        Tone::Warning
                    } else {
                        Tone::Bright
                    };
                    Span::styled(token.to_owned(), style(tone))
                })
                .collect();
            return spans.into();
        }
        let spans: Vec<_> = crate::markdown::inline(&text)
            .into_iter()
            .map(|span| {
                let mut style = Style::default();
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
                Span::styled(span.text, style)
            })
            .collect();
        spans.into()
    }

    /// The same spans feed linear ANSI and ratatui, so highlighting cannot
    /// change width or leak terminal control characters between renderers.
    pub fn render_row(self, text: &str, code: bool) -> String {
        use ratatui::style::{Color, Modifier};
        let row = self.row(text, code);
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
