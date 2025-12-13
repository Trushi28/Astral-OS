//! Window management for the GUI

use alloc::string::String;
use alloc::vec::Vec;
use super::theme;

/// Window type determines the content
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WindowType {
    Terminal,
    FileManager,
    Settings,
    About,
    Custom,
}

/// Window state
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WindowState {
    Normal,
    Minimized,
    Maximized,
}

/// Terminal data for Terminal windows
pub struct TerminalData {
    pub lines: Vec<String>,
    pub input: String,
    pub cursor_pos: usize,
}

impl TerminalData {
    pub fn new() -> Self {
        Self {
            lines: alloc::vec![
                String::from("Astral OS Terminal v1.0"),
                String::from("Type 'help' for commands"),
                String::from(""),
            ],
            input: String::new(),
            cursor_pos: 0,
        }
    }
    
    pub fn add_line(&mut self, line: &str) {
        self.lines.push(String::from(line));
        // Keep last 100 lines
        if self.lines.len() > 100 {
            self.lines.remove(0);
        }
    }
    
    pub fn process_input(&mut self) {
        let cmd = self.input.clone();
        self.add_line(&alloc::format!("> {}", cmd));
        
        // Process command
        match cmd.trim() {
            "help" => {
                self.add_line("Commands: help, clear, date, mem, exit");
            }
            "clear" => {
                self.lines.clear();
            }
            "date" => {
                self.add_line("Date: 2024-12-12 (simulated)");
            }
            "mem" => {
                let (total, used, free) = crate::memory::frame::get_stats();
                self.add_line(&alloc::format!("Memory: {} total, {} used, {} free", total, used, free));
            }
            "exit" => {
                self.add_line("Use 'q' to exit GUI");
            }
            "" => {}
            _ => {
                self.add_line(&alloc::format!("Unknown: {}", cmd));
            }
        }
        
        self.input.clear();
        self.cursor_pos = 0;
    }
}

/// A GUI window
pub struct Window {
    pub id: u64,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub state: WindowState,
    pub focused: bool,
    pub z_order: i32,
    pub window_type: WindowType,
    
    // Window-specific data
    pub terminal: Option<TerminalData>,
    
    // Saved position for restore from maximize
    saved_x: i32,
    saved_y: i32,
    saved_w: u32,
    saved_h: u32,
}

impl Window {
    pub fn new(id: u64, title: &str, x: i32, y: i32, width: u32, height: u32) -> Self {
        let window_type = match title {
            "Terminal" => WindowType::Terminal,
            "Files" | "File Manager" => WindowType::FileManager,
            "Settings" => WindowType::Settings,
            "About" => WindowType::About,
            _ => WindowType::Custom,
        };
        
        let terminal = if window_type == WindowType::Terminal {
            Some(TerminalData::new())
        } else {
            None
        };
        
        Self {
            id,
            title: String::from(title),
            x,
            y,
            width,
            height,
            state: WindowState::Normal,
            focused: false,
            z_order: 0,
            window_type,
            terminal,
            saved_x: x,
            saved_y: y,
            saved_w: width,
            saved_h: height,
        }
    }
    
    /// Handle key input for this window
    pub fn handle_key(&mut self, key: u8) {
        if let Some(ref mut term) = self.terminal {
            match key {
                b'\n' => {
                    term.process_input();
                }
                8 | 127 => { // Backspace
                    if term.cursor_pos > 0 {
                        term.input.remove(term.cursor_pos - 1);
                        term.cursor_pos -= 1;
                    }
                }
                32..=126 => { // Printable
                    term.input.insert(term.cursor_pos, key as char);
                    term.cursor_pos += 1;
                }
                _ => {}
            }
        }
    }
    
    /// Get content area bounds
    pub fn content_bounds(&self) -> (i32, i32, u32, u32) {
        let theme = theme::current();
        let title_h = theme.window_title_height;
        (
            self.x + 1,
            self.y + title_h as i32,
            self.width - 2,
            self.height.saturating_sub(title_h + 1),
        )
    }
    
    /// Check if point is in window
    pub fn point_in_window(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.x + self.width as i32 &&
        py >= self.y && py < self.y + self.height as i32
    }
    
    /// Toggle maximize
    pub fn toggle_maximize(&mut self, screen_w: u32, screen_h: u32, taskbar_h: u32) {
        match self.state {
            WindowState::Maximized => {
                self.x = self.saved_x;
                self.y = self.saved_y;
                self.width = self.saved_w;
                self.height = self.saved_h;
                self.state = WindowState::Normal;
            }
            _ => {
                self.saved_x = self.x;
                self.saved_y = self.y;
                self.saved_w = self.width;
                self.saved_h = self.height;
                self.x = 0;
                self.y = 0;
                self.width = screen_w;
                self.height = screen_h - taskbar_h;
                self.state = WindowState::Maximized;
            }
        }
    }
    
    /// Render the window with content
    pub fn render(&self, fb: &mut impl FnMut(i32, i32, u32), text_fn: &mut impl FnMut(i32, i32, &str, u32)) {
        if self.state == WindowState::Minimized {
            return;
        }
        
        let theme = theme::current();
        
        // Window background
        for dy in 0..self.height {
            for dx in 0..self.width {
                fb(self.x + dx as i32, self.y + dy as i32, theme.window_bg);
            }
        }
        
        // Title bar
        let title_bg = if self.focused {
            theme.window_title_bg
        } else {
            theme.window_title_bg_inactive
        };
        
        for dy in 0..theme.window_title_height {
            for dx in 0..self.width {
                fb(self.x + dx as i32, self.y + dy as i32, title_bg);
            }
        }
        
        // Window buttons (macOS style)
        draw_circle(fb, self.x + 16, self.y + 15, 6, theme.button_close);
        draw_circle(fb, self.x + 36, self.y + 15, 6, theme.button_maximize);
        draw_circle(fb, self.x + 56, self.y + 15, 6, theme.button_minimize);
        
        // Title text
        text_fn(self.x + 80, self.y + 8, &self.title, theme.window_title_text);
        
        // Border
        for dx in 0..self.width {
            fb(self.x + dx as i32, self.y, theme.window_border);
            fb(self.x + dx as i32, self.y + self.height as i32 - 1, theme.window_border);
        }
        for dy in 0..self.height {
            fb(self.x, self.y + dy as i32, theme.window_border);
            fb(self.x + self.width as i32 - 1, self.y + dy as i32, theme.window_border);
        }
        
        // Render content based on type
        let (cx, cy, _cw, ch) = self.content_bounds();
        
        match self.window_type {
            WindowType::Terminal => self.render_terminal(fb, text_fn, cx, cy, ch),
            WindowType::FileManager => self.render_file_manager(fb, text_fn, cx, cy),
            WindowType::Settings => self.render_settings(fb, text_fn, cx, cy),
            WindowType::About => self.render_about(fb, text_fn, cx, cy),
            WindowType::Custom => {}
        }
    }
    
    fn render_terminal(&self, _fb: &mut impl FnMut(i32, i32, u32), text_fn: &mut impl FnMut(i32, i32, &str, u32), x: i32, y: i32, h: u32) {
        let line_height = 18;
        
        if let Some(ref term) = self.terminal {
            // Render output lines
            let max_lines = (h as i32 / line_height - 2) as usize;
            let start = term.lines.len().saturating_sub(max_lines);
            
            for (i, line) in term.lines.iter().skip(start).enumerate() {
                text_fn(x + 8, y + 8 + (i as i32 * line_height), line, 0x00FF88);
            }
            
            // Render input line
            let input_y = y + h as i32 - 25;
            let prompt = alloc::format!("> {}_", term.input);
            text_fn(x + 8, input_y, &prompt, 0xFFFFFF);
        }
    }
    
    fn render_file_manager(&self, _fb: &mut impl FnMut(i32, i32, u32), text_fn: &mut impl FnMut(i32, i32, &str, u32), x: i32, y: i32) {
        let files = [
            "  Documents",
            "  Downloads",
            "  Pictures",
            "  Music",
            "  readme.txt",
            "  notes.md",
        ];
        
        text_fn(x + 8, y + 8, "/ Home", 0xFFFFFF);
        text_fn(x + 8, y + 28, "---------------", 0x666666);
        
        for (i, file) in files.iter().enumerate() {
            text_fn(x + 16, y + 50 + (i as i32 * 22), file, 0xCCCCCC);
        }
    }
    
    fn render_settings(&self, fb: &mut impl FnMut(i32, i32, u32), text_fn: &mut impl FnMut(i32, i32, &str, u32), x: i32, y: i32) {
        text_fn(x + 8, y + 8, "System Settings", 0xFFFFFF);
        text_fn(x + 8, y + 35, "-----------------", 0x666666);
        
        let settings = [
            ("Display", "1024x768"),
            ("Theme", "Dark"),
            ("Sound", "On"),
            ("Network", "Connected"),
        ];
        
        for (i, (name, value)) in settings.iter().enumerate() {
            let row_y = y + 60 + (i as i32 * 30);
            text_fn(x + 16, row_y, name, 0xCCCCCC);
            
            // Value box
            for dy in 0..20 {
                for dx in 0..80 {
                    fb(x + 150 + dx, row_y + dy - 2, 0x444444);
                }
            }
            text_fn(x + 158, row_y, value, 0x00AAFF);
        }
    }
    
    fn render_about(&self, _fb: &mut impl FnMut(i32, i32, u32), text_fn: &mut impl FnMut(i32, i32, &str, u32), x: i32, y: i32) {
        text_fn(x + 8, y + 20, "Astral OS", 0x00AAFF);
        text_fn(x + 8, y + 50, "Version 0.3.0", 0xCCCCCC);
        text_fn(x + 8, y + 75, "Reality Engine + Foundation", 0x888888);
        text_fn(x + 8, y + 110, "Built with Rust", 0xFF8844);
        text_fn(x + 8, y + 145, "(C) 2024 Astral Project", 0x666666);
    }
}

/// Draw a filled circle
fn draw_circle(fb: &mut impl FnMut(i32, i32, u32), cx: i32, cy: i32, r: i32, color: u32) {
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r {
                fb(cx + dx, cy + dy, color);
            }
        }
    }
}
