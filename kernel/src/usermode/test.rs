// src/usermode/test.rs
//! Test userspace program embedded in kernel for testing
//! This is a simple flat binary that writes "Hello from userspace!" and exits

/// Embedded test userspace program (flat binary)
/// Interactive shell that uses syscalls for I/O
/// 
/// Layout:
/// 0x00-0x0F: Prompt string "user@astral> "
/// 0x10-0x4F: Input buffer (64 bytes)
/// 0x50+: Code
pub static TEST_USERSPACE_BINARY: &[u8] = &[
    // ===== DATA SECTION (0x00 - 0x4F) =====
    // Prompt string at offset 0x00: "user@astral> " (14 bytes + 0 pad)
    b'u', b's', b'e', b'r', b'@', b'a', b's', b't',  // 0x00-0x07
    b'r', b'a', b'l', b'>', b' ', 0, 0, 0,            // 0x08-0x0F
    
    // Input buffer at offset 0x10 (64 bytes of zeros)
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,  // 0x10-0x1F
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,  // 0x20-0x2F
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,  // 0x30-0x3F
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,  // 0x40-0x4F
    
    // ===== CODE SECTION (0x50+) =====
    // Entry point is at 0x400050 (we'll adjust entry to skip data)
    
    // _start: (offset 0x50)
    // Print welcome message
    // mov rax, 1 (SYS_WRITE)
    0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00,
    // mov rbx, 1 (stdout)
    0x48, 0xC7, 0xC3, 0x01, 0x00, 0x00, 0x00,
    // mov rcx, 0x400000 (prompt addr relative to load)
    0x48, 0xC7, 0xC1, 0x00, 0x00, 0x40, 0x00,
    // mov rdx, 13 (prompt length)
    0x48, 0xC7, 0xC2, 0x0D, 0x00, 0x00, 0x00,
    // int 0x80
    0xCD, 0x80,
    
    // read_loop: (offset ~0x70)
    // mov rax, 0 (SYS_READ)
    0x48, 0xC7, 0xC0, 0x00, 0x00, 0x00, 0x00,
    // mov rbx, 0 (stdin)
    0x48, 0xC7, 0xC3, 0x00, 0x00, 0x00, 0x00,
    // mov rcx, 0x400010 (buffer addr)
    0x48, 0xC7, 0xC1, 0x10, 0x00, 0x40, 0x00,
    // mov rdx, 63 (max bytes)
    0x48, 0xC7, 0xC2, 0x3F, 0x00, 0x00, 0x00,
    // int 0x80
    0xCD, 0x80,
    
    // Check if we got input (rax > 0)
    // test rax, rax
    0x48, 0x85, 0xC0,
    // jz read_loop (jump back ~36 bytes if zero)
    0x74, 0xDB,  // -37 bytes back to read_loop
    
    // Echo the input back
    // Save bytes read in rdx for write
    // mov rdx, rax
    0x48, 0x89, 0xC2,
    // mov rax, 1 (SYS_WRITE)
    0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00,
    // mov rbx, 1 (stdout)
    0x48, 0xC7, 0xC3, 0x01, 0x00, 0x00, 0x00,
    // mov rcx, 0x400010 (buffer addr)
    0x48, 0xC7, 0xC1, 0x10, 0x00, 0x40, 0x00,
    // rdx already has length
    // int 0x80
    0xCD, 0x80,
    
    // Loop back to print prompt
    // jmp _start (jump back to offset 0x50, ~80 bytes back)
    0xE9, 0x7C, 0xFF, 0xFF, 0xFF,  // jmp -132 (back to start)
];

/// Run the embedded test userspace program using the new spawn API
pub fn run_test_program() -> ! {
    use crate::arch::x86_64::usermode::{
        create_user_address_space, 
        enter_usermode,
        USER_CODE_BASE,
    };
    use super::loader::load_flat_binary;
    
    crate::serial_println!("[USERMODE] Starting test userspace program...");
    
    // Create new user address space (page table with kernel mappings + user stack)
    let (mut page_table, stack_top) = create_user_address_space()
        .expect("Failed to create user address space");
    crate::serial_println!("[USERMODE] User address space created, stack at 0x{:x}", stack_top);
    
    // Load flat binary at user code base
    let load_addr = load_flat_binary(TEST_USERSPACE_BINARY, &mut page_table, USER_CODE_BASE)
        .expect("Failed to load user binary");
    // Entry point is at offset 0x50 (after data section)
    let entry_point = load_addr + 0x50;
    crate::serial_println!("[USERMODE] Loaded at 0x{:x}, entry: 0x{:x}", load_addr, entry_point);
    
    // Update TSS kernel stack for syscall returns
    crate::arch::x86_64::tss::update_kernel_stack(
        crate::arch::x86_64::tss::get_kernel_stack()
    );
    
    let page_table_phys = page_table.p4_physical().as_u64();
    crate::serial_println!("[USERMODE] Page table at phys 0x{:x}", page_table_phys);
    crate::serial_println!("[USERMODE] Entering ring 3...");
    
    // Switch to user page table and enter usermode
    unsafe {
        core::arch::asm!("mov cr3, {}", in(reg) page_table_phys, options(nostack));
        enter_usermode(entry_point, stack_top);
    }
    
    // Never reached - enter_usermode doesn't return
    loop {
        core::hint::spin_loop();
    }
}
