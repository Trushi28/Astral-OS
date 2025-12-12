// src/usermode/mod.rs
//! Usermode process support

pub mod loader;
pub mod syscall;
pub mod elf;
pub mod test;

use crate::process::{Process, Pid, ProcessState, Registers};
use crate::memory::{PageTableManager, VirtAddr, PhysAddr, PageTableEntry};
use crate::memory::frame::allocate_frame;
use alloc::vec::Vec;

// Userspace memory layout
pub const USER_STACK_SIZE: usize = 8 * 1024 * 1024; // 8MB
pub const USER_STACK_TOP: u64 = 0x0000_7FFF_FFFF_F000;
pub const USER_HEAP_START: u64 = 0x0000_0000_1000_0000;
pub const USER_CODE_START: u64 = 0x0000_0000_0040_0000;

/// Create a new user process from ELF binary
pub fn create_user_process(elf_data: &[u8]) -> Result<Pid, &'static str> {
    let pid = Pid::new();
    
    // Create new page table for user process
    let mut page_table = PageTableManager::new()
        .ok_or("Failed to create page table")?;
    
    // Map kernel space (higher half)
    map_kernel_space(&mut page_table)?;
    
    // Load ELF binary
    let entry_point = loader::load_elf(elf_data, &mut page_table)?;
    
    // Allocate and map user stack
    let stack_bottom = allocate_user_stack(&mut page_table)?;
    
    // Create process structure
    let mut process = Process::new(pid);
    process.state = ProcessState::Ready;
    process.page_table = page_table.p4_physical().as_u64();
    
    // Setup initial registers for usermode
    process.registers = Registers::new();
    process.registers.rip = entry_point;
    process.registers.rsp = USER_STACK_TOP;
    process.registers.cs = 0x1B; // User code segment (GDT entry 3, RPL=3)
    process.registers.ss = 0x23; // User data segment (GDT entry 4, RPL=3)
    process.registers.rflags = 0x202; // IF enabled
    
    // Allocate kernel stack for this process
    if let Some(frame) = allocate_frame() {
        let stack_top = frame.to_virt() + crate::PAGE_SIZE;
        process.kernel_stack = stack_top as u64;
    } else {
        return Err("Failed to allocate kernel stack");
    }
    
    // Add to process table and scheduler
    crate::process::process_table().lock().add(process)?;
    crate::process::scheduler::add_to_scheduler(pid);
    
    Ok(pid)
}

/// Map kernel space in user page table (for syscalls)
fn map_kernel_space(pt: &mut PageTableManager) -> Result<(), &'static str> {
    // Get current kernel page table
    let kernel_pt = unsafe { PageTableManager::current() };
    
    // Get HHDM offset for virtual address translation
    let hhdm = crate::get_hhdm_offset();
    
    // Copy kernel mappings (higher half: P4 entries 256-511)
    // This is needed so kernel code is accessible during syscalls
    unsafe {
        let kernel_p4_phys = kernel_pt.p4_physical().as_u64();
        let kernel_p4 = (kernel_p4_phys as usize + hhdm) as *const u64;
        
        let user_p4_phys = pt.p4_physical().as_u64();
        let user_p4 = (user_p4_phys as usize + hhdm) as *mut u64;
        
        // Copy entries 256-511 (higher half - kernel space)
        for i in 256..512 {
            let kernel_entry = *kernel_p4.add(i);
            *user_p4.add(i) = kernel_entry;
        }
    }
    
    Ok(()
)}

/// Allocate user stack
fn allocate_user_stack(pt: &mut PageTableManager) -> Result<u64, &'static str> {
    // Stack grows DOWN, so we need to map pages from (TOP - SIZE) to TOP inclusive
    // The +1 ensures the page containing USER_STACK_TOP is mapped
    let num_pages = (USER_STACK_SIZE / crate::PAGE_SIZE) + 1;
    let stack_start = USER_STACK_TOP - (USER_STACK_SIZE as u64);
    
    for i in 0..num_pages {
        let virt_addr = VirtAddr::new(stack_start + (i * crate::PAGE_SIZE) as u64);
        let frame = allocate_frame().ok_or("Out of memory")?;
        
        // Zero the frame before mapping
        unsafe {
            let ptr = frame.to_virt() as *mut u8;
            core::ptr::write_bytes(ptr, 0, crate::PAGE_SIZE);
        }
        
        pt.map(
            virt_addr,
            frame,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE | 
            PageTableEntry::USER | PageTableEntry::NO_EXECUTE
        )?;
    }
    
    Ok(stack_start)
}

/// Switch to user process (used by scheduler)
pub fn enter_usermode(pid: Pid) {
    let table = crate::process::process_table().lock();
    
    if let Some(proc) = table.get(pid) {
        // Load process page table
        unsafe {
            let pt_phys = proc.page_table;
            core::arch::asm!("mov cr3, {}", in(reg) pt_phys);
        }
        
        // Setup TSS RSP0 for syscall/interrupt entry
        unsafe {
            let cpu_data = crate::arch::cpu::get_current_cpu_data_mut();
            cpu_data.tss_rsp0 = proc.kernel_stack;
            cpu_data.current_pid = pid.as_u64();
        }
        
        // Jump to usermode
        unsafe {
            core::arch::asm!(
                "mov rsp, {rsp}",
                "push {ss}",
                "push {rsp}",
                "push {rflags}",
                "push {cs}",
                "push {rip}",
                "iretq",
                rsp = in(reg) proc.registers.rsp,
                ss = in(reg) proc.registers.ss,
                rflags = in(reg) proc.registers.rflags,
                cs = in(reg) proc.registers.cs,
                rip = in(reg) proc.registers.rip,
                options(noreturn)
            );
        }
    }
}

/// Direct enter usermode with explicit parameters (for testing)
pub unsafe fn direct_enter_usermode(
    page_table_phys: u64,
    kernel_stack: u64,
    rip: u64,
    rsp: u64,
    cs: u64,
    ss: u64,
    rflags: u64,
) -> ! {
    // Set TSS RSP0 BEFORE switching page tables
    // This ensures interrupts in ring 3 return to correct kernel stack
    crate::interrupts::idt::set_tss_rsp0(kernel_stack);
    
    // Load process page table
    core::arch::asm!("mov cr3, {}", in(reg) page_table_phys);
    
    crate::serial_println!("[USERMODE] Jumping to ring 3: RIP=0x{:x} RSP=0x{:x} CS=0x{:x} SS=0x{:x}", 
        rip, rsp, cs, ss);
    
    // Jump to usermode via IRETQ
    // Stack layout for IRETQ: [RIP, CS, RFLAGS, RSP, SS] (pushed in reverse)
    core::arch::asm!(
        "push {ss}",      // SS
        "push {rsp}",     // RSP
        "push {rflags}",  // RFLAGS (IF will enable interrupts)
        "push {cs}",      // CS
        "push {rip}",     // RIP
        "iretq",
        ss = in(reg) ss,
        rsp = in(reg) rsp,
        rflags = in(reg) rflags,
        cs = in(reg) cs,
        rip = in(reg) rip,
        options(noreturn)
    );
}