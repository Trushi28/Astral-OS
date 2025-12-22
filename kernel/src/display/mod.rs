//! Astral Display Server (ADS)
//! 
//! A Wayland-inspired display server with Hyprland-like tiling and animations.
//! Features:
//! - Dwindle Tiling Layout (Fibonacci-like)
//! - Smooth Window Animations (60 FPS target)
//! - Server-side decorations
//! - Compositing with double buffering

pub mod renderer;

use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

/// Layout mode for the display server
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    Floating,
    Dwindle,
}

/// A window managed by the display server
pub struct Window {
    pub id: u64,
    pub client_id: u64,
    pub title: String,
    
    // Current geometry (for rendering)
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    
    // Target geometry (for animation)
    pub target_x: i32,
    pub target_y: i32,
    pub target_width: u32,
    pub target_height: u32,
    
    pub surface_id: u64,
    pub visible: bool,
    pub focused: bool,
    pub z_order: i32,
    pub decorated: bool,  // Server-side decorations
    pub is_floating: bool, // Always float this window (e.g. dialogs)
}

/// Display server state
pub struct DisplayServer {
    clients: Vec<Client>,
    pub windows: Vec<Window>,
    next_client_id: u64,
    next_window_id: u64,
    pub focused_window: Option<u64>,
    
    // Screen info
    pub screen_width: u32,
    pub screen_height: u32,
    
    // Layout and Appearance
    pub layout_mode: LayoutMode,
    pub gap_size: i32,
    pub animation_speed: f32, // 0.0 to 1.0 (lerp factor)
    
    // Taskbar
    pub taskbar_height: u32,
    pub taskbar_visible: bool,
    
    // Running state
    pub running: bool,
    dirty: bool,
    _last_tick: u64,
}

/// Display server client (an application)
pub struct Client {
    pub id: u64,
    pub name: String,
    pub windows: Vec<u64>,
}

impl DisplayServer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            clients: Vec::new(),
            windows: Vec::new(),
            next_client_id: 1,
            next_window_id: 1,
            focused_window: None,
            screen_width: width,
            screen_height: height,
            layout_mode: LayoutMode::Dwindle, // Default to Dwindle/Tiling
            gap_size: 10,
            animation_speed: 0.35, // Fast and responsive
            taskbar_height: 40,
            taskbar_visible: true,
            running: false,
            dirty: true,
            _last_tick: 0,
        }
    }
    
    /// Register a new client
    pub fn register_client(&mut self, name: &str) -> u64 {
        let id = self.next_client_id;
        self.next_client_id += 1;
        
        self.clients.push(Client {
            id,
            name: String::from(name),
            windows: Vec::new(),
        });
        
        crate::serial_println!("[ADS] Client registered: {} (id={})", name, id);
        id
    }
    
    /// Create a window for a client
    pub fn create_window(&mut self, client_id: u64, title: &str, width: u32, height: u32) -> Result<u64, &'static str> {
        if !self.clients.iter().any(|c| c.id == client_id) {
            return Err("Client not found");
        }
        
        let surface_id = crate::graphics::create_surface(width, height)?;
        
        let win_id = self.next_window_id;
        self.next_window_id += 1;
        
        // Initial position (center of screen, small)
        let center_x = self.screen_width as i32 / 2;
        let center_y = self.screen_height as i32 / 2;
        
        let is_floating = matches!(title, "Calculator" | "About"); // Simple heuristic for now
        
        let window = Window {
            id: win_id,
            client_id,
            title: String::from(title),
            // Start from center for "pop in" animation
            x: center_x,
            y: center_y,
            width: 50,  
            height: 50,
            // Targets will be set by recalculate_layout
            target_x: center_x,
            target_y: center_y,
            target_width: width,
            target_height: height,
            surface_id,
            visible: true,
            focused: true,
            z_order: self.windows.len() as i32,
            decorated: is_floating, // Tiled windows might hide decorations
            is_floating,
        };
        
        // Unfocus previous
        if let Some(old_id) = self.focused_window {
            if let Some(w) = self.windows.iter_mut().find(|w| w.id == old_id) {
                w.focused = false;
            }
        }
        
        self.focused_window = Some(win_id);
        self.windows.push(window);
        
        if let Some(client) = self.clients.iter_mut().find(|c| c.id == client_id) {
            client.windows.push(win_id);
        }
        
        // Trigger layout update
        self.recalculate_layout();
        self.dirty = true;
        
        Ok(win_id)
    }
    
    /// Destroy a window
    pub fn destroy_window(&mut self, window_id: u64) -> Result<(), &'static str> {
        if let Some(pos) = self.windows.iter().position(|w| w.id == window_id) {
            let window = self.windows.remove(pos);
            let _ = crate::graphics::destroy_surface(window.surface_id);
            
            if let Some(client) = self.clients.iter_mut().find(|c| c.id == window.client_id) {
                client.windows.retain(|&id| id != window_id);
            }
            
            if self.focused_window == Some(window_id) {
                self.focused_window = self.windows.last().map(|w| w.id);
                if let Some(new_focus) = self.focused_window {
                    if let Some(w) = self.windows.iter_mut().find(|w| w.id == new_focus) {
                        w.focused = true;
                    }
                }
            }
            
            self.recalculate_layout();
            self.dirty = true;
            Ok(())
        } else {
            Err("Window not found")
        }
    }
    
    /// Recalculate window positions based on layout
    fn recalculate_layout(&mut self) {
        if self.windows.is_empty() {
            return;
        }
        
        let work_x = self.gap_size;
        let work_y = self.gap_size;
        let work_w = self.screen_width.saturating_sub(self.gap_size as u32 * 2);
        let work_h = self.screen_height.saturating_sub(self.taskbar_height + self.gap_size as u32 * 2);
        
        match self.layout_mode {
            LayoutMode::Floating => {
                // Do nothing for floating layout (manual positioning)
                // Just set targets to current rects for floating windows
                 for win in &mut self.windows {
                    if !win.is_floating { // Treat all as floating in this mode
                        win.target_width = win.width;
                        win.target_height = win.height;
                         // Keep current pos
                        win.target_x = win.x;
                        win.target_y = win.y;
                    }
                }
            }
            LayoutMode::Dwindle => {
                // Collect tiled windows
                let tiled_indices: Vec<usize> = self.windows.iter().enumerate()
                    .filter(|(_, w)| !w.is_floating && w.visible)
                    .map(|(i, _)| i)
                    .collect();
                
                // If no tiled windows, we are done
                if tiled_indices.is_empty() {
                    return;
                }
                
                let count = tiled_indices.len();
                let mut active_x = work_x;
                let mut active_y = work_y;
                let mut active_w = work_w;
                let mut active_h = work_h;
                
                // Dwindle algorithm: Split usually alternates (vertical -> horizontal) or strictly splits the biggest axis
                // Here we'll alternate splits
                
                for (i, &idx) in tiled_indices.iter().enumerate() {
                    let is_last = i == count - 1;
                    
                    let (tx, ty, tw, th) = if is_last {
                        (active_x, active_y, active_w, active_h)
                    } else {
                        // Split
                        if active_w > active_h {
                            // Split vertical (side by side)
                            let spawn_w = active_w / 2;
                            let keep_w = active_w - spawn_w;
                            
                            let rect = (active_x, active_y, spawn_w - self.gap_size as u32, active_h);
                            
                            // Remaining space
                            active_x += spawn_w as i32;
                            active_w = keep_w;
                            
                            rect
                        } else {
                            // Split horizontal (top and bottom)
                            let spawn_h = active_h / 2;
                            let keep_h = active_h - spawn_h;
                            
                            let rect = (active_x, active_y, active_w, spawn_h - self.gap_size as u32);
                            
                            // Remaining space
                            active_y += spawn_h as i32;
                            active_h = keep_h;
                            
                            rect
                        }
                    };
                    
                    self.windows[idx].target_x = tx;
                    self.windows[idx].target_y = ty;
                    self.windows[idx].target_width = tw;
                    self.windows[idx].target_height = th;
                }
            }
        }
        
        // Handle floating windows (center them or keep position)
        for win in &mut self.windows {
            if win.is_floating {
                // If width/height is 10 (initial), resize to preferred
                if win.width == 10 {
                    win.target_width = win.target_width.max(100);
                    win.target_height = win.target_height.max(100);
                    // Center
                    win.target_x = (self.screen_width as i32 - win.target_width as i32) / 2;
                    win.target_y = (self.screen_height as i32 - win.target_height as i32) / 2;
                }
                // Else keep current target
            }
        }
    }
    
    /// Tick animation: Interpolate positions
    /// Returns true if animations are active (needs redraw)
    pub fn tick(&mut self) -> bool {
        let mut animating = false;
        let speed = self.animation_speed;
        
        for win in &mut self.windows {
            if !win.visible { continue; }
            
            // Lerp Position
            let dx = (win.target_x - win.x) as f32;
            let dy = (win.target_y - win.y) as f32;
            let dw = (win.target_width as i32 - win.width as i32) as f32;
            let dh = (win.target_height as i32 - win.height as i32) as f32;
            
            if dx.abs() > 1.0 || dy.abs() > 1.0 || dw.abs() > 1.0 || dh.abs() > 1.0 {
                win.x += (dx * speed) as i32;
                win.y += (dy * speed) as i32;
                win.width = (win.width as i32 + (dw * speed) as i32) as u32;
                win.height = (win.height as i32 + (dh * speed) as i32) as u32;
                animating = true;
                self.dirty = true;
            } else {
                // Snap to target if close enough
                if win.x != win.target_x || win.y != win.target_y || 
                   win.width != win.target_width || win.height != win.target_height {
                    win.x = win.target_x;
                    win.y = win.target_y;
                    win.width = win.target_width;
                    win.height = win.target_height;
                    self.dirty = true;
                }
            }
        }
        
        animating || self.dirty
    }
    
    /// Focus a window
    pub fn focus_window(&mut self, window_id: u64) {
        if let Some(old_id) = self.focused_window {
            if let Some(w) = self.windows.iter_mut().find(|w| w.id == old_id) {
                w.focused = false;
            }
        }
        
        let max_z = self.windows.iter().map(|w| w.z_order).max().unwrap_or(0);
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == window_id) {
            w.focused = true;
            w.z_order = max_z + 1;
        }
        
        self.focused_window = Some(window_id);
        self.dirty = true;
    }
    
    /// Move a window (Force move for dragging)
    pub fn move_window(&mut self, window_id: u64, x: i32, y: i32) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == window_id) {
            // If dragging, we switch to floating or update target directly?
            // For now update target and current
            w.target_x = x;
            w.target_y = y;
            w.x = x;
            w.y = y;
            w.is_floating = true; // Dragging detaches from tiling usually
            self.recalculate_layout(); // Re-layout others
            self.dirty = true;
        }
    }

    // ... Helpers ...
    pub fn windows_sorted(&self) -> Vec<&Window> {
        let mut sorted: Vec<&Window> = self.windows.iter().collect();
        sorted.sort_by_key(|w| w.z_order);
        sorted
    }
    
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }
    
    /// Focus next window
    pub fn focus_next(&mut self) {
        if self.windows.is_empty() { return; }
        
        let current_idx = self.windows.iter()
            .position(|w| Some(w.id) == self.focused_window)
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % self.windows.len();
        let next_id = self.windows[next_idx].id;
        self.focus_window(next_id);
    }
    
    pub fn get_window_by_surface(&mut self, surface_id: u64) -> Option<&mut Window> {
        self.windows.iter_mut().find(|w| w.surface_id == surface_id)
    }
}

// Global display server
static DISPLAY_SERVER: Mutex<Option<DisplayServer>> = Mutex::new(None);

/// Initialize display server
pub fn init() {
    let (width, height) = crate::drivers::framebuffer::get_dimensions();
    let server = DisplayServer::new(width as u32, height as u32);
    *DISPLAY_SERVER.lock() = Some(server);
    crate::serial_println!("[ADS] Astral Display Server initialized: {}x{}", width, height);
}

/// Access display server
pub fn with_server<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut DisplayServer) -> R
{
    let mut guard = DISPLAY_SERVER.lock();
    guard.as_mut().map(f)
}

/// Register client
pub fn register_client(name: &str) -> Option<u64> {
    with_server(|s| s.register_client(name))
}

/// Create window
pub fn create_window(client_id: u64, title: &str, width: u32, height: u32) -> Result<u64, &'static str> {
    with_server(|s| s.create_window(client_id, title, width, height))
        .ok_or("Display server not initialized")?
}

/// Destroy window
pub fn destroy_window(window_id: u64) -> Result<(), &'static str> {
    with_server(|s| s.destroy_window(window_id))
        .ok_or("Display server not initialized")?
}

/// Focus window
pub fn focus_window(window_id: u64) {
    with_server(|s| s.focus_window(window_id));
}

/// Focus next window
pub fn focus_next() {
    with_server(|s| s.focus_next());
}

/// Animation Tick (Call from main loop)
pub fn tick() -> bool {
    with_server(|s| s.tick()).unwrap_or(false)
}
