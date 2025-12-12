// src/usermode/mod.rs
//! Usermode process support

pub mod loader;
pub mod syscall;
pub mod elf;

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
    
    // Copy kernel mappings (higher half: 0xFFFF800000000000+)
    // This is needed so kernel code is accessible during syscalls
    // Implementation would copy P4 entries 256-511
    
    // For now, we'll use a simplified approach:
    // The kernel will handle mapping on syscall entry
    
    Ok(())
}

/// Allocate user stack
fn allocate_user_stack(pt: &mut PageTableManager) -> Result<u64, &'static str> {
    let num_pages = USER_STACK_SIZE / crate::PAGE_SIZE;
    let stack_start = USER_STACK_TOP - (USER_STACK_SIZE as u64);
    
    for i in 0..num_pages {
        let virt_addr = VirtAddr::new(stack_start + (i * crate::PAGE_SIZE) as u64);
        let frame = allocate_frame().ok_or("Out of memory")?;
        
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