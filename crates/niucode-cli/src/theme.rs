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

    #[cfg(test)]
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
}
