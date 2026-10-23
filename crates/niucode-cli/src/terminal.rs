//! Owns terminal modes, the live region and transcript writes. No session state.
use crate::text_layout::{clean_multiline, clean_text, clip, wrap};
use crate::theme::Theme;
use crossterm::{
    cursor::{Hide, MoveDown, MoveToColumn, MoveUp, SetCursorStyle, Show},
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event,
    },
    execute, queue,
    terminal::{self, Clear, ClearType},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    style::{Modifier, Style},
    text::Line,
    widgets::{Paragraph, Widget},
    Terminal, TerminalOptions, Viewport,
};
use std::io::{self, stdout, Write};
use std::time::Duration;
use unicode_width::UnicodeWidthStr;

type Inline = Terminal<CrosstermBackend<io::Stdout>>;

#[derive(PartialEq, Eq)]
struct FrameState {
    size: (usize, usize),
    lines: Vec<String>,
    cursor: Option<(usize, usize)>,
    selected: Option<usize>,
    mouse: bool,
}
pub struct TerminalSession {
    last_frame: Option<FrameState>,
    origin: Option<u16>,
    inline: Option<Inline>,
    inline_height: u16,
    cursor_row: u16,
    drawn: bool,
    raw_before: bool,
    mouse: bool,
    ansi: bool,
    code_block: bool,
    table: Vec<String>,
    math: Option<Vec<String>>,
    pub notice: Option<String>,
}

impl TerminalSession {
    pub fn enter(inline: bool, ansi: bool) -> io::Result<Self> {
        let raw_before = terminal::is_raw_mode_enabled()?;
        terminal::enable_raw_mode()?;
        let mut session = Self {
            last_frame: None,
            origin: None,
            inline: None,
            inline_height: 0,
            cursor_row: 0,
            drawn: false,
            raw_before,
            mouse: false,
            ansi,
            code_block: false,
            table: Vec::new(),
            math: None,
            notice: None,
        };
        execute!(
            stdout(),
            EnableBracketedPaste,
            SetCursorStyle::SteadyBar,
            Show
        )?;
        if inline {
            match make_inline(3) {
                Ok(t) => {
                    session.inline = Some(t);
                    session.inline_height = 3;
                }
                Err(error) => {
                    session.notice = Some(format!(
                        "cursor capability unavailable; linear presentation: {error}"
                    ))
                }
            }
        }
        Ok(session)
    }
    pub fn size(&self) -> (usize, usize) {
        let (width, height) = terminal::size().unwrap_or((80, 24));
        (
            usize::from(width).saturating_sub(1).max(1),
            usize::from(height).max(1),
        )
    }
    pub fn poll(&mut self, timeout: Duration) -> io::Result<Option<Event>> {
        if event::poll(timeout)? {
            event::read().map(Some)
        } else {
            Ok(None)
        }
    }
    pub fn clear(&mut self) -> io::Result<()> {
        self.last_frame = None;
        self.set_mouse(false)?;
        if let Some(t) = &mut self.inline {
            clear_inline(t)?;
        } else if self.drawn {
            let mut out = stdout();
            queue!(out, Hide, MoveToColumn(0))?;
            if self.cursor_row > 0 {
                queue!(out, MoveUp(self.cursor_row))?;
            }
            queue!(out, Clear(ClearType::FromCursorDown))?;
            out.flush()?;
        }
        self.cursor_row = 0;
        self.drawn = false;
        Ok(())
    }
    pub fn clear_screen(&mut self) -> io::Result<()> {
        self.clear()?;
        execute!(
            stdout(),
            Clear(ClearType::All),
            crossterm::cursor::MoveTo(0, 0)
        )?;
        if self.inline.is_some() {
            self.inline = None;
            match make_inline(self.inline_height) {
                Ok(terminal) => self.inline = Some(terminal),
                Err(error) => {
                    self.notice = Some(format!(
                        "linear presentation after cursor query failure: {error}"
                    ))
                }
            }
        }
        Ok(())
    }
    /// Text is untrusted. This is the only path for runtime transcript output.
    pub fn print(&mut self, text: &str) -> io::Result<()> {
        self.last_frame = None;
        if self.inline.is_none() {
            self.clear()?;
        }
        for line in clean_multiline(text).split('\n') {
            let trimmed = line.trim_start();
            if !self.code_block && line.trim() == "$$" {
                self.flush_table()?;
                if let Some(rows) = self.math.take() {
                    self.emit_math(&rows)?;
                } else {
                    self.math = Some(Vec::new());
                }
                continue;
            }
            if let Some(rows) = &mut self.math {
                rows.push(line.to_string());
                continue;
            }
            if trimmed.starts_with("```") {
                self.flush_table()?;
                if self.code_block {
                    self.code_block = false;
                } else {
                    let language = trimmed.trim_start_matches("```").trim();
                    let label = if language.is_empty() {
                        "code"
                    } else {
                        language
                    };
                    self.emit_line(&format!("  ▎ {label}"))?;
                    self.code_block = true;
                }
                continue;
            }
            if !self.code_block && crate::table::is_row(line) {
                self.table.push(line.to_string());
                continue;
            }
            self.flush_table()?;
            self.emit_line(line)?;
        }
        stdout().flush()?;
        Ok(())
    }

    /// Render a buffered display-math block, if any.
    pub fn flush_math(&mut self) -> io::Result<()> {
        if let Some(rows) = self.math.take() {
            self.emit_math(&rows)?;
        }
        Ok(())
    }

    fn emit_math(&mut self, rows: &[String]) -> io::Result<()> {
        for row in rows {
            let fallback = crate::markdown::latex_fallback(row.trim());
            if fallback.is_empty() {
                continue;
            }
            self.emit_line(&format!("  ƒ {fallback}"))?;
        }
        stdout().flush()?;
        Ok(())
    }

    /// Render a buffered Markdown table block, if any.
    pub fn flush_table(&mut self) -> io::Result<()> {
        if self.table.is_empty() {
            return Ok(());
        }
        let width = self.size().0;
        let rows = std::mem::take(&mut self.table);
        for line in crate::table::render(&rows, width) {
            self.emit_table_line(&line.text, line.kind)?;
        }
        stdout().flush()?;
        Ok(())
    }

    fn emit_table_line(&mut self, line: &str, kind: crate::table::TableLineKind) -> io::Result<()> {
        let theme = if self.ansi {
            Theme::detect_stdout()
        } else {
            Theme::plain()
        };
        if let Some(t) = &mut self.inline {
            let width = t.size()?.width.saturating_sub(1).max(1) as usize;
            for row in wrap(line, width) {
                let row = theme.table_row(&row, kind);
                t.insert_before(1, |buf| {
                    Paragraph::new(row).render(buf.area, buf);
                    collapse_wide_continuations(buf);
                })?;
            }
        } else {
            write!(stdout(), "{}\r\n", theme.render_table_row(line, kind))?;
        }
        Ok(())
    }

    fn emit_line(&mut self, line: &str) -> io::Result<()> {
        let theme = if self.ansi {
            Theme::detect_stdout()
        } else {
            Theme::plain()
        };
        if let Some(t) = &mut self.inline {
            let width = t.size()?.width.saturating_sub(1).max(1) as usize;
            for row in wrap(line, width) {
                let row = theme.row(&row, self.code_block);
                t.insert_before(1, |buf| {
                    Paragraph::new(row).render(buf.area, buf);
                    collapse_wide_continuations(buf);
                })?;
            }
        } else {
            write!(stdout(), "{}\r\n", theme.render_row(line, self.code_block))?;
        }
        Ok(())
    }
    /// Returns the absolute top row when available, for honest mouse hit tests.
    pub fn draw(
        &mut self,
        lines: &[String],
        cursor: Option<(usize, usize)>,
        selected: Option<usize>,
        mouse: bool,
    ) -> io::Result<Option<u16>> {
        let (width, height) = self.size();
        let count = lines.len().min(height).max(1);
        let visible_cursor = cursor.is_some();
        let cursor = cursor.unwrap_or((selected.unwrap_or(0), 0));
        let cursor = (
            cursor.0.min(count - 1),
            cursor.1.min(width.saturating_sub(1)),
        );
        if self.inline.is_some() && self.inline_height != count as u16 {
            self.clear()?;
            self.inline = None;
            match make_inline(count as u16) {
                Ok(t) => {
                    self.inline = Some(t);
                    self.inline_height = count as u16;
                }
                Err(e) => {
                    self.notice = Some(format!(
                        "linear presentation after cursor query failure: {e}"
                    ))
                }
            }
        }
        let frame = FrameState {
            size: (width, height),
            lines: lines.to_vec(),
            cursor: visible_cursor.then_some(cursor),
            selected,
            mouse,
        };
        if self.last_frame.as_ref() == Some(&frame) {
            return Ok(self.origin);
        }
        let ansi = self.ansi;
        let result = if let Some(t) = &mut self.inline {
            let mut top = 0;
            t.draw(|f| {
                let area = f.area();
                top = area.y;
                let rows: Vec<Line> = lines
                    .iter()
                    .take(count)
                    .enumerate()
                    .map(|(i, line)| {
                        let style = if ansi && selected == Some(i) {
                            Style::default().add_modifier(Modifier::REVERSED)
                        } else {
                            Style::default()
                        };
                        let theme = if ansi {
                            Theme::detect_stdout()
                        } else {
                            Theme::plain()
                        };
                        theme
                            .row(&clip(&clean_text(line), width), false)
                            .style(style)
                    })
                    .collect();
                f.render_widget(Paragraph::new(rows), area);
                collapse_wide_continuations(f.buffer_mut());
                if visible_cursor {
                    f.set_cursor_position((area.x + cursor.1 as u16, area.y + cursor.0 as u16));
                }
            })?;
            Some(top)
        } else {
            self.clear()?;
            let mut out = stdout();
            queue!(out, Hide)?;
            for (i, line) in lines.iter().take(count).enumerate() {
                if i > 0 {
                    write!(out, "\r\n")?;
                }
                if ansi && selected == Some(i) {
                    write!(out, "\x1b[7m{}\x1b[0m", clip(&clean_text(line), width))?;
                } else {
                    let theme = if ansi {
                        Theme::detect_stdout()
                    } else {
                        Theme::plain()
                    };
                    write!(
                        out,
                        "{}",
                        theme.render_row(&clip(&clean_text(line), width), false)
                    )?;
                }
            }
            if count - 1 > cursor.0 {
                queue!(out, MoveUp((count - 1 - cursor.0) as u16))?;
            }
            queue!(out, MoveToColumn(cursor.1 as u16))?;
            if visible_cursor {
                queue!(out, Show)?;
            }
            out.flush()?;
            None
        };
        self.cursor_row = cursor.0 as u16;
        self.drawn = true;
        // Without an absolute origin, interpreting pointer rows would guess.
        self.set_mouse(mouse && result.is_some())?;
        self.last_frame = Some(frame);
        self.origin = result;
        Ok(result)
    }
    fn set_mouse(&mut self, wanted: bool) -> io::Result<()> {
        if self.mouse != wanted {
            if wanted {
                execute!(stdout(), EnableMouseCapture)?;
            } else {
                execute!(stdout(), DisableMouseCapture)?;
            }
            self.mouse = wanted;
        }
        Ok(())
    }
    pub fn finish(mut self) -> io::Result<()> {
        self.clear()?;
        execute!(stdout(), Show)?;
        if !self.raw_before {
            terminal::disable_raw_mode()?;
        }
        Ok(())
    }
}
impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.clear();
        let _ = execute!(
            stdout(),
            DisableMouseCapture,
            DisableBracketedPaste,
            SetCursorStyle::DefaultUserShape,
            Show
        );
        if !self.raw_before {
            let _ = terminal::disable_raw_mode();
        }
    }
}
fn make_inline(height: u16) -> io::Result<Inline> {
    Terminal::with_options(
        CrosstermBackend::new(stdout()),
        TerminalOptions {
            viewport: Viewport::Inline(height),
        },
    )
}
fn clear_inline<B: Backend>(terminal: &mut Terminal<B>) -> Result<(), B::Error> {
    let origin = terminal.get_frame().area().as_position();
    // ratatui clear() preserves the backend cursor, which insert_before() can
    // leave at the end of a transcript row. Hand off at the cleared viewport's
    // origin so subsequent plain output neither overwrites nor wraps that row.
    terminal.clear()?;
    terminal.set_cursor_position(origin)
}
/// A temporary full-screen view restores the caller's raw-mode state.
/// It never owns input polling; the same application loop keeps handling events.
pub struct DetailScreen {
    raw_before: bool,
    last_frame: Option<(u16, u16, Vec<String>)>,
}
impl DetailScreen {
    pub fn enter() -> io::Result<Self> {
        let raw_before = terminal::is_raw_mode_enabled()?;
        terminal::enable_raw_mode()?;
        let screen = Self {
            raw_before,
            last_frame: None,
        };
        execute!(stdout(), terminal::EnterAlternateScreen, Hide)?;
        Ok(screen)
    }
    pub fn draw(&mut self, title: &str, lines: &[String], offset: usize) -> io::Result<()> {
        let (width, height) = terminal::size()?;
        let width_cells = width.saturating_sub(1).max(1) as usize;
        let rows: Vec<String> = std::iter::once(title)
            .chain(
                lines
                    .iter()
                    .skip(offset)
                    .take(height.saturating_sub(2) as usize)
                    .map(String::as_str),
            )
            .map(|line| clip(&clean_text(line), width_cells))
            .collect();
        let mut rows = rows;
        if height > 1 {
            rows.push(clip(
                &format!(
                    "{}–{} / {} · ↑↓ scroll · [/] history · Esc close",
                    offset + 1,
                    (offset + height.saturating_sub(2) as usize).min(lines.len()),
                    lines.len()
                ),
                width_cells,
            ));
        }
        let frame = (width, height, rows);
        if self.last_frame.as_ref() == Some(&frame) {
            return Ok(());
        }
        let mut out = stdout();
        queue!(out, crossterm::cursor::MoveTo(0, 0), Clear(ClearType::All))?;
        let theme = Theme::detect_stdout();
        let mut code_block = lines
            .iter()
            .take(offset)
            .filter(|s| s.trim_start().starts_with("```"))
            .count()
            % 2
            != 0;
        for (index, line) in frame.2.iter().enumerate() {
            if index > 0 {
                queue!(out, MoveDown(1), MoveToColumn(0))?;
            }
            if index == 0 {
                write!(out, "{}", theme.accent(line))?;
            } else if index + 1 == frame.2.len() {
                write!(out, "{}", theme.dim(line))?;
            } else {
                write!(out, "{}", theme.render_row(line, code_block))?;
                if line.trim_start().starts_with("```") {
                    code_block = !code_block;
                }
            }
        }
        out.flush()?;
        self.last_frame = Some(frame);
        Ok(())
    }
}
/// Ratatui fills the second cell of a wide grapheme with a space. The backend
/// then writes that space after the glyph, which is the visible gap after CJK.
/// Clear the continuation cells so the renderer leaves them untouched.
fn collapse_wide_continuations(buffer: &mut ratatui::buffer::Buffer) {
    let area = buffer.area;
    for y in area.top()..area.bottom() {
        let mut x = area.left();
        while x < area.right() {
            let width = UnicodeWidthStr::width(buffer[(x, y)].symbol());
            if width > 1 {
                for offset in 1..width {
                    let next = x + offset as u16;
                    if next < area.right() {
                        buffer[(next, y)].set_symbol("");
                    }
                }
                x += width as u16;
            } else {
                x += 1;
            }
        }
    }
}

impl Drop for DetailScreen {
    fn drop(&mut self) {
        let _ = execute!(stdout(), terminal::LeaveAlternateScreen, Show);
        if !self.raw_before {
            let _ = terminal::disable_raw_mode();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wide_graphemes_do_not_leave_filler_spaces() {
        let mut buffer = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 20, 1));
        Paragraph::new("中文").render(buffer.area, &mut buffer);
        collapse_wide_continuations(&mut buffer);
        assert_eq!(buffer[(0, 0)].symbol(), "中");
        assert!(buffer[(1, 0)].symbol().is_empty());
        assert_eq!(buffer[(2, 0)].symbol(), "文");
        assert!(buffer[(3, 0)].symbol().is_empty());
    }

    #[test]
    fn clearing_inline_hands_off_at_viewport_origin_and_preserves_transcript() {
        let mut terminal = Terminal::with_options(
            ratatui::backend::TestBackend::new(80, 24),
            TerminalOptions {
                viewport: Viewport::Inline(3),
            },
        )
        .unwrap();
        terminal
            .insert_before(1, |buffer| {
                Paragraph::new("preserved transcript").render(buffer.area, buffer);
            })
            .unwrap();
        let origin = terminal.get_frame().area().as_position();
        assert!(origin.y > 0);
        terminal.set_cursor_position((79, origin.y - 1)).unwrap();
        clear_inline(&mut terminal).unwrap();
        assert_eq!(terminal.get_cursor_position().unwrap(), origin);
        assert_eq!(terminal.backend().buffer()[(0, origin.y - 1)].symbol(), "p");
    }
}
