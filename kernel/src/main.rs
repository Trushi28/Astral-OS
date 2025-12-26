#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

#[macro_use]
mod output;
mod arch;
mod memory;

use core::panic::PanicInfo;
use limine::request::FramebufferRequest;

// Limine framebuffer request
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

/// Kernel entry point
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Initialize serial output for debugging
    output::serial::init();
    
    serial_println!("🚀 Astral OS - Kernel Boot Sequence Initiated");
    serial_println!("========================================");
    
    // Initialize framebuffer output
    if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response() {
        output::framebuffer::init(framebuffer_response);
        serial_println!("[OK] Framebuffer initialized (Unicode support enabled)");
    } else {
        serial_println!("[WARN] No framebuffer available");
    }
    
    // Initialize architecture-specific features
    arch::init();
    serial_println!("[OK] Architecture initialization complete");
    
    // Initialize memory
    memory::init();
    serial_println!("[OK] Memory management initialized");
    
    // Print beautiful boot banner to framebuffer
    fb_println!("+========================================+");
    fb_println!("|         ASTRAL OS v0.1.0               |");
    fb_println!("|    Self-Assembling Kernel System       |");
    fb_println!("+========================================+");
    fb_println!();
    fb_println!("Phase 1: Core Kernel Bootstrap [OK]");
    fb_println!("Framebuffer: Enabled (Cyan on Black)");
    fb_println!();
    fb_println!("Reality Engine: Initializing...");
    fb_println!("Current Reality: Prime Timeline");
    fb_println!();
    fb_println!("Fractal Memory: Phase 2");
    fb_println!("Timeline Branching: Phase 7");
    fb_println!("Dream Mode: Phase 9");
    fb_println!();
    fb_println!("System Ready. HLT loop active.");
    fb_println!();
    
    serial_println!("\n[OK] Astral OS kernel initialization complete!");
    serial_println!("[INFO] Phase 1-2 COMPLETE: Framebuffer + Memory Management working!");
    serial_println!("[INFO] Enabling interrupts...\n");
    
    // Enable interrupts now that everything is initialized (including SS!)
    x86_64::instructions::interrupts::enable();
    
    // NOW unmask the IRQs after interrupts are enabled
    unsafe {
        arch::x86_64::pic::unmask_irqs();
    }
    
    serial_println!("[INFO] Interrupts enabled and IRQs unmasked!");
    serial_println!("[INFO] Entering idle loop...\n");
    serial_println!("[INFO] Try typing on the keyboard!\n");
    
    // Kernel idle loop
    loop {
        x86_64::instructions::hlt();
    }
}

/// Halt and catch fire - stop the CPU forever
pub fn hcf() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

/// Panic handler - called when something goes wrong
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("\n╔════════════════════════════════════════╗");
    serial_println!("║   KERNEL PANIC - REALITY COLLAPSED     ║");
    serial_println!("╚════════════════════════════════════════╝");
    serial_println!("{}", info);
    
    fb_println!("\n╔════════════════════════════════════════╗");
    fb_println!("║   KERNEL PANIC - REALITY COLLAPSED     ║");
    fb_println!("╚════════════════════════════════════════╝");
    
    hcf();
}
