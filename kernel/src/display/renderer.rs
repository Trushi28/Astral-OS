//! Display Server Renderer
//! Renders the desktop using the display server's window list

use super::{with_server, DisplayServer};
use crate::drivers::framebuffer;
use alloc::vec::Vec;

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

/// Render the complete display
pub fn render_frame() {
    with_server(|server| {
        render_background(server);
        render_windows(server);
        render_taskbar(server);
        render_cursor(server);
        server.mark_clean();
    });
}

fn render_background(server: &DisplayServer) {
    let height = server.screen_height - server.taskbar_height;
    let width = server.screen_width;
    
    // Optimized: render gradient row by row
    for y in 0..height {
        let t = y as f32 / height as f32;
        let color = lerp_color(THEME.background_top, THEME.background_bottom, t);
        let row: Vec<u32> = alloc::vec![color; width as usize];
        framebuffer::blit_buffer(&row, 0, y as usize, width as usize, 1);
    }
}

fn render_windows(server: &DisplayServer) {
    let title_height = 30u32;
    
    for window in server.windows_sorted() {
        if !window.visible {
            continue;
        }
        
        // Window background
        let win_buf: Vec<u32> = alloc::vec![THEME.window_bg; (window.width * window.height) as usize];
        framebuffer::blit_buffer(&win_buf, window.x as usize, window.y as usize, window.width as usize, window.height as usize);
        
        // Title bar
        let title_color = if window.focused { THEME.window_title } else { THEME.window_title_inactive };
        let title_buf: Vec<u32> = alloc::vec![title_color; (window.width * title_height) as usize];
        framebuffer::blit_buffer(&title_buf, window.x as usize, window.y as usize, window.width as usize, title_height as usize);
        
        // Window buttons (macOS style)
        draw_circle(window.x + 16, window.y + 15, 6, THEME.button_close);
        draw_circle(window.x + 36, window.y + 15, 6, THEME.button_maximize);
        draw_circle(window.x + 56, window.y + 15, 6, THEME.button_minimize);
        
        // Title text
        draw_text(window.x + 80, window.y + 8, &window.title, THEME.taskbar_text);
        
        // Border
        draw_rect_outline(window.x, window.y, window.width, window.height, THEME.window_border);
        
        // Render surface content (from graphics server)
        render_surface_content(window.surface_id, window.x, window.y + title_height as i32, 
                              window.width, window.height - title_height);
    }
}

fn render_surface_content(surface_id: u64, x: i32, y: i32, w: u32, h: u32) {
    // Get surface pixels from graphics server and blit
    crate::graphics::with_server(|gs| {
        if let Some(surface) = gs.get_surface_mut(surface_id) {
            // Convert RGBA to screen format and blit
            let bytes_per_pixel = surface.format.bytes_per_pixel();
            let pixels: Vec<u32> = (0..(w * h) as usize)
                .map(|i| {
                    let offset = i * bytes_per_pixel;
                    if offset + 3 < surface.pixels.len() {
                        let r = surface.pixels[offset] as u32;
                        let g = surface.pixels[offset + 1] as u32;
                        let b = surface.pixels[offset + 2] as u32;
                        (r << 16) | (g << 8) | b
                    } else {
                        THEME.window_bg
                    }
                })
                .collect();
            
            framebuffer::blit_buffer(&pixels, x as usize, y as usize, w as usize, h as usize);
        }
    });
}

fn render_taskbar(server: &DisplayServer) {
    if !server.taskbar_visible {
        return;
    }
    
    let y = server.screen_height - server.taskbar_height;
    let width = server.screen_width;
    
    // Taskbar background
    let taskbar_buf: Vec<u32> = alloc::vec![THEME.taskbar_bg; (width * server.taskbar_height) as usize];
    framebuffer::blit_buffer(&taskbar_buf, 0, y as usize, width as usize, server.taskbar_height as usize);
    
    // Start button
    let start_buf: Vec<u32> = alloc::vec![0x3d3d3d; (60 * 30) as usize];
    framebuffer::blit_buffer(&start_buf, 10, (y + 5) as usize, 60, 30);
    draw_text(20, (y + 12) as i32, "Start", THEME.taskbar_text);
    
    // Window buttons
    let mut btn_x = 80u32;
    for window in server.windows_sorted() {
        let btn_color = if window.focused { THEME.accent } else { 0x3d3d3d };
        let btn_buf: Vec<u32> = alloc::vec![btn_color; (100 * 30) as usize];
        framebuffer::blit_buffer(&btn_buf, btn_x as usize, (y + 5) as usize, 100, 30);
        
        // Truncate title
        let title: alloc::string::String = if window.title.len() > 10 {
            alloc::format!("{}...", &window.title[..8])
        } else {
            window.title.clone()
        };
        draw_text((btn_x + 8) as i32, (y + 12) as i32, &title, THEME.taskbar_text);
        
        btn_x += 110;
        if btn_x > width - 150 {
            break;
        }
    }
    
    // Clock
    let clock_x = width - 70;
    let clock_buf: Vec<u32> = alloc::vec![0x3d3d3d; (60 * 30) as usize];
    framebuffer::blit_buffer(&clock_buf, clock_x as usize, (y + 5) as usize, 60, 30);
    
    // Get real time from RTC
    let time_str = crate::drivers::rtc::format_time();
    draw_text((clock_x + 10) as i32, (y + 12) as i32, &time_str, THEME.taskbar_text);
}

fn render_cursor(_server: &DisplayServer) {
    let (mx, my) = crate::drivers::mouse::get_position();
    let is_clicking = crate::drivers::mouse::is_left_pressed();
    
    // Cursor colors
    let fill_color = if is_clicking { 0x00AAFF } else { 0xFFFFFF };
    let outline_color = 0x000000;
    
    // Proper arrow cursor pattern (classic pointer shape)
    // Each row defines the cursor pixels
    let cursor_rows: &[(i32, i32)] = &[
        // Row 0: tip
        (0, 0),
        // Row 1-10: main arrow body
        (0, 1), (1, 1),
        (0, 2), (1, 2), (2, 2),
        (0, 3), (1, 3), (2, 3), (3, 3),
        (0, 4), (1, 4), (2, 4), (3, 4), (4, 4),
        (0, 5), (1, 5), (2, 5), (3, 5), (4, 5), (5, 5),
        (0, 6), (1, 6), (2, 6), (3, 6), (4, 6), (5, 6), (6, 6),
        (0, 7), (1, 7), (2, 7), (3, 7), (4, 7), (5, 7),
        (0, 8), (1, 8), (2, 8), (3, 8), (4, 8), (5, 8),
        (0, 9), (1, 9), (2, 9), (3, 9),
        // Arrow tail
        (0, 10), (1, 10), (4, 10), (5, 10),
        (0, 11), (1, 11), (4, 11), (5, 11), (6, 11),
        (5, 12), (6, 12), (7, 12),
        (6, 13), (7, 13),
    ];
    
    // Draw outline (black border)
    for &(dx, dy) in cursor_rows {
        let x = (mx + dx) as usize;
        let y = (my + dy) as usize;
        
        // Draw outline pixels around each cursor pixel
        if dx > 0 {
            framebuffer::blit_buffer(&[outline_color], x - 1, y, 1, 1);
        }
        framebuffer::blit_buffer(&[outline_color], x + 1, y, 1, 1);
        if dy > 0 {
            framebuffer::blit_buffer(&[outline_color], x, y - 1, 1, 1);
        }
        framebuffer::blit_buffer(&[outline_color], x, y + 1, 1, 1);
    }
    
    // Draw cursor fill
    for &(dx, dy) in cursor_rows {
        let x = (mx + dx) as usize;
        let y = (my + dy) as usize;
        framebuffer::blit_buffer(&[fill_color], x, y, 1, 1);
    }
}

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

fn draw_circle(cx: i32, cy: i32, r: i32, color: u32) {
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r {
                let x = (cx + dx) as usize;
                let y = (cy + dy) as usize;
                framebuffer::blit_buffer(&[color], x, y, 1, 1);
            }
        }
    }
}

fn draw_rect_outline(x: i32, y: i32, w: u32, h: u32, color: u32) {
    // Top and bottom
    let horiz: Vec<u32> = alloc::vec![color; w as usize];
    framebuffer::blit_buffer(&horiz, x as usize, y as usize, w as usize, 1);
    framebuffer::blit_buffer(&horiz, x as usize, (y + h as i32 - 1) as usize, w as usize, 1);
    
    // Left and right
    for dy in 0..h {
        framebuffer::blit_buffer(&[color], x as usize, (y + dy as i32) as usize, 1, 1);
        framebuffer::blit_buffer(&[color], (x + w as i32 - 1) as usize, (y + dy as i32) as usize, 1, 1);
    }
}

fn draw_text(x: i32, y: i32, text: &str, color: u32) {
    framebuffer::set_cursor_pos(x as usize, y as usize);
    framebuffer::print_colored(text, color);
}
