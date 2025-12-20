// src/graphics/compositor.rs
#![allow(dead_code)] // Compositor internals
//! Hardware-accelerated compositor for Astral OS
//!
//! Features:
//! - Double buffering for tear-free rendering
//! - Global damage tracking for efficient updates
//! - Optimized alpha blending with fast paths
//! - Layer-based compositing

use super::surface::{Surface, PixelFormat};
use alloc::vec::Vec;
use core::ptr::write_volatile;

/// Damage rectangle for efficient updates
#[derive(Clone, Copy, Debug)]
pub struct DamageRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl DamageRect {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }
    
    /// Merge with another rectangle (union)
    pub fn merge(&self, other: &DamageRect) -> DamageRect {
        let x1 = self.x.min(other.x);
        let y1 = self.y.min(other.y);
        let x2 = (self.x + self.width as i32).max(other.x + other.width as i32);
        let y2 = (self.y + self.height as i32).max(other.y + other.height as i32);
        
        DamageRect {
            x: x1,
            y: y1,
            width: (x2 - x1) as u32,
            height: (y2 - y1) as u32,
        }
    }
    
    /// Check if rectangles overlap
    pub fn overlaps(&self, other: &DamageRect) -> bool {
        self.x < other.x + other.width as i32 &&
        self.x + self.width as i32 > other.x &&
        self.y < other.y + other.height as i32 &&
        self.y + self.height as i32 > other.y
    }
}

/// Buffer index for triple buffering
#[derive(Clone, Copy, PartialEq, Eq)]
enum BufferIndex {
    Front = 0,
    Back = 1,
    Middle = 2,
}

pub struct Compositor {
    framebuffer: *mut u32,
    width: usize,
    height: usize,
    pitch: usize,
    
    // Double buffering (simpler than triple to reduce memory)
    back_buffer: Option<Vec<u32>>,
    double_buffered: bool,
    
    // Global damage tracking
    damage_rects: Vec<DamageRect>,
    full_redraw: bool,
    
    // Performance stats
    pub frame_count: u64,
    pub surface_count: usize,
    pub pixels_composited: u64,
    pub blend_operations: u64,
}

unsafe impl Send for Compositor {}
unsafe impl Sync for Compositor {}

impl Compositor {
    pub fn new(framebuffer_addr: *mut u8, width: usize, height: usize, pitch: usize) -> Self {
        let double_buffered = true;
        
        // Create back buffer for double buffering
        let back_buffer = if double_buffered {
            Some(alloc::vec![0u32; width * height])
        } else {
            None
        };
        
        Self {
            framebuffer: framebuffer_addr as *mut u32,
            width,
            height,
            pitch: pitch / 4, // Convert bytes to u32
            back_buffer,
            double_buffered,
            damage_rects: Vec::with_capacity(32),
            full_redraw: true,
            frame_count: 0,
            surface_count: 0,
            pixels_composited: 0,
            blend_operations: 0,
        }
    }
    
    /// Add damage rectangle
    pub fn add_damage(&mut self, rect: DamageRect) {
        // Merge with existing overlapping rects
        for existing in &mut self.damage_rects {
            if existing.overlaps(&rect) {
                *existing = existing.merge(&rect);
                return;
            }
        }
        self.damage_rects.push(rect);
    }
    
    /// Mark entire screen for redraw
    pub fn mark_full_redraw(&mut self) {
        self.full_redraw = true;
        self.damage_rects.clear();
    }
    
    pub fn begin_frame(&mut self) {
        // Clear back buffer
        if let Some(ref mut buffer) = self.back_buffer {
            if self.full_redraw {
                buffer.fill(0xFF000000); // Black with full alpha
            }
        }
        
        self.surface_count = 0;
        self.pixels_composited = 0;
    }
    
    pub fn end_frame(&mut self) {
        // Copy back buffer to framebuffer
        if let Some(ref buffer) = self.back_buffer {
            unsafe {
                // Optimized scanline copy
                if self.full_redraw {
                    // Full frame copy
                    for y in 0..self.height {
                        let src_offset = y * self.width;
                        let dst_offset = y * self.pitch;
                        
                        // Copy entire scanline at once
                        core::ptr::copy_nonoverlapping(
                            buffer.as_ptr().add(src_offset),
                            self.framebuffer.add(dst_offset),
                            self.width
                        );
                    }
                } else {
                    // Partial update - only copy damaged regions
                    for rect in &self.damage_rects {
                        let start_y = rect.y.max(0) as usize;
                        let end_y = (rect.y + rect.height as i32).min(self.height as i32) as usize;
                        let start_x = rect.x.max(0) as usize;
                        let end_x = (rect.x + rect.width as i32).min(self.width as i32) as usize;
                        
                        for y in start_y..end_y {
                            let src_offset = y * self.width + start_x;
                            let dst_offset = y * self.pitch + start_x;
                            
                            core::ptr::copy_nonoverlapping(
                                buffer.as_ptr().add(src_offset),
                                self.framebuffer.add(dst_offset),
                                end_x - start_x
                            );
                        }
                    }
                }
            }
        }
        
        // Reset damage tracking
        self.damage_rects.clear();
        self.full_redraw = false;
        self.frame_count += 1;
    }
    
    pub fn composite_surface(&mut self, surface: &Surface) {
        if !surface.visible {
            return;
        }
        
        self.surface_count += 1;
        
        // Use dirty rectangles for partial updates if available
        if !surface.dirty_rects.is_empty() {
            for rect in &surface.dirty_rects {
                self.blit_region(surface, rect.x, rect.y, rect.width, rect.height);
            }
        } else {
            // Full blit
            self.blit_surface(surface);
        }
    }
    
    fn blit_surface(&mut self, surface: &Surface) {
        let dest = if let Some(ref mut buffer) = self.back_buffer {
            buffer.as_mut_ptr()
        } else {
            self.framebuffer
        };
        
        // Calculate clipping
        let start_x = surface.x.max(0) as usize;
        let start_y = surface.y.max(0) as usize;
        let end_x = (surface.x + surface.width as i32).min(self.width as i32) as usize;
        let end_y = (surface.y + surface.height as i32).min(self.height as i32) as usize;
        
        if start_x >= end_x || start_y >= end_y {
            return; // Completely clipped
        }
        
        let src_offset_x = if surface.x < 0 { (-surface.x) as usize } else { 0 };
        let src_offset_y = if surface.y < 0 { (-surface.y) as usize } else { 0 };
        
        let copy_width = end_x - start_x;
        let copy_height = end_y - start_y;
        let bytes_per_pixel = surface.format.bytes_per_pixel();
        
        // Fast path: opaque RGB888 surfaces - use bulk copy
        if matches!(surface.format, PixelFormat::RGB888) {
            for row in 0..copy_height {
                let src_y = src_offset_y + row;
                let dst_y = start_y + row;
                
                for col in 0..copy_width {
                    let src_x = src_offset_x + col;
                    let src_offset = (src_y * surface.width as usize + src_x) * bytes_per_pixel;
                    let dst_offset = dst_y * self.width + start_x + col;
                    
                    let r = surface.pixels[src_offset] as u32;
                    let g = surface.pixels[src_offset + 1] as u32;
                    let b = surface.pixels[src_offset + 2] as u32;
                    let color = (0xFF << 24) | (r << 16) | (g << 8) | b;
                    
                    unsafe { *dest.add(dst_offset) = color; }
                }
                self.pixels_composited += copy_width as u64;
            }
            return;
        }
        
        // Per-pixel path for formats with alpha
        for row in 0..copy_height {
            let src_y = src_offset_y + row;
            let dst_y = start_y + row;
            
            for col in 0..copy_width {
                let src_x = src_offset_x + col;
                let src_offset = (src_y * surface.width as usize + src_x) * bytes_per_pixel;
                let dst_offset = dst_y * self.width + start_x + col;
                
                let color = match surface.format {
                    PixelFormat::RGBA8888 => {
                        let r = surface.pixels[src_offset] as u32;
                        let g = surface.pixels[src_offset + 1] as u32;
                        let b = surface.pixels[src_offset + 2] as u32;
                        let a = surface.pixels[src_offset + 3] as u32;
                        
                        // Skip fully transparent pixels
                        if a == 0 { continue; }
                        
                        // Fast path for fully opaque
                        if a == 255 {
                            (0xFF << 24) | (r << 16) | (g << 8) | b
                        } else {
                            self.blend_pixel(r, g, b, a, unsafe { *dest.add(dst_offset) })
                        }
                    }
                    PixelFormat::BGRA8888 => {
                        let b = surface.pixels[src_offset] as u32;
                        let g = surface.pixels[src_offset + 1] as u32;
                        let r = surface.pixels[src_offset + 2] as u32;
                        let a = surface.pixels[src_offset + 3] as u32;
                        
                        if a == 0 { continue; }
                        if a == 255 {
                            (0xFF << 24) | (r << 16) | (g << 8) | b
                        } else {
                            self.blend_pixel(r, g, b, a, unsafe { *dest.add(dst_offset) })
                        }
                    }
                    PixelFormat::RGB888 => {
                        // Already handled above
                        let r = surface.pixels[src_offset] as u32;
                        let g = surface.pixels[src_offset + 1] as u32;
                        let b = surface.pixels[src_offset + 2] as u32;
                        (0xFF << 24) | (r << 16) | (g << 8) | b
                    }
                };
                
                unsafe { *dest.add(dst_offset) = color; }
                self.pixels_composited += 1;
            }
        }
    }
    
    fn blit_region(&mut self, surface: &Surface, rect_x: u32, rect_y: u32, rect_w: u32, rect_h: u32) {
        let dest = if let Some(ref mut buffer) = self.back_buffer {
            buffer.as_mut_ptr()
        } else {
            self.framebuffer
        };
        
        let bytes_per_pixel = surface.format.bytes_per_pixel();
        
        for y in rect_y..rect_y + rect_h {
            let screen_y = surface.y + y as i32;
            
            if screen_y < 0 || screen_y >= self.height as i32 {
                continue;
            }
            
            for x in rect_x..rect_x + rect_w {
                let screen_x = surface.x + x as i32;
                
                if screen_x < 0 || screen_x >= self.width as i32 {
                    continue;
                }
                
                let src_offset = (y * surface.width + x) as usize * bytes_per_pixel;
                let dst_offset = screen_y as usize * self.width + screen_x as usize;
                
                let color = match surface.format {
                    PixelFormat::RGBA8888 => {
                        let r = surface.pixels[src_offset] as u32;
                        let g = surface.pixels[src_offset + 1] as u32;
                        let b = surface.pixels[src_offset + 2] as u32;
                        let a = surface.pixels[src_offset + 3] as u32;
                        
                        self.blend_pixel(r, g, b, a, unsafe { *dest.add(dst_offset) })
                    }
                    PixelFormat::BGRA8888 => {
                        let b = surface.pixels[src_offset] as u32;
                        let g = surface.pixels[src_offset + 1] as u32;
                        let r = surface.pixels[src_offset + 2] as u32;
                        let a = surface.pixels[src_offset + 3] as u32;
                        
                        self.blend_pixel(r, g, b, a, unsafe { *dest.add(dst_offset) })
                    }
                    PixelFormat::RGB888 => {
                        let r = surface.pixels[src_offset] as u32;
                        let g = surface.pixels[src_offset + 1] as u32;
                        let b = surface.pixels[src_offset + 2] as u32;
                        
                        (0xFF << 24) | (r << 16) | (g << 8) | b
                    }
                };
                
                unsafe {
                    if self.double_buffered {
                        *dest.add(dst_offset) = color;
                    } else {
                        write_volatile(dest.add(dst_offset), color);
                    }
                }
            }
        }
    }
    
    /// Alpha blending (Porter-Duff over operator)
    #[inline]
    fn blend_pixel(&self, src_r: u32, src_g: u32, src_b: u32, src_a: u32, dst: u32) -> u32 {
        if src_a == 255 {
            // Opaque source - no blending needed
            return (0xFF << 24) | (src_r << 16) | (src_g << 8) | src_b;
        }
        
        if src_a == 0 {
            // Fully transparent - keep destination
            return dst;
        }
        
        let dst_r = (dst >> 16) & 0xFF;
        let dst_g = (dst >> 8) & 0xFF;
        let dst_b = dst & 0xFF;
        
        let inv_alpha = 255 - src_a;
        
        let out_r = (src_r * src_a + dst_r * inv_alpha) / 255;
        let out_g = (src_g * src_a + dst_g * inv_alpha) / 255;
        let out_b = (src_b * src_a + dst_b * inv_alpha) / 255;
        
        (0xFF << 24) | (out_r << 16) | (out_g << 8) | out_b
    }
    
    /// Draw filled rectangle (for window decorations, etc.)
    pub fn fill_rect(&mut self, x: i32, y: i32, width: u32, height: u32, color: u32) {
        let dest = if let Some(ref mut buffer) = self.back_buffer {
            buffer.as_mut_ptr()
        } else {
            self.framebuffer
        };
        
        for dy in 0..height as i32 {
            let screen_y = y + dy;
            
            if screen_y < 0 || screen_y >= self.height as i32 {
                continue;
            }
            
            for dx in 0..width as i32 {
                let screen_x = x + dx;
                
                if screen_x < 0 || screen_x >= self.width as i32 {
                    continue;
                }
                
                let offset = screen_y as usize * self.width + screen_x as usize;
                
                unsafe {
                    if self.double_buffered {
                        *dest.add(offset) = color;
                    } else {
                        write_volatile(dest.add(offset), color);
                    }
                }
            }
        }
    }
    
    /// Clear entire framebuffer
    pub fn clear(&mut self, color: u32) {
        if let Some(ref mut buffer) = self.back_buffer {
            buffer.fill(color);
        } else {
            unsafe {
                for y in 0..self.height {
                    for x in 0..self.width {
                        let offset = y * self.pitch + x;
                        write_volatile(self.framebuffer.add(offset), color);
                    }
                }
            }
        }
        self.full_redraw = true;
    }
}