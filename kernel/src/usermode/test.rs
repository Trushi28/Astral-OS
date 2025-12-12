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

/// Run the embedded test userspace program
pub fn run_test_program() -> Result<(), &'static str> {
    use super::{USER_CODE_START, USER_STACK_TOP, loader::load_flat_binary};
    use crate::memory::PageTableManager;
    use crate::memory::frame::allocate_frame;
    
    crate::println!("[USERMODE] Starting test userspace program...");
    
    // Create new page table for user process
    let mut page_table = PageTableManager::new()
        .ok_or("Failed to create page table")?;
    
    // Map kernel space (higher half)
    super::map_kernel_space(&mut page_table)?;
    crate::println!("[USERMODE] Kernel space mapped");
    
    // Print IDT address for debugging
    let idt_addr = crate::interrupts::idt::get_idt_addr();
    crate::println!("[USERMODE] IDT at 0x{:x} (P4 index {})", idt_addr, idt_addr >> 39 & 0x1FF);
    
    // Load flat binary
    let _load_addr = load_flat_binary(TEST_USERSPACE_BINARY, &mut page_table, USER_CODE_START)?;
    // Entry point is at offset 0x50 (after data section)
    let entry_point = USER_CODE_START + 0x50;
    crate::println!("[USERMODE] Loaded at 0x{:x}, entry: 0x{:x}", USER_CODE_START, entry_point);
    
    // Map a few extra pages after code for CPU prefetch
    // CPU may speculatively fetch from next page
    for i in 1..4 {
        let extra_addr = crate::memory::VirtAddr::new(USER_CODE_START + (i * crate::PAGE_SIZE) as u64);
        if let Some(frame) = allocate_frame() {
            unsafe {
                let ptr = frame.to_virt() as *mut u8;
                core::ptr::write_bytes(ptr, 0x90u8, crate::PAGE_SIZE); // Fill with NOPs
            }
            let _ = page_table.map(
                extra_addr,
                frame,
                crate::memory::PageTableEntry::PRESENT | 
                crate::memory::PageTableEntry::USER
            );
            crate::println!("[USERMODE] Extra page mapped at 0x{:x}", extra_addr.as_u64());
        }
    }
    
    // Allocate and map user stack
    let _stack_bottom = super::allocate_user_stack(&mut page_table)?;
    crate::println!("[USERMODE] Stack at 0x{:x}", USER_STACK_TOP);
    
    // Allocate kernel stack for this process
    let kernel_stack = if let Some(frame) = allocate_frame() {
        frame.to_virt() + crate::PAGE_SIZE
    } else {
        return Err("Failed to allocate kernel stack");
    };
    crate::println!("[USERMODE] Kernel stack at 0x{:x}", kernel_stack);
    
    let page_table_phys = page_table.p4_physical().as_u64();
    crate::println!("[USERMODE] Page table at phys 0x{:x}", page_table_phys);
    
    crate::println!("[USERMODE] Entering ring 3...");
    
    // Enter usermode directly
    unsafe {
        super::direct_enter_usermode(
            page_table_phys,
            kernel_stack as u64,
            entry_point,           // RIP
            USER_STACK_TOP,        // RSP
            0x1B,                  // CS = user code segment
            0x23,                  // SS = user data segment  
            0x202,                 // RFLAGS = IF enabled
        );
    }
}
