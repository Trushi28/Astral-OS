//! Desktop GUI System - Uses Astral Display Server
//! 
//! This module provides the user-facing desktop experience,
//! using the display server for all graphics operations.

pub mod window;
pub mod desktop;
pub mod theme;

use spin::Mutex;
use crate::display;

static GUI_RUNNING: Mutex<bool> = Mutex::new(false);

/// Initialize the GUI system using display server
pub fn init() {
    // Initialize display server
    display::init();
    crate::serial_println!("[GUI] Using Astral Display Server");
}

/// Run the GUI main loop
pub fn run() {
    // Clear screen
    crate::drivers::framebuffer::clear();
    
    // Initialize display server if not already
    display::init();
    
    // Register as desktop client
    let client_id = display::register_client("Desktop").unwrap_or(0);
    
    // Create initial windows via display server
    let _ = display::create_window(client_id, "Terminal", 500, 350);
    let _ = display::create_window(client_id, "Files", 400, 300);
    let _ = display::create_window(client_id, "About", 320, 220);
    
    *GUI_RUNNING.lock() = true;
    
    // Initial render
    display::renderer::render_frame();
    
    crate::println!("");
    crate::println!("Astral Display Server Active");
    crate::println!("Press Tab to switch windows, 'q' to exit");
    
    // Main loop
    loop {
        if let Some(key) = crate::interrupts::getchar() {
            match key {
                b'q' | 27 => break,  // Quit
                b'\t' => {
                    display::focus_next();
                    display::renderer::render_frame();
                }
                _ => {
                    // Route to focused window
                    // For now just redraw
                    display::renderer::render_frame();
                }
            }
        }
        
        // Check if needs redraw
        let needs_redraw = display::with_server(|s| s.needs_redraw()).unwrap_or(false);
        if needs_redraw {
            display::renderer::render_frame();
        }
        
        unsafe { core::arch::asm!("hlt"); }
    }
    
    *GUI_RUNNING.lock() = false;
    crate::drivers::framebuffer::clear();
}

/// Check if GUI is running
pub fn is_running() -> bool {
    *GUI_RUNNING.lock()
}
