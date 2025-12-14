//! Astral Display Server (ADS)
//! 
//! A Wayland-inspired display server that provides:
//! - Surface management for clients
//! - Window decorations (server-side)
//! - Compositing with double buffering
//! - Input event routing
//!
//! Architecture:
//! ```
//! ┌─────────────────────┐
//! │    Client Apps      │
//! └─────────┬───────────┘
//!           │ Protocol (syscalls)
//! ┌─────────▼───────────┐
//! │   Display Server    │
//! │  ┌───────────────┐  │
//! │  │ Window Manager│  │
//! │  └───────────────┘  │
//! │  ┌───────────────┐  │
//! │  │  Compositor   │  │
//! │  └───────────────┘  │
//! └─────────┬───────────┘
//!           │
//! ┌─────────▼───────────┐
//! │    Framebuffer      │
//! └─────────────────────┘
//! ```

pub mod renderer;

use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

/// Display server client (an application)
pub struct Client {
    pub id: u64,
    pub name: String,
    pub windows: Vec<u64>,
}

/// A window managed by the display server
pub struct Window {
    pub id: u64,
    pub client_id: u64,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub surface_id: u64,
    pub visible: bool,
    pub focused: bool,
    pub z_order: i32,
    pub decorated: bool,  // Server-side decorations
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
    
    // Taskbar
    pub taskbar_height: u32,
    pub taskbar_visible: bool,
    
    // Background
    pub background_color: u32,
    
    // Running state
    pub running: bool,
    dirty: bool,
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
            taskbar_height: 40,
            taskbar_visible: true,
            background_color: 0x1a1a2e,
            running: false,
            dirty: true,
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
        // Verify client exists
        if !self.clients.iter().any(|c| c.id == client_id) {
            return Err("Client not found");
        }
        
        // Create surface in graphics server
        let surface_id = crate::graphics::create_surface(width, height)?;
        
        // Calculate position (cascade new windows)
        let win_count = self.windows.len() as i32;
        let x = 50 + (win_count * 30) % 300;
        let y = 50 + (win_count * 30) % 200;
        
        let win_id = self.next_window_id;
        self.next_window_id += 1;
        
        let window = Window {
            id: win_id,
            client_id,
            title: String::from(title),
            x,
            y,
            width,
            height,
            surface_id,
            visible: true,
            focused: true,
            z_order: self.windows.len() as i32,
            decorated: true,
        };
        
        // Unfocus previous
        if let Some(old_id) = self.focused_window {
            if let Some(w) = self.windows.iter_mut().find(|w| w.id == old_id) {
                w.focused = false;
            }
        }
        
        self.focused_window = Some(win_id);
        self.windows.push(window);
        
        // Add to client's window list
        if let Some(client) = self.clients.iter_mut().find(|c| c.id == client_id) {
            client.windows.push(win_id);
        }
        
        self.dirty = true;
        crate::serial_println!("[ADS] Window created: {} (id={}, surface={})", title, win_id, surface_id);
        
        Ok(win_id)
    }
    
    /// Destroy a window
    pub fn destroy_window(&mut self, window_id: u64) -> Result<(), &'static str> {
        if let Some(pos) = self.windows.iter().position(|w| w.id == window_id) {
            let window = self.windows.remove(pos);
            
            // Destroy surface
            let _ = crate::graphics::destroy_surface(window.surface_id);
            
            // Remove from client
            if let Some(client) = self.clients.iter_mut().find(|c| c.id == window.client_id) {
                client.windows.retain(|&id| id != window_id);
            }
            
            // Update focus
            if self.focused_window == Some(window_id) {
                self.focused_window = self.windows.last().map(|w| w.id);
                if let Some(new_focus) = self.focused_window {
                    if let Some(w) = self.windows.iter_mut().find(|w| w.id == new_focus) {
                        w.focused = true;
                    }
                }
            }
            
            self.dirty = true;
            Ok(())
        } else {
            Err("Window not found")
        }
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
    
    /// Move a window
    pub fn move_window(&mut self, window_id: u64, x: i32, y: i32) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == window_id) {
            w.x = x;
            w.y = y;
            self.dirty = true;
        }
    }
    
    /// Get focused window
    pub fn get_focused_window(&self) -> Option<&Window> {
        self.focused_window.and_then(|id| self.windows.iter().find(|w| w.id == id))
    }
    
    /// Get mutable focused window
    pub fn get_focused_window_mut(&mut self) -> Option<&mut Window> {
        let id = self.focused_window?;
        self.windows.iter_mut().find(|w| w.id == id)
    }
    
    /// Cycle focus to next window
    pub fn focus_next(&mut self) {
        if self.windows.is_empty() {
            return;
        }
        
        let current_idx = self.windows.iter()
            .position(|w| Some(w.id) == self.focused_window)
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % self.windows.len();
        let next_id = self.windows[next_idx].id;
        self.focus_window(next_id);
    }
    
    /// Check if dirty
    pub fn needs_redraw(&self) -> bool {
        self.dirty
    }
    
    /// Mark clean after redraw
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }
    
    /// Get all windows sorted by z-order
    pub fn windows_sorted(&self) -> Vec<&Window> {
        let mut sorted: Vec<&Window> = self.windows.iter().collect();
        sorted.sort_by_key(|w| w.z_order);
        sorted
    }
    
    /// Get mutable window by surface id
    pub fn get_window_by_surface(&mut self, surface_id: u64) -> Option<&mut Window> {
        self.windows.iter_mut().find(|w| w.surface_id == surface_id)
    }
    
    /// Update window content via surface
    pub fn update_window_surface(&mut self, window_id: u64, pixels: &[u8]) -> Result<(), &'static str> {
        let surface_id = self.windows.iter()
            .find(|w| w.id == window_id)
            .map(|w| w.surface_id)
            .ok_or("Window not found")?;
        
        crate::graphics::update_surface(surface_id, pixels)?;
        self.dirty = true;
        Ok(())
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
