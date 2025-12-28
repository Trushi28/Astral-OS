#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(abi_x86_interrupt)]
#![feature(naked_functions)]

extern crate alloc;

#[macro_use]
mod output;
mod arch;
mod memory;
mod process;
mod scheduler;
mod syscall;

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
    
    // Demonstrate hybrid allocation (buddy + fractal)
    memory::hybrid::example_allocations();
    
    // Initialize scheduler
    let cpu_count = arch::x86_64::smp::cpu_count();
    scheduler::init(cpu_count as usize);
    serial_println!("[OK] Scheduler initialized for {} CPUs", cpu_count);
    
    // Initialize syscall interface
    unsafe {
        syscall::init();
    }
    serial_println!("[OK] Syscall interface ready");
    
    // Create demo kernel threads to test scheduler
    fn thread1() -> ! {
        loop {
            serial_println!("[THREAD 1] Hello from high priority thread!");
            for _ in 0..100000 { x86_64::instructions::nop(); }
        }
    }
    
    fn thread2() -> ! {
        loop {
            serial_println!("[THREAD 2] Hello from normal priority thread!");
            for _ in 0..100000 { x86_64::instructions::nop(); }
        }
    }
    
    fn thread3() -> ! {
        loop {
            serial_println!("[THREAD 3] Hello from low priority thread!");
            for _ in 0..100000 { x86_64::instructions::nop(); }
        }
    }
    
    // Spawn demo threads
    process::spawn::spawn_kernel_thread(thread1, process::Priority::High);
    process::spawn::spawn_kernel_thread(thread2, process::Priority::Normal);
    process::spawn::spawn_kernel_thread(thread3, process::Priority::Low);
    
    serial_println!("[OK] Created 3 demo kernel threads");
    
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
        // Check if using APIC or PIC
        if arch::x86_64::apic::local::LOCAL_APIC.is_some() {
            // Unmask APIC timer
            arch::x86_64::apic::unmask_timer();
            // Unmask keyboard via I/O APIC
            arch::x86_64::apic::unmask_keyboard();
        } else {
            // Unmask PIC IRQs
            arch::x86_64::pic::unmask_irqs();
        }
    }
    
    serial_println!("[INFO] Interrupts enabled and IRQs unmasked!");
    serial_println!("[INFO] Starting scheduler...\n");
    
    // Main scheduler loop - run processes!
    loop {
        unsafe {
            // Try to schedule a process
            if let Some(ref sched) = *scheduler::SCHEDULER.lock() {
                // Get current CPU ID (for now, assume CPU 0)
                let cpu_id = 0;
                
                if let Some(process) = sched.schedule(cpu_id) {
                    serial_println!("[SCHED] Running process PID {}", process.pid.as_u32());
                    
                    // Context switching handled by timer interrupt preemption
                    // Process will run when we return to user mode
                    sched.set_current(cpu_id, Some(process));
                }
            }
        }
        
        // Yield to allow interrupts
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
