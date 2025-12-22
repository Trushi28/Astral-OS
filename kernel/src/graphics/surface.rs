// src/graphics/surface.rs
//! Graphics surface (texture/framebuffer for applications)

use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    RGBA8888,
    RGB888,
    BGRA8888,
}

impl PixelFormat {
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            PixelFormat::RGBA8888 | PixelFormat::BGRA8888 => 4,
            PixelFormat::RGB888 => 3,
        }
    }
}

pub struct Surface {
    pub id: u64,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub z_order: i32,
    pub visible: bool,
    pub format: PixelFormat,
    pub pixels: Vec<u8>,
    dirty: AtomicBool,
    pub present_requested: bool,
    
    // Dirty rectangles for partial updates
    pub dirty_rects: Vec<DirtyRect>,
}

#[derive(Clone, Copy, Debug)]
pub struct DirtyRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Surface {
    pub fn new(id: u64, width: u32, height: u32) -> Result<Self, &'static str> {
        if width == 0 || height == 0 {
            return Err("Invalid surface dimensions");
        }
        
        if width > 8192 || height > 8192 {
            return Err("Surface too large");
        }
        
        let format = PixelFormat::RGBA8888;
        let pixel_count = (width * height) as usize;
        let buffer_size = pixel_count * format.bytes_per_pixel();
        
        let pixels = alloc::vec![0u8; buffer_size];
        
        Ok(Self {
            id,
            width,
            height,
            x: 0,
            y: 0,
            z_order: 0,
            visible: true,
            format,
            pixels,
            dirty: AtomicBool::new(true),
            present_requested: false,
            dirty_rects: Vec::new(),
        })
    }
    
    pub fn update_pixels(&mut self, data: &[u8]) -> Result<(), &'static str> {
        let expected_size = (self.width * self.height) as usize * self.format.bytes_per_pixel();
        
        if data.len() != expected_size {
            return Err("Invalid pixel data size");
        }
        
        self.pixels.copy_from_slice(data);
        self.mark_dirty();
        
        Ok(())
    }
    
    pub fn update_region(&mut self, x: u32, y: u32, width: u32, height: u32, data: &[u8]) -> Result<(), &'static str> {
        if x + width > self.width || y + height > self.height {
            return Err("Region out of bounds");
        }
        
        let bytes_per_pixel = self.format.bytes_per_pixel();
        let expected_size = (width * height) as usize * bytes_per_pixel;
        
        if data.len() != expected_size {
            return Err("Invalid pixel data size");
        }
        
        // Copy region
        for row in 0..height {
            let src_offset = (row * width) as usize * bytes_per_pixel;
            let dst_offset = ((y + row) * self.width + x) as usize * bytes_per_pixel;
            let row_size = width as usize * bytes_per_pixel;
            
            self.pixels[dst_offset..dst_offset + row_size]
                .copy_from_slice(&data[src_offset..src_offset + row_size]);
        }
        
        // Mark dirty rectangle
        self.dirty_rects.push(DirtyRect { x, y, width, height });
        self.mark_dirty();
        
        Ok(())
    }
    
    /// Resize the surface (clears content)
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), &'static str> {
        if width == 0 || height == 0 || width > 8192 || height > 8192 {
            return Err("Invalid dimensions");
        }
        
        if self.width == width && self.height == height {
            return Ok(());
        }
        
        self.width = width;
        self.height = height;
        
        let pixel_count = (width * height) as usize;
        let buffer_size = pixel_count * self.format.bytes_per_pixel();
        
        // Reallocate
        self.pixels = alloc::vec![0u8; buffer_size];
        self.dirty.store(true, Ordering::Release);
        self.dirty_rects.clear(); // Invalidate dirty rects as they might be out of bounds
        
        Ok(())
    }
    
    pub fn clear(&mut self, color: u32) {
        let bytes_per_pixel = self.format.bytes_per_pixel();
        
        match self.format {
            PixelFormat::RGBA8888 => {
                let r = ((color >> 24) & 0xFF) as u8;
                let g = ((color >> 16) & 0xFF) as u8;
                let b = ((color >> 8) & 0xFF) as u8;
                let a = (color & 0xFF) as u8;
                
                for i in 0..(self.width * self.height) as usize {
                    let offset = i * bytes_per_pixel;
                    self.pixels[offset] = r;
                    self.pixels[offset + 1] = g;
                    self.pixels[offset + 2] = b;
                    self.pixels[offset + 3] = a;
                }
            }
            PixelFormat::BGRA8888 => {
                let r = ((color >> 24) & 0xFF) as u8;
                let g = ((color >> 16) & 0xFF) as u8;
                let b = ((color >> 8) & 0xFF) as u8;
                let a = (color & 0xFF) as u8;
                
                for i in 0..(self.width * self.height) as usize {
                    let offset = i * bytes_per_pixel;
                    self.pixels[offset] = b;
                    self.pixels[offset + 1] = g;
                    self.pixels[offset + 2] = r;
                    self.pixels[offset + 3] = a;
                }
            }
            PixelFormat::RGB888 => {
                let r = ((color >> 16) & 0xFF) as u8;
                let g = ((color >> 8) & 0xFF) as u8;
                let b = (color & 0xFF) as u8;
                
                for i in 0..(self.width * self.height) as usize {
                    let offset = i * bytes_per_pixel;
                    self.pixels[offset] = r;
                    self.pixels[offset + 1] = g;
                    self.pixels[offset + 2] = b;
                }
            }
        }
        
        self.mark_dirty();
    }
    
    pub fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Release);
    }
    
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Acquire)
    }
    
    pub fn clear_dirty(&mut self) {
        self.dirty.store(false, Ordering::Release);
        self.dirty_rects.clear();
    }
    
    pub fn set_position(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
        self.mark_dirty();
    }
    
    pub fn set_z_order(&mut self, z: i32) {
        self.z_order = z;
        self.mark_dirty();
    }
    
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
        self.mark_dirty();
    }
    
    /// Get pixel at position (with bounds check)
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<u32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        
        let bytes_per_pixel = self.format.bytes_per_pixel();
        let offset = (y * self.width + x) as usize * bytes_per_pixel;
        
        match self.format {
            PixelFormat::RGBA8888 => {
                let r = self.pixels[offset] as u32;
                let g = self.pixels[offset + 1] as u32;
                let b = self.pixels[offset + 2] as u32;
                let a = self.pixels[offset + 3] as u32;
                Some((r << 24) | (g << 16) | (b << 8) | a)
            }
            PixelFormat::BGRA8888 => {
                let b = self.pixels[offset] as u32;
                let g = self.pixels[offset + 1] as u32;
                let r = self.pixels[offset + 2] as u32;
                let a = self.pixels[offset + 3] as u32;
                Some((r << 24) | (g << 16) | (b << 8) | a)
            }
            PixelFormat::RGB888 => {
                let r = self.pixels[offset] as u32;
                let g = self.pixels[offset + 1] as u32;
                let b = self.pixels[offset + 2] as u32;
                Some((r << 16) | (g << 8) | b)
            }
        }
    }
}