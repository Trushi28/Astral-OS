use crate::serial_println;

pub mod gdt;
pub mod idt;
pub mod pic;  // Legacy PIC (will be disabled)
pub mod timer;
pub mod keyboard;
pub mod msr;   // MSR access for APIC
pub mod apic;  // Modern APIC
pub mod smp;   // Symmetric Multiprocessing

pub fn init() {
    gdt::init();
    idt::init();
    
    // Try APIC first, fall back to PIC if unavailable
    let use_apic = unsafe {
        match apic::init() {
            Ok(_) => {
                // Initialize SMP after APIC is ready
                smp::init();
                true
            },
            Err(e) => {
                serial_println!("[WARN] APIC init failed: {}, falling back to PIC", e);
                pic::init();
                timer::init();
                keyboard::init();
                false
            }
        }
    };
    
    if use_apic {
        serial_println!("[OK] Using APIC for interrupt handling");
    } else {
        serial_println!("[OK] Using legacy PIC for interrupt handling");
    }
}
