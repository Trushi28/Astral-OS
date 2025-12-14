//! Login Screen
//! User authentication before accessing system

use crate::drivers::framebuffer;
use crate::drivers::keyboard;
use alloc::string::String;

const MAX_ATTEMPTS: u32 = 3;

/// Show login screen and authenticate user
pub fn show_login() -> bool {
    let mut attempts = 0;
    
    loop {
        framebuffer::clear();
        
        // Header
        framebuffer::print_colored("\n\n", 0xFFFFFF);
        framebuffer::print_colored("  ╔═══════════════════════════════════════╗\n", 0x00AAFF);
        framebuffer::print_colored("  ║         ASTRAL OS LOGIN               ║\n", 0x00AAFF);
        framebuffer::print_colored("  ╚═══════════════════════════════════════╝\n\n", 0x00AAFF);
        
        if attempts > 0 {
            framebuffer::print_colored("  ⚠ Invalid credentials. Try again.\n\n", 0xFF5555);
        }
        
        // Username prompt
        framebuffer::print_colored("  Username: ", 0xAAAAAA);
        let username = read_line();
        
        if username.is_empty() {
            continue;
        }
        
        // Password prompt
        framebuffer::print_colored("  Password: ", 0xAAAAAA);
        let password = read_password();
        
        // Authenticate
        let db = super::AUTH_DB.lock();
        if let Some(user) = db.authenticate(&username, &password) {
            drop(db);
            
            // Login successful
            super::CURRENT_SESSION.lock().login(user);
            
            framebuffer::print_colored("\n\n  ✓ Login successful!\n", 0x55FF55);
            
            // Brief delay
            for _ in 0..5000000 { unsafe { core::arch::asm!("nop"); } }
            
            return true;
        }
        
        attempts += 1;
        if attempts >= MAX_ATTEMPTS {
            framebuffer::print_colored("\n\n  ✗ Too many failed attempts. System locked.\n", 0xFF5555);
            loop { unsafe { core::arch::asm!("hlt"); } }
        }
    }
}

fn read_line() -> String {
    let mut buffer = String::new();
    
    loop {
        if let Some(ch) = keyboard::try_read_char() {
            match ch {
                '\n' | '\r' => {
                    framebuffer::print_colored("\n", 0xFFFFFF);
                    return buffer;
                }
                '\x08' => {
                    if !buffer.is_empty() {
                        buffer.pop();
                        framebuffer::print_colored("\x08 \x08", 0xFFFFFF);
                    }
                }
                c if c.is_ascii_graphic() || c == ' ' => {
                    buffer.push(c);
                    framebuffer::print_char(c, 0xFFFFFF);
                }
                _ => {}
            }
        }
        
        // Small yield
        unsafe { core::arch::asm!("pause"); }
    }
}

fn read_password() -> String {
    let mut buffer = String::new();
    
    loop {
        if let Some(ch) = keyboard::try_read_char() {
            match ch {
                '\n' | '\r' => {
                    framebuffer::print_colored("\n", 0xFFFFFF);
                    return buffer;
                }
                '\x08' => {
                    if !buffer.is_empty() {
                        buffer.pop();
                        framebuffer::print_colored("\x08 \x08", 0xFFFFFF);
                    }
                }
                c if c.is_ascii_graphic() => {
                    buffer.push(c);
                    framebuffer::print_colored("•", 0xFFFFFF);  // Show dots
                }
                _ => {}
            }
        }
        
        unsafe { core::arch::asm!("pause"); }
    }
}
