use crate::serial_println;

pub mod gdt;
pub mod idt;
pub mod pic;  // Legacy PIC (will be disabled)
pub mod timer;
pub mod keyboard;
pub mod msr;   // MSR access for APIC
pub mod apic;  // Modern APIC
pub mod smp;   // Symmetric Multiprocessing
pub mod usermode; // Ring 3 transition

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
    
    // Enable SSE (CR0.MP=1, CR0.EM=0, CR4.OSFXSR=1, CR4.OSXMMEXCPT=1)
    unsafe {
        use x86_64::registers::control::{Cr0, Cr0Flags, Cr4, Cr4Flags};
        
        let mut cr0 = Cr0::read();
        // Clear EM (bit 2) and Set MP (bit 1)
        let mut bits = cr0.bits();
        bits &= !(1 << 2); // Clear EM
        bits |= (1 << 1);  // Set MP
        let new_cr0 = Cr0Flags::from_bits_truncate(bits);
        Cr0::write(new_cr0);
        
        let mut cr4 = Cr4::read();
        cr4.insert(Cr4Flags::OSFXSR | Cr4Flags::OSXMMEXCPT_ENABLE);
        Cr4::write(cr4);
        
        serial_println!("[ARC] SSE/SIMD features enabled");
    }

    if use_apic {
        serial_println!("[OK] Using APIC for interrupt handling");
    } else {
        serial_println!("[OK] Using legacy PIC for interrupt handling");
    }
}
