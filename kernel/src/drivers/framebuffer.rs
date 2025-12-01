use core::ptr::write_volatile;
use core::sync::atomic::{AtomicBool, Ordering};
use core::mem::ManuallyDrop;
use spin::Mutex;
use fontdue::{Font, FontSettings};
use crate::util::{outb, inb};
fn debug_print_dec(mut val: usize) {
    let com1 = 0x3F8;
    unsafe {
        if val == 0 {
            while (inb(com1 + 5) & 0x20) == 0 {}
            outb(com1, b'0');
            return;
        }

        let mut buffer = [0u8; 20];
        let mut i = 0;

        while val > 0 {
            buffer[19 - i] = b'0' + (val % 10) as u8;
            val /= 10;
            i += 1;
        }

        for &byte in &buffer[20 - i..] {
            while (inb(com1 + 5) & 0x20) == 0 {}
            outb(com1, byte);
        }
    }
}

fn debug_print(msg: &[u8]) {
    let com1 = 0x3F8;
    unsafe {
        for &byte in msg {
            while (inb(com1 + 5) & 0x20) == 0 {}
            outb(com1, byte);
        }
        // Print newline
        while (inb(com1 + 5) & 0x20) == 0 {}
        outb(com1, b'\n');
        while (inb(com1 + 5) & 0x20) == 0 {}
        outb(com1, b'\r');
    }
}
const FONT_DATA: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/SpaceMono-Regular.ttf"));

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
    
    // Calculated once at startup for consistent grid layout
    char_width: usize,
    
    // Colors
    fg_color: u32,
    bg_color: u32,
}

unsafe impl Send for Framebuffer {}
unsafe impl Sync for Framebuffer {}

impl Framebuffer {
    pub fn new(addr: *mut u8, width: usize, height: usize, pitch: usize, bpp: u16) -> Self {
        // Load font with fontdue
        debug_print(b"[DEBUG] FB: Checking Font Data...");
        let len = FONT_DATA.len();
        debug_print(b"[DEBUG] FB: Font bytes found:");
        debug_print_dec(len); // No
        if len == 0 {
            debug_print(b"[FATAL] FB: Font file is empty or missing!");
            loop {}
        }

        debug_print(b"[DEBUG] FB: Parsing font with fontdue...");

        let font = Font::from_bytes(FONT_DATA, FontSettings::default())
            .expect("Failed to load font");
        debug_print(b"[DEBUG] FB: Font parsed successfully!");
        let font_size = 16.0;
        let line_height = 20; // Slightly more than font size for spacing
        
        // Calculate fixed character width using a standard char (like 'A' or '0')
        // This enforces a strict grid, making backspace and tabs robust.
        let metrics = font.metrics('A', font_size);
        let advance = metrics.advance_width;
        let char_width = if advance > (advance as usize as f32) {
            (advance as usize) + 1
        } else {
            advance as usize
        };
        
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
            char_width,
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
                self.newline();
            }
            '\r' => {
                self.x = 0;
            }
            '\t' => {
                // Perfect grid alignment logic
                let tab_size = 4 * self.char_width;
                // Calculate distance to next tab stop
                let next_tab = (self.x / tab_size + 1) * tab_size;
                self.x = next_tab;
                
                // Wrap if we go past edge
                if self.x >= self.width {
                    self.newline();
                }
            }
            '\x08' | '\x7F' => { // Backspace / DEL
                self.backspace();
            }
            _ => {
                // Rasterize character
                let (metrics, bitmap) = self.font.rasterize(c, self.font_size);
                
                // Draw background for the specific cell size
                // We use char_width here to ensure we fill the whole "grid cell" background
                for row in 0..self.line_height {
                    for col in 0..self.char_width {
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
                                
                                // Calculate position
                                let px = self.x + col;
                                
                                // SAFE Y CALCULATION:
                                // ymin is i32 and can be negative. 
                                // We must use i32 arithmetic before casting to usize.
                                let base_y = self.y as i32 + metrics.ymin;
                                let py = base_y + row as i32;
                                
                                // Bounds check using signed comparison for y
                                if px < self.width && py >= 0 && (py as usize) < self.height {
                                    self.put_pixel(px, py as usize, color);
                                }
                            }
                        }
                    }
                }
                
                // Advance cursor by FIXED width, not variable metric
                self.x += self.char_width;
                
                if self.x + self.char_width > self.width {
                    self.newline();
                }
            }
        }
    }
    
    /// Handle newline logic
    fn newline(&mut self) {
        self.x = 0;
        self.y += self.line_height;
        if self.y + self.line_height > self.height {
            self.scroll();
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
    

    fn scroll(&mut self) {
        unsafe {

            let lines_to_copy = self.height - self.line_height;
            let src = self.addr.add(self.line_height * self.pitch);
            core::ptr::copy(src, self.addr, lines_to_copy * self.pitch);
            

            let clear_start = self.addr.add(lines_to_copy * self.pitch);
            for i in 0..(self.line_height * self.pitch) {
                write_volatile(clear_start.add(i), self.bg_color);
            }
        }
        
        self.y = self.height - self.line_height;
    }
    

    fn backspace(&mut self) {
        if self.x >= self.char_width {
            self.x -= self.char_width;
        } else if self.y >= self.line_height {
            self.y -= self.line_height;
            
            let cols = self.width / self.char_width;
            self.x = (cols * self.char_width) - self.char_width;
        } else {
            return;
        }
        
        for row in 0..self.line_height {
            for col in 0..self.char_width {
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
    
    pub fn get_cursor(&self) -> (usize, usize) {
        (self.x, self.y)
    }
}



pub struct IrqSafeMutex<T> {
    data: spin::Mutex<T>,
}

impl<T> IrqSafeMutex<T> {
    pub const fn new(data: T) -> Self {
        Self {
            data: spin::Mutex::new(data),
        }
    }
    
    pub fn lock(&self) -> IrqSafeGuard<T> {
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

// --- Global Instance and Macros ---

static FB: IrqSafeMutex<Option<Framebuffer>> = IrqSafeMutex::new(None);

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