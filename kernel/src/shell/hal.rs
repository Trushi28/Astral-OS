//! Shell Hardware Abstraction Layer (HAL)
//! 
//! This module provides an environment-agnostic interface for shell I/O.
//! Simplified for Kernel-only mode (Ring 0).

use alloc::string::String;
use alloc::vec::Vec;

/// Print a string to the console
pub fn print(s: &str) {
    crate::drivers::framebuffer::print(s);
}

/// Print a string with a specific color
pub fn print_colored(s: &str, color: u32) {
    crate::drivers::framebuffer::print_colored(s, color);
}

/// Print a line (with newline)
pub fn println(s: &str) {
    print(s);
    print("\n");
}

/// Clear the screen
pub fn clear_screen() {
    crate::drivers::framebuffer::clear();
}

/// Read a character from keyboard (non-blocking)
pub fn read_char() -> Option<u8> {
    crate::interrupts::getchar()
}

/// Get cursor position
pub fn get_cursor_pos() -> (usize, usize) {
    crate::drivers::framebuffer::get_cursor_pos()
}

/// Set cursor position
pub fn set_cursor_pos(x: usize, y: usize) {
    crate::drivers::framebuffer::set_cursor_pos(x, y);
}

/// Clear current line
pub fn clear_line() {
    crate::drivers::framebuffer::clear_line();
}

/// Get memory info (total_kb, free_kb)
pub fn get_mem_info() -> (u64, u64) {
    let (total, _used, free) = crate::memory::frame::get_stats();
    ((total * 4) as u64, (free * 4) as u64)  // Pages to KB
}

/// Get CPU info (returns core count)
pub fn get_cpu_info() -> u64 {
    crate::arch::x86_64::cpu::get_cpu_count() as u64
}

/// List files in filesystem
pub fn list_files() -> Vec<String> {
    crate::fs::psychicfs::fs_list()
}

/// Yield to scheduler
pub fn yield_cpu() {
    // In kernel mode, just hint
    unsafe { core::arch::asm!("hlt"); }
}

/// Exit the shell
pub fn exit() -> ! {
    // Return to kernel main loop
    crate::drivers::framebuffer::clear();
    crate::print!("Shell exited. Returning to boot menu...\n");
    loop { unsafe { core::arch::asm!("hlt"); } }
}
