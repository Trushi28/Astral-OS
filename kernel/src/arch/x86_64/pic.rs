use x86_64::instructions::port::Port;
use crate::serial_println;

const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_COMMAND: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

const ICW1_INIT: u8 = 0x10;
const ICW1_ICW4: u8 = 0x01;
const ICW4_8086: u8 = 0x01;

const PIC_EOI: u8 = 0x20;

pub struct Pic {
    command: Port<u8>,
    data: Port<u8>,
}

impl Pic {
    const unsafe fn new(command_port: u16, data_port: u16) -> Self {
        Self {
            command: Port::new(command_port),
            data: Port::new(data_port),
        }
    }

    unsafe fn write_command(&mut self, cmd: u8) {
        self.command.write(cmd);
    }

    unsafe fn write_data(&mut self, data: u8) {
        self.data.write(data);
    }

    unsafe fn read_data(&mut self) -> u8 {
        self.data.read()
    }

    pub unsafe fn end_of_interrupt(&mut self) {
        self.write_command(PIC_EOI);
    }
}

pub struct ChainedPics {
    master: Pic,
    slave: Pic,
}

impl ChainedPics {
    pub const unsafe fn new(offset1: u8, offset2: u8) -> Self {
        Self {
            master: Pic::new(PIC1_COMMAND, PIC1_DATA),
            slave: Pic::new(PIC2_COMMAND, PIC2_DATA),
        }
    }

    pub unsafe fn initialize(&mut self, offset1: u8, offset2: u8) {
        // Save masks
        let mask1 = self.master.read_data();
        let mask2 = self.slave.read_data();

        // Start initialization sequence
        self.master.write_command(ICW1_INIT | ICW1_ICW4);
        io_wait();
        self.slave.write_command(ICW1_INIT | ICW1_ICW4);
        io_wait();

        // Set vector offsets
        self.master.write_data(offset1);
        io_wait();
        self.slave.write_data(offset2);
        io_wait();

        // Tell Master PIC that there is a slave PIC at IRQ2
        self.master.write_data(4);
        io_wait();

        // Tell Slave PIC its cascade identity
        self.slave.write_data(2);
        io_wait();

        // Set 8086 mode
        self.master.write_data(ICW4_8086);
        io_wait();
        self.slave.write_data(ICW4_8086);
        io_wait();

        // Restore masks
        self.master.write_data(mask1);
        self.slave.write_data(mask2);

        serial_println!("[PIC] Initialized with offsets: Master={}, Slave={}", offset1, offset2);
    }

    pub unsafe fn disable(&mut self) {
        self.master.write_data(0xFF);
        self.slave.write_data(0xFF);
        serial_println!("[PIC] Disabled (all IRQs masked)");
    }

    pub unsafe fn set_mask(&mut self, irq: u8) {
        let port = if irq < 8 {
            &mut self.master.data
        } else {
            &mut self.slave.data
        };

        let value = port.read() | (1 << (irq % 8));
        port.write(value);
    }

    pub unsafe fn clear_mask(&mut self, irq: u8) {
        let port = if irq < 8 {
            &mut self.master.data
        } else {
            &mut self.slave.data
        };

        let value = port.read() & !(1 << (irq % 8));
        port.write(value);
    }

    pub unsafe fn notify_end_of_interrupt(&mut self, irq: u8) {
        if irq >= 8 {
            self.slave.end_of_interrupt();
        }
        self.master.end_of_interrupt();
    }
}

unsafe fn io_wait() {
    // Port 0x80 is used for POST codes, writing to it causes a small delay
    let mut port: Port<u8> = Port::new(0x80);
    port.write(0);
}

pub static mut PICS: ChainedPics = unsafe { ChainedPics::new(32, 40) };

pub unsafe fn init() {
    serial_println!("[PIC] Initializing Programmable Interrupt Controller...");
    PICS.initialize(32, 40);
    
    // Mask ALL IRQs initially for safety
    PICS.master.write_data(0xFF);
    PICS.slave.write_data(0xFF);
    
    serial_println!("[PIC] All IRQs masked (will unmask after interrupt enable)");
}

pub unsafe fn unmask_irqs() {
    // Unmask timer (IRQ 0) and keyboard (IRQ 1)
    PICS.clear_mask(0);  // Timer
    PICS.clear_mask(1);  // Keyboard
    
    serial_println!("[PIC] Timer and keyboard IRQs unmasked");
}
