//! Boot Menu
//! Post-login selection screen

use crate::drivers::framebuffer;
use crate::drivers::keyboard;
use crate::auth::CURRENT_SESSION;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BootOption {
    Shell,      // Ring 3 shell
    Graphics,   // Ring 3 GUI
    KShell,     // Ring 0 kernel shell (root only)
}

/// Show boot menu and get user selection
pub fn show_menu() -> BootOption {
    let session = CURRENT_SESSION.lock();
    let username = session.username();
    let is_root = session.is_root();
    drop(session);
    
    loop {
        framebuffer::clear();
        
        // Welcome header
        framebuffer::print_colored("\n\n", 0xFFFFFF);
        framebuffer::print_colored("  ╔═══════════════════════════════════════╗\n", 0x00AAFF);
        let welcome = alloc::format!("  ║  Welcome, {:27}║\n", username);
        framebuffer::print_colored(&welcome, 0x00AAFF);
        framebuffer::print_colored("  ╚═══════════════════════════════════════╝\n\n", 0x00AAFF);
        
        // Menu options
        framebuffer::print_colored("  Select an option:\n\n", 0xAAAAAA);
        
        framebuffer::print_colored("    [1] ", 0x55FF55);
        framebuffer::print_colored("Shell       ", 0xFFFFFF);
        framebuffer::print_colored("- Command line interface\n", 0x888888);
        
        framebuffer::print_colored("    [2] ", 0x55FF55);
        framebuffer::print_colored("Graphics    ", 0xFFFFFF);
        framebuffer::print_colored("- Desktop GUI\n", 0x888888);
        
        if is_root {
            framebuffer::print_colored("    [3] ", 0xFF5555);
            framebuffer::print_colored("Kernel Shell", 0xFFFFFF);
            framebuffer::print_colored(" - Root access (Ring 0)\n", 0x888888);
        } else {
            framebuffer::print_colored("    [3] ", 0x444444);
            framebuffer::print_colored("Kernel Shell", 0x666666);
            framebuffer::print_colored(" - Requires root\n", 0x444444);
        }
        
        framebuffer::print_colored("\n\n  Choice: ", 0xAAAAAA);
        
        // Wait for input
        loop {
            if let Some(ch) = keyboard::try_read_char() {
                match ch {
                    '1' => {
                        framebuffer::print_colored("1\n\n  Starting Shell...\n", 0x55FF55);
                        delay();
                        return BootOption::Shell;
                    }
                    '2' => {
                        framebuffer::print_colored("2\n\n  Starting Graphics...\n", 0x55FF55);
                        delay();
                        return BootOption::Graphics;
                    }
                    '3' if is_root => {
                        framebuffer::print_colored("3\n\n  Starting Kernel Shell...\n", 0xFF5555);
                        delay();
                        return BootOption::KShell;
                    }
                    '3' => {
                        framebuffer::print_colored("3\n\n  ⚠ Access denied. Root required.\n", 0xFF5555);
                        delay();
                        break; // Redraw menu
                    }
                    _ => {}
                }
            }
            unsafe { core::arch::asm!("pause"); }
        }
    }
}

fn delay() {
    for _ in 0..3000000 { unsafe { core::arch::asm!("nop"); } }
}
