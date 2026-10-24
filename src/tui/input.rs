//! A tiny single-line text editor for the prompt modal.

#[derive(Debug, Clone, Default)]
pub struct TextInput {
    value: String,
    /// Cursor position in characters, not bytes.
    cursor: usize,
}

impl TextInput {
    pub fn new(value: &str) -> Self {
        let value = sanitize(value);
        let cursor = value.chars().count();
        Self { value, cursor }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    fn byte_at(&self, char_index: usize) -> usize {
        self.value
            .char_indices()
            .nth(char_index)
            .map(|(byte, _)| byte)
            .unwrap_or(self.value.len())
    }

    pub fn insert(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        let byte = self.byte_at(self.cursor);
        self.value.insert(byte, c);
        self.cursor += 1;
    }

    pub fn insert_str(&mut self, text: &str) {
        let clean = sanitize(text);
        if clean.is_empty() {
            return;
        }
        let byte = self.byte_at(self.cursor);
        self.value.insert_str(byte, &clean);
        self.cursor += clean.chars().count();
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let start = self.byte_at(self.cursor - 1);
        let end = self.byte_at(self.cursor);
        self.value.replace_range(start..end, "");
        self.cursor -= 1;
    }

    pub fn delete(&mut self) {
        if self.cursor >= self.value.chars().count() {
            return;
        }
        let start = self.byte_at(self.cursor);
        let end = self.byte_at(self.cursor + 1);
        self.value.replace_range(start..end, "");
    }

    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn right(&mut self) {
        if self.cursor < self.value.chars().count() {
            self.cursor += 1;
        }
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.value.chars().count();
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    /// Delete the word before the cursor (Ctrl+W).
    pub fn delete_word(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let chars: Vec<char> = self.value.chars().collect();
        let mut start = self.cursor;
        while start > 0 && chars[start - 1].is_whitespace() {
            start -= 1;
        }
        while start > 0 && !chars[start - 1].is_whitespace() {
            start -= 1;
        }
        let byte_start = self.byte_at(start);
        let byte_end = self.byte_at(self.cursor);
        self.value.replace_range(byte_start..byte_end, "");
        self.cursor = start;
    }

    /// Characters for rendering, windowed so the cursor stays visible.
    pub fn view(&self, width: usize) -> (String, char, String) {
        let chars: Vec<char> = self.value.chars().collect();
        let width = width.max(1);
        let mut start = 0;
        if self.cursor + 1 > width {
            start = self.cursor + 1 - width;
        }
        let visible_end = (start + width).min(chars.len());
        let before: String = chars[start..self.cursor.min(chars.len())].iter().collect();
        let at = chars.get(self.cursor).copied().unwrap_or(' ');
        let after_start = (self.cursor + 1).min(chars.len()).max(start);
        let after: String = chars[after_start..visible_end.max(after_start)]
            .iter()
            .collect();
        (before, at, after)
    }
}

fn sanitize(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\n' | '\t' | '\r' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect::<String>()
        .trim_start()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_at_the_cursor() {
        let mut input = TextInput::new("abc");
        input.left();
        input.insert('X');
        assert_eq!(input.value(), "abXc");
        input.backspace();
        assert_eq!(input.value(), "abc");
        input.home();
        input.delete();
        assert_eq!(input.value(), "bc");
    }

    #[test]
    fn word_deletion_and_views() {
        let mut input = TextInput::new("one two three");
        input.delete_word();
        assert_eq!(input.value(), "one two ");
        input.home();
        input.right();
        input.right();
        input.right();
        let (before, at, after) = input.view(4);
        assert_eq!(before, "one");
        assert_eq!(at, ' ');
        assert_eq!(after, "");
    }
}
