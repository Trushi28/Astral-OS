pub mod local;
pub mod ioapic;

use crate::serial_println;

pub unsafe fn init() -> Result<(), &'static str> {
    serial_println!("[APIC] Starting APIC initialization...");
    
    // Disable legacy PIC first
    disable_pic();
    
    // Initialize Local APIC
    local::init()?;
    
    // Initialize I/O APIC for external interrupts (keyboard, etc.)
    ioapic::init()?;
    
    // Configure Local APIC timer (replaces PIT) but keep masked
    if let Some(ref mut apic) = local::LOCAL_APIC {
        // Calibrate and setup timer at ~100 Hz
        let ticks = apic.calibrate_timer();
        let initial_count = ticks / 10; // Approximate 100Hz
        
        // Setup timer: vector 32, divide by 16, periodic mode, MASKED
        apic.setup_timer_masked(32, 0x03, initial_count);
        
        serial_println!("[APIC] Timer configured at ~100 Hz (masked until interrupt enable)");
    }
    
    serial_println!("[APIC] APIC initialization complete");
    Ok(())
}

unsafe fn disable_pic() {
    use x86_64::instructions::port::Port;
    
    let mut master_data: Port<u8> = Port::new(0x21);
    let mut slave_data: Port<u8> = Port::new(0xA1);
    
    // Mask all interrupts on both PICs
    master_data.write(0xFF);
    slave_data.write(0xFF);
    
    serial_println!("[APIC] Legacy PIC disabled");
}

pub unsafe fn end_of_interrupt() {
    local::end_of_interrupt();
}

pub unsafe fn unmask_timer() {
    if let Some(ref mut apic) = local::LOCAL_APIC {
        apic.unmask_timer(32); // Vector 32
    }
}

pub unsafe fn unmask_keyboard() {
    if let Some(ref mut ioapic) = ioapic::IOAPIC {
        ioapic.unmask_irq(1); // IRQ 1 = keyboard
        serial_println!("[IOAPIC] Keyboard IRQ unmasked");
    }
}
