//src/drivers/serial.rs
use crate::util::{outb, inb};
use spin::Mutex;

const COM1: u16 = 0x3F8;

pub struct SerialPort {
    port: u16,
}

impl SerialPort {
    pub const fn new(port: u16) -> Self {
        Self { port }
    }
    
    pub fn init(&self) {
        unsafe {
            outb(self.port + 1, 0x00); // Disable interrupts
            outb(self.port + 3, 0x80); // Enable DLAB
            outb(self.port + 0, 0x03); // Set divisor (low)
            outb(self.port + 1, 0x00); // Set divisor (high)
            outb(self.port + 3, 0x03); // 8 bits, no parity, one stop bit
            outb(self.port + 2, 0xC7); // Enable FIFO
            outb(self.port + 4, 0x0B); // IRQs enabled, RTS/DSR set
        }
    }
    
    pub fn send(&self, data: u8) {
        unsafe {
            while (inb(self.port + 5) & 0x20) == 0 {}
            outb(self.port, data);
        }
    }
    
    pub fn write_str(&self, s: &str) {
        for byte in s.bytes() {
            if byte == b'\n' {
                self.send(b'\r');
            }
            self.send(byte);
        }
    }
}

static SERIAL: Mutex<Option<SerialPort>> = Mutex::new(None);

pub fn init() {
    let port = SerialPort::new(COM1);
    port.init();
    *SERIAL.lock() = Some(port);
}

pub fn print(s: &str) {
    if let Some(ref port) = *SERIAL.lock() {
        port.write_str(s);
    }
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!($crate::drivers::serial::SerialWriter, $($arg)*);
    }};
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::drivers::serial::print("\n"));
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!($crate::drivers::serial::SerialWriter, $($arg)*);
        $crate::drivers::serial::print("\n");
    }};
}

pub struct SerialWriter;

impl core::fmt::Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        print(s);
        Ok(())
    }
}