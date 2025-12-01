//src/main.rs
#![no_std]
#![no_main]

extern crate alloc;

use core::panic::PanicInfo;
use astral_kernel::*;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Initialize serial FIRST for debugging
    early_serial_init();
    enable_sse(); 
    serial_println(b"=== ASTRAL OS BOOT SEQUENCE ===");
    serial_println(b"[BOOT] CPU SSE Enabled");
    serial_println(b"[BOOT] Kernel entry point reached");
    serial_println(b"[BOOT] Serial initialized");
    
    // Check if we have Limine responses
    serial_println(b"[BOOT] Checking Limine responses...");
    
    // Debug: Print request addresses
    serial_print(b"[BOOT]   HHDM_REQUEST addr: 0x");
    serial_print_hex(&HHDM_REQUEST as *const _ as u64);
    serial_println(b"");
    serial_print(b"[BOOT]   MEMORY_MAP_REQUEST addr: 0x");
    serial_print_hex(&MEMORY_MAP_REQUEST as *const _ as u64);
    serial_println(b"");
    serial_print(b"[BOOT]   FRAMEBUFFER_REQUEST addr: 0x");
    serial_print_hex(&FRAMEBUFFER_REQUEST as *const _ as u64);
    serial_println(b"");
    
    // Check HHDM
    serial_println(b"[BOOT] Checking HHDM...");
    if let Some(hhdm) = HHDM_REQUEST.get_response() {
        let offset = hhdm.offset();
        serial_print(b"[BOOT]   HHDM offset: 0x");
        serial_print_hex(offset as u64);
        serial_println(b"");
        crate::set_hhdm_offset(offset as usize);
    } else {
        serial_println(b"[BOOT] FATAL: No HHDM response!");
        loop { unsafe { core::arch::asm!("cli", "hlt"); } }
    }
    
    // Check Memory Map
    serial_println(b"[BOOT] Checking memory map...");
    if MEMORY_MAP_REQUEST.get_response().is_some() {
        serial_println(b"[BOOT]   Memory map: OK");
    } else {
        serial_println(b"[BOOT] FATAL: No memory map!");
        loop { unsafe { core::arch::asm!("cli", "hlt"); } }
    }
    
    // Check Framebuffer (but don't read properties yet - they might cause page faults)
    serial_println(b"[BOOT] Checking framebuffer...");
    if FRAMEBUFFER_REQUEST.get_response().is_some() {
        serial_println(b"[BOOT]   Framebuffer response exists - will initialize after paging");
    } else {
        serial_println(b"[BOOT] FATAL: No framebuffer response!");
        loop { unsafe { core::arch::asm!("cli", "hlt"); } }
    }
    
    serial_println(b"[1/10] Initializing memory management...");
    println!("[1/10] Memory management...");
    
    memory::init();
    serial_println(b"[DEBUG] Testing Heap before Font load...");
    // Try to allocate a Box. If memory::init() failed, this will panic HERE.
    let b = alloc::boxed::Box::new(42);
    serial_println(b"[DEBUG] Small allocation OK");
    
    // Try a larger allocation (Vector) which fontdue uses
    let mut v = alloc::vec::Vec::new();
    v.push(1);
    v.push(2);
    serial_println(b"[DEBUG] Vector allocation OK");
    serial_println(b"");
    serial_println(b"[2/10] Initializing framebuffer...");
    
    // Now it's safe to access framebuffer properties
    serial_println(b"[2/10] Accessing framebuffer properties...");
    if let Some(fb_resp) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(fb) = fb_resp.framebuffers().next() {
            serial_print(b"[1/10]   Address: 0x");
            serial_print_hex(fb.addr() as *const u8 as u64);
            serial_println(b"");
            serial_print(b"[1/10]   Size: ");
            serial_print_dec(fb.width() as u64);
            serial_print(b"x");
            serial_print_dec(fb.height() as u64);
            serial_println(b"");
        }
    }
    
    drivers::framebuffer::init();
    serial_println(b"[2/10] Framebuffer initialized");
    
    serial_println(b"[2/10] Clearing screen...");
    drivers::framebuffer::clear();
    serial_println(b"[1/10] Screen cleared");
    
    serial_println(b"[2/10] Testing framebuffer output...");
    drivers::framebuffer::print("TEST ");
    serial_println(b"[2/10] Basic print works");
    
    // Boot banner
    serial_println(b"[2/10] Printing banner...");
    drivers::framebuffer::print_colored("╔═══════════════════════════════════════════╗\n", 0x00AAFF);
    drivers::framebuffer::print_colored("║      ", 0x00AAFF);
    drivers::framebuffer::print_colored("ASTRAL OS", 0xFFFFFF);
    drivers::framebuffer::print_colored(" v0.3.0                 ║\n", 0x00AAFF);
    drivers::framebuffer::print_colored("║  ", 0x00AAFF);
    drivers::framebuffer::print_colored("Modern Foundation + Reality Engine", 0x888888);
    drivers::framebuffer::print_colored("   ║\n", 0x00AAFF);
    drivers::framebuffer::print_colored("╚═══════════════════════════════════════════╝\n", 0x00AAFF);
    println!();
    serial_println(b"[1/10] Banner printed");
    
    // Initialize memory subsystem
    serial_println(b"");
    
    
    serial_println(b"[2/10] Memory initialized");
    
    // Test heap allocation
    serial_println(b"[2/10] Testing heap allocation...");
    {
        use alloc::vec::Vec;
        let test_vec = Vec::from([1u32, 2, 3]);
        serial_print(b"[2/10] Heap test: Vec with ");
        serial_print_dec(test_vec.len() as u64);
        serial_println(b" elements - OK");
    }
    
    // Initialize Reality Engine (needs heap)
    serial_println(b"");
    serial_println(b"[3/10] Initializing Reality Engine...");
    println!("[3/10] Reality Engine...");
    
    reality::init();
    
    let root_reality = reality::causality::RealityId::root();
    reality::causality::set_current_reality(root_reality);
    println!("      Root Reality: {}", root_reality.as_u64());
    serial_print(b"[3/10] Reality Engine initialized, root reality: ");
    serial_print_dec(root_reality.as_u64());
    serial_println(b"");
    
    // Initialize interrupts
    serial_println(b"");
    serial_println(b"[4/10] Initializing interrupts...");
    println!("[4/10] Interrupts...");
    
    interrupts::init();
    
    serial_println(b"[4/10] Interrupts enabled");
    
    // Initialize drivers
    serial_println(b"");
    serial_println(b"[5/10] Initializing drivers...");
    println!("[5/10] Drivers...");
    
    drivers::init();
    
    serial_println(b"[5/10] Drivers initialized");
    
    // Test allocator
    serial_println(b"");
    serial_println(b"[6/10] Testing allocator...");
    println!("[6/10] Testing allocator...");
    
    test_allocator();
    
    serial_println(b"[6/10] Allocator tests passed");
    
    // Success banner
    serial_println(b"");
    serial_println(b"=== BOOT COMPLETE ===");
    
    println!();
    drivers::framebuffer::print_colored("╔═══════════════════════════════════════════╗\n", 0x00AA00);
    drivers::framebuffer::print_colored("║  ✓ All systems operational                ║\n", 0x00FF00);
    drivers::framebuffer::print_colored("║  ✓ Reality Engine: ACTIVE                 ║\n", 0x00FF00);
    drivers::framebuffer::print_colored("║  ✓ Dream State: STANDBY                   ║\n", 0x00FF00);
    drivers::framebuffer::print_colored("║  ✓ Multiverse Layer: READY                ║\n", 0x00FF00);
    drivers::framebuffer::print_colored("╚═══════════════════════════════════════════╝\n", 0x00AA00);
    println!();
    
    serial_println(b"[7/10] Starting interactive shell...");
    
    // Start shell
    let mut shell = shell::Shell::new();
    shell.run();

    // Should never reach here
    serial_println(b"[FATAL] Shell exited unexpectedly!");
    loop {
        unsafe { core::arch::asm!("cli", "hlt", options(nostack, nomem)); }
    }
}

fn test_allocator() {
    use alloc::vec::Vec;
    use alloc::string::String;
    use alloc::boxed::Box;
    
    let v = Vec::from([1u32, 2, 3, 4, 5]);
    println!("      Vec: {} elements ✓", v.len());
    serial_print(b"      Vec: ");
    serial_print_dec(v.len() as u64);
    serial_println(b" elements");
    
    let s = String::from("Astral OS");
    println!("      String: \"{}\" ✓", s);
    serial_println(b"      String: OK");
    
    let b = Box::new(42i64);
    println!("      Box: {} ✓", *b);
    serial_println(b"      Box: OK");
    
    let reality_id = reality::causality::RealityId::current();
    println!("      Current Reality: {} ✓", reality_id.as_u64());
    serial_print(b"      Current Reality: ");
    serial_print_dec(reality_id.as_u64());
    serial_println(b"");
}

// Serial port functions
const COM1: u16 = 0x3F8;

fn early_serial_init() {
    unsafe {
        crate::util::outb(COM1 + 1, 0x00); // Disable interrupts
        crate::util::outb(COM1 + 3, 0x80); // Enable DLAB
        crate::util::outb(COM1 + 0, 0x03); // Divisor low
        crate::util::outb(COM1 + 1, 0x00); // Divisor high
        crate::util::outb(COM1 + 3, 0x03); // 8N1
        crate::util::outb(COM1 + 2, 0xC7); // Enable FIFO
        crate::util::outb(COM1 + 4, 0x0B); // RTS/DSR
    }
}

fn serial_print(msg: &[u8]) {
    unsafe {
        for &byte in msg {
            while (crate::util::inb(COM1 + 5) & 0x20) == 0 {}
            if byte == b'\n' {
                crate::util::outb(COM1, b'\r');
                while (crate::util::inb(COM1 + 5) & 0x20) == 0 {}
            }
            crate::util::outb(COM1, byte);
        }
    }
}

fn serial_println(msg: &[u8]) {
    serial_print(msg);
    serial_print(b"\n");
}

fn serial_print_hex(mut val: u64) {
    let hex_chars = b"0123456789ABCDEF";
    let mut buffer = [0u8; 16];
    let mut i = 0;
    
    if val == 0 {
        serial_print(b"0");
        return;
    }
    
    while val > 0 {
        buffer[15 - i] = hex_chars[(val & 0xF) as usize];
        val >>= 4;
        i += 1;
    }
    
    serial_print(&buffer[16 - i..]);
}

fn serial_print_dec(mut val: u64) {
    if val == 0 {
        serial_print(b"0");
        return;
    }
    
    let mut buffer = [0u8; 20];
    let mut i = 0;
    
    while val > 0 {
        buffer[19 - i] = b'0' + (val % 10) as u8;
        val /= 10;
        i += 1;
    }
    
    serial_print(&buffer[20 - i..]);
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Disable interrupts immediately
    unsafe { core::arch::asm!("cli", options(nostack, nomem)); }
    
    serial_println(b"");
    serial_println(b"!!! KERNEL PANIC !!!");
    serial_println(b"");
    
    // Try to print location
    if let Some(location) = info.location() {
        serial_print(b"Location: ");
        serial_print(location.file().as_bytes());
        serial_print(b":");
        serial_print_dec(location.line() as u64);
        serial_print(b":");
        serial_print_dec(location.column() as u64);
        serial_println(b"");
    }
    
    // Try to print message
    serial_print(b"Message: ");
    serial_println(b"<see framebuffer>");
    
    // Try framebuffer (may not work)
    drivers::framebuffer::print_colored("\n╔═══════════════════════════════════════════╗\n", 0xFF0000);
    drivers::framebuffer::print_colored("║           KERNEL PANIC                    ║\n", 0xFF0000);
    drivers::framebuffer::print_colored("╚═══════════════════════════════════════════╝\n", 0xFF0000);
    
    if let Some(location) = info.location() {
        println!("at {}:{}:{}", 
            location.file(), 
            location.line(), 
            location.column()
        );
    }
    
    // Print message
    let msg = info.message();
    println!("{}", msg);
    
    // Dump context if possible
    let reality = reality::causality::RealityId::current();
    println!("\nContext:");
    println!("  Reality: {}", reality.as_u64());
    println!("  Ticks: {}", crate::get_timestamp());
    
    serial_println(b"");
    serial_println(b"System halted. Please reboot.");
    serial_println(b"");
    
    loop {
        unsafe { core::arch::asm!("cli", "hlt", options(nostack, nomem)); }
    }
}
fn enable_sse() {
    unsafe {
        use core::arch::asm;
        let mut cr0: u64;
        let mut cr4: u64;

        // 1. Read CR0, clear EM (Emulation), set MP (Monitor Co-processor)
        asm!("mov {}, cr0", out(reg) cr0);
        cr0 &= !(1 << 2); // Clear EM bit (bit 2)
        cr0 |= (1 << 1);  // Set MP bit (bit 1)
        asm!("mov cr0, {}", in(reg) cr0);

        // 2. Read CR4, set OSFXSR (bit 9) and OSXMMEXCPT (bit 10)
        asm!("mov {}, cr4", out(reg) cr4);
        cr4 |= (1 << 9);  
        cr4 |= (1 << 10); 
        asm!("mov cr4, {}", in(reg) cr4);
    }
}