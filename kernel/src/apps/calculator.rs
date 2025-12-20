//! Simple Calculator Application
//! Basic calculator with keyboard input

use alloc::string::String;
use alloc::format;

/// Calculator state
pub struct Calculator {
    display: String,
    operand1: Option<f64>,
    operand2: Option<f64>,
    operator: Option<char>,
    clear_on_next: bool,
}

impl Calculator {
    pub fn new() -> Self {
        Self {
            display: String::from("0"),
            operand1: None,
            operand2: None,
            operator: None,
            clear_on_next: false,
        }
    }
    
    /// Get current display value
    pub fn display(&self) -> &str {
        &self.display
    }
    
    /// Handle key press
    pub fn handle_key(&mut self, key: u8) {
        match key {
            // Digits 0-9
            b'0'..=b'9' => {
                if self.clear_on_next {
                    self.display.clear();
                    self.clear_on_next = false;
                }
                if self.display == "0" {
                    self.display.clear();
                }
                self.display.push(key as char);
            }
            
            // Decimal point
            b'.' => {
                if self.clear_on_next {
                    self.display = String::from("0");
                    self.clear_on_next = false;
                }
                if !self.display.contains('.') {
                    self.display.push('.');
                }
            }
            
            // Operators
            b'+' | b'-' | b'*' | b'/' => {
                self.store_operand();
                self.operator = Some(key as char);
                self.clear_on_next = true;
            }
            
            // Equals
            b'=' | b'\n' => {
                self.calculate();
            }
            
            // Clear
            b'c' | b'C' => {
                self.clear();
            }
            
            // Backspace
            8 | 127 => {
                if !self.display.is_empty() && self.display != "0" {
                    self.display.pop();
                    if self.display.is_empty() {
                        self.display = String::from("0");
                    }
                }
            }
            
            // Negate
            b'n' | b'N' => {
                if let Ok(val) = self.display.parse::<f64>() {
                    self.display = format!("{}", -val);
                }
            }
            
            _ => {}
        }
    }
    
    fn store_operand(&mut self) {
        if let Ok(val) = self.display.parse::<f64>() {
            if self.operand1.is_none() {
                self.operand1 = Some(val);
            } else {
                self.operand2 = Some(val);
                self.calculate();
            }
        }
    }
    
    fn calculate(&mut self) {
        if let Ok(val2) = self.display.parse::<f64>() {
            if let (Some(val1), Some(op)) = (self.operand1, self.operator) {
                let result = match op {
                    '+' => val1 + val2,
                    '-' => val1 - val2,
                    '*' => val1 * val2,
                    '/' => {
                        if val2 != 0.0 {
                            val1 / val2
                        } else {
                            self.display = String::from("Error");
                            self.operand1 = None;
                            self.operator = None;
                            self.clear_on_next = true;
                            return;
                        }
                    }
                    _ => val2,
                };
                
                // Format result nicely (no fract() in no_std)
                // Check if it's an integer by truncating and comparing
                let truncated = result as i64;
                if (result - truncated as f64).abs() < 0.0000001 && result.abs() < 1e10 {
                    self.display = format!("{}", truncated);
                } else {
                    // Just show a reasonable number of digits
                    self.display = format!("{}", result);
                    // Trim trailing zeros after decimal
                    if self.display.contains('.') {
                        while self.display.ends_with('0') {
                            self.display.pop();
                        }
                        if self.display.ends_with('.') {
                            self.display.pop();
                        }
                    }
                }
                
                self.operand1 = Some(result);
                self.operator = None;
                self.clear_on_next = true;
            }
        }
    }
    
    pub fn clear(&mut self) {
        self.display = String::from("0");
        self.operand1 = None;
        self.operand2 = None;
        self.operator = None;
        self.clear_on_next = false;
    }
    
    /// Get current operator symbol
    pub fn current_operator(&self) -> Option<char> {
        self.operator
    }
}
