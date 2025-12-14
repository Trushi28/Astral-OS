//! Desktop GUI System - Interactive Windows
//! 
//! This module provides the user-facing desktop experience with
//! interactive windows for file management and system info.

pub mod window;
pub mod desktop;
pub mod theme;
pub mod font;

use alloc::vec::Vec;
use alloc::string::String;
use alloc::format;
use spin::Mutex;
use crate::display;

static GUI_RUNNING: Mutex<bool> = Mutex::new(false);

/// State for the file browser
struct FileBrowser {
    files: Vec<String>,
    selected: usize,
}

impl FileBrowser {
    fn new() -> Self {
        Self {
            files: crate::fs::psychicfs::fs_list(),
            selected: 0,
        }
    }
    
    fn refresh(&mut self) {
        self.files = crate::fs::psychicfs::fs_list();
        if self.selected >= self.files.len() && !self.files.is_empty() {
            self.selected = self.files.len() - 1;
        }
    }
    
    fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }
    
    fn move_down(&mut self) {
        if self.selected + 1 < self.files.len() {
            self.selected += 1;
        }
    }
    
    fn selected_file(&self) -> Option<&String> {
        self.files.get(self.selected)
    }
}

/// Initialize the GUI system using display server
pub fn init() {
    display::init();
    crate::serial_println!("[GUI] Using Astral Display Server");
}

/// Run the GUI main loop
pub fn run() {
    crate::drivers::framebuffer::clear();
    
    display::init();
    
    let client_id = display::register_client("Desktop").unwrap_or(0);
    
    // Create windows
    let files_id = display::create_window(client_id, "Files", 400, 350).ok();
    let about_id = display::create_window(client_id, "About Astral", 350, 250).ok();
    let sysinfo_id = display::create_window(client_id, "System Info", 350, 280).ok();
    
    // File browser state
    let mut file_browser = FileBrowser::new();
    
    *GUI_RUNNING.lock() = true;
    
    // Draw initial content
    if let Some(id) = files_id {
        draw_file_browser(id, &file_browser);
    }
    if let Some(id) = about_id {
        draw_about_window(id);
    }
    if let Some(id) = sysinfo_id {
        draw_sysinfo_window(id);
    }
    
    display::renderer::render_frame();
    
    // Mouse state for click and drag
    let mut last_mouse_pressed = false;
    let mut dragging: Option<(u64, i32, i32)> = None;  // (window_id, offset_x, offset_y)
    let mut last_mouse_pos = crate::drivers::mouse::get_position();
    
    // Main loop - continuous rendering for mouse
    loop {
        // Get current mouse state
        let (mx, my) = crate::drivers::mouse::get_position();
        let mouse_pressed = crate::drivers::mouse::is_left_pressed();
        let mouse_moved = mx != last_mouse_pos.0 || my != last_mouse_pos.1;
        last_mouse_pos = (mx, my);
        
        // Handle mouse click
        if mouse_pressed && !last_mouse_pressed {
            // Mouse just clicked - check what's under cursor
            if let Some((win_id, hit_x, hit_y)) = hit_test_window(mx, my) {
                // Focus the window
                display::focus_window(win_id);
                
                // Check if clicking title bar (first 30 pixels)
                if hit_y < 30 {
                    // Start dragging
                    dragging = Some((win_id, hit_x, hit_y));
                }
            }
        }
        
        // Handle mouse release
        if !mouse_pressed && last_mouse_pressed {
            dragging = None;
        }
        
        // Handle dragging
        if mouse_pressed {
            if let Some((win_id, offset_x, offset_y)) = dragging {
                let new_x = mx - offset_x;
                let new_y = my - offset_y;
                display::with_server(|s| s.move_window(win_id, new_x, new_y));
            }
        }
        
        last_mouse_pressed = mouse_pressed;
        
        // Handle keyboard input
        if let Some(key) = crate::interrupts::getchar() {
            let focused = display::with_server(|s| s.focused_window).flatten();
            
            match key {
                b'q' | 27 => break,  // Quit
                b'\t' => {
                    display::focus_next();
                }
                b'c' => {
                    if let Some(win) = focused {
                        let _ = display::destroy_window(win);
                    }
                }
                // File browser navigation
                b'j' | 80 => {
                    if focused == files_id {
                        file_browser.move_down();
                        if let Some(id) = files_id {
                            draw_file_browser(id, &file_browser);
                        }
                    }
                }
                b'k' | 72 => {
                    if focused == files_id {
                        file_browser.move_up();
                        if let Some(id) = files_id {
                            draw_file_browser(id, &file_browser);
                        }
                    }
                }
                b'r' => {
                    if focused == files_id {
                        file_browser.refresh();
                        if let Some(id) = files_id {
                            draw_file_browser(id, &file_browser);
                        }
                    }
                }
                b'd' => {
                    if focused == files_id {
                        if let Some(filename) = file_browser.selected_file() {
                            crate::fs::psychicfs::fs_delete(filename);
                            file_browser.refresh();
                            if let Some(id) = files_id {
                                draw_file_browser(id, &file_browser);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        
        // Redraw on click/release or keyboard input
        // Mouse movement also triggers redraw (will optimize later with dirty rects)
        let needs_redraw = mouse_moved || (mouse_pressed != last_mouse_pressed);
        
        if needs_redraw {
            display::renderer::render_frame();
        }
        
        // Yield to interrupts
        unsafe { 
            core::arch::asm!(
                "sti",
                "hlt",
                options(nomem, nostack)
            ); 
        }
    }
    
    *GUI_RUNNING.lock() = false;
    crate::drivers::framebuffer::clear();
}

/// Hit test to find window under cursor
/// Returns (window_id, relative_x, relative_y) if hit
fn hit_test_window(mx: i32, my: i32) -> Option<(u64, i32, i32)> {
    display::with_server(|server| {
        // Check windows in reverse z-order (top first)
        let mut sorted = server.windows_sorted();
        sorted.reverse();
        
        for window in sorted {
            if mx >= window.x && mx < window.x + window.width as i32 &&
               my >= window.y && my < window.y + window.height as i32 {
                return Some((window.id, mx - window.x, my - window.y));
            }
        }
        None
    }).flatten()
}

fn draw_file_browser(window_id: u64, browser: &FileBrowser) {
    display::with_server(|server| {
        if let Some(window) = server.windows.iter().find(|w| w.id == window_id) {
            let surface_id = window.surface_id;
            crate::graphics::with_server(|gs| {
                if let Some(surface) = gs.get_surface_mut(surface_id) {
                    let bpp = surface.format.bytes_per_pixel();
                    let w = surface.width;
                    let h = surface.height;
                    
                    // Dark background
                    for i in 0..(w * h) as usize {
                        let offset = i * bpp;
                        if offset + 3 < surface.pixels.len() {
                            surface.pixels[offset] = 0x2d;
                            surface.pixels[offset + 1] = 0x2d;
                            surface.pixels[offset + 2] = 0x2d;
                            surface.pixels[offset + 3] = 0xFF;
                        }
                    }
                    
                    // Header bar
                    for y in 0..30u32 {
                        for x in 0..w {
                            let idx = ((y * w + x) as usize) * bpp;
                            if idx + 3 < surface.pixels.len() {
                                surface.pixels[idx] = 0x38;
                                surface.pixels[idx + 1] = 0x38;
                                surface.pixels[idx + 2] = 0x38;
                                surface.pixels[idx + 3] = 0xFF;
                            }
                        }
                    }
                    
                    // Header text with REAL FONT
                    font::draw_string(surface, 10, 8, "File Browser", 0xFFFFFF);
                    let count_str = format!("{} files", browser.files.len());
                    font::draw_string(surface, 180, 8, &count_str, 0x888888);
                    
                    // File list
                    let mut y = 40u32;
                    for (i, file) in browser.files.iter().enumerate() {
                        if y > h - 40 {
                            break;
                        }
                        
                        // Selection highlight
                        if i == browser.selected {
                            for dy in 0..20u32 {
                                for x in 5..w-5 {
                                    let idx = (((y + dy) * w + x) as usize) * bpp;
                                    if idx + 3 < surface.pixels.len() {
                                        surface.pixels[idx] = 0x0a;
                                        surface.pixels[idx + 1] = 0x84;
                                        surface.pixels[idx + 2] = 0xff;
                                        surface.pixels[idx + 3] = 0xFF;
                                    }
                                }
                            }
                        }
                        
                        // File icon (simple square)
                        for dy in 0..12u32 {
                            for dx in 0..12u32 {
                                let px = 15 + dx;
                                let py = y + 4 + dy;
                                if px < w && py < h {
                                    let idx = ((py * w + px) as usize) * bpp;
                                    if idx + 3 < surface.pixels.len() {
                                        surface.pixels[idx] = 0x44;
                                        surface.pixels[idx + 1] = 0x88;
                                        surface.pixels[idx + 2] = 0xFF;
                                        surface.pixels[idx + 3] = 0xFF;
                                    }
                                }
                            }
                        }
                        
                        // Filename with REAL FONT
                        let text_color = if i == browser.selected { 0xFFFFFF } else { 0xE0E0E0 };
                        font::draw_string(surface, 35, y + 4, file, text_color);
                        
                        y += 24;
                    }
                    
                    // Empty state
                    if browser.files.is_empty() {
                        font::draw_string(surface, 120, 100, "No files", 0x888888);
                    }
                    
                    // Help text at bottom with REAL FONT
                    font::draw_string(surface, 10, h - 20, "j/k:Navigate  d:Delete  r:Refresh", 0x666666);
                }
            });
        }
    });
}

fn draw_about_window(window_id: u64) {
    display::with_server(|server| {
        if let Some(window) = server.windows.iter().find(|w| w.id == window_id) {
            let surface_id = window.surface_id;
            crate::graphics::with_server(|gs| {
                if let Some(surface) = gs.get_surface_mut(surface_id) {
                    let bpp = surface.format.bytes_per_pixel();
                    let w = surface.width;
                    let h = surface.height;
                    
                    // Gradient background
                    for y in 0..h {
                        for x in 0..w {
                            let idx = ((y * w + x) as usize) * bpp;
                            if idx + 3 < surface.pixels.len() {
                                let t = y as f32 / h as f32;
                                surface.pixels[idx] = lerp(0x16, 0x0f, t);
                                surface.pixels[idx + 1] = lerp(0x21, 0x34, t);
                                surface.pixels[idx + 2] = lerp(0x3e, 0x60, t);
                                surface.pixels[idx + 3] = 0xFF;
                            }
                        }
                    }
                    
                    // Logo area
                    let logo_color = 0x00AAFF;
                    for y in 30u32..70 {
                        for x in 60u32..100 {
                            let idx = ((y * w + x) as usize) * bpp;
                            if idx + 3 < surface.pixels.len() {
                                surface.pixels[idx] = ((logo_color >> 16) & 0xFF) as u8;
                                surface.pixels[idx + 1] = ((logo_color >> 8) & 0xFF) as u8;
                                surface.pixels[idx + 2] = (logo_color & 0xFF) as u8;
                                surface.pixels[idx + 3] = 0xFF;
                            }
                        }
                    }
                    
                    font::draw_string(surface, 120, 40, "Astral OS", 0xFFFFFF);
                    font::draw_string(surface, 120, 60, "v0.3.1", 0x888888);
                    
                    font::draw_string(surface, 30, 100, "Reality-Aware Operating System", 0xE0E0E0);
                    font::draw_string(surface, 30, 130, "Features:", 0x888888);
                    font::draw_string(surface, 40, 150, "- Extent-based filesystem", 0xE0E0E0);
                    font::draw_string(surface, 40, 170, "- IPC with blocking", 0xE0E0E0);
                    font::draw_string(surface, 40, 190, "- GUI display server", 0xE0E0E0);
                }
            });
        }
    });
}

fn draw_sysinfo_window(window_id: u64) {
    display::with_server(|server| {
        if let Some(window) = server.windows.iter().find(|w| w.id == window_id) {
            let surface_id = window.surface_id;
            crate::graphics::with_server(|gs| {
                if let Some(surface) = gs.get_surface_mut(surface_id) {
                    let bpp = surface.format.bytes_per_pixel();
                    let w = surface.width;
                    let h = surface.height;
                    
                    // Dark background
                    for i in 0..(w * h) as usize {
                        let offset = i * bpp;
                        if offset + 3 < surface.pixels.len() {
                            surface.pixels[offset] = 0x1a;
                            surface.pixels[offset + 1] = 0x1a;
                            surface.pixels[offset + 2] = 0x2e;
                            surface.pixels[offset + 3] = 0xFF;
                        }
                    }
                    
                    font::draw_string(surface, 10, 10, "System Information", 0x00AAFF);
                    
                    // Memory info
                    let (total, used, free) = crate::memory::frame::get_stats();
                    
                    font::draw_string(surface, 10, 40, "Memory:", 0x888888);
                    let used_str = format!("Used: {} KB", used / 1024);
                    font::draw_string(surface, 20, 60, &used_str, 0xE0E0E0);
                    let free_str = format!("Free: {} KB", free / 1024);
                    font::draw_string(surface, 20, 80, &free_str, 0xE0E0E0);
                    
                    // Filesystem info
                    let files = crate::fs::psychicfs::fs_list().len();
                    font::draw_string(surface, 10, 110, "Filesystem:", 0x888888);
                    let files_str = format!("Files: {}", files);
                    font::draw_string(surface, 20, 130, &files_str, 0xE0E0E0);
                    
                    // CPU info
                    let cpu_count = crate::get_cpu_count();
                    font::draw_string(surface, 10, 160, "CPU:", 0x888888);
                    let cpu_str = format!("Cores: {}", cpu_count);
                    font::draw_string(surface, 20, 180, &cpu_str, 0xE0E0E0);
                    
                    // Screen info
                    let (sw, sh) = crate::drivers::framebuffer::get_dimensions();
                    font::draw_string(surface, 10, 210, "Display:", 0x888888);
                    let display_str = format!("{}x{}", sw, sh);
                    font::draw_string(surface, 20, 230, &display_str, 0xE0E0E0);
                }
            });
        }
    });
}

fn draw_text_on_surface(surface: &mut crate::graphics::surface::Surface, x: u32, y: u32, text: &str, color: u32) {
    let bpp = surface.format.bytes_per_pixel();
    let r = ((color >> 16) & 0xFF) as u8;
    let g = ((color >> 8) & 0xFF) as u8;
    let b = (color & 0xFF) as u8;
    
    for (i, _ch) in text.chars().enumerate() {
        for dy in 0..8u32 {
            for dx in 0..6u32 {
                let px = x + (i as u32 * 8) + dx;
                let py = y + dy;
                if px < surface.width && py < surface.height {
                    let idx = ((py * surface.width + px) as usize) * bpp;
                    if idx + 3 < surface.pixels.len() {
                        if dy > 1 && dy < 7 && dx > 0 && dx < 5 {
                            surface.pixels[idx] = r;
                            surface.pixels[idx + 1] = g;
                            surface.pixels[idx + 2] = b;
                            surface.pixels[idx + 3] = 0xFF;
                        }
                    }
                }
            }
        }
    }
}

fn draw_icon(surface: &mut crate::graphics::surface::Surface, x: u32, y: u32, color: u32, size: u32) {
    let bpp = surface.format.bytes_per_pixel();
    let r = ((color >> 16) & 0xFF) as u8;
    let g = ((color >> 8) & 0xFF) as u8;
    let b = (color & 0xFF) as u8;
    
    for dy in 0..size {
        for dx in 0..size {
            let px = x + dx;
            let py = y + dy;
            if px < surface.width && py < surface.height {
                let idx = ((py * surface.width + px) as usize) * bpp;
                if idx + 3 < surface.pixels.len() {
                    surface.pixels[idx] = r;
                    surface.pixels[idx + 1] = g;
                    surface.pixels[idx + 2] = b;
                    surface.pixels[idx + 3] = 0xFF;
                }
            }
        }
    }
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t) as u8
}

/// Check if GUI is running
pub fn is_running() -> bool {
    *GUI_RUNNING.lock()
}
