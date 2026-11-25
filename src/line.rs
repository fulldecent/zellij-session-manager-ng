//! One line of text with the editing keys shared by Find and Name.
//!
//! Esc and Ctrl+c clear a line that has text. On an empty line they cancel.

use zellij_tile::prelude::{BareKey, KeyModifier, KeyWithModifier};

const UNDO_LIMIT: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    text: String,
    cursor: usize,
    undo: Vec<(String, usize)>,
    redo: Vec<(String, usize)>,
    last_was_insert: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edit {
    Changed,
    Submit,
    Cancel,
    Complete,
    Ignored,
}

impl Default for Line {
    fn default() -> Self {
        Self::new()
    }
}

impl Line {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            undo: Vec::new(),
            redo: Vec::new(),
            last_was_insert: false,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn set(&mut self, text: String) {
        self.cursor = text.chars().count();
        self.text = text;
        self.undo.clear();
        self.redo.clear();
        self.last_was_insert = false;
    }

    pub fn clear(&mut self) {
        if self.text.is_empty() {
            return;
        }
        self.snapshot();
        self.text.clear();
        self.cursor = 0;
        self.last_was_insert = false;
    }

    pub fn handle(&mut self, key: &KeyWithModifier) -> Edit {
        if key.has_only_modifiers(&[KeyModifier::Ctrl, KeyModifier::Shift])
            && matches!(key.bare_key, BareKey::Char('z') | BareKey::Char('Z'))
        {
            self.redo();
            return Edit::Changed;
        }
        if key.has_only_modifiers(&[KeyModifier::Ctrl]) {
            return self.ctrl(key.bare_key);
        }
        if key.has_only_modifiers(&[KeyModifier::Alt]) {
            return self.alt(key.bare_key);
        }
        if !key.has_no_modifiers() {
            return Edit::Ignored;
        }
        match key.bare_key {
            BareKey::Enter => Edit::Submit,
            BareKey::Esc => self.cancel_or_clear(),
            BareKey::Tab => Edit::Complete,
            BareKey::Backspace => {
                self.backspace();
                Edit::Changed
            }
            BareKey::Delete => {
                self.delete();
                Edit::Changed
            }
            BareKey::Left => {
                self.move_left();
                Edit::Changed
            }
            BareKey::Right => {
                self.move_right();
                Edit::Changed
            }
            BareKey::Home => {
                self.cursor = 0;
                Edit::Changed
            }
            BareKey::End => {
                self.cursor = self.text.chars().count();
                Edit::Changed
            }
            BareKey::Char(ch) => {
                self.insert(ch);
                Edit::Changed
            }
            _ => Edit::Ignored,
        }
    }

    fn ctrl(&mut self, key: BareKey) -> Edit {
        match key {
            BareKey::Char('a') => {
                self.cursor = 0;
                Edit::Changed
            }
            BareKey::Char('e') => {
                self.cursor = self.text.chars().count();
                Edit::Changed
            }
            BareKey::Char('c') => self.cancel_or_clear(),
            BareKey::Char('z') => {
                self.undo();
                Edit::Changed
            }
            BareKey::Char('y') => {
                self.redo();
                Edit::Changed
            }
            BareKey::Left => {
                self.word_left();
                Edit::Changed
            }
            BareKey::Right => {
                self.word_right();
                Edit::Changed
            }
            BareKey::Backspace => {
                self.delete_word_left();
                Edit::Changed
            }
            BareKey::Delete => {
                self.delete_word_right();
                Edit::Changed
            }
            _ => Edit::Ignored,
        }
    }

    fn alt(&mut self, key: BareKey) -> Edit {
        match key {
            BareKey::Left => {
                self.word_left();
                Edit::Changed
            }
            BareKey::Right => {
                self.word_right();
                Edit::Changed
            }
            BareKey::Backspace => {
                self.delete_word_left();
                Edit::Changed
            }
            BareKey::Delete => {
                self.delete_word_right();
                Edit::Changed
            }
            _ => Edit::Ignored,
        }
    }

    fn cancel_or_clear(&mut self) -> Edit {
        if self.text.is_empty() {
            Edit::Cancel
        } else {
            self.clear();
            Edit::Changed
        }
    }

    fn insert(&mut self, ch: char) {
        if ch.is_control() {
            return;
        }
        if !self.last_was_insert {
            self.snapshot();
        }
        self.last_was_insert = true;
        let byte = self.byte_at(self.cursor);
        self.text.insert(byte, ch);
        self.cursor += 1;
        self.redo.clear();
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.snapshot();
        self.last_was_insert = false;
        let end = self.byte_at(self.cursor);
        self.cursor -= 1;
        let start = self.byte_at(self.cursor);
        self.text.replace_range(start..end, "");
    }

    fn delete(&mut self) {
        let count = self.text.chars().count();
        if self.cursor >= count {
            return;
        }
        self.snapshot();
        self.last_was_insert = false;
        let start = self.byte_at(self.cursor);
        let end = self.byte_at(self.cursor + 1);
        self.text.replace_range(start..end, "");
    }

    fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
        self.last_was_insert = false;
    }

    fn move_right(&mut self) {
        let count = self.text.chars().count();
        if self.cursor < count {
            self.cursor += 1;
        }
        self.last_was_insert = false;
    }

    fn word_left(&mut self) {
        let chars: Vec<char> = self.text.chars().collect();
        let mut i = self.cursor.min(chars.len());
        while i > 0 && chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !chars[i - 1].is_whitespace() {
            i -= 1;
        }
        self.cursor = i;
        self.last_was_insert = false;
    }

    fn word_right(&mut self) {
        let chars: Vec<char> = self.text.chars().collect();
        let mut i = self.cursor.min(chars.len());
        while i < chars.len() && !chars[i].is_whitespace() {
            i += 1;
        }
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        self.cursor = i;
        self.last_was_insert = false;
    }

    fn delete_word_left(&mut self) {
        let before = self.cursor;
        self.word_left();
        let start_chars = self.cursor;
        if start_chars == before {
            return;
        }
        self.cursor = before;
        self.snapshot();
        let start = self.byte_at(start_chars);
        let end = self.byte_at(before);
        self.text.replace_range(start..end, "");
        self.cursor = start_chars;
        self.last_was_insert = false;
    }

    fn delete_word_right(&mut self) {
        let before = self.cursor;
        self.word_right();
        let end_chars = self.cursor;
        if end_chars == before {
            return;
        }
        self.cursor = before;
        self.snapshot();
        let start = self.byte_at(before);
        let end = self.byte_at(end_chars);
        self.text.replace_range(start..end, "");
        self.last_was_insert = false;
    }

    fn snapshot(&mut self) {
        if self.undo.len() == UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.undo.push((self.text.clone(), self.cursor));
        self.redo.clear();
    }

    fn undo(&mut self) {
        if let Some((text, cursor)) = self.undo.pop() {
            self.redo.push((self.text.clone(), self.cursor));
            self.text = text;
            self.cursor = cursor.min(self.text.chars().count());
        }
        self.last_was_insert = false;
    }

    fn redo(&mut self) {
        if let Some((text, cursor)) = self.redo.pop() {
            self.undo.push((self.text.clone(), self.cursor));
            self.text = text;
            self.cursor = cursor.min(self.text.chars().count());
        }
        self.last_was_insert = false;
    }

    fn byte_at(&self, chars: usize) -> usize {
        self.text
            .char_indices()
            .nth(chars)
            .map(|(byte, _)| byte)
            .unwrap_or(self.text.len())
    }
}

#[cfg(test)]
pub fn key(bare: BareKey) -> KeyWithModifier {
    KeyWithModifier {
        bare_key: bare,
        key_modifiers: std::collections::BTreeSet::new(),
    }
}

#[cfg(test)]
pub fn key_with(bare: BareKey, modifiers: &[KeyModifier]) -> KeyWithModifier {
    KeyWithModifier {
        bare_key: bare,
        key_modifiers: modifiers
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_and_backspace_edit_the_line() {
        let mut line = Line::new();
        assert_eq!(line.handle(&key(BareKey::Char('a'))), Edit::Changed);
        assert_eq!(line.handle(&key(BareKey::Char('b'))), Edit::Changed);
        assert_eq!(line.text(), "ab");
        assert_eq!(line.handle(&key(BareKey::Backspace)), Edit::Changed);
        assert_eq!(line.text(), "a");
    }

    #[test]
    fn esc_clears_text_then_cancels() {
        let mut line = Line::new();
        line.handle(&key(BareKey::Char('a')));
        assert_eq!(line.handle(&key(BareKey::Esc)), Edit::Changed);
        assert!(line.text().is_empty());
        assert_eq!(line.handle(&key(BareKey::Esc)), Edit::Cancel);
    }

    #[test]
    fn ctrl_z_undoes_an_insert() {
        let mut line = Line::new();
        line.handle(&key(BareKey::Char('a')));
        line.handle(&key_with(BareKey::Char('z'), &[KeyModifier::Ctrl]));
        assert!(line.text().is_empty());
        line.handle(&key_with(BareKey::Char('y'), &[KeyModifier::Ctrl]));
        assert_eq!(line.text(), "a");
    }

    #[test]
    fn alt_w_is_left_for_the_page() {
        let mut line = Line::new();
        let edit = line.handle(&key_with(BareKey::Char('w'), &[KeyModifier::Alt]));
        assert_eq!(edit, Edit::Ignored);
    }
}
