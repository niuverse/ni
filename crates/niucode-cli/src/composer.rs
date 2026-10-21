//! Terminal-independent editing. Byte offsets are always grapheme boundaries.
use crate::text_layout::{self, Layout};
use std::collections::VecDeque;
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

const MAX_INPUT: usize = 1_048_576;
const UNDO_BYTES: usize = 8 * MAX_INPUT;
const HISTORY_BYTES: usize = 4 * MAX_INPUT;

#[derive(Clone, Default)]
struct Buffer {
    text: String,
    cursor: usize,
}

#[derive(Clone, Copy)]
pub enum Edit {
    Left,
    Right,
    WordLeft,
    WordRight,
    Home,
    End,
    Backspace,
    Delete,
    DeleteWord,
    KillStart,
    KillEnd,
    Undo,
    Redo,
}

#[derive(Default)]
pub struct Composer {
    buffer: Buffer,
    undo: VecDeque<Buffer>,
    redo: VecDeque<Buffer>,
    history: VecDeque<String>,
    history_index: Option<usize>,
    draft: Option<Buffer>,
    search: Option<String>,
    preferred_column: Option<usize>,
}

impl Composer {
    pub fn text(&self) -> &str {
        &self.buffer.text
    }
    pub fn cursor(&self) -> usize {
        self.buffer.cursor
    }
    pub fn clear_history(&mut self) {
        self.history.clear();
        self.history_index = None;
        self.draft = None;
        self.search = None;
    }
    pub fn clear(&mut self) {
        self.buffer = Buffer::default();
        self.undo.clear();
        self.redo.clear();
        self.history_index = None;
        self.draft = None;
        self.search = None;
        self.preferred_column = None;
    }
    pub fn submit(&mut self) -> String {
        let text = self.buffer.text.clone();
        if !text.trim().is_empty() && self.history.back() != Some(&text) {
            self.history.push_back(text.clone());
            while self.history.len() > 100
                || self.history.iter().map(String::len).sum::<usize>() > HISTORY_BYTES
            {
                self.history.pop_front();
            }
        }
        self.clear();
        text
    }
    pub fn insert(&mut self, text: &str) -> Result<(), &'static str> {
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        let normalized: String = normalized
            .chars()
            .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
            .collect();
        self.replace(self.cursor()..self.cursor(), &normalized)
    }
    pub fn replace(&mut self, range: Range<usize>, text: &str) -> Result<(), &'static str> {
        if !self.boundary(range.start) || !self.boundary(range.end) || range.start > range.end {
            return Err("replacement must end on character boundaries");
        }
        if self.text().len() - range.len() + text.len() > MAX_INPUT {
            return Err("input exceeds 1 MiB; draft preserved");
        }
        if &self.text()[range.clone()] == text {
            return Ok(());
        }
        self.checkpoint();
        let cursor = range.start + text.len();
        self.buffer.text.replace_range(range, text);
        // Insertion can join the following combining sequence into one grapheme.
        self.buffer.cursor = self
            .buffer
            .text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .find(|&i| i >= cursor)
            .unwrap_or(self.buffer.text.len());
        self.changed();
        Ok(())
    }
    fn boundary(&self, index: usize) -> bool {
        index == self.text().len() || self.text().grapheme_indices(true).any(|(i, _)| i == index)
    }
    fn checkpoint(&mut self) {
        self.undo.push_back(self.buffer.clone());
        trim_snapshots(&mut self.undo);
        self.redo.clear();
    }
    fn changed(&mut self) {
        self.search = None;
        self.preferred_column = None;
    }
    fn previous(&self) -> usize {
        self.text()[..self.cursor()]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(i, _)| i)
    }
    fn next(&self) -> usize {
        self.text()[self.cursor()..]
            .graphemes(true)
            .next()
            .map_or(self.cursor(), |s| self.cursor() + s.len())
    }
    fn word_left(&self) -> usize {
        let mut position = self.cursor();
        let mut seen_word = false;
        for (i, g) in self.text()[..position].grapheme_indices(true).rev() {
            let space = g.chars().all(char::is_whitespace);
            if seen_word && space {
                break;
            }
            seen_word |= !space;
            position = i;
        }
        position
    }
    fn word_right(&self) -> usize {
        let mut position = self.cursor();
        let mut seen_word = false;
        for g in self.text()[position..].graphemes(true) {
            let space = g.chars().all(char::is_whitespace);
            if seen_word && space {
                break;
            }
            seen_word |= !space;
            position += g.len();
        }
        position
    }
    pub fn edit(&mut self, action: Edit) {
        let cursor = self.cursor();
        let start = self.text()[..cursor].rfind('\n').map_or(0, |i| i + 1);
        let end = self.text()[cursor..]
            .find('\n')
            .map_or(self.text().len(), |i| cursor + i);
        let delete = match action {
            Edit::Left => {
                self.buffer.cursor = self.previous();
                None
            }
            Edit::Right => {
                self.buffer.cursor = self.next();
                None
            }
            Edit::WordLeft => {
                self.buffer.cursor = self.word_left();
                None
            }
            Edit::WordRight => {
                self.buffer.cursor = self.word_right();
                None
            }
            Edit::Home => {
                self.buffer.cursor = start;
                None
            }
            Edit::End => {
                self.buffer.cursor = end;
                None
            }
            Edit::Backspace => Some(self.previous()..cursor),
            Edit::Delete => Some(cursor..self.next()),
            Edit::DeleteWord => Some(self.word_left()..cursor),
            Edit::KillStart => Some(start..cursor),
            Edit::KillEnd => Some(cursor..end),
            Edit::Undo => {
                if let Some(previous) = self.undo.pop_back() {
                    self.redo
                        .push_back(std::mem::replace(&mut self.buffer, previous));
                    trim_snapshots(&mut self.redo);
                }
                None
            }
            Edit::Redo => {
                if let Some(next) = self.redo.pop_back() {
                    self.undo
                        .push_back(std::mem::replace(&mut self.buffer, next));
                    trim_snapshots(&mut self.undo);
                }
                None
            }
        };
        if let Some(range) = delete {
            self.replace(range, "")
                .expect("deletion within existing buffer");
        }
        self.changed();
    }
    /// Move by visual row, using exactly the same wrapping as the renderer.
    /// History is visited only when moving beyond the first/last visual row.
    pub fn vertical(&mut self, delta: isize, width: usize, prefix: &str) {
        let layout = self.layout(width, prefix);
        let column = *self.preferred_column.get_or_insert(layout.cursor.1);
        let target = layout.cursor.0.checked_add_signed(delta);
        if let Some(target) = target.filter(|&row| row < layout.rows.len()) {
            if let Some(&(offset, _, _)) = layout
                .positions
                .iter()
                .filter(|(_, row, _)| *row == target)
                .min_by_key(|(_, _, col)| col.abs_diff(column))
            {
                self.buffer.cursor = offset;
            }
        } else {
            self.history_move(delta);
        }
    }
    pub fn history_move(&mut self, delta: isize) {
        if self.history.is_empty() {
            return;
        }
        let index = self.history_index.unwrap_or(self.history.len());
        let next = index.saturating_add_signed(delta).min(self.history.len());
        if index == next {
            return;
        }
        if self.history_index.is_none() {
            self.draft = Some(self.buffer.clone());
        }
        self.checkpoint();
        if next == self.history.len() {
            self.buffer = self.draft.take().unwrap_or_default();
            self.history_index = None;
        } else {
            self.buffer = Buffer {
                text: self.history[next].clone(),
                cursor: self.history[next].len(),
            };
            self.history_index = Some(next);
        }
        self.changed();
    }
    pub fn search_history(&mut self) {
        let query = self
            .search
            .clone()
            .unwrap_or_else(|| self.text().to_string());
        let before = self.history_index.unwrap_or(self.history.len());
        if let Some(index) = (0..before)
            .rev()
            .find(|&i| self.history[i].contains(&query))
        {
            if self.history_index.is_none() {
                self.draft = Some(self.buffer.clone());
            }
            self.checkpoint();
            self.buffer = Buffer {
                text: self.history[index].clone(),
                cursor: self.history[index].len(),
            };
            self.history_index = Some(index);
        }
        self.search = Some(query);
        self.preferred_column = None;
    }
    pub fn layout(&self, width: usize, prefix: &str) -> Layout {
        text_layout::input(self.text(), self.cursor(), width, prefix)
    }
}

fn trim_snapshots(snapshots: &mut VecDeque<Buffer>) {
    while snapshots.len() > 100
        || snapshots.iter().map(|b| b.text.len()).sum::<usize>() > UNDO_BYTES
    {
        snapshots.pop_front();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;
    #[test]
    fn paste_and_undo_preserve_multiline_unicode() {
        let mut c = Composer::default();
        c.insert("草稿").unwrap();
        c.insert("\r\n👩‍💻e\u{301}\rnext\x1b").unwrap();
        assert_eq!(c.text(), "草稿\n👩‍💻e\u{301}\nnext");
        c.edit(Edit::Undo);
        assert_eq!(c.text(), "草稿");
        c.edit(Edit::Redo);
        assert!(c.text().contains("👩‍💻"));
        for _ in 0..5 {
            c.edit(Edit::Backspace);
        }
        c.edit(Edit::Backspace);
        assert_eq!(c.text(), "草稿\n👩‍💻");
        c.edit(Edit::Backspace);
        assert_eq!(c.text(), "草稿\n");
    }
    #[test]
    fn history_restores_draft_and_search_keeps_original_query() {
        let mut c = Composer::default();
        for s in ["fix one", "other", "fix two"] {
            c.insert(s).unwrap();
            c.submit();
        }
        c.insert("fix").unwrap();
        c.search_history();
        assert_eq!(c.text(), "fix two");
        c.search_history();
        assert_eq!(c.text(), "fix one");
        c.history_move(3);
        assert_eq!(c.text(), "fix");
    }
    #[test]
    fn visual_navigation_preserves_column_and_grapheme_boundaries() {
        let mut c = Composer::default();
        c.insert("abc\n短\nabcdef").unwrap();
        c.edit(Edit::Home);
        c.edit(Edit::Right);
        c.edit(Edit::Right);
        c.edit(Edit::Right);
        c.vertical(-1, 12, "");
        c.vertical(-1, 12, "");
        assert_eq!(c.cursor(), 3);
        for width in 1..20 {
            let l = c.layout(width, "niucode ▸ ");
            assert!(l
                .rows
                .iter()
                .all(|r| UnicodeWidthStr::width(r.as_str()) <= width));
            assert!(l.cursor.1 < width);
        }
    }
    #[test]
    fn rejected_large_paste_preserves_buffer() {
        let mut c = Composer::default();
        c.insert("keep").unwrap();
        assert!(c.insert(&"x".repeat(MAX_INPUT)).is_err());
        assert_eq!(c.text(), "keep");
    }
    #[test]
    fn deterministic_action_sequences_preserve_cursor_and_layout() {
        let mut c = Composer::default();
        let mut seed = 27_u64;
        for _ in 0..2000 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            match seed % 9 {
                0 => c.insert("中").unwrap(),
                1 => c.insert("👩‍💻").unwrap(),
                2 => c.insert("\n").unwrap(),
                3 => c.edit(Edit::Left),
                4 => c.edit(Edit::Right),
                5 => c.edit(Edit::Backspace),
                6 => c.edit(Edit::Undo),
                7 => c.edit(Edit::Redo),
                _ => c.vertical(-1, 8, "> "),
            }
            assert!(c.boundary(c.cursor()));
            let l = c.layout(8, "> ");
            assert!(l.cursor.0 < l.rows.len());
            assert!(l.cursor.1 < 8);
        }
    }
}
