//! Simple Notes Application
//! Basic text note-taking with save/load support

use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

/// Notes application state
pub struct Notes {
    content: Vec<String>,
    cursor_line: usize,
    cursor_col: usize,
    filename: String,
    modified: bool,
}

impl Notes {
    pub fn new() -> Self {
        Self {
            content: alloc::vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
            filename: String::from("untitled.txt"),
            modified: false,
        }
    }
    
    /// Load content from filesystem
    pub fn load(&mut self, filename: &str) -> bool {
        if let Some(data) = crate::fs::psychicfs::fs_read(filename) {
            self.content.clear();
            let text = String::from_utf8_lossy(&data);
            for line in text.lines() {
                self.content.push(String::from(line));
            }
            if self.content.is_empty() {
                self.content.push(String::new());
            }
            self.filename = String::from(filename);
            self.cursor_line = 0;
            self.cursor_col = 0;
            self.modified = false;
            true
        } else {
            false
        }
    }
    
    /// Save content to filesystem
    pub fn save(&mut self) -> bool {
        let text = self.content.join("\n");
        let result = crate::fs::psychicfs::fs_write(&self.filename, text.as_bytes());
        if result {
            self.modified = false;
        }
        result
    }
    
    /// Save with new filename
    pub fn save_as(&mut self, filename: &str) -> bool {
        self.filename = String::from(filename);
        self.save()
    }
    
    /// Get current filename
    pub fn filename(&self) -> &str {
        &self.filename
    }
    
    /// Check if modified
    pub fn is_modified(&self) -> bool {
        self.modified
    }
    
    /// Get visible lines (for rendering)
    pub fn lines(&self) -> &[String] {
        &self.content
    }
    
    /// Get cursor position
    pub fn cursor(&self) -> (usize, usize) {
        (self.cursor_line, self.cursor_col)
    }
    
    /// Handle key input
    pub fn handle_key(&mut self, key: u8) {
        match key {
            // Printable characters
            32..=126 => {
                self.insert_char(key as char);
            }
            
            // Enter
            b'\n' => {
                self.insert_newline();
            }
            
            // Backspace
            8 | 127 => {
                self.delete_backward();
            }
            
            // Arrow keys (using common escape sequences)
            // These would need proper handling in the GUI
            
            _ => {}
        }
    }
    
    /// Insert character at cursor
    fn insert_char(&mut self, ch: char) {
        if self.cursor_line < self.content.len() {
            let line = &mut self.content[self.cursor_line];
            if self.cursor_col <= line.len() {
                line.insert(self.cursor_col, ch);
                self.cursor_col += 1;
                self.modified = true;
            }
        }
    }
    
    /// Insert newline
    fn insert_newline(&mut self) {
        if self.cursor_line < self.content.len() {
            let current_line = &self.content[self.cursor_line];
            let (before, after) = if self.cursor_col <= current_line.len() {
                (
                    String::from(&current_line[..self.cursor_col]),
                    String::from(&current_line[self.cursor_col..])
                )
            } else {
                (current_line.clone(), String::new())
            };
            
            self.content[self.cursor_line] = before;
            self.cursor_line += 1;
            self.content.insert(self.cursor_line, after);
            self.cursor_col = 0;
            self.modified = true;
        }
    }
    
    /// Delete character before cursor
    fn delete_backward(&mut self) {
        if self.cursor_col > 0 {
            let line = &mut self.content[self.cursor_line];
            self.cursor_col -= 1;
            line.remove(self.cursor_col);
            self.modified = true;
        } else if self.cursor_line > 0 {
            // Join with previous line
            let current = self.content.remove(self.cursor_line);
            self.cursor_line -= 1;
            let prev_len = self.content[self.cursor_line].len();
            self.content[self.cursor_line].push_str(&current);
            self.cursor_col = prev_len;
            self.modified = true;
        }
    }
    
    /// Move cursor
    pub fn move_cursor(&mut self, direction: Direction) {
        match direction {
            Direction::Up => {
                if self.cursor_line > 0 {
                    self.cursor_line -= 1;
                    let line_len = self.content[self.cursor_line].len();
                    self.cursor_col = self.cursor_col.min(line_len);
                }
            }
            Direction::Down => {
                if self.cursor_line + 1 < self.content.len() {
                    self.cursor_line += 1;
                    let line_len = self.content[self.cursor_line].len();
                    self.cursor_col = self.cursor_col.min(line_len);
                }
            }
            Direction::Left => {
                if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                } else if self.cursor_line > 0 {
                    self.cursor_line -= 1;
                    self.cursor_col = self.content[self.cursor_line].len();
                }
            }
            Direction::Right => {
                let line_len = self.content[self.cursor_line].len();
                if self.cursor_col < line_len {
                    self.cursor_col += 1;
                } else if self.cursor_line + 1 < self.content.len() {
                    self.cursor_line += 1;
                    self.cursor_col = 0;
                }
            }
        }
    }
    
    /// Get line count
    pub fn line_count(&self) -> usize {
        self.content.len()
    }
    
    /// Get status line text
    pub fn status(&self) -> String {
        let modified = if self.modified { "[+]" } else { "" };
        format!("{} {} L:{} C:{}", self.filename, modified, self.cursor_line + 1, self.cursor_col + 1)
    }
}

/// Cursor movement direction
#[derive(Clone, Copy)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}
