use spin::Mutex;
use core::fmt;
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::Rgb888,
    prelude::*,
    text::Text,
    Drawable,
};

static FRAMEBUFFER: Mutex<Option<FramebufferWriter>> = Mutex::new(None);

pub struct FramebufferWriter {
    buffer: *mut u8,
    width: usize,
    height: usize,
    pitch: usize,
    bpp: usize,
    x: i32,
    y: i32,
}

unsafe impl Send for FramebufferWriter {}

impl FramebufferWriter {
    fn new(buffer: *mut u8, width: usize, height: usize, pitch: usize, bpp: usize) -> Self {
        Self {
            buffer,
            width,
            height,
            pitch,
            bpp,
            x: 10,
            y: 20,
        }
    }
    
    fn newline(&mut self) {
        self.x = 10;
        self.y += 22;
        if self.y > (self.height as i32 - 30) {
            self.clear();
            self.x = 10;
            self.y = 20;
        }
    }
    
    fn clear(&mut self) {
        unsafe {
            for y in 0..self.height {
                for x in 0..self.width {
                    let offset = (y * self.pitch) + (x * self.bpp);
                    if offset + 2 < self.pitch * self.height {
                        *self.buffer.add(offset) = 0x00;      // Blue
                        *self.buffer.add(offset + 1) = 0x00;  // Green
                        *self.buffer.add(offset + 2) = 0x00;  // Red
                    }
                }
            }
        }
        self.x = 10;
        self.y = 20;
    }
    
    fn putpixel(&mut self, x: u32, y: u32, color: Rgb888) {
        if (x as usize) < self.width && (y as usize) < self.height {
            let offset = ((y as usize) * self.pitch) + ((x as usize) * self.bpp);
            unsafe {
                if offset + 2 < self.pitch * self.height {
                    *self.buffer.add(offset) = color.b();
                    *self.buffer.add(offset + 1) = color.g();
                    *self.buffer.add(offset + 2) = color.r();
                }
            }
        }
    }
    
    fn write_char(&mut self, c: char) {
        match c {
            '\n' => self.newline(),
            '\r' => self.x = 10,
            c => {
                let text_style = MonoTextStyle::new(&FONT_10X20, Rgb888::new(0, 255, 255));
                let mut s = [0u8; 4];
                let s_str = c.encode_utf8(&mut s);
                
                if self.x + 12 > self.width as i32 {
                    self.newline();
                }
                
                let _ = Text::new(s_str, Point::new(self.x, self.y), text_style)
                    .draw(self);
                
                self.x += 12;
            }
        }
    }
}

// Implement DrawTarget for embedded-graphics
impl DrawTarget for FramebufferWriter {
    type Color = Rgb888;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels.into_iter() {
            if coord.x >= 0 && coord.y >= 0 {
                self.putpixel(coord.x as u32, coord.y as u32, color);
            }
        }

        Ok(())
    }
}

impl OriginDimensions for FramebufferWriter {
    fn size(&self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}

impl fmt::Write for FramebufferWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            self.write_char(c);
        }
        Ok(())
    }
}

pub fn init(framebuffer: &'static limine::response::FramebufferResponse) {
    if let Some(fb) = framebuffer.framebuffers().next() {
        let mut writer = FramebufferWriter::new(
            fb.addr() as *mut u8,
            fb.width() as usize,
            fb.height() as usize,
            fb.pitch() as usize,
            (fb.bpp() as usize) / 8,
        );
        
        writer.clear();
        *FRAMEBUFFER.lock() = Some(writer);
    }
}

pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    if let Some(fb) = FRAMEBUFFER.lock().as_mut() {
        fb.write_fmt(args).ok();
    }
}

#[macro_export]
macro_rules! fb_print {
    ($($arg:tt)*) => {
        $crate::output::framebuffer::_print(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! fb_println {
    () => ($crate::fb_print!("\n"));
    ($fmt:expr) => ($crate::fb_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::fb_print!(concat!($fmt, "\n"), $($arg)*));
}

pub use fb_print;
pub use fb_println;
