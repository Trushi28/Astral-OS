//! Desktop environment - background, taskbar, window management

use alloc::string::String;
use alloc::vec::Vec;
use super::window::{Window, WindowState};
use super::theme;
use crate::drivers::framebuffer;

/// Desktop environment
pub struct Desktop {
    pub screen_width: u32,
    pub screen_height: u32,
    
    windows: Vec<Window>,
    next_window_id: u64,
    focused_window: Option<u64>,
    
    // Mouse state
    pub cursor_x: i32,
    pub cursor_y: i32,
    cursor_visible: bool,
    
    // Rendering
    dirty: bool,
}

impl Desktop {
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        Self {
            screen_width,
            screen_height,
            windows: Vec::new(),
            next_window_id: 1,
            focused_window: None,
            cursor_x: (screen_width / 2) as i32,
            cursor_y: (screen_height / 2) as i32,
            cursor_visible: true,
            dirty: true,
        }
    }
    
    /// Create a new window
    pub fn create_window(&mut self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.next_window_id;
        self.next_window_id += 1;
        
        let mut window = Window::new(id, title, x, y, width, height);
        window.z_order = self.windows.len() as i32;
        window.focused = true;
        
        // Unfocus previous window
        if let Some(focused_id) = self.focused_window {
            if let Some(w) = self.windows.iter_mut().find(|w| w.id == focused_id) {
                w.focused = false;
            }
        }
        
        self.focused_window = Some(id);
        self.windows.push(window);
        self.dirty = true;
        
        id
    }
    
    /// Close a window
    pub fn close_window(&mut self, id: u64) {
        self.windows.retain(|w| w.id != id);
        
        // Update focus
        if self.focused_window == Some(id) {
            self.focused_window = self.windows.last().map(|w| w.id);
            if let Some(new_focus) = self.focused_window {
                if let Some(w) = self.windows.iter_mut().find(|w| w.id == new_focus) {
                    w.focused = true;
                }
            }
        }
        
        self.dirty = true;
    }
    
    /// Focus a window (bring to front)
    pub fn focus_window(&mut self, id: u64) {
        // Unfocus current
        if let Some(focused_id) = self.focused_window {
            if let Some(w) = self.windows.iter_mut().find(|w| w.id == focused_id) {
                w.focused = false;
            }
        }
        
        // Focus new and bring to front
        let max_z = self.windows.iter().map(|w| w.z_order).max().unwrap_or(0);
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.focused = true;
            w.z_order = max_z + 1;
        }
        
        self.focused_window = Some(id);
        self.dirty = true;
    }
    
    /// Handle keyboard input
    pub fn handle_key(&mut self, key: u8) {
        // Tab to cycle windows
        if key == b'\t' && !self.windows.is_empty() {
            let current_idx = self.windows.iter()
                .position(|w| Some(w.id) == self.focused_window)
                .unwrap_or(0);
            let next_idx = (current_idx + 1) % self.windows.len();
            let next_id = self.windows[next_idx].id;
            self.focus_window(next_id);
            return;
        }
        
        // 'c' to close focused window (only if no terminal focused)
        if key == b'c' {
            if let Some(id) = self.focused_window {
                // Check if it's not a terminal (terminals need 'c' for input)
                let is_terminal = self.windows.iter()
                    .find(|w| w.id == id)
                    .map(|w| w.terminal.is_some())
                    .unwrap_or(false);
                
                if !is_terminal {
                    self.close_window(id);
                    return;
                }
            }
        }
        
        // Send key to focused window
        if let Some(id) = self.focused_window {
            if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
                w.handle_key(key);
                self.dirty = true;
            }
        }
    }
    
    /// Handle mouse/input
    pub fn handle_input(&mut self) {
        // For now, just mark as needing redraw
    }
    
    /// Check if redraw is needed
    pub fn needs_redraw(&self) -> bool {
        self.dirty
    }
    
    /// Render the entire desktop
    pub fn render(&mut self) {
        let theme = theme::current();
        
        // Render desktop background with gradient
        self.render_background(theme);
        
        // Render windows (sorted by z-order)
        self.windows.sort_by_key(|w| w.z_order);
        
        for window in &self.windows {
            window.render(
                &mut |x, y, color| self.put_pixel(x, y, color),
                &mut |x, y, text, color| self.draw_text(x, y, text, color),
            );
        }
        
        // Render taskbar
        self.render_taskbar(theme);
        
        // Render cursor
        if self.cursor_visible {
            self.render_cursor();
        }
        
        self.dirty = false;
    }
    
    fn render_background(&self, theme: &theme::Theme) {
        let h = self.screen_height - theme.taskbar_height;
        
        // Use optimized gradient rendering
        for y in 0..h {
            let t = y as f32 / h as f32;
            let color = interpolate_color(theme.desktop_gradient_top, theme.desktop_gradient_bottom, t);
            
            // Draw entire row at once using blit
            let row: alloc::vec::Vec<u32> = alloc::vec![color; self.screen_width as usize];
            framebuffer::blit_buffer(&row, 0, y as usize, self.screen_width as usize, 1);
        }
    }
    
    fn render_taskbar(&self, theme: &theme::Theme) {
        let taskbar_y = self.screen_height - theme.taskbar_height;
        
        // Taskbar background
        let taskbar_row: alloc::vec::Vec<u32> = alloc::vec![theme.taskbar_bg; self.screen_width as usize];
        for dy in 0..theme.taskbar_height {
            framebuffer::blit_buffer(&taskbar_row, 0, (taskbar_y + dy) as usize, self.screen_width as usize, 1);
        }
        
        // Start button
        for dy in 5..(theme.taskbar_height - 5) {
            for dx in 10..70 {
                self.put_pixel(dx as i32, (taskbar_y + dy) as i32, theme.taskbar_button_bg);
            }
        }
        self.draw_text(20, (taskbar_y + 12) as i32, "Start", theme.taskbar_text);
        
        // Window buttons in taskbar
        let mut btn_x = 80;
        for window in &self.windows {
            if window.state != WindowState::Minimized {
                let btn_color = if Some(window.id) == self.focused_window {
                    theme.accent
                } else {
                    theme.taskbar_button_bg
                };
                
                for dy in 5..(theme.taskbar_height - 5) {
                    for dx in 0..100 {
                        if btn_x + dx < self.screen_width - 100 {
                            self.put_pixel((btn_x + dx) as i32, (taskbar_y + dy) as i32, btn_color);
                        }
                    }
                }
                
                // Window title in button
                let title: String = if window.title.len() > 10 {
                    alloc::format!("{}...", &window.title[..8])
                } else {
                    window.title.clone()
                };
                self.draw_text((btn_x + 8) as i32, (taskbar_y + 12) as i32, &title, theme.taskbar_text);
                
                btn_x += 110;
            }
        }
        
        // Clock (right side)
        let clock_x = self.screen_width - 80;
        for dy in 5..(theme.taskbar_height - 5) {
            for dx in 0..70 {
                self.put_pixel((clock_x + dx) as i32, (taskbar_y + dy) as i32, theme.taskbar_button_bg);
            }
        }
        self.draw_text((clock_x + 15) as i32, (taskbar_y + 12) as i32, "12:34", theme.taskbar_text);
    }
    
    fn render_cursor(&self) {
        // Simple arrow cursor
        let cursor_points: [(i32, i32); 16] = [
            (0, 0), (0, 1), (0, 2), (0, 3), (0, 4), (0, 5), (0, 6), (0, 7), (0, 8),
            (1, 1), (1, 2), (1, 3), (1, 4), (1, 5),
            (2, 2), (2, 3),
        ];
        
        // Shadow
        for &(dx, dy) in &cursor_points {
            self.put_pixel(self.cursor_x + dx + 1, self.cursor_y + dy + 1, 0x000000);
        }
        
        // Cursor
        for &(dx, dy) in &cursor_points {
            self.put_pixel(self.cursor_x + dx, self.cursor_y + dy, 0xFFFFFF);
        }
    }
    
    #[inline]
    fn put_pixel(&self, x: i32, y: i32, color: u32) {
        if x >= 0 && y >= 0 && (x as u32) < self.screen_width && (y as u32) < self.screen_height {
            let pixels = &[color];
            framebuffer::blit_buffer(pixels, x as usize, y as usize, 1, 1);
        }
    }
    
    fn draw_text(&self, x: i32, y: i32, text: &str, color: u32) {
        // Use framebuffer's text rendering
        use crate::drivers::framebuffer::print_colored;
        
        // Set cursor position and draw
        framebuffer::set_cursor_pos(x as usize, y as usize);
        print_colored(text, color);
    }
}

/// Interpolate between two colors
fn interpolate_color(c1: u32, c2: u32, t: f32) -> u32 {
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
