//! Small Vim keymap for the built-in editor; saves still use the viewer's conflict prompts.
use super::*;

#[derive(Clone)]
pub(super) struct Snapshot {
    pub lines: Vec<String>,
    cursor: TextPos,
}

#[derive(Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Normal,
    Insert,
    Visual,
}

#[derive(Default)]
pub(super) struct Vim {
    mode: Mode,
    command: Option<String>,
    pending: Option<char>,
    count: usize,
    register: String,
    linewise: bool,
    pub undo: Vec<Snapshot>,
    pub redo: Vec<Snapshot>,
    pub restoring: bool,
}

impl Vim {
    pub fn label(&self) -> String {
        if let Some(command) = &self.command {
            return format!(":{command}");
        }
        match self.mode {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Visual => "VISUAL",
        }
        .into()
    }
}

impl Editor {
    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot {
            lines: self.lines.clone(),
            cursor: self.cursor,
        }
    }

    fn restore_vim(&mut self, redo: bool) {
        let snapshot = if redo {
            self.vim.redo.pop()
        } else {
            self.vim.undo.pop()
        };
        if let Some(snapshot) = snapshot {
            let current = self.snapshot();
            if redo {
                self.vim.undo.push(current);
            } else {
                self.vim.redo.push(current);
            }
            self.lines = snapshot.lines;
            self.cursor = snapshot.cursor;
            self.mark_changed();
            self.dirty = self.encoded_bytes() != self.original_bytes;
            self.vim.restoring = true;
        }
    }

    fn normal_cursor(&mut self) {
        self.cursor.col = self
            .cursor
            .col
            .min(char_len(&self.lines[self.cursor.line]).saturating_sub(1));
        self.preferred_x = None;
    }

    fn vim_word(&mut self, backwards: bool) {
        // Treat punctuation as a separate word, as Vim's w/b do.
        fn class(c: char) -> u8 {
            if c.is_whitespace() {
                0
            } else if c.is_alphanumeric() || c == '_' {
                1
            } else {
                2
            }
        }
        let text: Vec<char> = self.text().chars().collect();
        let base: usize = self.lines[..self.cursor.line]
            .iter()
            .map(|l| char_len(l) + 1)
            .sum();
        let mut at = base + self.cursor.col;
        if backwards {
            at = at.saturating_sub(1);
            while at > 0 && class(text[at]) == 0 {
                at -= 1;
            }
            if let Some(&ch) = text.get(at) {
                let kind = class(ch);
                while at > 0 && class(text[at - 1]) == kind {
                    at -= 1;
                }
            }
        } else if let Some(&ch) = text.get(at) {
            let kind = class(ch);
            while at < text.len() && class(text[at]) == kind {
                at += 1;
            }
            while at < text.len() && class(text[at]) == 0 {
                at += 1;
            }
        }
        let mut remaining = at;
        for (line, text) in self.lines.iter().enumerate() {
            let len = char_len(text);
            if remaining <= len {
                self.cursor = TextPos {
                    line,
                    col: remaining,
                };
                break;
            }
            remaining -= len + 1;
        }
        self.preferred_x = None;
    }

    fn vim_lines(&mut self, op: char, count: usize) {
        let start = self.cursor.line;
        let end = (start + count).min(self.lines.len());
        self.vim.register = self.lines[start..end].join("\n");
        self.vim.linewise = true;
        if op != 'y' {
            self.lines.drain(start..end);
            if op == 'c' || self.lines.is_empty() {
                self.lines
                    .insert(start.min(self.lines.len()), String::new());
            }
            self.cursor.line = start.min(self.lines.len() - 1);
            self.cursor.col = 0;
            self.mark_changed();
            if op == 'c' {
                self.vim.mode = Mode::Insert;
            }
        }
    }

    fn vim_range(&mut self, op: char, start: TextPos, end: TextPos) {
        self.anchor = Some(start);
        self.cursor = end;
        self.vim.register = self.selected_text().unwrap_or_default();
        self.vim.linewise = false;
        if op != 'y' {
            if self.delete_selection() {
                self.mark_changed();
            }
        } else {
            self.cursor = start.min(end);
            self.anchor = None;
        }
        self.vim.mode = if op == 'c' {
            Mode::Insert
        } else {
            Mode::Normal
        };
        if op != 'c' {
            self.normal_cursor();
        }
    }

    pub(super) fn on_vim_key(&mut self, key: KeyEvent) -> Option<EditAction> {
        let shortcut = (key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT))
            || key.modifiers.contains(KeyModifiers::SUPER);
        if let Some(mut command) = self.vim.command.take() {
            match key.code {
                KeyCode::Esc => {}
                KeyCode::Enter => {
                    return Some(match command.trim() {
                        "w" => EditAction::Save,
                        "q" => EditAction::Close,
                        "wq" | "x" => EditAction::SaveAndClose,
                        "preview" => EditAction::Leave,
                        _ => {
                            self.set_status("Supported commands: :w :q :wq :preview");
                            EditAction::None
                        }
                    });
                }
                KeyCode::Backspace => {
                    command.pop();
                    self.vim.command = Some(command);
                }
                KeyCode::Char(c) if !shortcut => {
                    command.push(c);
                    self.vim.command = Some(command);
                }
                _ => self.vim.command = Some(command),
            }
            return Some(EditAction::None);
        }
        if key.code == KeyCode::Esc {
            if self.vim.mode == Mode::Insert {
                self.cursor.col = self.cursor.col.saturating_sub(1);
            }
            self.vim.mode = Mode::Normal;
            self.vim.pending = None;
            self.vim.count = 0;
            self.anchor = None;
            self.status = None;
            self.normal_cursor();
            return Some(EditAction::None);
        }
        if shortcut {
            if key.code == KeyCode::Char('r') && self.vim.mode != Mode::Insert {
                self.restore_vim(true);
                return Some(EditAction::None);
            }
            return None; // Keep existing save, find and clipboard shortcuts.
        }
        if self.vim.mode == Mode::Insert {
            return None;
        }
        self.status = None;
        let c = match key.code {
            KeyCode::Char(c) => c,
            KeyCode::Left => 'h',
            KeyCode::Right => 'l',
            KeyCode::Up => 'k',
            KeyCode::Down => 'j',
            KeyCode::Home => '0',
            KeyCode::End => '$',
            _ => return Some(EditAction::None),
        };
        if c.is_ascii_digit() && (c != '0' || self.vim.count > 0) {
            self.vim.count = (self.vim.count * 10 + c.to_digit(10).unwrap() as usize).min(5000);
            return Some(EditAction::None);
        }
        let count = std::mem::take(&mut self.vim.count).max(1);
        if let Some(op) = self.vim.pending.take() {
            if op == 'g' {
                if c == 'g' {
                    self.cursor = TextPos {
                        line: (count - 1).min(self.lines.len() - 1),
                        col: 0,
                    };
                }
            } else if op == c {
                self.vim_lines(op, count);
            } else if matches!(c, 'w' | 'b' | '$' | '0') {
                let start = self.cursor;
                match c {
                    'w' | 'b' => {
                        for _ in 0..count {
                            self.vim_word(c == 'b');
                        }
                    }
                    '$' => self.cursor.col = char_len(&self.lines[self.cursor.line]),
                    '0' => self.cursor.col = 0,
                    _ => unreachable!(),
                }
                self.vim_range(op, start, self.cursor);
            }
            self.normal_cursor_if_needed();
            return Some(EditAction::None);
        }
        let visual = self.vim.mode == Mode::Visual;
        if !visual {
            self.anchor = None;
        }
        match c {
            'h' => self.cursor.col = self.cursor.col.saturating_sub(count),
            'l' => self.cursor.col = self.cursor.col.saturating_add(count),
            'j' => self.cursor.line = (self.cursor.line + count).min(self.lines.len() - 1),
            'k' => self.cursor.line = self.cursor.line.saturating_sub(count),
            'w' | 'b' => {
                for _ in 0..count {
                    self.vim_word(c == 'b');
                }
            }
            '0' => self.cursor.col = 0,
            '^' => {
                self.cursor.col = self.lines[self.cursor.line]
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .count()
            }
            '$' => self.cursor.col = char_len(&self.lines[self.cursor.line]).saturating_sub(1),
            'G' => self.cursor.line = self.lines.len() - 1,
            'g' => self.vim.pending = Some('g'),
            'i' => self.vim.mode = Mode::Insert,
            'a' => {
                self.cursor.col =
                    (self.cursor.col + 1).min(char_len(&self.lines[self.cursor.line]));
                self.vim.mode = Mode::Insert;
            }
            'I' => {
                self.cursor.col = 0;
                self.vim.mode = Mode::Insert;
            }
            'A' => {
                self.cursor.col = char_len(&self.lines[self.cursor.line]);
                self.vim.mode = Mode::Insert;
            }
            'o' | 'O' => {
                let line = self.cursor.line + usize::from(c == 'o');
                self.lines.insert(line, String::new());
                self.cursor = TextPos { line, col: 0 };
                self.mark_changed();
                self.vim.mode = Mode::Insert;
            }
            'v' => {
                self.vim.mode = if visual { Mode::Normal } else { Mode::Visual };
                self.anchor = if visual { None } else { Some(self.cursor) };
            }
            'd' | 'c' | 'y' | 'x' if visual => {
                let anchor = self.anchor.unwrap_or(self.cursor);
                let start = anchor.min(self.cursor);
                let mut end = anchor.max(self.cursor);
                end.col = (end.col + 1).min(char_len(&self.lines[end.line]));
                self.vim_range(if c == 'x' { 'd' } else { c }, start, end);
            }
            'd' | 'c' | 'y' => {
                self.vim.pending = Some(c);
                self.vim.count = count;
            }
            'x' => {
                let start = self.cursor;
                let end = TextPos {
                    line: start.line,
                    col: (start.col + count).min(char_len(&self.lines[start.line])),
                };
                self.vim_range('d', start, end);
            }
            'D' | 'C' => {
                let start = self.cursor;
                let end = TextPos {
                    line: start.line,
                    col: char_len(&self.lines[start.line]),
                };
                self.vim_range(if c == 'C' { 'c' } else { 'd' }, start, end);
            }
            'p' | 'P' if !self.vim.register.is_empty() => {
                let text = self.vim.register.clone();
                if self.vim.linewise {
                    let at = self.cursor.line + usize::from(c == 'p');
                    let lines: Vec<String> = text.split('\n').map(str::to_string).collect();
                    self.lines.splice(at..at, lines);
                    self.cursor = TextPos { line: at, col: 0 };
                    self.mark_changed();
                } else {
                    if c == 'p' {
                        self.cursor.col =
                            (self.cursor.col + 1).min(char_len(&self.lines[self.cursor.line]));
                    }
                    self.insert_text(&text);
                    self.cursor.col = self.cursor.col.saturating_sub(1);
                }
            }
            'u' => self.restore_vim(false),
            ':' => self.vim.command = Some(String::new()),
            '/' => self.search = Some(Search::default()),
            _ => {}
        }
        self.normal_cursor_if_needed();
        Some(EditAction::None)
    }

    fn normal_cursor_if_needed(&mut self) {
        if self.vim.mode != Mode::Insert {
            self.normal_cursor();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn editor(text: &str) -> Editor {
        Editor::from_bytes(
            PathBuf::from("test.txt"),
            text.as_bytes().to_vec(),
            1_000_000,
            5000,
        )
        .unwrap()
    }
    fn keys(e: &mut Editor, text: &str) {
        for c in text.chars() {
            e.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE), 80, 20);
        }
    }
    fn esc(e: &mut Editor) {
        assert_eq!(
            e.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), 80, 20),
            EditAction::None
        );
    }

    #[test]
    fn normal_keys_never_insert_and_escape_stays_in_editor() {
        let mut e = editor("héllo\nworld");
        keys(&mut e, "ljkh");
        assert_eq!(e.text(), "héllo\nworld");
        keys(&mut e, "i🙂");
        esc(&mut e);
        assert_eq!(e.text(), "🙂héllo\nworld");
        assert_eq!(e.vim.label(), "NORMAL");
        assert_eq!(e.cursor.col, 0);
    }
    #[test]
    fn delete_yank_paste_undo_redo() {
        let mut e = editor("one\ntwo\nthree");
        keys(&mut e, "dd");
        assert_eq!(e.text(), "two\nthree");
        keys(&mut e, "p");
        assert_eq!(e.text(), "two\none\nthree");
        keys(&mut e, "u");
        assert_eq!(e.text(), "two\nthree");
        e.on_key(
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
            80,
            20,
        );
        assert_eq!(e.text(), "two\none\nthree");
        keys(&mut e, "yyP");
        assert_eq!(e.text(), "two\none\none\nthree");
    }
    #[test]
    fn counted_deletion_and_unicode_word_motion() {
        let mut e = editor("héllo.world next\nlast");
        keys(&mut e, "w");
        assert_eq!(e.cursor.col, 5);
        keys(&mut e, "w");
        assert_eq!(e.cursor.col, 6);
        keys(&mut e, "0dw");
        assert_eq!(e.text(), ".world next\nlast");
        keys(&mut e, "2dd");
        assert_eq!(e.text(), "");
        keys(&mut e, "u");
        assert_eq!(e.text(), ".world next\nlast");
    }
    #[test]
    fn visual_delete_is_inclusive_and_undoable() {
        let mut e = editor("a🙂bc");
        keys(&mut e, "vld");
        assert_eq!(e.text(), "bc");
        keys(&mut e, "u");
        assert_eq!(e.text(), "a🙂bc");
    }
    #[test]
    fn commands_use_existing_safe_save_and_close_actions() {
        let mut e = editor("hello");
        keys(&mut e, ":wq");
        assert_eq!(
            e.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), 80, 20),
            EditAction::SaveAndClose
        );
        keys(&mut e, ":q");
        assert_eq!(
            e.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), 80, 20),
            EditAction::Close
        );
        keys(&mut e, ":w");
        esc(&mut e);
        assert!(e.vim.command.is_none());
    }
    #[test]
    fn append_open_change_and_empty_buffer() {
        let mut e = editor("abc");
        keys(&mut e, "AZ");
        esc(&mut e);
        keys(&mut e, "ohello");
        esc(&mut e);
        assert_eq!(e.text(), "abcZ\nhello");
        keys(&mut e, "0cwnew");
        esc(&mut e);
        assert_eq!(e.text(), "abcZ\nnew");
        let mut e = editor("");
        keys(&mut e, "wbxdda🙂");
        esc(&mut e);
        assert_eq!(e.text(), "🙂");
    }
}
