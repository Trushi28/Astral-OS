//src/interrupts/pic.rs
use crate::util::{outb, io_wait};

const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_COMMAND: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

pub fn init_pic() {
    unsafe {
        // Start initialization
        outb(PIC1_COMMAND, 0x11);
        io_wait();
        outb(PIC2_COMMAND, 0x11);
        io_wait();
        
        // Set offsets
        outb(PIC1_DATA, 32);  // IRQ 0-7 → INT 32-39
        io_wait();
        outb(PIC2_DATA, 40);  // IRQ 8-15 → INT 40-47
        io_wait();
        
        // Set cascade
        outb(PIC1_DATA, 0x04);
        io_wait();
        outb(PIC2_DATA, 0x02);
        io_wait();
        
        // Set mode
        outb(PIC1_DATA, 0x01);
        io_wait();
        outb(PIC2_DATA, 0x01);
        io_wait();
        
        // Unmask all interrupts
        outb(PIC1_DATA, 0x00);
        io_wait();
        outb(PIC2_DATA, 0x00);
        io_wait();
    }
}

pub unsafe fn pic_send_eoi(irq: u8) {
    if irq >= 8 {
        outb(PIC2_COMMAND, 0x20);
    }
    outb(PIC1_COMMAND, 0x20);
}