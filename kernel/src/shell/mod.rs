//src/shell/mod.rs
pub mod commands;
pub mod hal;

use alloc::vec::Vec;
use alloc::string::String;
use alloc::string::ToString;
use crate::interrupts::{KB_ARROW_UP, KB_ARROW_DOWN, KB_ARROW_LEFT, KB_ARROW_RIGHT};


// Re-export HAL for commands module
pub use hal::{print_colored, clear_screen, get_cursor_pos, set_cursor_pos, clear_line, read_char};

const MAX_CMD_LEN: usize = 256;
const MAX_HISTORY: usize = 50;

#[derive(Clone, Copy)]
pub struct ShellTheme {
    pub prompt_color: u32,
    pub prompt_symbol_color: u32,
    pub command_color: u32,
    pub output_color: u32,
    pub error_color: u32,
    pub success_color: u32,
    pub info_color: u32,
    pub reality_color: u32,
}

impl ShellTheme {
    pub const REALITY: Self = Self {
        prompt_color: 0x00AAFF,
        prompt_symbol_color: 0x00FF00,
        command_color: 0xFFFFFF,
        output_color: 0xCCCCCC,
        error_color: 0xFF6666,
        success_color: 0x00FF00,
        info_color: 0xFFFF00,
        reality_color: 0xFF00FF,
    };
    
    pub const DREAM: Self = Self {
        prompt_color: 0x9966FF,
        prompt_symbol_color: 0xFF66FF,
        command_color: 0xFFFFFF,
        output_color: 0xBBBBFF,
        error_color: 0xFF6666,
        success_color: 0x66FFAA,
        info_color: 0xFFDD66,
        reality_color: 0xFF66FF,
    };
}

pub struct Shell {
    cmd_buffer: [u8; MAX_CMD_LEN],
    cmd_len: usize,
    cursor_pos: usize,
    history: Vec<String>,
    history_index: Option<usize>,
    theme: ShellTheme,
    prompt_x: usize,
    prompt_y: usize,
}

impl Shell {
    pub fn new() -> Self {
        Self {
            cmd_buffer: [0; MAX_CMD_LEN],
            cmd_len: 0,
            cursor_pos: 0,
            history: Vec::with_capacity(MAX_HISTORY),
            history_index: None,
            theme: ShellTheme::REALITY,
            prompt_x: 0,
            prompt_y: 0,
        }
    }
    
    pub fn print_banner(&self) {
        print_colored("╔═══════════════════════════════════════════╗\n", 0x00AAFF);
        print_colored("║      ", 0x00AAFF);
        print_colored("ASTRAL OS", 0xFFFFFF);
        print_colored(" v0.3.0                 ║\n", 0x00AAFF);
        print_colored("║  ", 0x00AAFF);
        print_colored("Modern Foundation + Reality Engine", 0x888888);
        print_colored("   ║\n", 0x00AAFF);
        print_colored("╚═══════════════════════════════════════════╝\n", 0x00AAFF);
        hal::println("");
        hal::println("Welcome to Astral OS Shell");
        hal::println("Type 'help' for available commands");
        hal::println("");
    }
    
    pub fn print_prompt(&mut self) {
        // User mode shell prompt: user@astral
        print_colored("user", 0x00FF88);  // Username in green
        print_colored("@", 0x888888);     // @ in gray
        print_colored("astral", self.theme.prompt_color);
        
        // Reality engine features only in kernel mode
        use crate::reality::causality::RealityId;
        use crate::reality::dream::get_system_state;
        
        let reality_id = RealityId::current().as_u64();
        let state = get_system_state();
        
        match state {
            crate::reality::dream::SystemState::Dreaming => print_colored("💤", 0x9966FF),
            crate::reality::dream::SystemState::DeepDream => print_colored("🌙", 0x6633FF),
            _ => {}
        }
        
        if reality_id != 0 {
            use alloc::format;
            hal::print(&format!(":{}", reality_id));
        }
        
        print_colored("> ", self.theme.prompt_symbol_color);
        
        // Save prompt position
        let (x, y) = get_cursor_pos();
        self.prompt_x = x;
        self.prompt_y = y;
    }
    
    pub fn run(&mut self) {
        self.print_banner();
        self.print_prompt();
        
        loop {
            if let Some(key) = read_char() {
                match key {
                    KB_ARROW_UP => {
                        self.history_up();
                    }
                    KB_ARROW_DOWN => {
                        self.history_down();
                    }
                    KB_ARROW_LEFT => {
                        self.move_cursor_left();
                    }
                    KB_ARROW_RIGHT => {
                        self.move_cursor_right();
                    }
                    b'\n' => {
                        hal::println("");
                        self.execute_command();
                        
                        if self.cmd_len > 0 {
                            let cmd = self.get_cmd_str().to_string();
                            
                            // Only add to history if different from last command
                            let should_add = self.history.last()
                                .map(|last| last != &cmd)
                                .unwrap_or(true);
                            
                            if should_add {
                                if self.history.len() >= MAX_HISTORY {
                                    self.history.remove(0);
                                }
                                self.history.push(cmd);
                            }
                        }
                        
                        self.cmd_len = 0;
                        self.cursor_pos = 0;
                        self.history_index = None;
                        self.print_prompt();
                    }
                    8 => {
                        self.handle_backspace();
                    }
                    0x7F => { // Delete key
                        self.handle_delete();
                    }
                    1 => { // Ctrl+A - Home
                        self.cursor_pos = 0;
                        self.redraw_line();
                    }
                    5 => { // Ctrl+E - End
                        self.cursor_pos = self.cmd_len;
                        self.redraw_line();
                    }
                    11 => { // Ctrl+K - Kill to end
                        self.cmd_len = self.cursor_pos;
                        self.redraw_line();
                    }
                    21 => { // Ctrl+U - Kill to start
                        if self.cursor_pos > 0 {
                            let remaining = self.cmd_len - self.cursor_pos;
                            for i in 0..remaining {
                                self.cmd_buffer[i] = self.cmd_buffer[self.cursor_pos + i];
                            }
                            self.cmd_len = remaining;
                            self.cursor_pos = 0;
                            self.redraw_line();
                        }
                    }
                    12 => { // Ctrl+L - Clear screen
                        clear_screen();
                        self.print_prompt();
                        self.redraw_line();
                    }
                    32..=126 => {
                        self.insert_char(key);
                    }
                    _ => {}
                }
            }
            
            // Dream cycle integration (only in kernel mode)
            use crate::reality::dream::{get_system_state, dream_cycle};
            if get_system_state() != crate::reality::dream::SystemState::Active {
                dream_cycle();
            }
            
            hal::yield_cpu();
        }
    }
    
    fn insert_char(&mut self, c: u8) {
        if self.cmd_len < MAX_CMD_LEN - 1 {
            // Shift characters to make room
            for i in (self.cursor_pos..self.cmd_len).rev() {
                self.cmd_buffer[i + 1] = self.cmd_buffer[i];
            }
            
            self.cmd_buffer[self.cursor_pos] = c;
            self.cmd_len += 1;
            self.cursor_pos += 1;
            
            self.redraw_line();
        }
    }
    
    fn handle_backspace(&mut self) {
        if self.cursor_pos > 0 {
            // Shift characters left
            for i in self.cursor_pos..self.cmd_len {
                self.cmd_buffer[i - 1] = self.cmd_buffer[i];
            }
            
            self.cmd_len -= 1;
            self.cursor_pos -= 1;
            
            self.redraw_line();
        }
    }
    
    fn handle_delete(&mut self) {
        if self.cursor_pos < self.cmd_len {
            // Shift characters left
            for i in (self.cursor_pos + 1)..self.cmd_len {
                self.cmd_buffer[i - 1] = self.cmd_buffer[i];
            }
            
            self.cmd_len -= 1;
            self.redraw_line();
        }
    }
    
    fn move_cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.update_cursor_position();
        }
    }
    
    fn move_cursor_right(&mut self) {
        if self.cursor_pos < self.cmd_len {
            self.cursor_pos += 1;
            self.update_cursor_position();
        }
    }
    
    fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        
        let new_index = match self.history_index {
            None => Some(self.history.len() - 1),
            Some(0) => Some(0),
            Some(i) => Some(i - 1),
        };
        
        if let Some(idx) = new_index {
            self.history_index = Some(idx);
            self.load_history_command(idx);
        }
    }
    
    fn history_down(&mut self) {
        if self.history.is_empty() {
            return;
        }
        
        match self.history_index {
            None => {}
            Some(i) if i >= self.history.len() - 1 => {
                self.history_index = None;
                self.clear_command();
            }
            Some(i) => {
                let new_idx = i + 1;
                self.history_index = Some(new_idx);
                self.load_history_command(new_idx);
            }
        }
    }
    
    fn load_history_command(&mut self, index: usize) {
        if let Some(cmd) = self.history.get(index) {
            self.cmd_len = 0;
            self.cursor_pos = 0;
            
            for &byte in cmd.as_bytes() {
                if self.cmd_len < MAX_CMD_LEN {
                    self.cmd_buffer[self.cmd_len] = byte;
                    self.cmd_len += 1;
                }
            }
            
            self.cursor_pos = self.cmd_len;
            self.redraw_line();
        }
    }
    
    fn clear_command(&mut self) {
        self.cmd_len = 0;
        self.cursor_pos = 0;
        self.redraw_line();
    }
    
    fn redraw_line(&self) {
        set_cursor_pos(self.prompt_x, self.prompt_y);
        clear_line();
        
        // Redraw command with proper color
        let cmd_str = self.get_cmd_str();
        print_colored(cmd_str, self.theme.command_color);
        
        // Position cursor
        self.update_cursor_position();
    }
    
    fn update_cursor_position(&self) {
        // Calculate cursor position based on cursor_pos
        // Simplified: assumes fixed-width chars
        let char_width = 12;
        let cursor_x = self.prompt_x + (self.cursor_pos * char_width);
        set_cursor_pos(cursor_x, self.prompt_y);
    }
    
    fn get_cmd_str(&self) -> &str {
        core::str::from_utf8(&self.cmd_buffer[..self.cmd_len]).unwrap_or("")
    }
    
    fn execute_command(&mut self) {
        let cmd = self.get_cmd_str().trim().to_string();
        
        if cmd.is_empty() {
            return;
        }
        
        let mut parts = cmd.split_whitespace();
        let command = parts.next().unwrap_or("");
        
        commands::execute(command, parts, &mut self.theme);
    }
}