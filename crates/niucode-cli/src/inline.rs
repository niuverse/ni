//! Opt-in inline ratatui editor.
//!
//! The transcript remains native terminal output. This module only owns a
//! small inline viewport at the bottom: picker/help, input, footer.

use std::fs;
use std::io::stdout;
use std::path::{Path, PathBuf};

use crossterm::cursor::Show;
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{List, ListItem, Paragraph};
use ratatui::{Terminal, TerminalOptions, Viewport};
use unicode_width::UnicodeWidthStr;

use crate::editor::{Input, LineEditor};
use crate::slash::CommandRegistry;
use crate::theme::Theme;

const VIEWPORT_HEIGHT: u16 = 8;
const MENU_ROWS: usize = 4;
const INPUT_ROWS: usize = 3;

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
    cwd: PathBuf,
    files: Vec<String>,
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
            cwd: PathBuf::from("."),
            files: Vec::new(),
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

    pub fn set_cwd(&mut self, cwd: PathBuf) {
        self.files = collect_files(&cwd);
        self.cwd = cwd;
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
        let _ = execute!(stdout(), DisableMouseCapture, DisableBracketedPaste, Show);
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
        let mut menu_rect: Option<Rect> = None;
        let mut mouse_captured = false;
        loop {
            let line: String = chars.iter().collect();
            let candidates = self.candidates(&line);
            let menu_open = show_menu && !candidates.is_empty();
            let lines = line.split('\n').collect::<Vec<_>>();
            if menu_open {
                selected = selected.min(candidates.len().saturating_sub(1));
            } else {
                selected = 0;
            }

            if menu_open && !mouse_captured {
                let _ = execute!(stdout(), EnableMouseCapture);
                mouse_captured = true;
            } else if !menu_open && mouse_captured {
                let _ = execute!(stdout(), DisableMouseCapture);
                mouse_captured = false;
            }
            let ansi = self.ansi();
            terminal.draw(|frame| {
                let area = frame.area();
                let rows = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(MENU_ROWS as u16),
                        Constraint::Length(INPUT_ROWS as u16),
                        Constraint::Length(1),
                    ])
                    .split(area);
                menu_rect = Some(rows[0]);
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
                        ListItem::new("  tab complete · ctrl-j newline · ctrl-r history"),
                        ListItem::new("  esc close menu · @ files · / commands"),
                    ]
                };
                frame.render_widget(List::new(items), rows[0]);
                let input_style = if ansi {
                    Style::default().fg(Color::White)
                } else {
                    Style::default()
                };
                let (cursor_line, cursor_column) = cursor_position(&chars, cursor);
                let first_line = cursor_line.saturating_sub(INPUT_ROWS - 1);
                let mut input_lines = Vec::new();
                for (row, index) in (first_line..first_line + INPUT_ROWS).enumerate() {
                    let Some(text) = lines.get(index) else {
                        input_lines.push(Line::from(""));
                        continue;
                    };
                    let prefix = if index == 0 { prompt } else { "  " };
                    let width = rows[1].width as usize;
                    let mut text = format!("{prefix}{text}");
                    if text.chars().count() > width {
                        text = text.chars().take(width).collect();
                    }
                    input_lines.push(Line::from(Span::styled(text, input_style)));
                    if index == cursor_line {
                        let x = rows[1].x
                            + if index == 0 {
                                UnicodeWidthStr::width(prompt) + cursor_column
                            } else {
                                2 + cursor_column
                            } as u16;
                        frame.set_cursor_position((
                            x.min(rows[1].right().saturating_sub(1)),
                            rows[1].y + row as u16,
                        ));
                    }
                }
                frame.render_widget(Paragraph::new(Text::from(input_lines)), rows[1]);
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
                        KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                            chars.insert(cursor, '\n');
                            cursor += 1;
                        }
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
                            } else if chars.contains(&'\n') {
                                cursor = vertical_cursor(&chars, cursor, -1);
                            } else if history_index > 0 {
                                history_index -= 1;
                                chars = self.history[history_index].chars().collect();
                                cursor = chars.len();
                            }
                        }
                        KeyCode::Down => {
                            if menu_open {
                                selected = (selected + 1).min(candidates.len().saturating_sub(1));
                            } else if chars.contains(&'\n') {
                                cursor = vertical_cursor(&chars, cursor, 1);
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
                Event::Mouse(mouse) => {
                    let Some(rect) = menu_rect else {
                        continue;
                    };
                    if mouse.column < rect.x
                        || mouse.column >= rect.right()
                        || mouse.row < rect.y
                        || mouse.row >= rect.bottom()
                    {
                        continue;
                    }
                    let row = (mouse.row - rect.y) as usize;
                    match mouse.kind {
                        MouseEventKind::Moved => {
                            if row < candidates.len().min(MENU_ROWS) {
                                selected = row;
                            }
                        }
                        MouseEventKind::ScrollUp => {
                            selected = selected.saturating_sub(1);
                        }
                        MouseEventKind::ScrollDown => {
                            selected = (selected + 1).min(candidates.len().saturating_sub(1));
                        }
                        MouseEventKind::Down(MouseButton::Left)
                            if row < candidates.len().min(MENU_ROWS) =>
                        {
                            selected = row;
                            let item = candidates[row].clone();
                            chars = item.insert.chars().collect();
                            cursor = chars.len();
                            show_menu = true;
                        }
                        _ => {}
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
        if let Some(at) = line.rfind('@') {
            let before = &line[..at];
            let valid = at == 0 || before.chars().last().is_some_and(char::is_whitespace);
            let prefix = &line[at + 1..];
            if valid && !prefix.chars().any(char::is_whitespace) {
                let needle = format!("@{prefix}");
                return self
                    .files
                    .iter()
                    .filter(|file| file.starts_with(&needle))
                    .take(50)
                    .map(|file| Item {
                        insert: format!("{before}{file} "),
                        name: file.clone(),
                        description: "workspace file".into(),
                    })
                    .collect();
            }
        }
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

fn cursor_position(chars: &[char], cursor: usize) -> (usize, usize) {
    let before = &chars[..cursor.min(chars.len())];
    let line = before
        .iter()
        .filter(|character| **character == '\n')
        .count();
    let column = before
        .iter()
        .rposition(|character| *character == '\n')
        .map_or(before.len(), |index| before.len() - index - 1);
    (line, column)
}

fn vertical_cursor(chars: &[char], cursor: usize, delta: isize) -> usize {
    let before = &chars[..cursor.min(chars.len())];
    let line_start = before
        .iter()
        .rposition(|character| *character == '\n')
        .map_or(0, |index| index + 1);
    let column = cursor.saturating_sub(line_start);
    if delta < 0 {
        if line_start == 0 {
            return cursor;
        }
        let previous_end = line_start - 1;
        let previous_start = chars[..previous_end]
            .iter()
            .rposition(|character| *character == '\n')
            .map_or(0, |index| index + 1);
        previous_start + column.min(previous_end - previous_start)
    } else {
        let Some(next) = chars[cursor..]
            .iter()
            .position(|character| *character == '\n')
        else {
            return cursor;
        };
        let next_start = cursor + next + 1;
        if next_start > chars.len() {
            return cursor;
        }
        let next_end = chars[next_start..]
            .iter()
            .position(|character| *character == '\n')
            .map_or(chars.len(), |index| next_start + index);
        next_start + column.min(next_end - next_start)
    }
}

fn collect_files(root: &Path) -> Vec<String> {
    fn walk(
        root: &Path,
        directory: &Path,
        output: &mut Vec<String>,
        ignored: &[String],
        depth: usize,
    ) {
        if depth > 8 || output.len() >= 2_000 {
            return;
        }
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == ".git"
                || name == "target"
                || name == "node_modules"
                || name == ".niucode"
                || name == ".agents"
            {
                continue;
            }
            if name.starts_with(".env") {
                continue;
            }
            let relative = path.strip_prefix(root).unwrap_or(&path);
            let relative = relative.to_string_lossy().replace('\\', "/");
            if ignored_pattern(ignored, &relative) {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, output, ignored, depth + 1);
            } else {
                output.push(format!("@{relative}"));
            }
            if output.len() >= 2_000 {
                return;
            }
        }
    }
    let ignored = fs::read_to_string(root.join(".gitignore"))
        .map(|text| {
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('!'))
                .map(|line| {
                    line.trim_start_matches("./")
                        .trim_start_matches('/')
                        .trim_end_matches('/')
                        .to_string()
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut output = Vec::new();
    walk(root, root, &mut output, &ignored, 0);
    output.sort();
    output
}

fn ignored_pattern(patterns: &[String], relative: &str) -> bool {
    patterns.iter().any(|pattern| {
        if pattern.contains('/') {
            relative == pattern || relative.starts_with(&format!("{pattern}/"))
        } else if pattern.contains('*') {
            if pattern.starts_with('*') {
                relative.ends_with(pattern.trim_start_matches('*'))
            } else {
                relative.starts_with(pattern.trim_end_matches('*'))
            }
        } else {
            relative.split('/').any(|component| component == pattern)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("niucode-inline-{tag}-{stamp}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("src")).expect("temp dir");
        fs::write(path.join("src/main.rs"), "fn main() {}").expect("file");
        fs::create_dir_all(path.join("target")).expect("target");
        fs::write(path.join("target/ignored.rs"), "").expect("ignored");
        fs::write(path.join(".env"), "secret").expect("env");
        fs::write(path.join("ignored.rs"), "").expect("ignored file");
        fs::write(path.join(".gitignore"), "ignored.rs\n").expect("gitignore");
        path
    }

    #[test]
    fn file_completion_excludes_generated_directories() {
        let directory = temp_dir("files");
        let mut editor = InlineEditor::new(CommandRegistry::builtin());
        editor.set_cwd(directory.clone());
        let all = editor.candidates("@");
        assert!(all.iter().any(|item| item.name == "@src/main.rs"));
        assert!(!all.iter().any(|item| item.name.contains("target")));
        assert!(!all.iter().any(|item| item.name.contains(".env")));
        assert!(!all.iter().any(|item| item.name.contains("ignored.rs")));
        let filtered = editor.candidates("@src");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].insert, "@src/main.rs ");
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn vertical_cursor_preserves_columns() {
        let text = "abc\nde\nfghi".chars().collect::<Vec<_>>();
        let end_first = 3;
        let down = vertical_cursor(&text, end_first, 1);
        assert_eq!(down, 4 + 2);
        let down_again = vertical_cursor(&text, down, 1);
        assert_eq!(down_again, 7 + 2);
        assert_eq!(vertical_cursor(&text, down_again, 1), down_again);
        assert_eq!(vertical_cursor(&text, end_first, -1), end_first);
    }
}
