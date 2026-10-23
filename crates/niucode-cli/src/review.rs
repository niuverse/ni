//! Full-screen approval pager for structured diffs.
//!
//! This is an overlay for approval only; the session itself stays linear. The
//! pager owns no session state and disappears after a decision.

use std::io::{self, Write};

use crate::text_layout::{clean_text, clip};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::queue;
use crossterm::style::{Attribute, Print, ResetColor, SetAttribute};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use niucode_agent::ApprovalRequest;
use niucode_tools::ChangePreview;
use unicode_width::UnicodeWidthStr;

use crate::diff::{self, file_starts, Row, RowKind};
use crate::theme::{Theme, Tone};
use crate::ApprovalKeys;

const MIN_WIDTH: usize = 40;
const MIN_HEIGHT: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalChoice {
    Allow,
    Deny,
    Session,
}

/// Map a key press to an approval choice using the configured keys.
pub(crate) fn choice_for_key(code: KeyCode, keys: ApprovalKeys) -> Option<ApprovalChoice> {
    match code {
        KeyCode::Char(character) if character.eq_ignore_ascii_case(&keys.allow) => {
            Some(ApprovalChoice::Allow)
        }
        KeyCode::Char(character) if character.eq_ignore_ascii_case(&keys.deny) => {
            Some(ApprovalChoice::Deny)
        }
        KeyCode::Char(character) if character.eq_ignore_ascii_case(&keys.session) => {
            Some(ApprovalChoice::Session)
        }
        _ => None,
    }
}

/// Show a width-aware diff pager and return the user's decision.
///
/// `Ok(None)` means the terminal is too small, so the caller should fall back
/// to a plain preview and the line-based prompt.
pub fn review_diff(
    request: &ApprovalRequest,
    preview: &ChangePreview,
    theme: Theme,
    keys: ApprovalKeys,
) -> io::Result<Option<ApprovalChoice>> {
    let (columns, rows) = terminal::size()?;
    let width = columns.saturating_sub(1) as usize;
    let height = rows as usize;
    if width < MIN_WIDTH || height < MIN_HEIGHT {
        return Ok(None);
    }

    let _guard = ScreenGuard::enter()?;
    let mut state = ReviewState::new(request, preview, width, height, theme, keys);
    let mut out = io::stdout();

    loop {
        state.draw(&mut out)?;
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if let Some(choice) = choice_for_key(key.code, keys) {
                    return Ok(Some(choice));
                }
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                        return Ok(Some(ApprovalChoice::Deny));
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(Some(ApprovalChoice::Deny));
                    }
                    KeyCode::Char('j') | KeyCode::Down => state.scroll_by(1),
                    KeyCode::Char('k') | KeyCode::Up => state.scroll_by(-1),
                    KeyCode::PageDown | KeyCode::Char(' ') => {
                        state.scroll_by(state.body_height() as isize)
                    }
                    KeyCode::PageUp | KeyCode::Char('b') => {
                        state.scroll_by(-(state.body_height() as isize))
                    }
                    KeyCode::Char('g') | KeyCode::Home => state.scroll = 0,
                    KeyCode::Char('G') | KeyCode::End => state.scroll = state.max_scroll(),
                    KeyCode::Char('n') | KeyCode::Char(']') => state.jump_file(1),
                    KeyCode::Char('p') | KeyCode::Char('[') => state.jump_file(-1),
                    _ => {}
                }
            }
            Event::Resize(columns, rows) => {
                let width = columns.saturating_sub(1) as usize;
                let height = rows as usize;
                if width < MIN_WIDTH || height < MIN_HEIGHT {
                    return Ok(None);
                }
                state.relayout(width, height);
            }
            _ => {}
        }
    }
}

struct ReviewState<'a> {
    request: &'a ApprovalRequest,
    preview: &'a ChangePreview,
    rows: Vec<Row>,
    file_starts: Vec<usize>,
    width: usize,
    height: usize,
    scroll: usize,
    theme: Theme,
    keys: ApprovalKeys,
}

impl<'a> ReviewState<'a> {
    fn new(
        request: &'a ApprovalRequest,
        preview: &'a ChangePreview,
        width: usize,
        height: usize,
        theme: Theme,
        keys: ApprovalKeys,
    ) -> Self {
        let rows = diff::layout(preview, width);
        let file_starts = file_starts(&rows);
        Self {
            request,
            preview,
            rows,
            file_starts,
            width,
            height,
            scroll: 0,
            theme,
            keys,
        }
    }

    fn body_height(&self) -> usize {
        self.height.saturating_sub(2)
    }

    fn max_scroll(&self) -> usize {
        self.rows.len().saturating_sub(self.body_height())
    }

    fn clamp(&mut self) {
        self.scroll = self.scroll.min(self.max_scroll());
    }

    fn scroll_by(&mut self, delta: isize) {
        let target = self.scroll as isize + delta;
        self.scroll = target.clamp(0, self.max_scroll() as isize) as usize;
    }

    fn current_file(&self) -> Option<usize> {
        self.file_starts
            .iter()
            .rposition(|start| *start <= self.scroll)
    }

    fn jump_file(&mut self, delta: isize) {
        if self.file_starts.is_empty() {
            return;
        }
        let next = match self.current_file() {
            Some(current) => {
                (current as isize + delta).rem_euclid(self.file_starts.len() as isize) as usize
            }
            None if delta >= 0 => 0,
            None => self.file_starts.len() - 1,
        };
        self.scroll = self.file_starts[next];
    }

    fn relayout(&mut self, width: usize, height: usize) {
        let anchor = self.scroll;
        self.width = width;
        self.height = height;
        self.rows = diff::layout(self.preview, width);
        self.file_starts = file_starts(&self.rows);
        self.scroll = anchor.min(self.max_scroll());
    }

    fn draw(&mut self, out: &mut impl Write) -> io::Result<()> {
        self.clamp();
        let width = self.width;

        queue!(out, MoveTo(0, 0), Clear(ClearType::CurrentLine))?;
        if self.theme.is_ansi() {
            queue!(out, SetAttribute(Attribute::Reverse))?;
        }
        let title = format!(
            " approval · {} · +{} -{} ",
            clean_text(&self.request.summary),
            self.preview.added,
            self.preview.removed
        );
        write!(out, "{}", pad_to(&title, width))?;
        if self.theme.is_ansi() {
            queue!(out, SetAttribute(Attribute::Reset))?;
        }

        for offset in 0..self.body_height() {
            queue!(
                out,
                MoveTo(0, (offset + 1) as u16),
                Clear(ClearType::CurrentLine)
            )?;
            if let Some(row) = self.rows.get(self.scroll + offset) {
                print_row(out, row, self.theme)?;
            }
        }

        queue!(
            out,
            MoveTo(0, (self.height - 1) as u16),
            Clear(ClearType::CurrentLine)
        )?;
        if self.theme.is_ansi() {
            queue!(out, SetAttribute(Attribute::Reverse))?;
        }
        write!(out, "{}", pad_to(&self.footer(), width))?;
        if self.theme.is_ansi() {
            queue!(out, SetAttribute(Attribute::Reset))?;
        }
        out.flush()
    }

    fn footer(&self) -> String {
        let file_position = if self.file_starts.is_empty() {
            "no files".to_string()
        } else {
            match self.current_file() {
                Some(current) => format!("file {}/{}", current + 1, self.file_starts.len()),
                None => format!("file -/{}", self.file_starts.len()),
            }
        };
        let visible_end = (self.scroll + self.body_height()).min(self.rows.len());
        let range = if self.rows.is_empty() {
            "0/0".to_string()
        } else {
            format!("{}-{}/{}", self.scroll + 1, visible_end, self.rows.len())
        };
        let footer_keys = if self.width >= 90 {
            format!(
                "{} allow · {} deny · {} session · j/k scroll · n/p file",
                self.keys.allow, self.keys.deny, self.keys.session
            )
        } else if self.width >= 60 {
            format!(
                "{} allow · {} deny · {} session · j/k",
                self.keys.allow, self.keys.deny, self.keys.session
            )
        } else {
            format!("{} allow · {} deny", self.keys.allow, self.keys.deny)
        };
        format!(" {file_position} · {range}   {footer_keys} ")
    }
}

fn print_row(out: &mut impl Write, row: &Row, theme: Theme) -> io::Result<()> {
    let tone = match row.kind {
        RowKind::File => Some(Tone::Accent),
        RowKind::Hunk => Some(Tone::Magenta),
        RowKind::Add => Some(Tone::Success),
        RowKind::Remove => Some(Tone::Danger),
        RowKind::Warning | RowKind::Note => Some(Tone::Warning),
        RowKind::Context => None,
    };
    match tone {
        Some(tone) => queue!(out, Print(theme.paint(tone, &row.text)))?,
        None => queue!(out, Print(&row.text))?,
    }
    Ok(())
}

fn pad_to(text: &str, width: usize) -> String {
    let mut padded = clip(text, width);
    padded.push_str(&" ".repeat(width.saturating_sub(padded.width())));
    padded
}

struct ScreenGuard;

impl ScreenGuard {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let mut out = io::stdout();
        if let Err(error) = queue!(
            out,
            EnterAlternateScreen,
            Hide,
            MoveTo(0, 0),
            Clear(ClearType::All)
        )
        .and_then(|()| out.flush())
        {
            let _ = terminal::disable_raw_mode();
            return Err(error);
        }
        Ok(Self)
    }
}

impl Drop for ScreenGuard {
    fn drop(&mut self) {
        let mut out = io::stdout();
        let _ = queue!(out, LeaveAlternateScreen, Show, ResetColor);
        let _ = out.flush();
        let _ = terminal::disable_raw_mode();
    }
}

#[cfg(test)]
mod tests {
    use niucode_tools::FilePreview;

    use super::*;

    fn request(preview: &ChangePreview) -> ApprovalRequest {
        ApprovalRequest {
            tool: "write".to_string(),
            summary: "write a.rs".to_string(),
            preview: Some(preview.clone()),
            image: None,
        }
    }

    #[test]
    fn file_jump_starts_at_the_first_file_after_warnings() {
        let file = FilePreview::from_text("a.rs", "", "a\n", true);
        let preview = ChangePreview::diff(vec![file]).with_warning("read first");
        let request = request(&preview);
        let mut state = ReviewState::new(
            &request,
            &preview,
            80,
            10,
            Theme::plain(),
            ApprovalKeys::default(),
        );

        assert_eq!(state.current_file(), None);
        state.jump_file(1);
        assert_eq!(state.current_file(), Some(0));
        assert_eq!(state.scroll, 1);
    }

    #[test]
    fn file_jump_wraps_between_files() {
        let first = FilePreview::from_text("a.rs", "", "a\n", true);
        let second = FilePreview::from_text("b.rs", "", "b\n", true);
        let preview = ChangePreview::diff(vec![first, second]);
        let request = request(&preview);
        let mut state = ReviewState::new(
            &request,
            &preview,
            80,
            10,
            Theme::plain(),
            ApprovalKeys::default(),
        );

        assert_eq!(state.current_file(), Some(0));
        state.jump_file(1);
        assert_eq!(state.current_file(), Some(1));
        state.jump_file(1);
        assert_eq!(state.current_file(), Some(0));
        state.jump_file(-1);
        assert_eq!(state.current_file(), Some(1));
    }

    #[test]
    fn custom_keys_are_matched_and_shown_in_the_footer() {
        let keys = ApprovalKeys::custom('y', 'n', 's').unwrap();
        assert_eq!(
            choice_for_key(KeyCode::Char('y'), keys),
            Some(ApprovalChoice::Allow)
        );
        assert_eq!(
            choice_for_key(KeyCode::Char('N'), keys),
            Some(ApprovalChoice::Deny)
        );
        assert_eq!(
            choice_for_key(KeyCode::Char('s'), keys),
            Some(ApprovalChoice::Session)
        );
        assert_eq!(choice_for_key(KeyCode::Char('a'), keys), None);
        assert!(ApprovalKeys::custom('a', 'A', 's').is_none());

        let file = FilePreview::from_text("a.rs", "", "a\n", true);
        let preview = ChangePreview::diff(vec![file]);
        let request = request(&preview);
        let state = ReviewState::new(&request, &preview, 90, 10, Theme::plain(), keys);
        let footer = state.footer();
        assert!(footer.contains("y allow"));
        assert!(footer.contains("n deny"));
        assert!(footer.contains("s session"));
    }
}
