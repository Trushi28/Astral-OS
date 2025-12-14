//src/drivers/framebuffer.rs
use core::ptr::write_volatile;
use core::mem::ManuallyDrop;
use noto_sans_mono_bitmap::{
    get_raster, FontWeight, RasterHeight,
};

const FONT_SIZE: RasterHeight = RasterHeight::Size20;
const CHAR_WIDTH: usize = 12;
const CHAR_HEIGHT: usize = 20;
const LINE_HEIGHT: usize = 23;

pub struct Framebuffer {
    addr: *mut u32,
    width: usize,
    height: usize,
    pitch: usize,
    x: usize,
    y: usize,
    
    fg_color: u32,
    bg_color: u32,
}

unsafe impl Send for Framebuffer {}
unsafe impl Sync for Framebuffer {}

impl Framebuffer {
    pub fn new(addr: *mut u8, width: usize, height: usize, pitch: usize, bpp: u16) -> Self {
        Self {
            addr: addr as *mut u32,
            width,
            height,
            pitch: pitch / (bpp as usize / 8),
            x: 0,
            y: 0,
            fg_color: 0xFFFFFF,
            bg_color: 0x000000,
        }
    }
    
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
    
    pub fn draw_char(&mut self, c: char, fg: u32, bg: u32) {
        match c {
            '\n' => {
                self.x = 0;
                self.y += LINE_HEIGHT;
                if self.y + LINE_HEIGHT > self.height {
                    self.scroll();
                }
            }
            '\r' => {
                self.x = 0;
            }
            '\t' => {
                let tab_size = 4 * CHAR_WIDTH;
                let next_tab = (self.x / tab_size + 1) * tab_size;
                self.x = next_tab;
                
                if self.x >= self.width {
                    self.x = 0;
                    self.y += LINE_HEIGHT;
                    if self.y + LINE_HEIGHT > self.height {
                        self.scroll();
                    }
                }
            }
            '\x08' | '\x7F' => {
                self.backspace();
            }
            _ => {
                let raster = get_raster(c, FontWeight::Regular, FONT_SIZE);
                
                if let Some(raster) = raster {
                    let char_width = raster.width();
                    
                    // Draw background for the character
                    for row in 0..CHAR_HEIGHT {
                        for col in 0..char_width {
                            if self.x + col < self.width && self.y + row < self.height {
                                self.put_pixel(self.x + col, self.y + row, bg);
                            }
                        }
                    }
                    
                    // Draw character bitmap with proper alpha blending
                    for (row_idx, row) in raster.raster().iter().enumerate() {
                        for (col_idx, &intensity) in row.iter().enumerate() {
                            if intensity > 0 {
                                let color = if intensity > 127 {
                                    fg
                                } else {
                                    // Alpha blend for anti-aliasing
                                    let alpha = intensity as u32;
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
                                };
                                
                                let px = self.x + col_idx;
                                let py = self.y + row_idx;
                                
                                if px < self.width && py < self.height {
                                    self.put_pixel(px, py, color);
                                }
                            }
                        }
                    }
                    
                    self.x += char_width;
                } else {
                    // Character not in font - draw a replacement box
                    self.draw_replacement_char(fg, bg);
                }
                
                // Handle line wrapping
                if self.x + CHAR_WIDTH > self.width {
                    self.x = 0;
                    self.y += LINE_HEIGHT;
                    if self.y + LINE_HEIGHT > self.height {
                        self.scroll();
                    }
                }
            } 
        }
    }
    
    fn draw_replacement_char(&mut self, fg: u32, bg: u32) {
        // Draw a small box for missing characters
        for row in 0..CHAR_HEIGHT {
            for col in 0..CHAR_WIDTH {
                let px = self.x + col;
                let py = self.y + row;
                
                if px < self.width && py < self.height {
                    let color = if row == 0 || row == CHAR_HEIGHT - 1 || 
                                   col == 0 || col == CHAR_WIDTH - 1 {
                        fg
                    } else {
                        bg
                    };
                    self.put_pixel(px, py, color);
                }
            }
        }
        self.x += CHAR_WIDTH;
    }
    
    fn scroll(&mut self) {
        unsafe {
            // Copy screen content up by one line
            let lines_to_copy = self.height - LINE_HEIGHT;
            let src = self.addr.add(LINE_HEIGHT * self.pitch);
            core::ptr::copy(src, self.addr, lines_to_copy * self.pitch);
            
            // Clear the last line
            let clear_start = self.addr.add(lines_to_copy * self.pitch);
            for i in 0..(LINE_HEIGHT * self.pitch) {
                write_volatile(clear_start.add(i), self.bg_color);
            }
        }
        
        self.y = self.height - LINE_HEIGHT;
    }
    
    fn backspace(&mut self) {
        if self.x >= CHAR_WIDTH {
            self.x -= CHAR_WIDTH;
        } else if self.y >= LINE_HEIGHT {
            self.y -= LINE_HEIGHT;
            let cols = self.width / CHAR_WIDTH;
            self.x = (cols - 1) * CHAR_WIDTH;
        } else {
            return;
        }
        
        // Clear the character space with proper height
        for row in 0..CHAR_HEIGHT {
            for col in 0..CHAR_WIDTH {
                let px = self.x + col;
                let py = self.y + row;
                
                if px < self.width && py < self.height {
                    self.put_pixel(px, py, self.bg_color);
                }
            }
        }
    }
    
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
    
    pub fn get_cursor_pos(&self) -> (usize, usize) {
        (self.x, self.y)
    }
    
    pub fn set_cursor_pos(&mut self, x: usize, y: usize) {
        self.x = x;
        self.y = y;
    }
    
    pub fn clear_line(&mut self) {
        let start_x = self.x;
        let y = self.y;
        
        for row in 0..LINE_HEIGHT {
            for x in start_x..self.width {
                self.put_pixel(x, y + row, self.bg_color);
            }
        }
    }
    
    pub fn clear_current_line(&mut self) {
        let y = self.y;
        
        for row in 0..LINE_HEIGHT {
            for x in 0..self.width {
                self.put_pixel(x, y + row, self.bg_color);
            }
        }
        
        self.x = 0;
    }
    
    pub fn draw_char_at(&mut self, c: char, x: usize, y: usize, fg: u32, bg: u32) {
        let old_x = self.x;
        let old_y = self.y;
        
        self.x = x;
        self.y = y;
        self.draw_char(c, fg, bg);
        
        self.x = old_x;
        self.y = old_y;
    }
}

// Interrupt-safe mutex
pub struct IrqSafeMutex<T> {
    data: spin::Mutex<T>,
}

impl<T> IrqSafeMutex<T> {
    pub const fn new(data: T) -> Self {
        Self {
            data: spin::Mutex::new(data),
        }
    }
    
    pub fn lock(&self) -> IrqSafeGuard<'_, T> {
        let flags: u64;
        unsafe {
            core::arch::asm!(
                "pushfq",
                "pop {}",
                "cli",
                out(reg) flags,
                options(nomem, preserves_flags)
            );
        }
        
        let guard = self.data.lock();
        
        IrqSafeGuard {
            guard: ManuallyDrop::new(guard),
            old_flags: flags,
        }
    }
}

pub struct IrqSafeGuard<'a, T> {
    guard: ManuallyDrop<spin::MutexGuard<'a, T>>,
    old_flags: u64,
}

impl<'a, T> Drop for IrqSafeGuard<'a, T> {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.guard);
        }
        
        unsafe {
            if self.old_flags & 0x200 != 0 {
                core::arch::asm!("sti", options(nomem, nostack));
            }
        }
    }
}

impl<'a, T> core::ops::Deref for IrqSafeGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.guard
    }
}

impl<'a, T> core::ops::DerefMut for IrqSafeGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.guard
    }
}

static FB: IrqSafeMutex<Option<Framebuffer>> = IrqSafeMutex::new(None);

pub fn init() {
    use crate::FRAMEBUFFER_REQUEST;
    
    if let Some(fb_resp) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(framebuffer) = fb_resp.framebuffers().next() {
            let mut fb_lock = FB.lock();
            
            let fb = Framebuffer::new(
                framebuffer.addr(),
                framebuffer.width() as usize,
                framebuffer.height() as usize,
                framebuffer.pitch() as usize,
                framebuffer.bpp(),
            );
            
            *fb_lock = Some(fb);
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

pub fn print_char(c: char, color: u32) {
    let mut fb = FB.lock();
    if let Some(ref mut framebuffer) = *fb {
        let old_fg = framebuffer.fg_color;
        framebuffer.fg_color = color;
        framebuffer.draw_char(c, color, framebuffer.bg_color);
        framebuffer.fg_color = old_fg;
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

pub fn get_cursor_pos() -> (usize, usize) {
    let fb = FB.lock();
    if let Some(ref framebuffer) = *fb {
        framebuffer.get_cursor_pos()
    } else {
        (0, 0)
    }
}

pub fn set_cursor_pos(x: usize, y: usize) {
    let mut fb = FB.lock();
    if let Some(ref mut framebuffer) = *fb {
        framebuffer.set_cursor_pos(x, y);
    }
}

pub fn clear_line() {
    let mut fb = FB.lock();
    if let Some(ref mut framebuffer) = *fb {
        framebuffer.clear_line();
    }
}

pub fn clear_current_line() {
    let mut fb = FB.lock();
    if let Some(ref mut framebuffer) = *fb {
        framebuffer.clear_current_line();
    }
}

/// Get framebuffer dimensions (width, height)
pub fn get_dimensions() -> (usize, usize) {
    let fb = FB.lock();
    if let Some(ref framebuffer) = *fb {
        (framebuffer.width, framebuffer.height)
    } else {
        (0, 0)
    }
}

/// Get raw framebuffer info for graphics system
pub fn get_framebuffer_info() -> Option<(*mut u8, usize, usize, usize)> {
    let fb = FB.lock();
    if let Some(ref framebuffer) = *fb {
        Some((
            framebuffer.addr as *mut u8,
            framebuffer.width,
            framebuffer.height,
            framebuffer.pitch * 4  // Convert back to bytes
        ))
    } else {
        None
    }
}

/// Blit a pixel buffer directly to the framebuffer
pub fn blit_buffer(pixels: &[u32], x: usize, y: usize, w: usize, h: usize) {
    let mut fb = FB.lock();
    if let Some(ref mut framebuffer) = *fb {
        for dy in 0..h {
            let screen_y = y + dy;
            if screen_y >= framebuffer.height {
                break;
            }
            
            for dx in 0..w {
                let screen_x = x + dx;
                if screen_x >= framebuffer.width {
                    break;
                }
                
                let src_idx = dy * w + dx;
                if src_idx < pixels.len() {
                    framebuffer.put_pixel(screen_x, screen_y, pixels[src_idx]);
                }
            }
        }
    }
}

/// Draw a filled rectangle directly on framebuffer
pub fn fill_rect(x: usize, y: usize, w: usize, h: usize, color: u32) {
    let fb = FB.lock();
    if let Some(ref framebuffer) = *fb {
        for dy in 0..h {
            let screen_y = y + dy;
            if screen_y >= framebuffer.height {
                break;
            }
            
            for dx in 0..w {
                let screen_x = x + dx;
                if screen_x >= framebuffer.width {
                    break;
                }
                
                framebuffer.put_pixel(screen_x, screen_y, color);
            }
        }
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