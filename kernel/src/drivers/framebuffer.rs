//src/drivers/framebuffer.rs
use core::ptr::{write_volatile, read_volatile};
use spin::Mutex;
use fontdue::{Font, FontSettings};

const FONT_DATA: &[u8] = include_bytes!(env!("FONT_PATH"));

pub struct Framebuffer {
    addr: *mut u32,
    width: usize,
    height: usize,
    pitch: usize,
    x: usize,
    y: usize,
    
    // Font rendering
    font: Font,
    font_size: f32,
    line_height: usize,
    
    // Colors
    fg_color: u32,
    bg_color: u32,
}

unsafe impl Send for Framebuffer {}
unsafe impl Sync for Framebuffer {}

impl Framebuffer {
    pub fn new(addr: *mut u8, width: usize, height: usize, pitch: usize, bpp: u16) -> Self {
        // Load font with fontdue
        let font = Font::from_bytes(FONT_DATA, FontSettings::default())
            .expect("Failed to load font");
        
        let font_size = 16.0;
        let line_height = 20; // Slightly more than font size for spacing
        
        Self {
            addr: addr as *mut u32,
            width,
            height,
            pitch: pitch / (bpp as usize / 8),
            x: 0,
            y: 0,
            font,
            font_size,
            line_height,
            fg_color: 0xFFFFFF,
            bg_color: 0x000000,
        }
    }
    
    /// Clear screen
    pub fn clear(&mut self) {
        self.clear_with_color(self.bg_color);
        self.x = 0;
        self.y = 0;
    }
    
    pub fn clear_with_color(&mut self, color: u32) {
        unsafe {
            for y in 0..self.height {
                let row_offset = y * self.pitch;
                for x in 0..self.width {
                    let pixel = self.addr.add(row_offset + x);
                    write_volatile(pixel, color);
                }
            }
        }
    }
    
    /// Put pixel
    #[inline]
    fn put_pixel(&self, x: usize, y: usize, color: u32) {
        if x >= self.width || y >= self.height {
            return;
        }
        
        unsafe {
            let offset = y * self.pitch + x;
            let pixel = self.addr.add(offset);
            write_volatile(pixel, color);
        }
    }
    
    /// Draw character using fontdue
    pub fn draw_char(&mut self, c: char, fg: u32, bg: u32) {
        match c {
            '\n' => {
                self.x = 0;
                self.y += self.line_height;
                if self.y + self.line_height > self.height {
                    self.scroll();
                }
            }
            '\r' => {
                self.x = 0;
            }
            '\t' => {
                let spaces = 4 - (self.x / 8 % 4);
                for _ in 0..spaces {
                    self.draw_char(' ', fg, bg);
                }
            }
            '\x08' | '\x7F' => { // Backspace / DEL
                self.backspace();
            }
            _ => {
                // Rasterize character
                let (metrics, bitmap) = self.font.rasterize(c, self.font_size);
                
                // Draw background
                for row in 0..self.line_height {
                    for col in 0..metrics.advance_width as usize {
                        if self.x + col < self.width && self.y + row < self.height {
                            self.put_pixel(self.x + col, self.y + row, bg);
                        }
                    }
                }
                
                // Draw glyph
                for row in 0..metrics.height {
                    for col in 0..metrics.width {
                        let bitmap_idx = row * metrics.width + col;
                        if bitmap_idx < bitmap.len() {
                            let alpha = bitmap[bitmap_idx];
                            
                            if alpha > 0 {
                                // Alpha blend
                                let color = if alpha == 255 {
                                    fg
                                } else {
                                    self.blend_colors(fg, bg, alpha)
                                };
                                
                                let px = self.x + col;
                                let py = self.y + metrics.ymin as usize + row;
                                
                                if px < self.width && py < self.height {
                                    self.put_pixel(px, py, color);
                                }
                            }
                        }
                    }
                }
                
                // Advance cursor
                self.x += metrics.advance_width as usize;
                if self.x + metrics.advance_width as usize > self.width {
                    self.x = 0;
                    self.y += self.line_height;
                    if self.y + self.line_height > self.height {
                        self.scroll();
                    }
                }
            }
        }
    }
    
    /// Blend two colors with alpha
    fn blend_colors(&self, fg: u32, bg: u32, alpha: u8) -> u32 {
        let alpha = alpha as u32;
        let inv_alpha = 255 - alpha;
        
        let fg_r = (fg >> 16) & 0xFF;
        let fg_g = (fg >> 8) & 0xFF;
        let fg_b = fg & 0xFF;
        
        let bg_r = (bg >> 16) & 0xFF;
        let bg_g = (bg >> 8) & 0xFF;
        let bg_b = bg & 0xFF;
        
        let r = (fg_r * alpha + bg_r * inv_alpha) / 255;
        let g = (fg_g * alpha + bg_g * inv_alpha) / 255;
        let b = (fg_b * alpha + bg_b * inv_alpha) / 255;
        
        (r << 16) | (g << 8) | b
    }
    
    /// Scroll screen up by one line
    fn scroll(&mut self) {
        unsafe {
            // Copy lines up
            let lines_to_copy = self.height - self.line_height;
            let src = self.addr.add(self.line_height * self.pitch);
            core::ptr::copy(src, self.addr, lines_to_copy * self.pitch);
            
            // Clear bottom line
            let clear_start = self.addr.add(lines_to_copy * self.pitch);
            for i in 0..(self.line_height * self.pitch) {
                write_volatile(clear_start.add(i), self.bg_color);
            }
        }
        
        self.y = self.height - self.line_height;
    }
    
    /// Backspace
    fn backspace(&mut self) {
        if self.x >= 8 {
            self.x -= 8;
        } else if self.y >= self.line_height {
            self.y -= self.line_height;
            self.x = self.width - 8;
        } else {
            return;
        }
        
        // Clear character
        for row in 0..self.line_height {
            for col in 0..8 {
                self.put_pixel(self.x + col, self.y + row, self.bg_color);
            }
        }
    }
    
    /// Write string
    pub fn write_str(&mut self, s: &str) {
        for c in s.chars() {
            self.draw_char(c, self.fg_color, self.bg_color);
        }
    }
    
    pub fn write_str_colored(&mut self, s: &str, fg: u32) {
        for c in s.chars() {
            self.draw_char(c, fg, self.bg_color);
        }
    }
    
    pub fn set_fg_color(&mut self, color: u32) {
        self.fg_color = color;
    }
    
    pub fn set_bg_color(&mut self, color: u32) {
        self.bg_color = color;
    }
    
    pub fn get_cursor(&self) -> (usize, usize) {
        (self.x, self.y)
    }
}

static FB: Mutex<Option<Framebuffer>> = Mutex::new(None);

pub fn init() {
    use crate::FRAMEBUFFER_REQUEST;
    
    if let Some(fb_resp) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(framebuffer) = fb_resp.framebuffers().next() {
            let mut fb_lock = FB.lock();
            *fb_lock = Some(Framebuffer::new(
                framebuffer.addr(),
                framebuffer.width() as usize,
                framebuffer.height() as usize,
                framebuffer.pitch() as usize,
                framebuffer.bpp(),
            ));
        }
    }
}

pub fn print(s: &str) {
    let mut fb = FB.lock();
    if let Some(ref mut framebuffer) = *fb {
        framebuffer.write_str(s);
    }
}

pub fn print_colored(s: &str, color: u32) {
    let mut fb = FB.lock();
    if let Some(ref mut framebuffer) = *fb {
        framebuffer.write_str_colored(s, color);
    }
}

pub fn clear() {
    let mut fb = FB.lock();
    if let Some(ref mut framebuffer) = *fb {
        framebuffer.clear();
    }
}

pub fn set_fg_color(color: u32) {
    let mut fb = FB.lock();
    if let Some(ref mut framebuffer) = *fb {
        framebuffer.set_fg_color(color);
    }
}

pub fn set_bg_color(color: u32) {
    let mut fb = FB.lock();
    if let Some(ref mut framebuffer) = *fb {
        framebuffer.set_bg_color(color);
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!($crate::drivers::framebuffer::FbWriter, $($arg)*);
    }};
}

#[macro_export]
macro_rules! println {
    () => ($crate::drivers::framebuffer::print("\n"));
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!($crate::drivers::framebuffer::FbWriter, $($arg)*);
        $crate::drivers::framebuffer::print("\n");
    }};
}

pub struct FbWriter;

impl core::fmt::Write for FbWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        print(s);
        Ok(())
    }
}