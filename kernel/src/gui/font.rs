//! GUI Font Rendering using noto-sans-mono-bitmap
//! 
//! This module provides font rendering for GUI surfaces using the same
//! noto-sans-mono font that the framebuffer uses.

use noto_sans_mono_bitmap::{get_raster, FontWeight, RasterHeight};
use crate::graphics::surface::Surface;

const FONT_SIZE: RasterHeight = RasterHeight::Size20;  // Same as framebuffer
const CHAR_WIDTH: usize = 12;
const CHAR_HEIGHT: usize = 20;

/// Draw a single character on a surface
pub fn draw_char(surface: &mut Surface, x: u32, y: u32, ch: char, color: u32) {
    let raster = get_raster(ch, FontWeight::Regular, FONT_SIZE);
    
    if let Some(raster) = raster {
        let bpp = surface.format.bytes_per_pixel();
        let r = ((color >> 16) & 0xFF) as u8;
        let g = ((color >> 8) & 0xFF) as u8;
        let b = (color & 0xFF) as u8;
        
        // Draw character bitmap
        for (row_idx, row) in raster.raster().iter().enumerate() {
            for (col_idx, &intensity) in row.iter().enumerate() {
                if intensity > 50 {  // Threshold for visibility
                    let px = x + col_idx as u32;
                    let py = y + row_idx as u32;
                    
                    if px < surface.width && py < surface.height {
                        let idx = ((py * surface.width + px) as usize) * bpp;
                        if idx + 3 < surface.pixels.len() {
                            // Alpha blend based on intensity
                            let alpha = intensity;
                            let inv_alpha = 255 - alpha;
                            
                            let bg_r = surface.pixels[idx];
                            let bg_g = surface.pixels[idx + 1];
                            let bg_b = surface.pixels[idx + 2];
                            
                            surface.pixels[idx] = ((r as u16 * alpha as u16 + bg_r as u16 * inv_alpha as u16) / 255) as u8;
                            surface.pixels[idx + 1] = ((g as u16 * alpha as u16 + bg_g as u16 * inv_alpha as u16) / 255) as u8;
                            surface.pixels[idx + 2] = ((b as u16 * alpha as u16 + bg_b as u16 * inv_alpha as u16) / 255) as u8;
                            surface.pixels[idx + 3] = 0xFF;
                        }
                    }
                }
            }
        }
    }
}

/// Draw a string on a surface
pub fn draw_string(surface: &mut Surface, x: u32, y: u32, text: &str, color: u32) {
    let mut cursor_x = x;
    
    for ch in text.chars() {
        if ch == '\n' {
            break;  // GUI doesn't handle multiline for now
        }
        
        draw_char(surface, cursor_x, y, ch, color);
        cursor_x += CHAR_WIDTH as u32;
        
        if cursor_x >= surface.width {
            break;
        }
    }
}

/// Get the width of a string in pixels
pub fn string_width(text: &str) -> u32 {
    text.len() as u32 * CHAR_WIDTH as u32
}

/// Get character dimensions
pub fn char_dimensions() -> (u32, u32) {
    (CHAR_WIDTH as u32, CHAR_HEIGHT as u32)
}
