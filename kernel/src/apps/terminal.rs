//! In-GUI Terminal Application
//! Command-line terminal running within the GUI

use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use alloc::format;

/// Terminal configuration
const MAX_HISTORY: usize = 100;
const MAX_OUTPUT_LINES: usize = 500;

/// Terminal application state
pub struct Terminal {
    output: Vec<String>,
    input: String,
    cursor: usize,
    history: Vec<String>,
    history_idx: Option<usize>,
}

impl Terminal {
    pub fn new() -> Self {
        let mut term = Self {
            output: Vec::new(),
            input: String::new(),
            cursor: 0,
            history: Vec::new(),
            history_idx: None,
        };
        
        term.println("Astral OS Terminal v0.3");
        term.println("Type 'help' for commands");
        term.println("");
        
        term
    }
    
    /// Add a line to output
    pub fn println(&mut self, line: &str) {
        self.output.push(String::from(line));
        
        // Trim old output
        while self.output.len() > MAX_OUTPUT_LINES {
            self.output.remove(0);
        }
    }
    
    /// Get output lines for rendering
    pub fn output_lines(&self) -> &[String] {
        &self.output
    }
    
    /// Get current input line
    pub fn input_line(&self) -> &str {
        &self.input
    }
    
    /// Get cursor position
    pub fn cursor_pos(&self) -> usize {
        self.cursor
    }
    
    /// Handle key input
    pub fn handle_key(&mut self, key: u8) {
        match key {
            // Printable
            32..=126 => {
                self.input.insert(self.cursor, key as char);
                self.cursor += 1;
                self.history_idx = None;
            }
            
            // Enter
            b'\n' => {
                self.execute_command();
            }
            
            // Backspace
            8 | 127 => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.input.remove(self.cursor);
                }
            }
            
            // Tab - could be autocomplete
            b'\t' => {
                // Simple autocomplete for common commands
                if self.input.starts_with("hel") {
                    self.input = String::from("help");
                    self.cursor = self.input.len();
                } else if self.input.starts_with("cle") {
                    self.input = String::from("clear");
                    self.cursor = self.input.len();
                }
            }
            
            _ => {}
        }
    }
    
    /// Handle arrow key for history (called from GUI)
    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        
        self.history_idx = Some(match self.history_idx {
            Some(idx) if idx > 0 => idx - 1,
            Some(idx) => idx,
            None => self.history.len() - 1,
        });
        
        if let Some(idx) = self.history_idx {
            self.input = self.history[idx].clone();
            self.cursor = self.input.len();
        }
    }
    
    /// Handle arrow key for history
    pub fn history_down(&mut self) {
        if let Some(idx) = self.history_idx {
            if idx + 1 < self.history.len() {
                self.history_idx = Some(idx + 1);
                self.input = self.history[idx + 1].clone();
            } else {
                self.history_idx = None;
                self.input.clear();
            }
            self.cursor = self.input.len();
        }
    }
    
    fn execute_command(&mut self) {
        let cmd = String::from(self.input.trim());
        
        // Echo command
        self.println(&format!("> {}", cmd));
        
        // Add to history
        if !cmd.is_empty() {
            self.history.push(cmd.clone());
            if self.history.len() > MAX_HISTORY {
                self.history.remove(0);
            }
        }
        
        // Parse and execute
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if let Some(&command) = parts.first() {
            match command {
                "help" => {
                    self.println("Available commands:");
                    self.println("  help    - Show this help");
                    self.println("  clear   - Clear screen");
                    self.println("  ls      - List files");
                    self.println("  ps      - Show processes");
                    self.println("  mem     - Memory info");
                    self.println("  cpu     - CPU info");
                    self.println("  date    - Show date");
                    self.println("  echo    - Echo text");
                    self.println("  exit    - Close terminal");
                }
                
                "clear" => {
                    self.output.clear();
                }
                
                "ls" => {
                    let files = crate::fs::psychicfs::fs_list();
                    if files.is_empty() {
                        self.println("(no files)");
                    } else {
                        for file in files {
                            self.println(&format!("  {}", file));
                        }
                    }
                }
                
                "ps" => {
                    let processes = crate::process::process_table().lock();
                    self.println("PID  STATE     NAME");
                    for proc in processes.iter() {
                        self.println(&format!("{:4} {:9} shell", 
                            proc.pid.as_u64(),
                            match proc.state {
                                crate::process::ProcessState::Ready => "READY",
                                crate::process::ProcessState::Running => "RUNNING",
                                crate::process::ProcessState::Blocked => "BLOCKED",
                                crate::process::ProcessState::Zombie => "ZOMBIE",
                            }
                        ));
                    }
                }
                
                "mem" => {
                    let (total, used, free) = crate::memory::frame::get_stats();
                    self.println(&format!("Total: {} KB", total / 1024));
                    self.println(&format!("Used:  {} KB", used / 1024));
                    self.println(&format!("Free:  {} KB", free / 1024));
                }
                
                "cpu" => {
                    let count = crate::get_cpu_count();
                    self.println(&format!("CPUs: {}", count));
                    self.println("Arch: x86_64");
                }
                
                "date" => {
                    let ticks = crate::get_timestamp();
                    let secs = ticks / 100; // Assuming 100Hz timer
                    self.println(&format!("Uptime: {} seconds", secs));
                }
                
                "echo" => {
                    let rest = parts[1..].join(" ");
                    self.println(&rest);
                }
                
                "exit" => {
                    self.println("Use 'q' to exit GUI mode");
                }
                
                "" => {}
                
                _ => {
                    self.println(&format!("Unknown command: {}", command));
                }
            }
        }
        
        self.input.clear();
        self.cursor = 0;
        self.history_idx = None;
    }
}
