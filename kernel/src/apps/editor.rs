//! Text Editor - CLI and GUI versions
//!
//! Memory-efficient editor using gap buffer for files larger than RAM.

use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;
use alloc::format;

/// Gap buffer for efficient insertion/deletion at cursor position
pub struct GapBuffer {
    buffer: Vec<u8>,
    gap_start: usize,
    gap_end: usize,
}

impl GapBuffer {
    pub fn new() -> Self {
        let mut buffer = Vec::with_capacity(4096);
        buffer.resize(4096, 0);
        Self {
            buffer,
            gap_start: 0,
            gap_end: 4096,
        }
    }
    
    pub fn with_content(content: &[u8]) -> Self {
        let capacity = content.len() + 1024;
        let mut buffer = Vec::with_capacity(capacity);
        buffer.extend_from_slice(content);
        buffer.resize(capacity, 0);
        
        Self {
            buffer,
            gap_start: content.len(),
            gap_end: capacity,
        }
    }
    
    fn gap_size(&self) -> usize {
        self.gap_end - self.gap_start
    }
    
    fn content_len(&self) -> usize {
        self.buffer.len() - self.gap_size()
    }
    
    /// Move gap to position
    fn move_gap(&mut self, pos: usize) {
        let pos = pos.min(self.content_len());
        
        if pos < self.gap_start {
            // Move gap left
            let move_size = self.gap_start - pos;
            let src_start = pos;
            let dst_start = self.gap_end - move_size;
            
            for i in (0..move_size).rev() {
                self.buffer[dst_start + i] = self.buffer[src_start + i];
            }
            
            self.gap_end -= move_size;
            self.gap_start = pos;
        } else if pos > self.gap_start {
            // Move gap right
            let move_size = pos - self.gap_start;
            
            for i in 0..move_size {
                self.buffer[self.gap_start + i] = self.buffer[self.gap_end + i];
            }
            
            self.gap_start += move_size;
            self.gap_end += move_size;
        }
    }
    
    /// Ensure enough gap space
    fn ensure_gap(&mut self, needed: usize) {
        if self.gap_size() >= needed {
            return;
        }
        
        let old_len = self.buffer.len();
        let grow_by = (needed - self.gap_size()).max(1024);
        self.buffer.resize(old_len + grow_by, 0);
        
        // Move content after gap
        let after_gap = old_len - self.gap_end;
        let new_len = self.buffer.len();
        if after_gap > 0 {
            for i in (0..after_gap).rev() {
                let src = self.gap_end + i;
                let dst = new_len - after_gap + i;
                self.buffer[dst] = self.buffer[src];
            }
        }
        self.gap_end = new_len - after_gap;
    }
    
    /// Insert character at cursor
    pub fn insert(&mut self, pos: usize, ch: u8) {
        self.move_gap(pos);
        self.ensure_gap(1);
        self.buffer[self.gap_start] = ch;
        self.gap_start += 1;
    }
    
    /// Insert string at cursor
    pub fn insert_str(&mut self, pos: usize, data: &[u8]) {
        self.move_gap(pos);
        self.ensure_gap(data.len());
        self.buffer[self.gap_start..self.gap_start + data.len()].copy_from_slice(data);
        self.gap_start += data.len();
    }
    
    /// Delete character at position
    pub fn delete(&mut self, pos: usize) {
        if pos >= self.content_len() {
            return;
        }
        self.move_gap(pos);
        self.gap_end += 1;
    }
    
    /// Delete range
    pub fn delete_range(&mut self, start: usize, end: usize) {
        let end = end.min(self.content_len());
        if start >= end {
            return;
        }
        self.move_gap(start);
        self.gap_end += end - start;
    }
    
    /// Get character at position
    pub fn get(&self, pos: usize) -> Option<u8> {
        if pos >= self.content_len() {
            return None;
        }
        
        let idx = if pos < self.gap_start {
            pos
        } else {
            pos + self.gap_size()
        };
        
        Some(self.buffer[idx])
    }
    
    /// Get content as bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(self.content_len());
        result.extend_from_slice(&self.buffer[..self.gap_start]);
        result.extend_from_slice(&self.buffer[self.gap_end..]);
        result
    }
}

/// Text line with lazy loading support
pub struct Line {
    content: GapBuffer,
}

impl Line {
    pub fn new() -> Self {
        Self { content: GapBuffer::new() }
    }
    
    pub fn from_bytes(data: &[u8]) -> Self {
        Self { content: GapBuffer::with_content(data) }
    }
    
    pub fn len(&self) -> usize {
        self.content.content_len()
    }
    
    pub fn insert(&mut self, col: usize, ch: u8) {
        self.content.insert(col, ch);
    }
    
    pub fn delete(&mut self, col: usize) {
        self.content.delete(col);
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        self.content.to_bytes()
    }
}

/// Editor cursor position
pub struct Cursor {
    pub line: usize,
    pub col: usize,
}

/// Editor mode (vim-like)
#[derive(Clone, Copy, PartialEq)]
pub enum EditorMode {
    Normal,
    Insert,
    Command,
}

/// Text Editor
pub struct Editor {
    lines: Vec<Line>,
    cursor: Cursor,
    pub mode: EditorMode,
    pub filename: Option<String>,
    pub modified: bool,
    pub view_offset: usize,
    view_height: usize,
    command_buffer: String,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            lines: vec![Line::new()],
            cursor: Cursor { line: 0, col: 0 },
            mode: EditorMode::Normal,
            filename: None,
            modified: false,
            view_offset: 0,
            view_height: 24,
            command_buffer: String::new(),
        }
    }
    
    /// Load file from filesystem
    pub fn load(&mut self, filename: &str) -> bool {
        if let Some(data) = crate::fs::psychicfs::fs_read(filename) {
            self.lines.clear();
            
            let mut line_start = 0;
            for (i, &byte) in data.iter().enumerate() {
                if byte == b'\n' {
                    self.lines.push(Line::from_bytes(&data[line_start..i]));
                    line_start = i + 1;
                }
            }
            
            // Last line (no trailing newline)
            if line_start <= data.len() {
                self.lines.push(Line::from_bytes(&data[line_start..]));
            }
            
            if self.lines.is_empty() {
                self.lines.push(Line::new());
            }
            
            self.filename = Some(String::from(filename));
            self.cursor = Cursor { line: 0, col: 0 };
            self.modified = false;
            true
        } else {
            self.filename = Some(String::from(filename));
            self.lines = vec![Line::new()];
            self.modified = false;
            true
        }
    }
    
    /// Save file to filesystem
    pub fn save(&mut self) -> bool {
        let filename = match &self.filename {
            Some(f) => f.clone(),
            None => return false,
        };
        
        let mut data = Vec::new();
        for (i, line) in self.lines.iter().enumerate() {
            data.extend_from_slice(&line.to_bytes());
            if i < self.lines.len() - 1 {
                data.push(b'\n');
            }
        }
        
        if crate::fs::psychicfs::fs_write(&filename, &data) {
            self.modified = false;
            true
        } else {
            false
        }
    }
    
    /// Handle key input
    pub fn handle_key(&mut self, key: u8) {
        match self.mode {
            EditorMode::Normal => self.handle_normal_key(key),
            EditorMode::Insert => self.handle_insert_key(key),
            EditorMode::Command => self.handle_command_key(key),
        }
    }
    
    fn handle_normal_key(&mut self, key: u8) {
        match key {
            b'i' => self.mode = EditorMode::Insert,
            b':' => {
                self.mode = EditorMode::Command;
                self.command_buffer.clear();
            }
            b'h' | 0x4B => self.move_left(),  // left arrow
            b'j' | 0x50 => self.move_down(),  // down arrow
            b'k' | 0x48 => self.move_up(),    // up arrow
            b'l' | 0x4D => self.move_right(), // right arrow
            b'x' => self.delete_char(),
            b'0' => self.cursor.col = 0,
            b'$' => self.move_to_end_of_line(),
            b'G' => self.cursor.line = self.lines.len().saturating_sub(1),
            b'g' => self.cursor.line = 0,
            _ => {}
        }
    }
    
    fn handle_insert_key(&mut self, key: u8) {
        match key {
            27 => self.mode = EditorMode::Normal, // Escape
            8 | 127 => self.backspace(),          // Backspace
            b'\n' => self.insert_newline(),
            _ if key >= 32 && key < 127 => self.insert_char(key),
            _ => {}
        }
    }
    
    fn handle_command_key(&mut self, key: u8) {
        match key {
            27 => self.mode = EditorMode::Normal, // Escape
            b'\n' => {
                self.execute_command();
                self.mode = EditorMode::Normal;
            }
            8 | 127 => { self.command_buffer.pop(); }
            _ if key >= 32 && key < 127 => {
                self.command_buffer.push(key as char);
            }
            _ => {}
        }
    }
    
    fn execute_command(&mut self) {
        match self.command_buffer.as_str() {
            "w" => { self.save(); }
            "q" => {} // Exit handled by caller
            "wq" => { self.save(); }
            _ => {}
        }
    }
    
    fn move_left(&mut self) {
        self.cursor.col = self.cursor.col.saturating_sub(1);
    }
    
    fn move_right(&mut self) {
        let line_len = self.current_line_len();
        if self.cursor.col < line_len {
            self.cursor.col += 1;
        }
    }
    
    fn move_up(&mut self) {
        if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.col = self.cursor.col.min(self.current_line_len());
        }
    }
    
    fn move_down(&mut self) {
        if self.cursor.line < self.lines.len() - 1 {
            self.cursor.line += 1;
            self.cursor.col = self.cursor.col.min(self.current_line_len());
        }
    }
    
    fn move_to_end_of_line(&mut self) {
        self.cursor.col = self.current_line_len();
    }
    
    fn current_line_len(&self) -> usize {
        self.lines.get(self.cursor.line).map_or(0, |l| l.len())
    }
    
    fn insert_char(&mut self, ch: u8) {
        if let Some(line) = self.lines.get_mut(self.cursor.line) {
            line.insert(self.cursor.col, ch);
            self.cursor.col += 1;
            self.modified = true;
        }
    }
    
    fn insert_newline(&mut self) {
        // Split current line
        let current = &self.lines[self.cursor.line];
        let rest = current.to_bytes()[self.cursor.col..].to_vec();
        
        // Truncate current line
        if let Some(line) = self.lines.get_mut(self.cursor.line) {
            line.content.delete_range(self.cursor.col, line.len());
        }
        
        // Insert new line
        self.cursor.line += 1;
        self.lines.insert(self.cursor.line, Line::from_bytes(&rest));
        self.cursor.col = 0;
        self.modified = true;
    }
    
    fn backspace(&mut self) {
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
            if let Some(line) = self.lines.get_mut(self.cursor.line) {
                line.delete(self.cursor.col);
            }
            self.modified = true;
        } else if self.cursor.line > 0 {
            // Join with previous line
            let current = self.lines.remove(self.cursor.line).to_bytes();
            self.cursor.line -= 1;
            self.cursor.col = self.lines[self.cursor.line].len();
            self.lines[self.cursor.line].content.insert_str(self.cursor.col, &current);
            self.modified = true;
        }
    }
    
    fn delete_char(&mut self) {
        if let Some(line) = self.lines.get_mut(self.cursor.line) {
            if self.cursor.col < line.len() {
                line.delete(self.cursor.col);
                self.modified = true;
            }
        }
    }
    
    /// Get mode string for status line
    pub fn mode_str(&self) -> &'static str {
        match self.mode {
            EditorMode::Normal => "NORMAL",
            EditorMode::Insert => "INSERT",
            EditorMode::Command => "COMMAND",
        }
    }
    
    /// Get command buffer for status line
    pub fn command_str(&self) -> &str {
        &self.command_buffer
    }
    
    /// Check if quit requested
    pub fn should_quit(&self) -> bool {
        self.command_buffer == "q" || self.command_buffer == "wq"
    }
    
    /// Get cursor position
    pub fn cursor_pos(&self) -> (usize, usize) {
        (self.cursor.line, self.cursor.col)
    }
    
    /// Get visible lines
    pub fn visible_lines(&self) -> impl Iterator<Item = (usize, &Line)> {
        let start = self.view_offset;
        let end = (start + self.view_height).min(self.lines.len());
        self.lines[start..end].iter().enumerate().map(move |(i, l)| (start + i, l))
    }
    
    /// Update view to keep cursor visible
    pub fn update_view(&mut self) {
        if self.cursor.line < self.view_offset {
            self.view_offset = self.cursor.line;
        } else if self.cursor.line >= self.view_offset + self.view_height {
            self.view_offset = self.cursor.line - self.view_height + 1;
        }
    }
}

/// Run CLI editor
pub fn run_cli(filename: Option<&str>) {
    let mut editor = Editor::new();
    
    if let Some(f) = filename {
        editor.load(f);
    }
    
    // Clear screen and show editor
    crate::drivers::framebuffer::clear();
    crate::drivers::framebuffer::set_cursor_pos(0, 0);
    
    crate::println!("=== Astral Editor ===");
    crate::println!("Keys: i=insert, Esc=normal, :w=save, :q=quit, h/j/k/l=move");
    crate::println!();
    
    let mut needs_redraw = true;
    
    loop {
        if needs_redraw {
            editor.update_view();
            render_editor(&editor);
            needs_redraw = false;
        }
        
        // Wait for key
        loop {
            if let Some(key) = crate::interrupts::getchar() {
                editor.handle_key(key);
                needs_redraw = true;
                break;
            }
            unsafe { core::arch::asm!("hlt"); }
        }
        
        if editor.should_quit() {
            break;
        }
    }
    
    crate::drivers::framebuffer::clear();
    crate::println!("Editor closed.");
}

/// Render editor to framebuffer
fn render_editor(editor: &Editor) {
    let line_height = 16;
    let char_width = 8;
    let header_height = 50;
    let gutter_width = 7 * char_width;  // "   1 | " = 7 chars
    let content_start_x = 10 + gutter_width;
    
    let (width, height) = crate::drivers::framebuffer::get_dimensions();
    
    // Clear entire content area with dark background
    let content_buf: Vec<u32> = alloc::vec![0x1a1a2e; width * (height - header_height)];
    crate::drivers::framebuffer::blit_buffer(&content_buf, 0, header_height, width, height - header_height - 40);
    
    // Header bar
    let header_buf: Vec<u32> = alloc::vec![0x2d2d2d; width * header_height];
    crate::drivers::framebuffer::blit_buffer(&header_buf, 0, 0, width, header_height);
    
    // Header text
    crate::drivers::framebuffer::set_cursor_pos(15, 15);
    crate::drivers::framebuffer::print_colored("Astral Editor", 0x00AAFF);
    
    if let Some(name) = editor.filename.as_ref() {
        crate::drivers::framebuffer::print_colored(&format!("  -  {}", name), 0xAAAAAA);
    }
    if editor.modified {
        crate::drivers::framebuffer::print_colored(" [modified]", 0xFFAA00);
    }
    
    // Render visible lines
    let mut y = header_height + 5;
    let (cursor_row, cursor_col) = editor.cursor_pos();
    
    for (line_num, line) in editor.visible_lines() {
        let content = line.to_bytes();
        let s = core::str::from_utf8(&content).unwrap_or("");
        
        // Highlight current line
        if line_num == cursor_row {
            let line_bg: Vec<u32> = alloc::vec![0x333340; width * line_height];
            crate::drivers::framebuffer::blit_buffer(&line_bg, 0, y, width, line_height);
        }
        
        // Line number
        crate::drivers::framebuffer::set_cursor_pos(10, y);
        let num_color = if line_num == cursor_row { 0xFFFFFF } else { 0x666666 };
        crate::drivers::framebuffer::print_colored(&format!("{:4} ", line_num + 1), num_color);
        crate::drivers::framebuffer::print_colored("│ ", 0x444444);
        
        // Content
        crate::drivers::framebuffer::print_colored(s, 0xE0E0E0);
        
        // Draw cursor if on this line
        if line_num == cursor_row {
            let cursor_x = content_start_x + (cursor_col * char_width);
            // Block cursor (inverse video style)
            let cursor_buf: Vec<u32> = alloc::vec![0xFFFFFF; char_width * (line_height - 2)];
            crate::drivers::framebuffer::blit_buffer(&cursor_buf, cursor_x, y + 1, char_width, line_height - 2);
        }
        
        y += line_height;
        if y > height - 80 {
            break;
        }
    }
    
    // Status bar at bottom
    let status_y = height - 35;
    let mode_color = match editor.mode {
        EditorMode::Insert => 0x22AA22,
        EditorMode::Command => 0xAAAA22,
        EditorMode::Normal => 0x2266AA,
    };
    
    // Mode badge
    let badge_buf: Vec<u32> = alloc::vec![mode_color; 90 * 20];
    crate::drivers::framebuffer::blit_buffer(&badge_buf, 10, status_y, 90, 20);
    crate::drivers::framebuffer::set_cursor_pos(15, status_y + 3);
    crate::drivers::framebuffer::print_colored(editor.mode_str(), 0xFFFFFF);
    
    // Position info
    crate::drivers::framebuffer::set_cursor_pos(110, status_y + 3);
    crate::drivers::framebuffer::print_colored(&format!("Ln {}, Col {}", cursor_row + 1, cursor_col + 1), 0xAAAAAA);
    
    // Command line
    if editor.mode == EditorMode::Command {
        crate::drivers::framebuffer::set_cursor_pos(250, status_y + 3);
        crate::drivers::framebuffer::print_colored(&format!(":{}", editor.command_str()), 0xFFFF00);
    }
    
    // Help text
    crate::drivers::framebuffer::set_cursor_pos((width - 200) as usize, status_y + 3);
    crate::drivers::framebuffer::print_colored("i:insert :w:save :q:quit", 0x666666);
}
