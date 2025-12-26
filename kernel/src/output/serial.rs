use uart_16550::SerialPort;
use spin::Mutex;
use core::fmt;

static SERIAL: Mutex<Option<SerialPort>> = Mutex::new(None);

/// Initialize the serial port (COM1)
pub fn init() {
    let mut serial_port = unsafe { SerialPort::new(0x3F8) };
    serial_port.init();
    *SERIAL.lock() = Some(serial_port);
}

/// Write a string to the serial port
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    if let Some(serial) = SERIAL.lock().as_mut() {
        serial.write_fmt(args).unwrap();
    }
}

/// Print to serial port
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::output::serial::_print(format_args!($($arg)*))
    };
}

/// Print to serial port with newline
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(concat!($fmt, "\n"), $($arg)*));
}

pub use serial_print;
pub use serial_println;
