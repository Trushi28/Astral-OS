//! Display Server Renderer
//! Renders the desktop using the display server's window list
//! Uses double-buffering to prevent flickering

use super::{with_server, DisplayServer};
use crate::drivers::framebuffer;
use alloc::vec::Vec;
use spin::Mutex;

/// Maximum screen size for back buffer (1920x1080)
const MAX_BUFFER_SIZE: usize = 1920 * 1080;

/// Static back buffer for double-buffering
static BACK_BUFFER: Mutex<Option<Vec<u32>>> = Mutex::new(None);

/// Theme colors
pub struct Theme {
    pub background_top: u32,
    pub background_bottom: u32,
    pub taskbar_bg: u32,
    pub taskbar_text: u32,
    pub window_bg: u32,
    pub window_title: u32,
    pub window_title_inactive: u32,
    pub window_border: u32,
    pub button_close: u32,
    pub button_maximize: u32,
    pub button_minimize: u32,
    pub accent: u32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background_top: 0x16213e,
            background_bottom: 0x0f3460,
            taskbar_bg: 0x252525,
            taskbar_text: 0xffffff,
            window_bg: 0x2d2d2d,
            window_title: 0x383838,
            window_title_inactive: 0x2a2a2a,
            window_border: 0x3d3d3d,
            button_close: 0xff5f57,
            button_maximize: 0x28c840,
            button_minimize: 0xfebc2e,
            accent: 0x0a84ff,
        }
    }
}

static THEME: Theme = Theme {
    background_top: 0x16213e,
    background_bottom: 0x0f3460,
    taskbar_bg: 0x252525,
    taskbar_text: 0xffffff,
    window_bg: 0x2d2d2d,
    window_title: 0x383838,
    window_title_inactive: 0x2a2a2a,
    window_border: 0x3d3d3d,
    button_close: 0xff5f57,
    button_maximize: 0x28c840,
    button_minimize: 0xfebc2e,
    accent: 0x0a84ff,
};

/// Render the complete display with double-buffering
pub fn render_frame() {
    with_server(|server| {
        let width = server.screen_width as usize;
        let height = server.screen_height as usize;
        let size = width * height;
        
        // Initialize or resize back buffer
        {
            let mut buffer_guard = BACK_BUFFER.lock();
            if buffer_guard.is_none() || buffer_guard.as_ref().map(|b| b.len()).unwrap_or(0) < size {
                *buffer_guard = Some(alloc::vec![0u32; size.min(MAX_BUFFER_SIZE)]);
            }
        }
        
        // Render to back buffer
        render_background_to_buffer(server);
        render_windows_to_buffer(server);
        render_taskbar_to_buffer(server);
        render_cursor_to_buffer(server);
        
        // Single blit to framebuffer
        {
            let buffer_guard = BACK_BUFFER.lock();
            if let Some(ref buffer) = *buffer_guard {
                framebuffer::blit_buffer(buffer, 0, 0, width, height);
            }
        }
        
        server.mark_clean();
    });
}

fn render_background_to_buffer(server: &DisplayServer) {
    let height = server.screen_height - server.taskbar_height;
    let width = server.screen_width;
    
    let mut buffer_guard = BACK_BUFFER.lock();
    if let Some(ref mut buffer) = *buffer_guard {
        // Render gradient directly to buffer
        for y in 0..height {
            let t = y as f32 / height as f32;
            let color = lerp_color(THEME.background_top, THEME.background_bottom, t);
            let start = (y * width) as usize;
            let end = start + width as usize;
            if end <= buffer.len() {
                for i in start..end {
                    buffer[i] = color;
                }
            }
        }
    }
}

fn render_windows_to_buffer(server: &DisplayServer) {
    let title_height = 30u32;
    let screen_width = server.screen_width as usize;
    
    let mut buffer_guard = BACK_BUFFER.lock();
    if let Some(ref mut buffer) = *buffer_guard {
        for window in server.windows_sorted() {
            if !window.visible {
                continue;
            }
            
            let wx = window.x.max(0) as usize;
            let wy = window.y.max(0) as usize;
            
            // Window background
            for dy in 0..window.height as usize {
                let row_start = (wy + dy) * screen_width + wx;
                for dx in 0..window.width as usize {
                    if row_start + dx < buffer.len() {
                        buffer[row_start + dx] = THEME.window_bg;
                    }
                }
            }
            
            // Title bar
            let title_color = if window.focused { THEME.window_title } else { THEME.window_title_inactive };
            for dy in 0..title_height as usize {
                let row_start = (wy + dy) * screen_width + wx;
                for dx in 0..window.width as usize {
                    if row_start + dx < buffer.len() {
                        buffer[row_start + dx] = title_color;
                    }
                }
            }
            
            // Window buttons (macOS style)
            draw_circle_to_buffer(buffer, screen_width, window.x + 16, window.y + 15, 6, THEME.button_close);
            draw_circle_to_buffer(buffer, screen_width, window.x + 36, window.y + 15, 6, THEME.button_maximize);
            draw_circle_to_buffer(buffer, screen_width, window.x + 56, window.y + 15, 6, THEME.button_minimize);
            
            // Title text (use framebuffer text for now - will render on top)
            // Border
            draw_rect_outline_to_buffer(buffer, screen_width, window.x, window.y, window.width, window.height, THEME.window_border);
            
            // Render surface content
            render_surface_to_buffer(buffer, screen_width, window.surface_id, window.x, window.y + title_height as i32, 
                                    window.width, window.height - title_height);
        }
    }
    
    // Draw window titles (done separately to use framebuffer text rendering)
    for window in server.windows_sorted() {
        if window.visible {
            draw_text(window.x + 80, window.y + 8, &window.title, THEME.taskbar_text);
        }
    }
}

fn render_surface_to_buffer(buffer: &mut [u32], screen_width: usize, surface_id: u64, x: i32, y: i32, w: u32, h: u32) {
    crate::graphics::with_server(|gs| {
        if let Some(surface) = gs.get_surface_mut(surface_id) {
            let bytes_per_pixel = surface.format.bytes_per_pixel();
            
            for dy in 0..h as usize {
                for dx in 0..w as usize {
                    let buf_x = (x as usize).saturating_add(dx);
                    let buf_y = (y as usize).saturating_add(dy);
                    let buf_idx = buf_y * screen_width + buf_x;
                    
                    let surf_idx = (dy * w as usize + dx) * bytes_per_pixel;
                    
                    if buf_idx < buffer.len() && surf_idx + 3 < surface.pixels.len() {
                        let r = surface.pixels[surf_idx] as u32;
                        let g = surface.pixels[surf_idx + 1] as u32;
                        let b = surface.pixels[surf_idx + 2] as u32;
                        buffer[buf_idx] = (r << 16) | (g << 8) | b;
                    }
                }
            }
        }
    });
}

fn render_taskbar_to_buffer(server: &DisplayServer) {
    if !server.taskbar_visible {
        return;
    }
    
    let y = (server.screen_height - server.taskbar_height) as usize;
    let width = server.screen_width as usize;
    let bar_height = server.taskbar_height as usize;
    
    let mut buffer_guard = BACK_BUFFER.lock();
    if let Some(ref mut buffer) = *buffer_guard {
        // Taskbar background
        for dy in 0..bar_height {
            let row_start = (y + dy) * width;
            for dx in 0..width {
                if row_start + dx < buffer.len() {
                    buffer[row_start + dx] = THEME.taskbar_bg;
                }
            }
        }
        
        // Start button
        for dy in 5..35 {
            let row_start = (y + dy) * width;
            for dx in 10..70 {
                if row_start + dx < buffer.len() {
                    buffer[row_start + dx] = 0x3d3d3d;
                }
            }
        }
        
        // Window buttons
        let mut btn_x = 80usize;
        for window in server.windows_sorted() {
            let btn_color = if window.focused { THEME.accent } else { 0x3d3d3d };
            for dy in 5..35 {
                let row_start = (y + dy) * width;
                for dx in 0..100 {
                    let px = btn_x + dx;
                    if row_start + px < buffer.len() {
                        buffer[row_start + px] = btn_color;
                    }
                }
            }
            btn_x += 110;
            if btn_x > width - 150 {
                break;
            }
        }
        
        // Clock background
        let clock_x = width - 70;
        for dy in 5..35 {
            let row_start = (y + dy) * width;
            for dx in 0..60 {
                if row_start + clock_x + dx < buffer.len() {
                    buffer[row_start + clock_x + dx] = 0x3d3d3d;
                }
            }
        }
    }
    
    // Draw text labels (using framebuffer text)
    let tb_y = (server.screen_height - server.taskbar_height) as i32;
    draw_text(20, tb_y + 12, "Start", THEME.taskbar_text);
    
    let mut btn_x = 80u32;
    for window in server.windows_sorted() {
        let title: alloc::string::String = if window.title.len() > 10 {
            alloc::format!("{}...", &window.title[..8])
        } else {
            window.title.clone()
        };
        draw_text((btn_x + 8) as i32, tb_y + 12, &title, THEME.taskbar_text);
        btn_x += 110;
        if btn_x > server.screen_width - 150 {
            break;
        }
    }
    
    // Clock
    let time_str = crate::drivers::rtc::format_time();
    draw_text((server.screen_width - 60) as i32, tb_y + 12, &time_str, THEME.taskbar_text);
}

fn render_cursor_to_buffer(server: &DisplayServer) {
    let (mx, my) = crate::drivers::mouse::get_position();
    let is_clicking = crate::drivers::mouse::is_left_pressed();
    let screen_width = server.screen_width as usize;
    
    let fill_color = if is_clicking { 0x00AAFF } else { 0xFFFFFF };
    let outline_color = 0x000000;
    
    let cursor_rows: &[(i32, i32)] = &[
        (0, 0),
        (0, 1), (1, 1),
        (0, 2), (1, 2), (2, 2),
        (0, 3), (1, 3), (2, 3), (3, 3),
        (0, 4), (1, 4), (2, 4), (3, 4), (4, 4),
        (0, 5), (1, 5), (2, 5), (3, 5), (4, 5), (5, 5),
        (0, 6), (1, 6), (2, 6), (3, 6), (4, 6), (5, 6), (6, 6),
        (0, 7), (1, 7), (2, 7), (3, 7), (4, 7), (5, 7),
        (0, 8), (1, 8), (2, 8), (3, 8), (4, 8), (5, 8),
        (0, 9), (1, 9), (2, 9), (3, 9),
        (0, 10), (1, 10), (4, 10), (5, 10),
        (0, 11), (1, 11), (4, 11), (5, 11), (6, 11),
        (5, 12), (6, 12), (7, 12),
        (6, 13), (7, 13),
    ];
    
    let mut buffer_guard = BACK_BUFFER.lock();
    if let Some(ref mut buffer) = *buffer_guard {
        // Draw outline
        for &(dx, dy) in cursor_rows {
            let x = (mx + dx) as usize;
            let y = (my + dy) as usize;
            
            // Outline pixels
            for &(ox, oy) in &[(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let px = (x as i32 + ox) as usize;
                let py = (y as i32 + oy) as usize;
                let idx = py * screen_width + px;
                if idx < buffer.len() {
                    buffer[idx] = outline_color;
                }
            }
        }
        
        // Draw fill
        for &(dx, dy) in cursor_rows {
            let x = (mx + dx) as usize;
            let y = (my + dy) as usize;
            let idx = y * screen_width + x;
            if idx < buffer.len() {
                buffer[idx] = fill_color;
            }
        }
    }
}

// Keep old functions for compatibility during transition
#[allow(dead_code)]
fn render_background_stub(_server: &DisplayServer) {}
#[allow(dead_code)]
fn render_windows_stub(_server: &DisplayServer) {}
#[allow(dead_code)]
fn render_taskbar_stub(_server: &DisplayServer) {}
#[allow(dead_code)]
fn render_cursor_stub(_server: &DisplayServer) {}

// Helper functions
fn lerp_color(c1: u32, c2: u32, t: f32) -> u32 {
    let r1 = ((c1 >> 16) & 0xFF) as f32;
    let g1 = ((c1 >> 8) & 0xFF) as f32;
    let b1 = (c1 & 0xFF) as f32;
    let r2 = ((c2 >> 16) & 0xFF) as f32;
    let g2 = ((c2 >> 8) & 0xFF) as f32;
    let b2 = (c2 & 0xFF) as f32;
    
    let r = (r1 + (r2 - r1) * t) as u32;
    let g = (g1 + (g2 - g1) * t) as u32;
    let b = (b1 + (b2 - b1) * t) as u32;
    (r << 16) | (g << 8) | b
}

fn draw_circle_to_buffer(buffer: &mut [u32], screen_width: usize, cx: i32, cy: i32, r: i32, color: u32) {
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r {
                let x = (cx + dx) as usize;
                let y = (cy + dy) as usize;
                let idx = y * screen_width + x;
                if idx < buffer.len() {
                    buffer[idx] = color;
                }
            }
        }
    }
}

fn draw_rect_outline_to_buffer(buffer: &mut [u32], screen_width: usize, x: i32, y: i32, w: u32, h: u32, color: u32) {
    // Top and bottom
    for dx in 0..w as usize {
        let top_idx = (y as usize) * screen_width + (x as usize) + dx;
        let bot_idx = ((y + h as i32 - 1) as usize) * screen_width + (x as usize) + dx;
        if top_idx < buffer.len() {
            buffer[top_idx] = color;
        }
        if bot_idx < buffer.len() {
            buffer[bot_idx] = color;
        }
    }
    
    // Left and right
    for dy in 0..h as usize {
        let left_idx = ((y as usize) + dy) * screen_width + (x as usize);
        let right_idx = ((y as usize) + dy) * screen_width + (x + w as i32 - 1) as usize;
        if left_idx < buffer.len() {
            buffer[left_idx] = color;
        }
        if right_idx < buffer.len() {
            buffer[right_idx] = color;
        }
    }
}

fn draw_text(x: i32, y: i32, text: &str, color: u32) {
    framebuffer::set_cursor_pos(x as usize, y as usize);
    framebuffer::print_colored(text, color);
}

