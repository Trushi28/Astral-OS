//! Display Server Renderer
//! Renders the desktop using the display server's window list
//! Uses double-buffering to prevent flickering

use super::{with_server, DisplayServer};
use crate::drivers::framebuffer;
use crate::gui::theme;
use alloc::vec::Vec;
use spin::Mutex;

/// Maximum screen size for back buffer (1920x1080)
const MAX_BUFFER_SIZE: usize = 1920 * 1080;

/// Static back buffer for double-buffering
static BACK_BUFFER: Mutex<Option<Vec<u32>>> = Mutex::new(None);

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
    let theme = theme::current();
    
    let mut buffer_guard = BACK_BUFFER.lock();
    if let Some(ref mut buffer) = *buffer_guard {
        // Render gradient directly to buffer
        for y in 0..height {
            let t = y as f32 / height as f32;
            let color = lerp_color(theme.desktop_gradient_top, theme.desktop_gradient_bottom, t);
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
    let screen_width = server.screen_width as usize;
    let theme = theme::current();
    
    let mut buffer_guard = BACK_BUFFER.lock();
    if let Some(ref mut buffer) = *buffer_guard {
        // Draw windows from back to front
        for window in server.windows_sorted() {
            if !window.visible {
                continue;
            }
            
            let wx = window.x;
            let wy = window.y;
            let ww = window.width as i32;
            let wh = window.height as i32;
            let radius = theme.window_radius as i32;
            
            // Draw Shadow
            draw_shadow(buffer, screen_width, wx, wy, ww, wh, radius, theme.shadow);
            
            // Draw Window Background (Rounded)
            draw_rounded_rect(buffer, screen_width, wx, wy, ww, wh, radius, theme.window_bg);
            
            // Draw Border
            let border_color = if window.focused {
                 // Simple gradient effect for active border based on position
                 theme.window_border_active_start
            } else {
                theme.window_border
            };
            
            // We draw border by drawing a larger rounded rect behind? Or stroking?
            // Simple stroke for now:
             draw_rounded_outline(buffer, screen_width, wx, wy, ww, wh, radius, 2, border_color);

            // Title bar (Only if decorated/floating)
            let content_y_offset = if window.decorated {
                let title_h = theme.window_title_height as i32;
                
                // Title bar background (top rounded only)
                // For simplicity, just draw rect on top part
                // draw_rounded_rect_top(buffer, screen_width, wx, wy, ww, title_h, radius, theme.window_title_bg);
                
                // Buttons
                if window.focused {
                     draw_circle_to_buffer(buffer, screen_width, wx + 16, wy + 15, 6, theme.button_close);
                     draw_circle_to_buffer(buffer, screen_width, wx + 36, wy + 15, 6, theme.button_maximize);
                     draw_circle_to_buffer(buffer, screen_width, wx + 56, wy + 15, 6, theme.button_minimize);
                }
                
                title_h
            } else {
                0
            };
            
            // Render surface content clipped to rounded rect
            render_surface_clipped(buffer, screen_width, window.surface_id, 
                                 wx, wy + content_y_offset, 
                                 window.width, window.height - content_y_offset as u32, radius);
        }
    }
    
    // Draw window titles separately (text rendering on top)
    for window in server.windows_sorted() {
        if window.visible && window.decorated {
            draw_text(window.x + 80, window.y + 8, &window.title, theme.taskbar_text);
        }
    }
}

fn render_surface_clipped(buffer: &mut [u32], screen_width: usize, surface_id: u64, x: i32, y: i32, w: u32, h: u32, radius: i32) {
    crate::graphics::with_server(|gs| {
        if let Some(surface) = gs.get_surface_mut(surface_id) {
            let bytes_per_pixel = surface.format.bytes_per_pixel();
            
            for dy in 0..h as i32 {
                for dx in 0..w as i32 {
                    // Check rounded clip
                    // This is a simplified check. Ideally we map local coords to check against radius.
                    // For typical "bottom" of window in tiling, we might not need rounding at bottom if it hits screen edge.
                    // But let's assume fully rounded for "floating" feel even when tiled.
                    
                    let buf_x = x + dx;
                    let buf_y = y + dy;
                    
                    if buf_x < 0 || buf_x >= screen_width as i32 || buf_y < 0 { continue; }
                     // Note: We skip height check for buffer len safety later
                    
                    let buf_idx = (buf_y as usize) * screen_width + (buf_x as usize);
                    
                    if buf_idx >= buffer.len() { continue; }
                    
                    // Simple clipping: if pixel is outside rounded corners
                    // Top-left
                    if dx < radius && dy < radius {
                        if (radius - dx).pow(2) + (radius - dy).pow(2) > radius.pow(2) { continue; }
                    }
                    // Top-right
                     if dx >= (w as i32 - radius) && dy < radius {
                        if (dx - (w as i32 - radius)).pow(2) + (radius - dy).pow(2) > radius.pow(2) { continue; }
                    }
                    // Bottom-left
                    if dx < radius && dy >= (h as i32 - radius) {
                         if (radius - dx).pow(2) + (dy - (h as i32 - radius)).pow(2) > radius.pow(2) { continue; }
                    }
                    // Bottom-right
                     if dx >= (w as i32 - radius) && dy >= (h as i32 - radius) {
                         if (dx - (w as i32 - radius)).pow(2) + (dy - (h as i32 - radius)).pow(2) > radius.pow(2) { continue; }
                    }

                    // Use surface width for stride, not window width
                    let surf_w = surface.width as usize;
                    // If we go out of bounds of the surface (e.g. window larger than content), skip or fill black
                    if dx as usize >= surf_w || dy as usize >= surface.height as usize { continue; }

                    let surf_idx = (dy as usize * surf_w + dx as usize) * bytes_per_pixel;
                    
                    if surf_idx + 3 < surface.pixels.len() {
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
    
    let theme = theme::current();
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
                    buffer[row_start + dx] = theme.taskbar_bg;
                }
            }
        }
        
        // Start button pill
        draw_rounded_rect_raw(buffer, width, 10, y as i32 + 5, 60, 28, 14, theme.taskbar_button_bg);
        
        // Window pills
        let mut btn_x = 80;
        for window in server.windows_sorted() {
            if window.visible {
                let btn_color = if window.focused { theme.accent } else { theme.taskbar_button_bg };
                draw_rounded_rect_raw(buffer, width, btn_x, y as i32 + 5, 100, 28, 8, btn_color);
                
                btn_x += 110;
                if btn_x > width as i32 - 150 { break; }
            }
        }
        
        // Clock pill
        let clock_x = width as i32 - 90;
        draw_rounded_rect_raw(buffer, width, clock_x, y as i32 + 5, 80, 28, 14, theme.taskbar_button_bg);
    }
    
    // Draw text labels
    let tb_y = (server.screen_height - server.taskbar_height) as i32;
    draw_text(20, tb_y + 10, "Start", theme.taskbar_text);
    
    let mut btn_x = 80;
    for window in server.windows_sorted() {
         if window.visible {
            let title: alloc::string::String = if window.title.len() > 10 {
                alloc::format!("{}...", &window.title[..8])
            } else {
                window.title.clone()
            };
            draw_text(btn_x + 10, tb_y + 10, &title, theme.taskbar_text);
            btn_x += 110;
             if btn_x > server.screen_width as i32 - 150 { break; }
        }
    }
    
    let time_str = crate::drivers::rtc::format_time();
    draw_text((server.screen_width - 80) as i32, tb_y + 10, &time_str, theme.taskbar_text);
}


fn render_cursor_to_buffer(server: &DisplayServer) {
    // ... Cursor rendering remains same ...
    let (mx, my) = crate::drivers::mouse::get_position();
    let is_clicking = crate::drivers::mouse::is_left_pressed();
    let screen_width = server.screen_width as usize;
    let theme = theme::current();
    
    let fill_color = if is_clicking { theme.accent } else { 0xFFFFFF };
    let outline_color = 0x000000;
    
    // Simple cursor shape
    let cursor_rows: &[(i32, i32)] = &[
        (0, 0), (0, 1), (0, 2), (0, 3), (0, 4), (0, 5), (0, 6),
        (1, 1), (1, 2), (1, 3), (1, 4), (1, 5),
        (2, 2), (2, 3), (2, 4), (2, 5),
        (3, 3), (3, 4),
        (4, 4),
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

// --- Graphical Primitives ---

fn draw_rounded_rect(buffer: &mut [u32], screen_width: usize, x: i32, y: i32, w: i32, h: i32, radius: i32, color: u32) {
    draw_rounded_rect_raw(buffer, screen_width, x, y, w, h, radius, color);
}

fn draw_rounded_rect_raw(buffer: &mut [u32], screen_width: usize, x: i32, y: i32, w: i32, h: i32, radius: i32, color: u32) {
     for dy in 0..h {
        for dx in 0..w {
            let screen_y = y + dy;
            let screen_x = x + dx;
             if screen_x < 0 || screen_x >= screen_width as i32 || screen_y < 0 { continue; }
             
             let buf_idx = (screen_y as usize) * screen_width + (screen_x as usize);
             if buf_idx >= buffer.len() { continue; }
             
             // Check rounding
             let mut inside = true;
             if dx < radius && dy < radius { // Top-left
                  if (radius - dx).pow(2) + (radius - dy).pow(2) > radius.pow(2) { inside = false; }
             } else if dx >= (w - radius) && dy < radius { // Top-right
                  if (dx - (w - radius)).pow(2) + (radius - dy).pow(2) > radius.pow(2) { inside = false; }
             } else if dx < radius && dy >= (h - radius) { // Bottom-left
                  if (radius - dx).pow(2) + (dy - (h - radius)).pow(2) > radius.pow(2) { inside = false; }
             } else if dx >= (w - radius) && dy >= (h - radius) { // Bottom-right
                  if (dx - (w - radius)).pow(2) + (dy - (h - radius)).pow(2) > radius.pow(2) { inside = false; }
             }
             
             if inside {
                 buffer[buf_idx] = color;
             }
        }
    }
}

fn draw_rounded_outline(buffer: &mut [u32], screen_width: usize, x: i32, y: i32, w: i32, h: i32, radius: i32, thickness: i32, color: u32) {
    // Draw outer rounded rect then skip inner
    // This is expensive but simple for implementation plan
    // Optimized: Only iterate near edges
    
    // For now, full iteration is safer to implement correctly
     for dy in 0..h {
        for dx in 0..w {
             // Optimization: Skip center
             if dx > thickness + radius && dx < w - thickness - radius && dy > thickness + radius && dy < h - thickness - radius {
                 continue;
             }
             
             let screen_y = y + dy;
             let screen_x = x + dx;
              if screen_x < 0 || screen_x >= screen_width as i32 || screen_y < 0 { continue; }
             let buf_idx = (screen_y as usize) * screen_width + (screen_x as usize);
             if buf_idx >= buffer.len() { continue; }

             let mut inside_outer = true;
             if dx < radius && dy < radius {
                  if (radius - dx).pow(2) + (radius - dy).pow(2) > radius.pow(2) { inside_outer = false; }
             } else if dx >= (w - radius) && dy < radius {
                  if (dx - (w - radius)).pow(2) + (radius - dy).pow(2) > radius.pow(2) { inside_outer = false; }
             } else if dx < radius && dy >= (h - radius) {
                  if (radius - dx).pow(2) + (dy - (h - radius)).pow(2) > radius.pow(2) { inside_outer = false; }
             } else if dx >= (w - radius) && dy >= (h - radius) {
                  if (dx - (w - radius)).pow(2) + (dy - (h - radius)).pow(2) > radius.pow(2) { inside_outer = false; }
             }
             
             if !inside_outer { continue; }
             
             // Check strict interior
             let inset = thickness;
             let ir = radius.saturating_sub(thickness);
             let idx = dx - inset;
             let idy = dy - inset;
             let iw = w - inset * 2;
             let ih = h - inset * 2;
             
             let mut is_border = false;
             
             if idx < 0 || idy < 0 || idx >= iw || idy >= ih {
                 is_border = true;
             } else {
                 // Check inner corners
                 if idx < ir && idy < ir { 
                     if (ir - idx).pow(2) + (ir - idy).pow(2) > ir.pow(2) { is_border = true; }
                 } else if idx >= (iw - ir) && idy < ir {
                     if (idx - (iw - ir)).pow(2) + (ir - idy).pow(2) > ir.pow(2) { is_border = true; }
                 } else if idx < ir && idy >= (ih - ir) {
                     if (ir - idx).pow(2) + (idy - (ih - ir)).pow(2) > ir.pow(2) { is_border = true; }
                 } else if idx >= (iw - ir) && idy >= (ih - ir) {
                      if (idx - (iw - ir)).pow(2) + (idy - (ih - ir)).pow(2) > ir.pow(2) { is_border = true; }
                 }
             }
             
             if is_border {
                  buffer[buf_idx] = color;
             }
        }
     }
}

fn draw_shadow(buffer: &mut [u32], screen_width: usize, x: i32, y: i32, w: i32, h: i32, radius: i32, shadow_color: u32) {
    // Simple drop shadow: Offset by 4px, slightly larger, alpha blended
    let offset = 8;
    let blur_radius = 4; // Fake blur by expanding
    let sx = x + offset;
    let sy = y + offset;
    let sw = w;
    let sh = h;
    let s_radius = radius;
    
    // Very simple optimization: Just draw a transparent box behind
    let alpha = (shadow_color) & 0xFF; // Stored in low byte in theme
    if alpha == 0 { return; }
    
    // We only support full alpha compositing in software if we read back? 
    // buffer contains valid data.
    
    // Just draw a dark rect with alpha
    for dy in 0..sh {
        for dx in 0..sw {
            // Optimization: Skip logic to only draw potentially visible shadow (right/bottom edges)
            if dx < sw - offset - blur_radius && dy < sh - offset - blur_radius { continue; }

            let screen_y = sy + dy;
            let screen_x = sx + dx;
            
             if screen_x < 0 || screen_x >= screen_width as i32 || screen_y < 0 { continue; }
             let buf_idx = (screen_y as usize) * screen_width + (screen_x as usize);
             if buf_idx >= buffer.len() { continue; }
             
             // Check roundedness
             let mut inside = true;
             if dx < s_radius && dy < s_radius {
                  if (s_radius - dx).pow(2) + (s_radius - dy).pow(2) > s_radius.pow(2) { inside = false; }
             } else if dx >= (sw - s_radius) && dy < s_radius {
                  if (dx - (sw - s_radius)).pow(2) + (s_radius - dy).pow(2) > s_radius.pow(2) { inside = false; }
             } else if dx < s_radius && dy >= (sh - s_radius) {
                  if (s_radius - dx).pow(2) + (dy - (sh - s_radius)).pow(2) > s_radius.pow(2) { inside = false; }
             } else if dx >= (sw - s_radius) && dy >= (sh - s_radius) {
                  if (dx - (sw - s_radius)).pow(2) + (dy - (sh - s_radius)).pow(2) > s_radius.pow(2) { inside = false; }
             }
             
             if inside {
                 let bg = buffer[buf_idx];
                 // Simple blend: dst = src * a + dst * (1-a)
                 // shadow is black/colored with alpha
                 // Assuming shadow color is RGB... 
                 // Actually theme.shadow is 0xRRGGBBAA usually? Or just alpha?
                 // Theme says 0x00000060 -> Black with 0x60 alpha.
                 
                 let inv_a = 255 - alpha;
                 // Shadow color (0,0,0)
                 let r = ((bg >> 16) & 0xFF) * inv_a / 255;
                 let g = ((bg >> 8) & 0xFF) * inv_a / 255;
                 let b = (bg & 0xFF) * inv_a / 255;
                 
                 buffer[buf_idx] = (r << 16) | (g << 8) | b;
             }
        }
    }
}

// Helper functions (same as before or updated)
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

fn draw_text(x: i32, y: i32, text: &str, color: u32) {
    framebuffer::set_cursor_pos(x as usize, y as usize);
    framebuffer::print_colored(text, color);
}
