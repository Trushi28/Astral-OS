//src/interrupts/pic.rs
use crate::util::{outb, io_wait};

const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_COMMAND: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

pub fn init_pic() {
    unsafe {
        // Start initialization
        outb(0x21, 0xFF);
        outb(0xA1, 0xFF);
    }
}

pub unsafe fn pic_send_eoi(irq: u8) {
    if irq >= 8 {
        outb(PIC2_COMMAND, 0x20);
    }
    outb(PIC1_COMMAND, 0x20);
}