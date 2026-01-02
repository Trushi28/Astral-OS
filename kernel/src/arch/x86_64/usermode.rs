//! Usermode transition module
//! 
//! Provides functions to jump from kernel mode (Ring 0) to user mode (Ring 3)
//! using IRETQ instruction.

use x86_64::VirtAddr;
use crate::serial_println;

/// User code segment selector (GDT index 4, RPL=3)
pub const USER_CODE_SELECTOR: u16 = 0x23; // (4 << 3) | 3

/// User data segment selector (GDT index 3, RPL=3)  
pub const USER_DATA_SELECTOR: u16 = 0x1B; // (3 << 3) | 3

/// Jump to usermode using IRETQ
/// 
/// Sets up the stack frame required by IRETQ:
/// - SS (user stack segment)
/// - RSP (user stack pointer)
/// - RFLAGS (with interrupts enabled)
/// - CS (user code segment)
/// - RIP (user entry point)
/// 
/// # Safety
/// - entry_point must be a valid, mapped user-accessible address
/// - user_stack must point to a valid, mapped user stack
/// - This function never returns
#[inline(never)]
pub unsafe fn jump_to_usermode(entry_point: VirtAddr, user_stack: VirtAddr) -> ! {
    serial_println!("[USERMODE] Jumping to Ring 3:");
    serial_println!("  Entry: {:#x}", entry_point.as_u64());
    serial_println!("  Stack: {:#x}", user_stack.as_u64());
    serial_println!("  CS: {:#x}, SS: {:#x}", USER_CODE_SELECTOR, USER_DATA_SELECTOR);
    
    core::arch::asm!(
        // Disable interrupts during transition
        "cli",
        
        // Push SS (user data segment)
        "push {user_ss}",
        
        // Push RSP (user stack pointer)
        "push {user_rsp}",
        
        // Push RFLAGS with IF set (interrupts enabled after iretq)
        "push 0x202",
        
        // Push CS (user code segment)
        "push {user_cs}",
        
        // Push RIP (entry point)
        "push {user_rip}",
        
        // Fire! Jump to Ring 3
        "iretq",
        
        user_ss = in(reg) USER_DATA_SELECTOR as u64,
        user_rsp = in(reg) user_stack.as_u64(),
        user_cs = in(reg) USER_CODE_SELECTOR as u64,
        user_rip = in(reg) entry_point.as_u64(),
        options(noreturn)
    )
}

/// Test usermode transition with a simple inline test
/// 
/// This creates a minimal test by:
/// 1. Mapping a user-accessible page for code
/// 2. Writing a simple syscall (exit) to it
/// 3. Jumping to usermode and executing it
/// 
/// # Safety
/// Only call after GDT, IDT, and memory systems are initialized
pub unsafe fn test_usermode_transition() {
    use crate::memory::user_memory;
    
    serial_println!("[USERMODE] Starting Ring 3 transition test...");
    
    // Create a user address space
    let (user_cr3, _pml4_virt) = match user_memory::create_user_page_table() {
        Ok(result) => result,
        Err(e) => {
            serial_println!("[USERMODE] Failed to create user page table: {}", e);
            return;
        }
    };
    
    // Allocate user stack with guard page
    let user_stack_top = match user_memory::allocate_user_stack() {
        Ok(stack) => stack,
        Err(e) => {
            serial_println!("[USERMODE] Failed to allocate user stack: {}", e);
            return;
        }
    };
    
    // Allocate user code page
    let user_code_base = VirtAddr::new(user_memory::USER_CODE_START);
    if let Err(e) = user_memory::allocate_user_code_page(user_code_base) {
        serial_println!("[USERMODE] Failed to allocate user code page: {}", e);
        return;
    }
    
    // Write simple test code to user memory
    // This code does: mov rax, 60 (exit syscall); xor rdi, rdi (exit code 0); syscall
    let test_code: [u8; 16] = [
        0x48, 0xC7, 0xC0, 0x3C, 0x00, 0x00, 0x00,  // mov rax, 60 (exit)
        0x48, 0x31, 0xFF,                          // xor rdi, rdi (code = 0)
        0x0F, 0x05,                                // syscall
        0xEB, 0xFE,                                // jmp $ (infinite loop fallback)
        0x90, 0x90,                                // nop padding
    ];
    
    // Write code to user page
    let code_ptr = user_code_base.as_u64() as *mut u8;
    for (i, byte) in test_code.iter().enumerate() {
        core::ptr::write_volatile(code_ptr.add(i), *byte);
    }
    
    serial_println!("[USERMODE] Test code written at {:#x}", user_code_base.as_u64());
    serial_println!("[USERMODE] Switching to user CR3: {:#x}", user_cr3);
    
    // Switch to user page table
    x86_64::registers::control::Cr3::write(
        x86_64::structures::paging::PhysFrame::containing_address(
            x86_64::PhysAddr::new(user_cr3)
        ),
        x86_64::registers::control::Cr3Flags::empty()
    );
    
    // Jump to usermode!
    jump_to_usermode(user_code_base, user_stack_top);
}
