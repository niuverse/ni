//! Opt-in inline ratatui editor.
//!
//! The transcript remains native terminal output. This module only owns a
//! small inline viewport at the bottom: picker/help, input, footer.

use std::io::stdout;

use crossterm::cursor::Show;
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind, KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph};
use ratatui::{Terminal, TerminalOptions, Viewport};
use unicode_width::UnicodeWidthStr;

use crate::editor::{Input, LineEditor};
use crate::slash::CommandRegistry;
use crate::theme::Theme;

const VIEWPORT_HEIGHT: u16 = 8;
const MENU_ROWS: usize = 5;

#[derive(Clone)]
struct Item {
    insert: String,
    name: String,
    description: String,
}

pub struct InlineEditor {
    registry: CommandRegistry,
    history: Vec<String>,
    models: Vec<String>,
    current_model: String,
    permissions: Vec<(String, String)>,
    ansi: bool,
    fallback: LineEditor,
    fell_back: bool,
}

impl InlineEditor {
    pub fn new(registry: CommandRegistry) -> Self {
        Self {
            fallback: LineEditor::new(registry.clone()),
            registry,
            history: Vec::new(),
            models: Vec::new(),
            current_model: String::new(),
            permissions: Vec::new(),
            ansi: true,
            fell_back: false,
        }
    }

    pub fn set_models(&mut self, models: Vec<String>) {
        self.models = models.clone();
        self.fallback.set_models(models);
    }

    pub fn set_current_model(&mut self, model: impl Into<String>) {
        let model = model.into();
        self.current_model = model.clone();
        self.fallback.set_current_model(model);
    }

    pub fn set_permissions(&mut self, permissions: Vec<(String, String)>) {
        self.permissions = permissions.clone();
        self.fallback.set_permissions(permissions);
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
        self.fallback.clear_history();
    }

    pub fn read_line(
        &mut self,
        raw_prompt: &str,
        _styled_prompt: &str,
        footer: &str,
        theme: Theme,
    ) -> std::io::Result<Input> {
        self.ansi = theme.is_ansi();
        enable_raw_mode()?;
        let mut out = stdout();
        if let Err(error) = execute!(out, EnableBracketedPaste, Show) {
            let _ = disable_raw_mode();
            return Err(error);
        }
        let backend = CrosstermBackend::new(stdout());
        let mut terminal = match Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(VIEWPORT_HEIGHT),
            },
        ) {
            Ok(terminal) => terminal,
            Err(_) => {
                let _ = execute!(stdout(), DisableBracketedPaste, Show);
                let _ = disable_raw_mode();
                return self.fallback.read_line(raw_prompt, _styled_prompt, theme);
            }
        };

        let result = self.run(&mut terminal, raw_prompt, footer);
        let _ = terminal.clear();
        drop(terminal);
        let _ = execute!(stdout(), DisableBracketedPaste, Show);
        let _ = disable_raw_mode();
        if !self.fell_back {
            if let Ok(Input::Line(line)) = &result {
                println!("{raw_prompt}{line}");
            }
        }
        result
    }

    fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
        prompt: &str,
        footer: &str,
    ) -> std::io::Result<Input> {
        let mut chars = Vec::new();
        let mut cursor = 0_usize;
        let mut history_index = self.history.len();
        let mut selected = 0_usize;
        let mut show_menu = true;
        loop {
            let line: String = chars.iter().collect();
            let candidates = self.candidates(&line);
            let menu_open = show_menu && !candidates.is_empty();
            if menu_open {
                selected = selected.min(candidates.len().saturating_sub(1));
            } else {
                selected = 0;
            }

            let ansi = self.ansi();
            terminal.draw(|frame| {
                let area = frame.area();
                let rows = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(MENU_ROWS as u16),
                        Constraint::Length(2),
                        Constraint::Length(1),
                    ])
                    .split(area);
                let items = if menu_open {
                    candidates
                        .iter()
                        .take(MENU_ROWS)
                        .enumerate()
                        .map(|(index, item)| {
                            let marker = if index == selected { "▸" } else { " " };
                            let text = format!("{marker} {:<22} {}", item.name, item.description);
                            let style = if index == selected && ansi {
                                Style::default()
                                    .fg(Color::Black)
                                    .bg(Color::Cyan)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default()
                            };
                            ListItem::new(Line::from(Span::styled(text, style)))
                        })
                        .collect::<Vec<_>>()
                } else {
                    vec![
                        ListItem::new(
                            "  enter send · shift/ctrl-j newline · / commands · ↑↓ history",
                        ),
                        ListItem::new("  tab complete · ctrl-r history search · esc close menu"),
                        ListItem::new(""),
                        ListItem::new(""),
                        ListItem::new(""),
                    ]
                };
                frame.render_widget(List::new(items), rows[0]);
                let visible = format!("{prompt}{line}");
                let input_style = if ansi {
                    Style::default().fg(Color::White)
                } else {
                    Style::default()
                };
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(visible.clone(), input_style))),
                    rows[1],
                );
                let width = UnicodeWidthStr::width(visible.as_str()) as u16;
                let x = rows[1].x + width.min(rows[1].width.saturating_sub(1));
                frame.set_cursor_position((x, rows[1].y));
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        footer.to_string(),
                        if ansi {
                            Style::default().fg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ))),
                    rows[2],
                );
            })?;

            let event = event::read()?;
            match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        match key.code {
                            KeyCode::Char('c') => return Ok(Input::Interrupted),
                            KeyCode::Char('d') if chars.is_empty() => return Ok(Input::Eof),
                            KeyCode::Char('j') => {
                                chars.insert(cursor, '\n');
                                cursor += 1;
                            }
                            KeyCode::Char('a') => cursor = 0,
                            KeyCode::Char('e') => cursor = chars.len(),
                            KeyCode::Char('u') => {
                                chars.truncate(cursor);
                            }
                            KeyCode::Char('k') => {
                                chars.truncate(cursor);
                            }
                            KeyCode::Char('l') => {
                                let _ = terminal.clear();
                            }
                            KeyCode::Char('r') => {
                                let needle = line;
                                if let Some(found) = history_index.checked_sub(1).and_then(|_| {
                                    (0..history_index)
                                        .rev()
                                        .find(|index| self.history[*index].contains(&needle))
                                }) {
                                    history_index = found;
                                    chars = self.history[found].chars().collect();
                                    cursor = chars.len();
                                }
                            }
                            KeyCode::Left => {
                                while cursor > 0 && chars[cursor - 1].is_whitespace() {
                                    cursor -= 1;
                                }
                                while cursor > 0 && !chars[cursor - 1].is_whitespace() {
                                    cursor -= 1;
                                }
                            }
                            KeyCode::Right => {
                                while cursor < chars.len() && chars[cursor].is_whitespace() {
                                    cursor += 1;
                                }
                                while cursor < chars.len() && !chars[cursor].is_whitespace() {
                                    cursor += 1;
                                }
                            }
                            _ => {}
                        }
                        continue;
                    }
                    match key.code {
                        KeyCode::Enter => {
                            if menu_open {
                                if let Some(item) = candidates.get(selected) {
                                    if item.insert.trim_end() == line {
                                        self.history.push(line.clone());
                                        return Ok(Input::Line(line));
                                    }
                                    chars = item.insert.chars().collect();
                                    cursor = chars.len();
                                    selected = 0;
                                }
                            } else if !line.trim().is_empty() {
                                self.history.push(line.clone());
                                return Ok(Input::Line(line));
                            }
                        }
                        KeyCode::Char('\n') => {
                            chars.insert(cursor, '\n');
                            cursor += 1;
                        }
                        KeyCode::Tab => {
                            if let Some(item) = candidates.get(selected) {
                                chars = item.insert.chars().collect();
                                cursor = chars.len();
                                selected = 0;
                            }
                        }
                        KeyCode::Esc => {
                            show_menu = false;
                        }
                        KeyCode::Backspace => {
                            show_menu = true;
                            if cursor > 0 {
                                cursor -= 1;
                                chars.remove(cursor);
                            }
                        }
                        KeyCode::Delete => {
                            show_menu = true;
                            if cursor < chars.len() {
                                chars.remove(cursor);
                            }
                        }
                        KeyCode::Left => cursor = cursor.saturating_sub(1),
                        KeyCode::Right => cursor = (cursor + 1).min(chars.len()),
                        KeyCode::Home => cursor = 0,
                        KeyCode::End => cursor = chars.len(),
                        KeyCode::Up => {
                            if menu_open {
                                selected = selected.saturating_sub(1);
                            } else if history_index > 0 {
                                history_index -= 1;
                                chars = self.history[history_index].chars().collect();
                                cursor = chars.len();
                            }
                        }
                        KeyCode::Down => {
                            if menu_open {
                                selected = (selected + 1).min(candidates.len().saturating_sub(1));
                            } else if history_index + 1 < self.history.len() {
                                history_index += 1;
                                chars = self.history[history_index].chars().collect();
                                cursor = chars.len();
                            }
                        }
                        KeyCode::Char(character) if !character.is_control() => {
                            show_menu = true;
                            chars.insert(cursor, character);
                            cursor += 1;
                        }
                        _ => {}
                    }
                }
                Event::Paste(text) => {
                    show_menu = true;
                    for character in text.chars() {
                        chars.insert(cursor, character);
                        cursor += 1;
                    }
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }

    fn ansi(&self) -> bool {
        self.ansi
    }

    fn candidates(&self, line: &str) -> Vec<Item> {
        if line == "/model" || line.starts_with("/model ") {
            let argument = line.strip_prefix("/model").unwrap_or("").trim();
            return self
                .models
                .iter()
                .filter(|model| model.starts_with(argument))
                .map(|model| Item {
                    insert: format!("/model {model} "),
                    name: model.clone(),
                    description: if model == &self.current_model {
                        "current model".into()
                    } else {
                        "switch model".into()
                    },
                })
                .collect();
        }
        if line == "/permissions" || line.starts_with("/permissions ") {
            let argument = line.strip_prefix("/permissions").unwrap_or("").trim();
            return self
                .permissions
                .iter()
                .filter(|(name, _)| name.starts_with(argument))
                .map(|(name, description)| Item {
                    insert: format!("/permissions {name} "),
                    name: name.clone(),
                    description: description.clone(),
                })
                .collect();
        }
        if !line.starts_with('/') || line.chars().any(char::is_whitespace) {
            return Vec::new();
        }
        self.registry
            .candidates(line)
            .into_iter()
            .map(|spec| Item {
                insert: format!("{} ", spec.name),
                name: spec.name.clone(),
                description: spec.description.clone(),
            })
            .collect()
    }
}
