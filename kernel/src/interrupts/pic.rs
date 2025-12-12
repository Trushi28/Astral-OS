//src/interrupts/pic.rs
//! Legacy 8259 PIC management - for disabling when using APIC

use crate::util::{outb, io_wait};

const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_COMMAND: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

const ICW1_ICW4: u8 = 0x01;      // ICW4 needed
const ICW1_INIT: u8 = 0x10;      // Initialization command
const ICW4_8086: u8 = 0x01;      // 8086/88 mode

/// Properly disable the legacy PIC by remapping and masking all IRQs.
/// This MUST be called before enabling APIC interrupts to prevent conflicts.
/// 
/// The PIC is remapped to vectors 32-47 to avoid conflicts with CPU exceptions
/// (which use vectors 0-31), then all IRQs are masked.
pub fn disable_pic() {
    unsafe {
        // Save current masks (in case we need them later)
        let _mask1 = inb(PIC1_DATA);
        let _mask2 = inb(PIC2_DATA);
        
        // Start ICW1 - Begin initialization sequence
        outb(PIC1_COMMAND, ICW1_INIT | ICW1_ICW4);
        io_wait();
        outb(PIC2_COMMAND, ICW1_INIT | ICW1_ICW4);
        io_wait();
        
        // ICW2 - Remap PICs to vectors 32-47 (away from CPU exceptions)
        outb(PIC1_DATA, 32);  // IRQ 0-7  -> Vectors 32-39
        io_wait();
        outb(PIC2_DATA, 40);  // IRQ 8-15 -> Vectors 40-47
        io_wait();
        
        // ICW3 - Tell Master PIC that Slave is at IRQ2
        outb(PIC1_DATA, 4);   // Slave connected at IRQ2 (0000 0100)
        io_wait();
        outb(PIC2_DATA, 2);   // Slave cascade identity (0000 0010)
        io_wait();
        
        // ICW4 - Set 8086 mode
        outb(PIC1_DATA, ICW4_8086);
        io_wait();
        outb(PIC2_DATA, ICW4_8086);
        io_wait();
        
        // OCW1 - Mask ALL interrupts on both PICs (0xFF = all masked)
        outb(PIC1_DATA, 0xFF);
        io_wait();
        outb(PIC2_DATA, 0xFF);
        io_wait();
    }
}

/// Initialize PIC - just masks everything (legacy, use disable_pic instead)
pub fn init_pic() {
    disable_pic();
}

/// Send EOI to PIC (only needed if PIC is still active, shouldn't be used with APIC)
pub unsafe fn pic_send_eoi(irq: u8) {
    if irq >= 8 {
        outb(PIC2_COMMAND, 0x20);
    }
    outb(PIC1_COMMAND, 0x20);
}

/// Check if an IRQ is from the PIC (spurious interrupt detection)
pub fn is_spurious_irq(irq: u8) -> bool {
    unsafe {
        if irq == 7 {
            // Check PIC1 ISR for spurious IRQ7
            outb(PIC1_COMMAND, 0x0B);  // Read ISR
            return (inb(PIC1_COMMAND) & 0x80) == 0;
        } else if irq == 15 {
            // Check PIC2 ISR for spurious IRQ15
            outb(PIC2_COMMAND, 0x0B);  // Read ISR
            if (inb(PIC2_COMMAND) & 0x80) == 0 {
                // Spurious from slave, still need to EOI master
                outb(PIC1_COMMAND, 0x20);
                return true;
            }
        }
        false
    }
}

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack));
    value
}