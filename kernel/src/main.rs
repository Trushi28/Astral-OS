//src/main.rs
#![no_std]
#![no_main]

extern crate alloc;

use core::panic::PanicInfo;
use astral_kernel::*;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Initialize framebuffer first
    drivers::framebuffer::init();
    drivers::framebuffer::clear();
    
    // Boot banner
    drivers::framebuffer::print_colored("╔═══════════════════════════════════════════╗\n", 0x00AAFF);
    drivers::framebuffer::print_colored("║      ", 0x00AAFF);
    drivers::framebuffer::print_colored("ASTRAL OS", 0xFFFFFF);
    drivers::framebuffer::print_colored(" v0.3.0                 ║\n", 0x00AAFF);
    drivers::framebuffer::print_colored("║  ", 0x00AAFF);
    drivers::framebuffer::print_colored("Modern Foundation + Reality Engine", 0x888888);
    drivers::framebuffer::print_colored("   ║\n", 0x00AAFF);
    drivers::framebuffer::print_colored("╚═══════════════════════════════════════════╝\n", 0x00AAFF);
    println!();
    
    // Initialize subsystems
    println!("[1/10] Reality Engine...");
    reality::init();
    let root_reality = reality::causality::RealityId::root();
    reality::causality::set_current_reality(root_reality);
    println!("      Root Reality: {}", root_reality.as_u64());
    
    println!("[2/10] Memory management...");
    memory::init();
    
    println!("[3/10] Interrupts...");
    interrupts::init();
    
    println!("[4/10] Drivers...");
    drivers::init();
    
    println!("[5/10] Testing allocator...");
    test_allocator();
    
    println!();
    drivers::framebuffer::print_colored("╔═══════════════════════════════════════════╗\n", 0x00AA00);
    drivers::framebuffer::print_colored("  ✓ All systems operational\n", 0x00FF00);
    drivers::framebuffer::print_colored("  ✓ Reality Engine: ACTIVE\n", 0x00FF00);
    drivers::framebuffer::print_colored("  ✓ Dream State: STANDBY\n", 0x00FF00);
    drivers::framebuffer::print_colored("╚═══════════════════════════════════════════╝\n", 0x00AA00);
    println!();
    
    // Start shell
    let mut shell = shell::Shell::new();
    shell.run();

    loop {
        core::hint::spin_loop();
    }
}

fn test_allocator() {
    use alloc::vec::Vec;
    use alloc::string::String;
    use alloc::boxed::Box;
    
    let v = Vec::from([1u32, 2, 3, 4, 5]);
    println!("      Vec: {} elements ✓", v.len());
    
    let s = String::from("Astral OS");
    println!("      String: \"{}\" ✓", s);
    
    let b = Box::new(42i64);
    println!("      Box: {} ✓", *b);
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    drivers::framebuffer::print_colored("\n╔═══════════════════════════════════════════╗\n", 0xFF0000);
    drivers::framebuffer::print_colored("║           KERNEL PANIC                 ║\n", 0xFF0000);
    drivers::framebuffer::print_colored("╚═══════════════════════════════════════════╝\n", 0xFF0000);
    println!("{}", info);
    
    loop {
        unsafe { core::arch::asm!("cli", "hlt", options(nostack, nomem)); }
    }
}