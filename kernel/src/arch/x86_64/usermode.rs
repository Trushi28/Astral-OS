//! User-Mode Entry
//! Functions to jump to Ring 3 code

use core::arch::asm;
use crate::arch::x86_64::gdt::{USER_CODE_SELECTOR, USER_DATA_SELECTOR};

/// Jump to user mode at specified address
/// 
/// # Safety
/// The target address must be valid user-mode code.
pub unsafe fn enter_usermode(entry_point: u64, user_stack: u64) {
    // Prepare iretq frame on stack:
    // SS, RSP, RFLAGS, CS, RIP
    
    let rflags: u64 = 0x202;  // IF set (interrupts enabled)
    
    asm!(
        // Push SS (user data selector)
        "push {ss}",
        // Push user RSP
        "push {rsp_user}",
        // Push RFLAGS
        "push {rflags}",
        // Push CS (user code selector) 
        "push {cs}",
        // Push RIP (entry point)
        "push {rip}",
        // Jump to Ring 3
        "iretq",
        ss = in(reg) USER_DATA_SELECTOR as u64,
        rsp_user = in(reg) user_stack,
        rflags = in(reg) rflags,
        cs = in(reg) USER_CODE_SELECTOR as u64,
        rip = in(reg) entry_point,
        options(noreturn)
    );
}

/// Initialize Ring 3 support
pub fn init() {
    // Initialize TSS first
    crate::arch::x86_64::tss::init();
    
    // Initialize GDT with Ring 3 segments
    crate::arch::x86_64::gdt::init();
    
    // Initialize syscall interface
    crate::arch::x86_64::syscall::init();
    
    crate::serial_println!("[RING3] User-mode support initialized");
}

/// Allocate a user-mode stack
pub fn allocate_user_stack() -> u64 {
    // Allocate 64KB for user stack at fixed address
    const USER_STACK_BASE: u64 = 0x0000_7FFF_FFFF_0000;
    const USER_STACK_SIZE: u64 = 0x10000;  // 64KB
    
    // Map pages for user stack
    let stack_top = USER_STACK_BASE + USER_STACK_SIZE;
    
    // In a real implementation, we'd allocate and map these pages
    // For now, return the stack top
    stack_top
}
