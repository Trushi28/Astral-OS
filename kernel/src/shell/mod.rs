//src/shell/mod.rs
pub mod commands;

use alloc::vec::Vec;
use alloc::string::{String, ToString};
use crate::interrupts::{getchar, getchar_blocking};
use crate::drivers::framebuffer::{print_colored, print};
use core::arch::asm;

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
    history: Vec<String>,
    history_index: usize,
    theme: ShellTheme,
}

impl Shell {
    pub fn new() -> Self {
        Self {
            cmd_buffer: [0; MAX_CMD_LEN],
            cmd_len: 0,
            history: Vec::with_capacity(MAX_HISTORY),
            history_index: 0,
            theme: ShellTheme::REALITY,
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
        crate::println!();
        crate::println!("Welcome to Astral OS Shell");
        crate::println!("Type 'help' for available commands");
        crate::println!();
    }
    
    pub fn print_prompt(&self) {
        use crate::reality::causality::RealityId;
        use crate::reality::dream::get_system_state;
        
        let reality_id = RealityId::current().as_u64();
        let state = get_system_state();
        
        print_colored("astral", self.theme.prompt_color);
        
        match state {
            crate::reality::dream::SystemState::Dreaming => print_colored("💤", 0x9966FF),
            crate::reality::dream::SystemState::DeepDream => print_colored("🌙", 0x6633FF),
            _ => {}
        }
        
        if reality_id != 0 {
            crate::print!(":{}", reality_id);
        }
        
        print_colored("> ", self.theme.prompt_symbol_color);
    }
    
    pub fn run(&mut self) {
        self.print_banner();
        self.print_prompt();
        
        loop {
            if let Some(c) = getchar() {
                match c {
                    b'\n' => {
                        crate::println!();
                        self.execute_command();
                        if self.cmd_len > 0 {
                            let cmd = self.get_cmd_str().to_string();
                            if self.history.len() >= MAX_HISTORY {
                                self.history.remove(0);
                            }
                            self.history.push(cmd);
                            self.history_index = self.history.len();
                        }
                        
                        self.cmd_len = 0;
                        self.print_prompt();
                    }
                    8 | 127 => {
                        if self.cmd_len > 0 {
                            self.cmd_len -= 1;
                            print("\x08 \x08");
                        }
                    }
                    32..=126 => {
                        if self.cmd_len < MAX_CMD_LEN - 1 {
                            self.cmd_buffer[self.cmd_len] = c;
                            self.cmd_len += 1;
                            crate::print!("{}", c as char);
                        }
                    }
                    _ => {}
                }
            }
            
            use crate::reality::dream::{get_system_state, dream_cycle};
            if get_system_state() != crate::reality::dream::SystemState::Active {
                dream_cycle();
            }
            
            unsafe { asm!("hlt"); }
        }
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