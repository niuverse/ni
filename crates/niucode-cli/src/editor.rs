//! Minimal readline-style editor for the linear TUI.
//!
//! rustyline can list completions on Tab, but it cannot refresh a menu on every
//! keystroke. Slash commands are a small, deterministic set, so a focused editor
//! gives us the `/i -> /init` menu without pulling a terminal UI framework in.

use std::io::{stdout, IsTerminal, Write};
use std::sync::atomic::{AtomicU64, Ordering};

use crossterm::cursor::{position, Hide, MoveLeft, MoveRight, MoveToColumn, MoveUp, Show};
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{self, disable_raw_mode, enable_raw_mode, Clear, ClearType};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::slash::{CommandRegistry, CommandSpec};
use crate::theme::Theme;

const MAX_MENU_ROWS: usize = 6;
const MENU_NAME_WIDTH: usize = 10;
const MENU_PREFIX_WIDTH: usize = 2;
static MOUSE_EVENTS: AtomicU64 = AtomicU64::new(0);

/// Number of mouse events delivered to the line editor in this process.
pub fn mouse_events_seen() -> u64 {
    MOUSE_EVENTS.load(Ordering::Relaxed)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Line(String),
    Interrupted,
    Eof,
}

#[derive(Clone)]
struct MenuItem {
    insert: String,
    name: String,
    description: String,
}

pub struct LineEditor {
    registry: CommandRegistry,
    history: Vec<String>,
    models: Vec<String>,
    current_model: String,
    permissions: Vec<(String, String)>,
}

impl LineEditor {
    pub fn new(registry: CommandRegistry) -> Self {
        Self {
            registry,
            history: Vec::new(),
            models: Vec::new(),
            current_model: String::new(),
            permissions: Vec::new(),
        }
    }

    pub fn set_models(&mut self, models: Vec<String>) {
        self.models = models;
    }

    pub fn set_current_model(&mut self, model: impl Into<String>) {
        self.current_model = model.into();
    }

    pub fn set_permissions(&mut self, permissions: Vec<(String, String)>) {
        self.permissions = permissions;
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn read_line(
        &mut self,
        raw_prompt: &str,
        styled_prompt: &str,
        theme: Theme,
    ) -> std::io::Result<Input> {
        if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
            return Err(std::io::Error::other("line editor requires a TTY"));
        }

        let history_index = self.history.len();
        let mut state = State {
            registry: &self.registry,
            history: &mut self.history,
            models: &self.models,
            current_model: &self.current_model,
            permissions: &self.permissions,
            history_index,
            chars: Vec::new(),
            cursor: 0,
            selected: 0,
            menu_open: false,
            scroll: 0,
            history_search: None,
            history_query: None,
            prompt_row: None,
            mouse_failed: false,
            mouse_captured: false,
            mouse_anchor: None,
            menu_navigated: false,
            raw_prompt,
            styled_prompt,
            theme,
        };

        let _guard = TerminalGuard::enter()?;
        state.run(&mut stdout())
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> std::io::Result<Self> {
        enable_raw_mode()?;
        let mut out = stdout();
        if let Err(error) = execute!(out, EnableBracketedPaste, Hide) {
            let _ = disable_raw_mode();
            return Err(error);
        }
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut out = stdout();
        let _ = execute!(out, DisableMouseCapture, DisableBracketedPaste, Show);
        let _ = disable_raw_mode();
    }
}

struct State<'a> {
    registry: &'a CommandRegistry,
    history: &'a mut Vec<String>,
    models: &'a [String],
    current_model: &'a str,
    permissions: &'a [(String, String)],
    history_index: usize,
    chars: Vec<char>,
    cursor: usize,
    selected: usize,
    menu_open: bool,
    scroll: usize,
    history_search: Option<usize>,
    history_query: Option<String>,
    prompt_row: Option<u16>,
    mouse_failed: bool,
    mouse_captured: bool,
    mouse_anchor: Option<u16>,
    menu_navigated: bool,
    raw_prompt: &'a str,
    styled_prompt: &'a str,
    theme: Theme,
}

impl State<'_> {
    fn run(&mut self, out: &mut impl Write) -> std::io::Result<Input> {
        self.refresh_menu();
        self.sync_mouse_capture(out)?;
        self.render(out)?;

        loop {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Release {
                        continue;
                    }
                    if let Some(input) = self.handle_key(key, out)? {
                        return Ok(input);
                    }
                    self.refresh_menu();
                    self.sync_mouse_capture(out)?;
                    self.render(out)?;
                }
                Event::Paste(text) => {
                    let text = text.replace(['\r', '\n'], " ");
                    self.insert(&text);
                    self.refresh_menu();
                    self.sync_mouse_capture(out)?;
                    self.render(out)?;
                }
                Event::Mouse(mouse) => {
                    MOUSE_EVENTS.fetch_add(1, Ordering::Relaxed);
                    let changed = self.handle_mouse(mouse)?;
                    // If cursor-position support is missing, drop capture so
                    // terminal text selection and drag keep working.
                    self.sync_mouse_capture(out)?;
                    if changed {
                        self.refresh_menu();
                        self.render(out)?;
                    }
                }
                Event::Resize(_, _) => {
                    self.prompt_row = None;
                    self.mouse_captured = false;
                    self.sync_mouse_capture(out)?;
                    self.render(out)?;
                }
                _ => {}
            }
        }
    }

    fn handle_key(
        &mut self,
        key: event::KeyEvent,
        out: &mut impl Write,
    ) -> std::io::Result<Option<Input>> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        if ctrl {
            return match key.code {
                KeyCode::Char('c') => {
                    self.clear_final(out, false)?;
                    Ok(Some(Input::Interrupted))
                }
                KeyCode::Char('d') => {
                    if self.chars.is_empty() {
                        self.clear_final(out, false)?;
                        Ok(Some(Input::Eof))
                    } else {
                        self.delete_at_cursor();
                        Ok(None)
                    }
                }
                KeyCode::Char('j') => self.enter(out),
                KeyCode::Char('n') => {
                    if self.menu_open {
                        let visible = self.candidates().len().min(MAX_MENU_ROWS);
                        self.selected = (self.selected + 1).min(visible.saturating_sub(1));
                        self.menu_navigated = true;
                    } else {
                        self.history_down();
                    }
                    Ok(None)
                }
                KeyCode::Char('p') => {
                    if self.menu_open {
                        self.selected = self.selected.saturating_sub(1);
                        self.menu_navigated = true;
                    } else {
                        self.history_up();
                    }
                    Ok(None)
                }
                KeyCode::Char('r') => {
                    self.history_search_prev();
                    Ok(None)
                }
                KeyCode::Left => {
                    self.move_word_left();
                    Ok(None)
                }
                KeyCode::Right => {
                    self.move_word_right();
                    Ok(None)
                }
                KeyCode::Char('a') => {
                    self.cursor = 0;
                    Ok(None)
                }
                KeyCode::Char('e') => {
                    self.cursor = self.chars.len();
                    Ok(None)
                }
                KeyCode::Char('u') => {
                    self.chars.drain(..self.cursor);
                    self.cursor = 0;
                    Ok(None)
                }
                KeyCode::Char('k') => {
                    self.chars.truncate(self.cursor);
                    Ok(None)
                }
                KeyCode::Char('w') => {
                    self.delete_word();
                    Ok(None)
                }
                KeyCode::Char('l') => {
                    execute!(out, Clear(ClearType::All), MoveToColumn(0))?;
                    self.render(out)?;
                    Ok(None)
                }
                _ => Ok(None),
            };
        }

        if alt {
            return match key.code {
                KeyCode::Char('b') | KeyCode::Left => {
                    self.move_word_left();
                    Ok(None)
                }
                KeyCode::Char('f') | KeyCode::Right => {
                    self.move_word_right();
                    Ok(None)
                }
                _ => Ok(None),
            };
        }

        match key.code {
            KeyCode::Enter => self.enter(out),
            KeyCode::Char('\n') => self.enter(out),
            KeyCode::Tab => {
                if self.menu_open {
                    self.accept_selected();
                } else {
                    self.menu_open = !self.candidates().is_empty();
                }
                Ok(None)
            }
            KeyCode::Esc => {
                self.menu_open = false;
                Ok(None)
            }
            KeyCode::Up => {
                if self.menu_open {
                    self.selected = self.selected.saturating_sub(1);
                    self.menu_navigated = true;
                } else {
                    self.history_up();
                }
                Ok(None)
            }
            KeyCode::Down => {
                if self.menu_open {
                    let visible = self.candidates().len().min(MAX_MENU_ROWS);
                    self.selected = (self.selected + 1).min(visible.saturating_sub(1));
                    self.menu_navigated = true;
                } else {
                    self.history_down();
                }
                Ok(None)
            }
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
                Ok(None)
            }
            KeyCode::Right => {
                self.cursor = (self.cursor + 1).min(self.chars.len());
                Ok(None)
            }
            KeyCode::Home => {
                self.cursor = 0;
                Ok(None)
            }
            KeyCode::End => {
                self.cursor = self.chars.len();
                Ok(None)
            }
            KeyCode::Backspace => {
                self.menu_navigated = false;
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.chars.remove(self.cursor);
                }
                Ok(None)
            }
            KeyCode::Delete => {
                self.menu_navigated = false;
                self.delete_at_cursor();
                Ok(None)
            }
            KeyCode::Char(c) => {
                self.insert(&c.to_string());
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    fn enter(&mut self, out: &mut impl Write) -> std::io::Result<Option<Input>> {
        let line = self.line();
        if self.menu_open
            && !self.menu_navigated
            && matches!(line.as_str(), "/model" | "/permissions")
        {
            self.menu_open = false;
            self.remember(&line);
            self.clear_final(out, true)?;
            return Ok(Some(Input::Line(line)));
        }
        let candidates = self.candidates();
        let exact = candidates
            .get(self.selected)
            .is_some_and(|item| item.insert.trim_end() == line);
        if self.menu_open && !exact {
            self.accept_selected();
            Ok(None)
        } else {
            self.remember(&line);
            self.clear_final(out, true)?;
            Ok(Some(Input::Line(line)))
        }
    }

    fn insert(&mut self, text: &str) {
        self.menu_navigated = false;
        for c in text.chars() {
            self.chars.insert(self.cursor, c);
            self.cursor += 1;
        }
        self.history_index = self.history.len();
        self.history_search = None;
        self.history_query = None;
    }

    fn delete_at_cursor(&mut self) {
        if self.cursor < self.chars.len() {
            self.chars.remove(self.cursor);
        }
    }

    fn delete_word(&mut self) {
        while self.cursor > 0 && self.chars[self.cursor - 1].is_whitespace() {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
        }
        while self.cursor > 0 && !self.chars[self.cursor - 1].is_whitespace() {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
        }
    }

    fn line(&self) -> String {
        self.chars.iter().collect()
    }

    fn candidates(&self) -> Vec<MenuItem> {
        let line = self.line();
        if let Some(items) =
            argument_candidates(self.models, self.current_model, self.permissions, &line)
        {
            return items;
        }
        command_candidates(self.registry, &line)
            .into_iter()
            .map(|spec| MenuItem {
                insert: format!("{} ", spec.name),
                name: spec.name.clone(),
                description: spec.description.clone(),
            })
            .collect()
    }

    fn refresh_menu(&mut self) {
        let count = self.candidates().len();
        self.menu_open = count > 0;
        if count == 0 {
            self.selected = 0;
        } else {
            self.selected = self.selected.min(count.min(MAX_MENU_ROWS) - 1);
        }
    }

    fn accept_selected(&mut self) {
        let candidates = self.candidates();
        let Some(item) = candidates.get(self.selected) else {
            return;
        };
        self.chars = item.insert.chars().collect();
        self.cursor = self.chars.len();
        self.menu_open = false;
        self.menu_navigated = false;
        self.history_index = self.history.len();
    }

    fn move_word_left(&mut self) {
        while self.cursor > 0 && self.chars[self.cursor - 1].is_whitespace() {
            self.cursor -= 1;
        }
        while self.cursor > 0 && !self.chars[self.cursor - 1].is_whitespace() {
            self.cursor -= 1;
        }
    }

    fn move_word_right(&mut self) {
        while self.cursor < self.chars.len() && self.chars[self.cursor].is_whitespace() {
            self.cursor += 1;
        }
        while self.cursor < self.chars.len() && !self.chars[self.cursor].is_whitespace() {
            self.cursor += 1;
        }
    }

    fn history_search_prev(&mut self) {
        let needle = self.history_query.clone().unwrap_or_else(|| self.line());
        self.history_query = Some(needle.clone());
        let start = self.history_search.unwrap_or(self.history.len());
        for index in (0..start).rev() {
            if self.history[index].contains(&needle) {
                self.history_search = Some(index);
                self.history_index = index;
                self.set_line(&self.history[index].clone());
                return;
            }
        }
    }

    fn remember(&mut self, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        if self.history.last().map(String::as_str) != Some(line) {
            self.history.push(line.to_string());
        }
        self.history_index = self.history.len();
    }

    fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        if self.history_index == 0 {
            return;
        }
        self.history_index -= 1;
        self.history_search = None;
        self.history_query = None;
        self.set_line(&self.history[self.history_index].clone());
    }

    fn history_down(&mut self) {
        if self.history.is_empty() || self.history_index >= self.history.len() {
            return;
        }
        self.history_index += 1;
        self.history_search = None;
        self.history_query = None;
        if self.history_index == self.history.len() {
            self.set_line("");
        } else {
            self.set_line(&self.history[self.history_index].clone());
        }
    }

    fn set_line(&mut self, line: &str) {
        self.chars = line.chars().collect();
        self.cursor = self.chars.len();
    }

    fn render(&mut self, out: &mut impl Write) -> std::io::Result<()> {
        let (cols, rows) = terminal::size().unwrap_or((80, 24));
        let cols = cols.max(20) as usize;
        let prompt_width = UnicodeWidthStr::width(self.raw_prompt);
        let available = cols.saturating_sub(prompt_width + 1).max(1);

        let widths = self
            .chars
            .iter()
            .map(|c| UnicodeWidthChar::width(*c).unwrap_or(0))
            .collect::<Vec<_>>();
        let cursor_col = widths[..self.cursor].iter().sum::<usize>();
        if cursor_col < self.scroll {
            self.scroll = cursor_col;
        }
        if cursor_col >= self.scroll + available {
            self.scroll = cursor_col + 1 - available;
        }

        let prefix_width = usize::from(self.scroll > 0);
        let visible_width = available.saturating_sub(prefix_width);
        let mut visible = String::new();
        let mut used = 0_usize;
        let mut col = 0_usize;
        for (index, c) in self.chars.iter().enumerate() {
            let width = widths[index];
            col += width;
            if col <= self.scroll {
                continue;
            }
            if used + width > visible_width {
                break;
            }
            visible.push(*c);
            used += width;
        }
        let screen_cursor = prefix_width + cursor_col.saturating_sub(self.scroll);

        execute!(
            out,
            MoveToColumn(0),
            Clear(ClearType::CurrentLine),
            Clear(ClearType::FromCursorDown)
        )?;
        write!(out, "{}", self.styled_prompt)?;
        if self.scroll > 0 {
            write!(out, "…")?;
        }
        if self.chars.first() == Some(&'/') {
            if let Some(space) = visible.find(char::is_whitespace) {
                write!(
                    out,
                    "{}{}",
                    self.theme.accent(&visible[..space]),
                    &visible[space..]
                )?;
            } else {
                write!(out, "{}", self.theme.accent(&visible))?;
            }
        } else {
            write!(out, "{visible}")?;
        }

        let candidates = self.candidates();
        let menu_rows = if self.menu_open {
            candidates.len().min(MAX_MENU_ROWS)
        } else {
            0
        };
        for (index, spec) in candidates.iter().take(menu_rows).enumerate() {
            write!(out, "\r\n")?;
            let line = format!(
                "{:<width$} {}",
                spec.name,
                spec.description,
                width = MENU_NAME_WIDTH
            );
            // The selected/normal prefix occupies two columns. Truncating to
            // the full terminal width would wrap and desynchronise the cursor,
            // leaving old menu rows behind when the user edits the input.
            let line = truncate_columns(&line, cols.saturating_sub(MENU_PREFIX_WIDTH));
            if index == self.selected {
                if self.theme.is_ansi() {
                    write!(out, "\x1b[7m▸ {line}\x1b[0m")?;
                } else {
                    write!(out, "> {line}")?;
                }
            } else {
                write!(out, "  {line}")?;
            }
        }

        if menu_rows > 0 {
            if let Some(row) = self.prompt_row {
                let rows = rows.max(1) as usize;
                let overflow = (row as usize + menu_rows).saturating_sub(rows - 1);
                self.prompt_row = Some(row.saturating_sub(overflow as u16));
            }
            execute!(out, MoveUp(menu_rows as u16))?;
            execute!(out, MoveRight((prompt_width + screen_cursor) as u16))?;
        } else {
            let end = prompt_width + prefix_width + used;
            let want = prompt_width + screen_cursor;
            if end > want {
                execute!(out, MoveLeft((end - want) as u16))?;
            }
        }
        out.flush()
    }

    fn sync_mouse_capture(&mut self, out: &mut impl Write) -> std::io::Result<()> {
        let want = self.menu_open;
        if want == self.mouse_captured {
            return Ok(());
        }
        if want {
            // Query before enabling mouse reporting. Querying from inside a
            // mouse event can race with crossterm's event reader and leave
            // hover permanently disabled on some terminals.
            if self.prompt_row.is_none() && !self.mouse_failed {
                match position() {
                    Ok((_, row)) => self.prompt_row = Some(row),
                    Err(_) => self.mouse_failed = true,
                }
            }
            if execute!(out, EnableMouseCapture).is_err() {
                self.mouse_failed = true;
                return Ok(());
            }
            self.mouse_anchor = None;
        } else {
            let _ = execute!(out, DisableMouseCapture);
            self.mouse_anchor = None;
        }
        self.mouse_captured = want;
        Ok(())
    }

    fn handle_relative_mouse(&mut self, mouse: event::MouseEvent, rows: usize) -> bool {
        match mouse.kind {
            MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                let anchor = *self.mouse_anchor.get_or_insert(mouse.row);
                let delta = mouse.row as i32 - anchor as i32;
                if delta == 0 {
                    return false;
                }
                let next = (self.selected as i32 + delta).clamp(0, rows.saturating_sub(1) as i32);
                self.mouse_anchor = Some(mouse.row);
                if next as usize == self.selected {
                    return false;
                }
                self.selected = next as usize;
                self.menu_navigated = true;
                true
            }
            MouseEventKind::Down(MouseButton::Left) => {
                self.accept_selected();
                true
            }
            MouseEventKind::ScrollDown => {
                self.selected = (self.selected + 1).min(rows.saturating_sub(1));
                self.menu_navigated = true;
                true
            }
            MouseEventKind::ScrollUp => {
                self.selected = self.selected.saturating_sub(1);
                self.menu_navigated = true;
                true
            }
            _ => false,
        }
    }

    fn handle_mouse(&mut self, mouse: event::MouseEvent) -> std::io::Result<bool> {
        if !self.menu_open {
            return Ok(false);
        }
        let rows = self.candidates().len().min(MAX_MENU_ROWS);
        if rows == 0 {
            return Ok(false);
        }
        let Some(prompt_row) = self.prompt_row else {
            return Ok(self.handle_relative_mouse(mouse, rows));
        };
        let Some(index) = menu_index(mouse.row, prompt_row, rows) else {
            return Ok(false);
        };
        match mouse.kind {
            MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                if self.selected != index {
                    self.selected = index;
                    self.menu_navigated = true;
                    return Ok(true);
                }
                Ok(false)
            }
            MouseEventKind::Down(MouseButton::Left) => {
                self.selected = index;
                self.menu_navigated = true;
                self.accept_selected();
                Ok(true)
            }
            MouseEventKind::ScrollDown => {
                self.selected = (self.selected + 1).min(rows.saturating_sub(1));
                Ok(true)
            }
            MouseEventKind::ScrollUp => {
                self.selected = self.selected.saturating_sub(1);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn clear_final(&mut self, out: &mut impl Write, keep_line: bool) -> std::io::Result<()> {
        let line = self.line();
        execute!(
            out,
            MoveToColumn(0),
            Clear(ClearType::CurrentLine),
            Clear(ClearType::FromCursorDown)
        )?;
        if keep_line {
            write!(out, "{}", self.styled_prompt)?;
            write!(out, "{line}")?;
        }
        writeln!(out)?;
        out.flush()
    }
}

fn argument_candidates(
    models: &[String],
    current_model: &str,
    permissions: &[(String, String)],
    line: &str,
) -> Option<Vec<MenuItem>> {
    let (command, argument) = if line == "/model" {
        ("model", "")
    } else if let Some(rest) = line.strip_prefix("/model ") {
        ("model", rest)
    } else if line == "/permissions" {
        ("permissions", "")
    } else {
        let rest = line.strip_prefix("/permissions ")?;
        ("permissions", rest)
    };
    if argument.chars().any(char::is_whitespace) {
        return None;
    }

    let items = match command {
        "model" => models
            .iter()
            .filter(|model| model.starts_with(argument))
            .map(|model| MenuItem {
                insert: format!("/model {model} "),
                name: model.clone(),
                description: if model == current_model {
                    "current model · next turn".to_string()
                } else {
                    "switch model · next turn".to_string()
                },
            })
            .collect(),
        "permissions" => permissions
            .iter()
            .filter(|(name, _)| name.starts_with(argument))
            .map(|(name, description)| MenuItem {
                insert: format!("/permissions {name} "),
                name: name.clone(),
                description: description.clone(),
            })
            .collect(),
        _ => Vec::new(),
    };
    Some(items)
}

fn command_candidates<'a>(registry: &'a CommandRegistry, line: &str) -> Vec<&'a CommandSpec> {
    if !line.starts_with('/') || line.chars().any(char::is_whitespace) {
        return Vec::new();
    }
    registry.candidates(line)
}

fn menu_index(mouse_row: u16, prompt_row: u16, rows: usize) -> Option<usize> {
    if mouse_row <= prompt_row || mouse_row > prompt_row + rows as u16 {
        return None;
    }
    Some((mouse_row - prompt_row - 1) as usize)
}

fn truncate_columns(text: &str, columns: usize) -> String {
    let mut out = String::new();
    let mut used = 0_usize;
    for c in text.chars() {
        let width = UnicodeWidthChar::width(c).unwrap_or(0);
        if used + width > columns {
            break;
        }
        out.push(c);
        used += width;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_menu_lines_by_display_width() {
        assert_eq!(truncate_columns("abcdef", 4), "abcd");
        assert_eq!(truncate_columns("你好世界", 4), "你好");
    }

    #[test]
    fn menu_lines_leave_room_for_the_prefix() {
        let line = format!("{:<10} {}", "/model", "description ".repeat(20));
        let truncated = truncate_columns(&line, 40 - MENU_PREFIX_WIDTH);
        assert!(
            UnicodeWidthStr::width(truncated.as_str()) + MENU_PREFIX_WIDTH <= 40,
            "menu line would wrap"
        );
    }

    #[test]
    fn maps_mouse_rows_to_menu_candidates() {
        assert_eq!(menu_index(11, 10, 6), Some(0));
        assert_eq!(menu_index(16, 10, 6), Some(5));
        assert_eq!(menu_index(10, 10, 6), None);
        assert_eq!(menu_index(17, 10, 6), None);
    }

    #[test]
    fn argument_candidates_filter_models_and_permissions() {
        let models = vec![
            "deepseek-flash".to_string(),
            "deepseek-pro".to_string(),
            "deepseek-reasoner".to_string(),
        ];
        let permissions = vec![
            ("ask".to_string(), "workspace-write".to_string()),
            ("review".to_string(), "workspace-write".to_string()),
        ];
        let items = argument_candidates(&models, "deepseek-flash", &permissions, "/model ")
            .expect("model menu");
        assert_eq!(items.len(), 3);
        assert_eq!(items[1].insert, "/model deepseek-pro ");
        assert!(items[0].description.contains("current"));

        let partial = argument_candidates(&models, "deepseek-flash", &permissions, "/model d")
            .expect("partial menu");
        assert_eq!(partial.len(), 3);
        let filtered =
            argument_candidates(&models, "deepseek-flash", &permissions, "/model deepseek-p")
                .expect("filtered menu");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "deepseek-pro");

        let perms = argument_candidates(&models, "deepseek-flash", &permissions, "/permissions r")
            .expect("permission menu");
        assert_eq!(perms.len(), 1);
        assert_eq!(perms[0].insert, "/permissions review ");
        assert!(
            argument_candidates(&models, "deepseek-flash", &permissions, "/model pro max")
                .is_none()
        );
    }

    #[test]
    fn opens_the_menu_for_slash_prefixes_only() {
        let registry = CommandRegistry::builtin();
        let init = command_candidates(&registry, "/i");
        assert_eq!(init.len(), 1);
        assert_eq!(init[0].name, "/init");

        assert_eq!(
            command_candidates(&registry, "/").len(),
            registry.all().len()
        );
        assert!(command_candidates(&registry, "/goal all tests pass").is_empty());
        assert!(command_candidates(&registry, "hello").is_empty());
    }
}
