//src/drivers/keyboard.rs
// Keyboard driver - interrupt handling is in interrupts/handlers.rs
// This module provides high-level keyboard API

pub use crate::interrupts::handlers::{getchar, getchar_blocking};

pub fn init() {
    // Keyboard is initialized via interrupts
    // Nothing extra needed here
}

/// Try to read a character without blocking
/// Returns None if no character is available
pub fn try_read_char() -> Option<char> {
    getchar().map(|b| b as char)
}