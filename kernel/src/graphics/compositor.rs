// src/graphics/compositor.rs
//! Hardware-accelerated compositor (video game style)

use super::surface::{Surface, PixelFormat};
use core::ptr::write_volatile;

pub struct Compositor {
    framebuffer: *mut u32,
    width: usize,
    height: usize,
    pitch: usize,
    
    // Back buffer for double buffering
    back_buffer: Option<alloc::vec::Vec<u32>>,
    double_buffered: bool,
    
    // Performance stats
    pub frame_count: u64,
    pub surface_count: usize,
}

unsafe impl Send for Compositor {}
unsafe impl Sync for Compositor {}

impl Compositor {
    pub fn new(framebuffer_addr: *mut u8, width: usize, height: usize, pitch: usize) -> Self {
        let double_buffered = true;
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
            frame_count: 0,
            surface_count: 0,
        }
    }
    
    pub fn begin_frame(&mut self) {
        // Clear back buffer if using double buffering
        if let Some(ref mut buffer) = self.back_buffer {
            buffer.fill(0xFF000000); // Black with full alpha
        }
        
        self.surface_count = 0;
    }
    
    pub fn end_frame(&mut self) {
        // Copy back buffer to framebuffer if double buffering
        if let Some(ref buffer) = self.back_buffer {
            unsafe {
                for y in 0..self.height {
                    let src_offset = y * self.width;
                    let dst_offset = y * self.pitch;
                    
                    for x in 0..self.width {
                        let pixel = buffer[src_offset + x];
                        write_volatile(
                            self.framebuffer.add(dst_offset + x),
                            pixel
                        );
                    }
                }
            }
        }
        
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
        
        let bytes_per_pixel = surface.format.bytes_per_pixel();
        
        for y in 0..surface.height as i32 {
            let screen_y = surface.y + y;
            
            if screen_y < 0 || screen_y >= self.height as i32 {
                continue;
            }
            
            for x in 0..surface.width as i32 {
                let screen_x = surface.x + x;
                
                if screen_x < 0 || screen_x >= self.width as i32 {
                    continue;
                }
                
                let src_offset = (y as u32 * surface.width + x as u32) as usize * bytes_per_pixel;
                let dst_offset = screen_y as usize * self.width + screen_x as usize;
                
                // Read pixel from surface
                let color = match surface.format {
                    PixelFormat::RGBA8888 => {
                        let r = surface.pixels[src_offset] as u32;
                        let g = surface.pixels[src_offset + 1] as u32;
                        let b = surface.pixels[src_offset + 2] as u32;
                        let a = surface.pixels[src_offset + 3] as u32;
                        
                        // Premultiply alpha for blending
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
    }
}