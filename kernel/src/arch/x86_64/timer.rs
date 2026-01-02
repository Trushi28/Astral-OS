use x86_64::instructions::port::Port;
use crate::serial_println;

const PIT_CHANNEL_0: u16 = 0x40;
const PIT_COMMAND: u16 = 0x43;

const PIT_FREQUENCY: u32 = 1193182; // PIT base frequency in Hz

pub struct Timer {
    channel_0: Port<u8>,
    command: Port<u8>,
}

impl Timer {
    const unsafe fn new() -> Self {
        Self {
            channel_0: Port::new(PIT_CHANNEL_0),
            command: Port::new(PIT_COMMAND),
        }
    }

    pub unsafe fn init(&mut self, frequency: u32) {
        let divisor = (PIT_FREQUENCY / frequency) as u16;

        // Set command byte: Channel 0, lobyte/hibyte, rate generator
        self.command.write(0x36);

        // Send divisor
        self.channel_0.write((divisor & 0xFF) as u8);
        self.channel_0.write(((divisor >> 8) & 0xFF) as u8);

        serial_println!("[TIMER] Initialized PIT to {} Hz (divisor: {})", frequency, divisor);
    }
}

pub static mut TIMER: Timer = unsafe { Timer::new() };

static mut TICK_COUNT: u64 = 0;

pub unsafe fn init() {
    serial_println!("[TIMER] Initializing Programmable Interval Timer...");
    TIMER.init(100); // 100 Hz = 10ms per tick
    serial_println!("[TIMER] Timer will fire every 10ms");
}

pub unsafe fn tick() {
    TICK_COUNT += 1;
    
    // Print every second (100 ticks at 100 Hz)
    // Print every second (100 ticks at 100 Hz)
    if TICK_COUNT % 100 == 0 {
        // serial_println!("[TIMER] Tick: {} ({} seconds)", TICK_COUNT, TICK_COUNT / 100);
    }
}

pub fn get_ticks() -> u64 {
    unsafe { TICK_COUNT }
}
