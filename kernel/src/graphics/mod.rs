// src/graphics/mod.rs
//! Graphics server and compositor (video game style rendering)

pub mod compositor;
pub mod surface;
pub mod protocol;

use alloc::vec::Vec;
use spin::Mutex;
use surface::Surface;
use compositor::Compositor;

static GRAPHICS_SERVER: Mutex<Option<GraphicsServer>> = Mutex::new(None);

pub struct GraphicsServer {
    compositor: Compositor,
    surfaces: Vec<Surface>,
    next_surface_id: u64,
}

impl GraphicsServer {
    pub fn new(framebuffer_addr: *mut u8, width: usize, height: usize, pitch: usize) -> Self {
        Self {
            compositor: Compositor::new(framebuffer_addr, width, height, pitch),
            surfaces: Vec::new(),
            next_surface_id: 1,
        }
    }
    
    pub fn create_surface(&mut self, width: u32, height: u32) -> Result<u64, &'static str> {
        let id = self.next_surface_id;
        self.next_surface_id += 1;
        
        let surface = Surface::new(id, width, height)?;
        self.surfaces.push(surface);
        
        Ok(id)
    }
    
    pub fn destroy_surface(&mut self, id: u64) -> Result<(), &'static str> {
        self.surfaces.retain(|s| s.id != id);
        Ok(())
    }
    
    pub fn get_surface_mut(&mut self, id: u64) -> Option<&mut Surface> {
        self.surfaces.iter_mut().find(|s| s.id == id)
    }
    
    pub fn update_surface(&mut self, id: u64, pixels: &[u8]) -> Result<(), &'static str> {
        let surface = self.get_surface_mut(id)
            .ok_or("Surface not found")?;
        
        surface.update_pixels(pixels)?;
        surface.mark_dirty();
        
        Ok(())
    }
    
    pub fn present_surface(&mut self, id: u64) -> Result<(), &'static str> {
        let surface = self.surfaces.iter_mut()
            .find(|s| s.id == id)
            .ok_or("Surface not found")?;
        
        surface.present_requested = true;
        Ok(())
    }
    
    pub fn composite_frame(&mut self) {
        self.compositor.begin_frame();
        
        // Sort surfaces by z-order
        self.surfaces.sort_by_key(|s| s.z_order);
        
        // Composite each surface
        for surface in &mut self.surfaces {
            if surface.visible && surface.is_dirty() {
                self.compositor.composite_surface(surface);
                surface.clear_dirty();
            }
        }
        
        self.compositor.end_frame();
    }
}

pub fn init(framebuffer_addr: *mut u8, width: usize, height: usize, pitch: usize) {
    let server = GraphicsServer::new(framebuffer_addr, width, height, pitch);
    *GRAPHICS_SERVER.lock() = Some(server);
    
    crate::serial_println!("[GRAPHICS] Server initialized: {}x{}", width, height);
}

pub fn create_surface(width: u32, height: u32) -> Result<u64, &'static str> {
    let mut server = GRAPHICS_SERVER.lock();
    if let Some(ref mut s) = *server {
        s.create_surface(width, height)
    } else {
        Err("Graphics server not initialized")
    }
}

pub fn destroy_surface(id: u64) -> Result<(), &'static str> {
    let mut server = GRAPHICS_SERVER.lock();
    if let Some(ref mut s) = *server {
        s.destroy_surface(id)
    } else {
        Err("Graphics server not initialized")
    }
}

pub fn update_surface(id: u64, pixels: &[u8]) -> Result<(), &'static str> {
    let mut server = GRAPHICS_SERVER.lock();
    if let Some(ref mut s) = *server {
        s.update_surface(id, pixels)
    } else {
        Err("Graphics server not initialized")
    }
}

pub fn present_surface(id: u64) -> Result<(), &'static str> {
    let mut server = GRAPHICS_SERVER.lock();
    if let Some(ref mut s) = *server {
        s.present_surface(id)
    } else {
        Err("Graphics server not initialized")
    }
}

pub fn composite_frame() {
    let mut server = GRAPHICS_SERVER.lock();
    if let Some(ref mut s) = *server {
        s.composite_frame();
    }
}