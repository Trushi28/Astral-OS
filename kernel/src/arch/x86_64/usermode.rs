//! User-Mode Entry
//! Functions to jump to Ring 3 code

use core::arch::asm;
use crate::arch::x86_64::gdt::{USER_CODE_SELECTOR, USER_DATA_SELECTOR};
use crate::memory::{PageTableManager, VirtAddr, PhysAddr, PageTableEntry};
use crate::memory::frame::allocate_frame;

/// User stack configuration
/// Stack grows downward, so we place it near the top of user space
/// but ensure stack_top stays within canonical user addresses (< 0x7FFFFFFFFFFF)
pub const USER_STACK_BASE: u64 = 0x0000_7FFF_FFE0_0000;  // Base of stack region
pub const USER_STACK_SIZE: usize = 0x10000;  // 64KB (16 pages)
pub const USER_STACK_PAGES: usize = USER_STACK_SIZE / crate::PAGE_SIZE;

/// User code load address (standard Linux convention)
pub const USER_CODE_BASE: u64 = 0x0000_0000_0040_0000;

/// Jump to user mode at specified address
/// 
/// # Safety
/// The target address must be valid user-mode code.
/// The user_stack must point to a properly mapped user stack.
pub unsafe fn enter_usermode(entry_point: u64, user_stack: u64) -> ! {
    crate::serial_println!("[RING3] Entering user mode: RIP=0x{:x}, RSP=0x{:x}", entry_point, user_stack);
    crate::serial_println!("[RING3] Selectors: CS=0x{:x}, SS=0x{:x}", 
        USER_CODE_SELECTOR, USER_DATA_SELECTOR);
    
    crate::serial_println!("[RING3] About to iretq...");
    
    // iretq expects this stack layout (from high to low address):
    //   SS      (8 bytes)
    //   RSP     (8 bytes)  
    //   RFLAGS  (8 bytes)
    //   CS      (8 bytes)
    //   RIP     (8 bytes)
    
    let ss = USER_DATA_SELECTOR as u64;
    let cs = USER_CODE_SELECTOR as u64;
    let rflags: u64 = 0x202;  // IF set (interrupts enabled)
    
    // Ensure these values are loaded into registers BEFORE the pushes
    // to avoid any register allocation issues
    asm!(
        // Disable interrupts during the transition
        "cli",
        // Save values to known registers
        "mov r10, {entry}",
        "mov r11, {stack}",
        "mov r12, {cs_val}",
        "mov r13, {ss_val}",
        "mov r14, {flags}",
        // Build iretq frame on current stack
        "push r13",      // SS
        "push r11",      // User RSP
        "push r14",      // RFLAGS
        "push r12",      // CS
        "push r10",      // RIP (entry point)
        // Return to Ring 3
        "iretq",
        entry = in(reg) entry_point,
        stack = in(reg) user_stack,
        cs_val = in(reg) cs,
        ss_val = in(reg) ss,
        flags = in(reg) rflags,
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

/// Allocate and map a user-mode stack
/// 
/// Returns (stack_top, Vec of allocated frames) so caller can track memory
pub fn allocate_user_stack(page_table: &mut PageTableManager) -> Result<u64, &'static str> {
    crate::serial_println!("[RING3] Allocating user stack: {} pages at 0x{:x}", 
        USER_STACK_PAGES, USER_STACK_BASE);
    
    // Allocate and map pages for user stack
    for i in 0..USER_STACK_PAGES {
        let page_virt = VirtAddr::new(USER_STACK_BASE + (i * crate::PAGE_SIZE) as u64);
        let frame = allocate_frame().ok_or("Out of memory for user stack")?;
        
        // Zero the page before mapping
        unsafe {
            let hhdm = crate::get_hhdm_offset();
            let ptr = (frame.as_u64() as usize + hhdm) as *mut u8;
            core::ptr::write_bytes(ptr, 0, crate::PAGE_SIZE);
        }
        
        // Map as USER + WRITABLE + PRESENT + NO_EXECUTE (stack shouldn't be executable)
        let flags = PageTableEntry::PRESENT 
            | PageTableEntry::WRITABLE 
            | PageTableEntry::USER
            | PageTableEntry::NO_EXECUTE;
        
        page_table.map(page_virt, frame, flags)?;
    }
    
    // Return stack top (stack grows downward)
    let stack_top = USER_STACK_BASE + USER_STACK_SIZE as u64;
    crate::serial_println!("[RING3] User stack allocated, top at 0x{:x}", stack_top);
    
    Ok(stack_top)
}

/// Information about a created user address space
pub struct UserAddressSpace {
    pub page_table_phys: PhysAddr,
    pub stack_top: u64,
}

/// Create a new user address space with kernel mappings in upper half
/// 
/// This clones the kernel page table's upper half (kernel mappings) and
/// creates a fresh lower half for user space.
pub fn create_user_address_space() -> Result<(PageTableManager, u64), &'static str> {
    crate::serial_println!("[RING3] Creating user address space...");
    
    // Allocate a new PML4 (top-level page table)
    let pml4_frame = allocate_frame().ok_or("Out of memory for user page table")?;
    
    // Zero the new PML4
    unsafe {
        let hhdm = crate::get_hhdm_offset();
        let ptr = (pml4_frame.as_u64() as usize + hhdm) as *mut u8;
        core::ptr::write_bytes(ptr, 0, crate::PAGE_SIZE);
    }
    
    // Copy kernel mappings (upper half: entries 256-511)
    // The kernel uses the upper half of virtual address space
    unsafe {
        let hhdm = crate::get_hhdm_offset();
        let current_cr3: u64;
        asm!("mov {}, cr3", out(reg) current_cr3, options(nostack));
        
        let current_pml4 = ((current_cr3 & !0xFFF) as usize + hhdm) as *const u64;
        let new_pml4 = (pml4_frame.as_u64() as usize + hhdm) as *mut u64;
        
        // Copy entries 256-511 (kernel half)
        for i in 256..512 {
            let entry = *current_pml4.add(i);
            *new_pml4.add(i) = entry;
        }
        
        crate::serial_println!("[RING3] Copied kernel mappings to new page table");
    }
    
    // Create PageTableManager for the new address space
    let mut user_pt = PageTableManager::from_phys(pml4_frame);
    
    // Allocate user stack in the new address space
    let stack_top = allocate_user_stack(&mut user_pt)?;
    
    crate::serial_println!("[RING3] User address space created: PML4=0x{:x}, stack=0x{:x}",
        pml4_frame.as_u64(), stack_top);
    
    Ok((user_pt, stack_top))
}

/// Switch to a user page table (for context switching)
/// 
/// # Safety
/// The page table must be valid and have kernel mappings in the upper half.
pub unsafe fn switch_to_user_page_table(pml4_phys: PhysAddr) {
    asm!("mov cr3, {}", in(reg) pml4_phys.as_u64(), options(nostack));
}
