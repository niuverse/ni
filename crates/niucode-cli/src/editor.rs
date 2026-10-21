//! One editor state for linear and inline presentations, idle and running tasks.
use crate::{
    completion::{Completion, CompletionAction},
    composer::{Composer, Edit},
    slash::CommandRegistry,
    terminal::TerminalSession,
    theme::Theme,
};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static MOUSE_EVENTS: AtomicU64 = AtomicU64::new(0);
pub fn mouse_events_seen() -> u64 {
    MOUSE_EVENTS.load(Ordering::Relaxed)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Line(String),
    Queue(String),
    Details,
    Interrupted,
    Eof,
}

pub struct LineEditor {
    pub composer: Composer,
    completion: Completion,
    inline: bool,
    selected: usize,
    menu_focused: bool,
    menu_open: bool,
    menu_origin: Option<(u16, usize, usize)>,
    notice: Option<String>,
    clear_requested: bool,
}
impl LineEditor {
    pub fn new(registry: CommandRegistry, inline: bool) -> Self {
        Self {
            composer: Composer::default(),
            completion: Completion::new(registry),
            inline,
            selected: 0,
            menu_focused: false,
            menu_open: true,
            menu_origin: None,
            notice: None,
            clear_requested: false,
        }
    }
    pub fn set_models(&mut self, models: Vec<String>) {
        self.completion.models = models;
    }
    pub fn set_current_model(&mut self, model: impl Into<String>) {
        self.completion.current_model = model.into();
    }
    pub fn set_permissions(&mut self, permissions: Vec<(String, String)>) {
        self.completion.permissions = permissions;
    }
    pub fn set_skills(&mut self, skills: Vec<String>) {
        self.completion.skills = skills;
    }
    pub fn set_cwd(&mut self, cwd: PathBuf) {
        self.completion.set_cwd(cwd);
    }
    pub fn clear_history(&mut self) {
        self.composer.clear_history();
        self.composer.clear();
    }
    pub fn start(&self, theme: Theme) -> std::io::Result<TerminalSession> {
        TerminalSession::enter(self.inline, theme.is_ansi())
    }
    pub fn read_line(
        &mut self,
        prompt: &str,
        footer: &str,
        theme: Theme,
    ) -> std::io::Result<Input> {
        let mut terminal = self.start(theme)?;
        loop {
            self.draw(&mut terminal, prompt, footer, "")?;
            if let Some(event) = terminal.poll(Duration::from_millis(50))? {
                if let Some(input) = self.handle(event, terminal.size().0, prompt) {
                    match &input {
                        Input::Interrupted => self.composer.clear(),
                        Input::Line(text) | Input::Queue(text) => {
                            terminal.print(&format!("{prompt}{text}"))?
                        }
                        _ => {}
                    }
                    terminal.finish()?;
                    return Ok(input);
                }
            }
        }
    }
    pub fn draw(
        &mut self,
        terminal: &mut TerminalSession,
        prompt: &str,
        footer: &str,
        output: &str,
    ) -> std::io::Result<()> {
        if std::mem::take(&mut self.clear_requested) {
            terminal.clear_screen()?;
        }
        self.completion.refresh();
        let (width, height) = terminal.size();
        let candidates = self
            .completion
            .candidates(self.composer.text(), self.composer.cursor());
        self.selected = self.selected.min(candidates.len().saturating_sub(1));
        let menu_rows = if self.menu_open {
            candidates.len().min(4).min(height.saturating_sub(3))
        } else {
            0
        };
        let mut rows = Vec::new();
        if !output.is_empty() {
            let count = 3.min(height.saturating_sub(menu_rows + 3));
            rows.extend(
                crate::text_layout::wrap(output, width)
                    .into_iter()
                    .take(count),
            );
        }
        let output_rows = rows.len();
        let input_rows = height
            .saturating_sub(menu_rows + output_rows + 1)
            .clamp(1, 6);
        let layout = self.composer.layout(width, prompt);
        let first = layout.cursor.0.saturating_sub(input_rows - 1);
        rows.extend(layout.rows.iter().skip(first).take(input_rows).cloned());
        let menu_start = rows.len();
        let menu_offset = self.selected.saturating_sub(menu_rows.saturating_sub(1));
        for (i, item) in candidates
            .iter()
            .enumerate()
            .skip(menu_offset)
            .take(menu_rows)
        {
            rows.push(format!(
                "{} {}  {}",
                if i == self.selected { "▸" } else { " " },
                item.name,
                item.description
            ));
        }
        if rows.len() < height {
            let notice = self
                .notice
                .as_deref()
                .or(terminal.notice.as_deref())
                .or(self.completion.notice.as_deref());
            let hint = if menu_rows > 0 {
                if candidates[self.selected].action == CompletionAction::Apply {
                    if self.menu_focused
                        || !matches!(self.composer.text().trim_end(), "/model" | "/permissions")
                    {
                        "↑↓ select · Enter apply · Esc cancel"
                    } else {
                        "↑↓ select · Enter open · Tab complete"
                    }
                } else {
                    "↑↓ select · Enter/Tab complete · Esc close"
                }
            } else {
                footer
            };
            rows.push(notice.unwrap_or(hint).into());
        }
        let selected = (menu_rows > 0).then_some(menu_start + self.selected - menu_offset);
        let top = terminal.draw(
            &rows,
            (!self.menu_focused || menu_rows == 0)
                .then_some((output_rows + layout.cursor.0 - first, layout.cursor.1)),
            selected,
            menu_rows > 0,
        )?;
        self.menu_origin = top
            .filter(|_| menu_rows > 0)
            .map(|top| (top + menu_start as u16, menu_offset, menu_rows));
        Ok(())
    }
    pub fn handle(&mut self, event: Event, width: usize, prompt: &str) -> Option<Input> {
        let candidates = self
            .completion
            .candidates(self.composer.text(), self.composer.cursor());
        let menu = self.menu_open && !candidates.is_empty();
        match event {
            Event::Paste(text) => {
                self.insert(&text);
                self.menu_open = false;
            }
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                let alt = key.modifiers.contains(KeyModifiers::ALT);
                let shift = key.modifiers.contains(KeyModifiers::SHIFT);
                if ctrl
                    || !matches!(
                        key.code,
                        KeyCode::Enter | KeyCode::Tab | KeyCode::Up | KeyCode::Down | KeyCode::Esc
                    )
                {
                    self.menu_focused = false;
                }
                if ctrl {
                    match key.code {
                        KeyCode::Char('c') => return Some(Input::Interrupted),
                        KeyCode::Char('d') if self.composer.text().is_empty() => {
                            return Some(Input::Eof)
                        }
                        KeyCode::Char('d') => self.composer.edit(Edit::Delete),
                        KeyCode::Char('l') => self.clear_requested = true,
                        KeyCode::Char('j') => self.insert("\n"),
                        KeyCode::Char('a') => self.composer.edit(Edit::Home),
                        KeyCode::Char('e') => self.composer.edit(Edit::End),
                        KeyCode::Char('u') => self.composer.edit(Edit::KillStart),
                        KeyCode::Char('k') => self.composer.edit(Edit::KillEnd),
                        KeyCode::Char('w') => self.composer.edit(Edit::DeleteWord),
                        KeyCode::Char('z') => self.composer.edit(Edit::Undo),
                        KeyCode::Char('y') => self.composer.edit(Edit::Redo),
                        KeyCode::Char('r') => self.composer.search_history(),
                        KeyCode::Char('p') => self.composer.history_move(-1),
                        KeyCode::Char('n') => self.composer.history_move(1),
                        KeyCode::Char('o') => return Some(Input::Details),
                        KeyCode::Left => self.composer.edit(Edit::WordLeft),
                        KeyCode::Right => self.composer.edit(Edit::WordRight),
                        _ => {}
                    }
                    return None;
                }
                match key.code {
                    KeyCode::Enter if shift => self.insert("\n"),
                    KeyCode::Enter if alt => {
                        if !self.composer.text().trim().is_empty() {
                            return Some(Input::Queue(self.take()));
                        }
                    }
                    KeyCode::Enter => {
                        let line = self.composer.text();
                        let exact = candidates
                            .iter()
                            .any(|c| c.insert.trim_end() == line[c.range.clone()].trim_end());
                        if !candidates.is_empty()
                            && matches!(line.trim_end(), "/model" | "/permissions")
                            && !self.menu_focused
                        {
                            self.open_picker();
                        } else if menu && (self.menu_focused || !exact) {
                            return self.accept(true);
                        } else if !line.trim().is_empty() {
                            return Some(Input::Line(self.take()));
                        }
                    }
                    KeyCode::Char('b') if alt => self.composer.edit(Edit::WordLeft),
                    KeyCode::Char('f') if alt => self.composer.edit(Edit::WordRight),
                    KeyCode::Char(c) if !alt && !c.is_control() => {
                        self.insert(&c.to_string());
                        self.menu_open = true;
                    }
                    KeyCode::Char('\n') => self.insert("\n"),
                    KeyCode::Tab => {
                        self.menu_open = true;
                        self.accept(false);
                    }
                    KeyCode::Esc => {
                        if self.menu_focused
                            && candidates
                                .iter()
                                .any(|c| c.action == CompletionAction::Apply)
                        {
                            self.composer.clear();
                        }
                        self.menu_open = false;
                        self.menu_focused = false;
                    }
                    KeyCode::Up if menu => {
                        self.selected = self.selected.saturating_sub(1);
                        self.menu_focused = true;
                    }
                    KeyCode::Down if menu => {
                        self.selected = (self.selected + 1).min(candidates.len() - 1);
                        self.menu_focused = true;
                    }
                    KeyCode::Up => self.composer.vertical(-1, width, prompt),
                    KeyCode::Down => self.composer.vertical(1, width, prompt),
                    KeyCode::Left => self.composer.edit(Edit::Left),
                    KeyCode::Right => self.composer.edit(Edit::Right),
                    KeyCode::Home => self.composer.edit(Edit::Home),
                    KeyCode::End => self.composer.edit(Edit::End),
                    KeyCode::Backspace => {
                        self.composer.edit(Edit::Backspace);
                        self.menu_open = true;
                    }
                    KeyCode::Delete => {
                        self.composer.edit(Edit::Delete);
                        self.menu_open = true;
                    }
                    _ => {}
                }
            }
            Event::Mouse(mouse) if menu => {
                MOUSE_EVENTS.fetch_add(1, Ordering::Relaxed);
                if let Some((top, offset, rows)) = self.menu_origin {
                    if mouse.row >= top && usize::from(mouse.row - top) < rows {
                        let index = offset + usize::from(mouse.row - top);
                        match mouse.kind {
                            MouseEventKind::Moved => {
                                self.selected = index;
                                self.menu_focused = true;
                            }
                            MouseEventKind::Down(MouseButton::Left) => {
                                self.selected = index;
                                return self.accept(true);
                            }
                            MouseEventKind::ScrollUp => {
                                self.selected = self.selected.saturating_sub(1);
                                self.menu_focused = true;
                            }
                            MouseEventKind::ScrollDown => {
                                self.selected = (self.selected + 1).min(candidates.len() - 1);
                                self.menu_focused = true;
                            }
                            _ => {}
                        }
                    }
                }
            }
            Event::Resize(_, _) => self.menu_origin = None,
            _ => {}
        }
        None
    }
    fn take(&mut self) -> String {
        self.menu_open = true;
        self.selected = 0;
        self.menu_focused = false;
        self.notice = None;
        self.composer.submit()
    }
    fn insert(&mut self, text: &str) {
        self.notice = self.composer.insert(text).err().map(str::to_string);
        self.selected = 0;
        self.menu_focused = false;
    }
    fn open_picker(&mut self) {
        self.menu_open = true;
        self.menu_focused = true;
        let items = self
            .completion
            .candidates(self.composer.text(), self.composer.cursor());
        self.selected = items
            .iter()
            .position(|c| c.name == self.completion.current_model)
            .unwrap_or(0);
    }
    /// Tab inserts text; Enter/click confirms settings immediately. Other
    /// completions never execute a command as a side effect of accepting text.
    fn accept(&mut self, confirm: bool) -> Option<Input> {
        let items = self
            .completion
            .candidates(self.composer.text(), self.composer.cursor());
        let item = items.get(self.selected)?;
        if let Err(error) = self.composer.replace(item.range.clone(), &item.insert) {
            self.notice = Some(error.into());
            return None;
        }
        self.notice = None;
        self.menu_open = false;
        self.menu_focused = false;
        self.selected = 0;
        match item.action {
            CompletionAction::OpenPicker => self.open_picker(),
            CompletionAction::Apply if confirm => return Some(Input::Line(self.take())),
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;
    fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, modifiers))
    }
    #[test]
    fn both_presentations_share_paste_newline_and_submission() {
        for inline in [false, true] {
            let mut e = LineEditor::new(CommandRegistry::builtin(), inline);
            assert!(e
                .handle(Event::Paste("中文\r\n/exit".into()), 20, "> ")
                .is_none());
            e.handle(key(KeyCode::Char('j'), KeyModifiers::CONTROL), 20, "> ");
            assert_eq!(
                e.handle(key(KeyCode::Enter, KeyModifiers::NONE), 20, "> "),
                Some(Input::Line("中文\n/exit\n".into()))
            );
        }
    }
    #[test]
    fn escape_closes_menu_and_cancel_preserves_running_draft() {
        let mut e = LineEditor::new(CommandRegistry::builtin(), false);
        e.composer.insert("/i").unwrap();
        e.handle(key(KeyCode::Esc, KeyModifiers::NONE), 20, "> ");
        assert!(!e.menu_open);
        assert_eq!(
            e.handle(key(KeyCode::Char('c'), KeyModifiers::CONTROL), 20, "> "),
            Some(Input::Interrupted)
        );
        assert_eq!(e.composer.text(), "/i");
    }

    fn settings_editor(inline: bool, command: &str) -> LineEditor {
        let mut editor = LineEditor::new(CommandRegistry::builtin(), inline);
        editor.set_models(vec!["provider/first".into(), "provider/second".into()]);
        editor.set_current_model("provider/first");
        editor.set_permissions(vec![
            ("ask".into(), "ask before writes".into()),
            ("review".into(), "review changes".into()),
        ]);
        editor.composer.insert(command).unwrap();
        editor
    }
    fn press(editor: &mut LineEditor, code: KeyCode) -> Option<Input> {
        editor.handle(key(code, KeyModifiers::NONE), 80, "> ")
    }
    #[test]
    fn settings_open_then_apply_with_one_confirmation_in_both_presentations() {
        for inline in [false, true] {
            for (command, value) in [("/model", "provider/second"), ("/permissions", "review")] {
                let mut editor = settings_editor(inline, command);
                assert_eq!(press(&mut editor, KeyCode::Enter), None);
                assert!(editor.menu_focused);
                assert_eq!(press(&mut editor, KeyCode::Down), None);
                assert_eq!(
                    press(&mut editor, KeyCode::Enter),
                    Some(Input::Line(format!("{command} {value} ")))
                );
                assert!(editor.composer.text().is_empty());
                assert!(!editor.menu_focused);
            }
        }
    }
    #[test]
    fn settings_can_be_selected_directly_from_preview_or_clicked() {
        for command in ["/model", "/permissions"] {
            let mut editor = settings_editor(true, command);
            press(&mut editor, KeyCode::Down);
            let keyboard = press(&mut editor, KeyCode::Enter);
            let mut editor = settings_editor(true, command);
            editor.menu_origin = Some((10, 0, 2));
            let clicked = editor.handle(
                Event::Mouse(crossterm::event::MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column: 4,
                    row: 11,
                    modifiers: KeyModifiers::NONE,
                }),
                80,
                "> ",
            );
            assert_eq!(clicked, keyboard);
            assert!(matches!(clicked, Some(Input::Line(_))));
        }
    }
    #[test]
    fn slash_completion_opens_picker_and_escape_cancels_without_submission() {
        for prefix in ["/mod", "/perm"] {
            let mut editor = settings_editor(false, prefix);
            assert_eq!(press(&mut editor, KeyCode::Enter), None);
            assert!(editor.menu_open && editor.menu_focused);
            assert_eq!(press(&mut editor, KeyCode::Esc), None);
            assert!(editor.composer.text().is_empty());
            assert_eq!(press(&mut editor, KeyCode::Enter), None);
        }
    }
    #[test]
    fn tab_only_fills_arguments_and_typing_returns_focus_to_input() {
        let mut editor = settings_editor(false, "/model");
        press(&mut editor, KeyCode::Down);
        assert_eq!(press(&mut editor, KeyCode::Tab), None);
        assert_eq!(editor.composer.text(), "/model provider/second ");
        assert!(!editor.menu_focused);
        assert_eq!(
            press(&mut editor, KeyCode::Enter),
            Some(Input::Line("/model provider/second ".into()))
        );
        editor.composer.insert("/permissions").unwrap();
        press(&mut editor, KeyCode::Enter);
        press(&mut editor, KeyCode::Char(' '));
        assert!(!editor.menu_focused);
        press(&mut editor, KeyCode::Char('r'));
        assert_eq!(
            press(&mut editor, KeyCode::Enter),
            Some(Input::Line("/permissions review ".into()))
        );
    }
    #[test]
    fn pasted_settings_command_opens_picker_without_applying() {
        let mut editor = settings_editor(false, "");
        editor.handle(Event::Paste("/permissions".into()), 80, "> ");
        assert!(!editor.menu_open);
        assert_eq!(press(&mut editor, KeyCode::Enter), None);
        assert!(editor.menu_open && editor.menu_focused);
        assert_eq!(press(&mut editor, KeyCode::Esc), None);
        assert!(editor.composer.text().is_empty());
    }
    #[test]
    fn picker_starts_at_current_model_and_ordinary_completion_does_not_execute() {
        let mut editor = settings_editor(false, "/model");
        editor.set_current_model("provider/second");
        press(&mut editor, KeyCode::Enter);
        assert_eq!(
            press(&mut editor, KeyCode::Enter),
            Some(Input::Line("/model provider/second ".into()))
        );
        editor.composer.insert("/ex").unwrap();
        assert_eq!(press(&mut editor, KeyCode::Enter), None);
        assert_eq!(editor.composer.text(), "/exit ");
    }
}
