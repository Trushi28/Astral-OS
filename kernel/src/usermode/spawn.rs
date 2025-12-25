// src/usermode/spawn.rs
//! User process spawning - main API for launching Ring 3 processes

use crate::process::{Process, Pid, ProcessState, PriorityClass};
use crate::arch::x86_64::usermode::{
    create_user_address_space, 
    enter_usermode,
};
use crate::arch::x86_64::gdt::{USER_CODE_SELECTOR, USER_DATA_SELECTOR};
use crate::arch::x86_64::tss::update_kernel_stack;
use crate::memory::{PageTableManager, VirtAddr, PhysAddr, PageTableEntry};
use crate::memory::frame::allocate_frame;
use super::elf::{Elf64Header, PT_LOAD};

/// Kernel stack size per user process (16KB)
const KERNEL_STACK_SIZE: usize = 16384;

/// Information about a spawned user process
pub struct SpawnedProcess {
    pub pid: Pid,
    pub entry_point: u64,
    pub user_stack: u64,
    pub page_table_phys: u64,
}

/// Spawn a user process from ELF binary data
/// 
/// This:
/// 1. Creates a new user address space (page tables with kernel mappings)
/// 2. Loads ELF segments into the user address space
/// 3. Allocates kernel stack for syscall/interrupt handling
/// 4. Creates and registers Process with scheduler
/// 
/// Returns SpawnedProcess info on success
pub fn spawn_user_process(elf_data: &[u8]) -> Result<SpawnedProcess, &'static str> {
    crate::serial_println!("[SPAWN] Spawning user process from {} bytes of ELF data", elf_data.len());
    
    // Parse ELF header
    let header = Elf64Header::parse(elf_data)?;
    let entry_point = header.entry_point();
    
    crate::serial_println!("[SPAWN] ELF entry point: 0x{:x}", entry_point);
    
    // Validate entry point is in user space
    if entry_point >= 0x0000_8000_0000_0000 {
        return Err("ELF entry point is not in user space");
    }
    
    // Create user address space (new page table + user stack)
    let (mut user_pt, stack_top) = create_user_address_space()?;
    let page_table_phys = user_pt.p4_physical().as_u64();
    
    // Track mapped pages across all segments (page_vaddr -> frame_phys)
    // This prevents "Page already mapped" errors when segments share pages
    let mut mapped_pages: alloc::collections::BTreeMap<u64, PhysAddr> = alloc::collections::BTreeMap::new();
    
    // Load ELF segments and track highest address for heap start
    let mut highest_addr: u64 = 0;
    for ph in header.program_headers(elf_data)? {
        if ph.p_type == PT_LOAD {
            load_segment_with_tracking(ph, elf_data, &mut user_pt, &mut mapped_pages)?;
            let seg_end = ph.p_vaddr + ph.p_memsz;
            if seg_end > highest_addr {
                highest_addr = seg_end;
            }
        }
    }
    
    // Calculate heap start (page-aligned, after all loaded segments + gap)
    let heap_start = crate::util::align_up((highest_addr + 0x1000) as usize, crate::PAGE_SIZE) as u64;
    let heap_end = heap_start; // Initially empty heap
    let mmap_base: u64 = 0x0000_4000_0000_0000; // mmap region starts here
    
    crate::serial_println!("[SPAWN] Heap region: start=0x{:x}, mmap_base=0x{:x}", heap_start, mmap_base);
    
    // Allocate kernel stack for this process
    let kernel_stack = allocate_kernel_stack()?;
    crate::serial_println!("[SPAWN] Kernel stack at 0x{:x}", kernel_stack);
    
    // Create Process
    let pid = Pid::new();
    let mut process = Process::new(pid);
    
    // Set up for Ring 3
    process.is_user_process = true;
    process.user_stack = stack_top;
    process.page_table = page_table_phys;
    process.kernel_stack = kernel_stack;
    process.state = ProcessState::Ready;
    process.priority = PriorityClass::Interactive;
    
    // Initialize heap region
    process.heap_start = heap_start;
    process.heap_end = heap_end;
    process.mmap_base = mmap_base;
    
    // Set up initial register state for user mode
    process.registers.rip = entry_point;
    process.registers.rsp = stack_top;
    process.registers.cs = USER_CODE_SELECTOR as u64;
    process.registers.ss = USER_DATA_SELECTOR as u64;
    process.registers.rflags = 0x202; // Interrupts enabled
    
    // Register with process table and scheduler
    {
        let mut table = crate::process::process_table().lock();
        table.add(process)?;
    }
    crate::process::scheduler::add_to_scheduler_with_priority(pid, PriorityClass::Interactive);
    
    crate::serial_println!("[SPAWN] User process {} ready: entry=0x{:x}, stack=0x{:x}",
        pid.as_u64(), entry_point, stack_top);
    
    Ok(SpawnedProcess {
        pid,
        entry_point,
        user_stack: stack_top,
        page_table_phys,
    })
}

/// Load a single ELF segment into user page table with page tracking
/// 
/// Uses `mapped_pages` to track which pages have already been allocated/mapped,
/// allowing segments that share pages (e.g., end of .text and start of .rodata
/// falling in the same page) to work correctly.
fn load_segment_with_tracking(
    ph: &super::elf::Elf64ProgramHeader,
    elf_data: &[u8],
    page_table: &mut PageTableManager,
    mapped_pages: &mut alloc::collections::BTreeMap<u64, PhysAddr>,
) -> Result<(), &'static str> {
    let virt_addr = ph.p_vaddr;
    let mem_size = ph.p_memsz as usize;
    let file_size = ph.p_filesz as usize;
    
    if mem_size == 0 {
        return Ok(());
    }
    
    crate::serial_println!("[SPAWN] Loading segment: vaddr=0x{:x}, memsz={}, filesz={}, flags=0x{:x}",
        virt_addr, mem_size, file_size, ph.p_flags);
    
    // Calculate page-aligned bounds
    let virt_start = virt_addr & !0xFFF;
    let virt_end = crate::util::align_up((virt_addr + mem_size as u64) as usize, crate::PAGE_SIZE);
    let num_pages = (virt_end - virt_start as usize) / crate::PAGE_SIZE;
    
    crate::serial_println!("[SPAWN]   virt_start=0x{:x}, virt_end=0x{:x}, num_pages={}", 
        virt_start, virt_end, num_pages);
    
    // Determine page flags based on segment permissions
    let mut flags = PageTableEntry::PRESENT | PageTableEntry::USER;
    
    if ph.is_writable() {
        flags |= PageTableEntry::WRITABLE;
    }
    
    // Note: We set NO_EXECUTE only if segment is NOT executable
    let is_executable = ph.is_executable();
    if !is_executable {
        flags |= PageTableEntry::NO_EXECUTE;
    }
    
    let hhdm = crate::get_hhdm_offset();
    
    // Collect frames for this segment (some may already be mapped)
    let mut segment_frames: alloc::vec::Vec<PhysAddr> = alloc::vec::Vec::new();
    
    for i in 0..num_pages {
        let page_virt = virt_start + (i * crate::PAGE_SIZE) as u64;
        
        // Check if this page is already mapped (from a previous segment)
        let frame = if let Some(&existing_frame) = mapped_pages.get(&page_virt) {
            // Reuse existing frame - page is already mapped
            crate::serial_println!("[SPAWN]   Reusing page 0x{:x} -> frame 0x{:x}", page_virt, existing_frame.as_u64());
            existing_frame
        } else {
            // Allocate new frame
            crate::serial_println!("[SPAWN]   Allocating NEW page 0x{:x}", page_virt);
            let new_frame = allocate_frame().ok_or("Out of memory loading ELF")?;
            
            crate::serial_println!("[SPAWN]   Got frame 0x{:x}, zeroing...", new_frame.as_u64());
            
            // Zero the page
            unsafe {
                let ptr = (new_frame.as_u64() as usize + hhdm) as *mut u8;
                core::ptr::write_bytes(ptr, 0, crate::PAGE_SIZE);
            }
            
            // Map the page
            let page_virt_addr = VirtAddr::new(page_virt);
            crate::serial_println!("[SPAWN]   Mapping page 0x{:x} -> frame 0x{:x}, exec={}", 
                page_virt, new_frame.as_u64(), is_executable);
            
            if is_executable {
                page_table.map_executable(page_virt_addr, new_frame, flags)?;
            } else {
                page_table.map(page_virt_addr, new_frame, flags)?;
            }
            
            // Track this mapping
            mapped_pages.insert(page_virt, new_frame);
            crate::serial_println!("[SPAWN]   Mapped successfully");
            
            new_frame
        };
        
        segment_frames.push(frame);
    }
    
    // Copy segment data
    if file_size > 0 {
        let segment_data = ph.get_data(elf_data)
            .ok_or("Invalid segment offset")?;
        
        for (offset, &byte) in segment_data.iter().enumerate() {
            let virt = virt_addr + offset as u64;
            let page_idx = ((virt & !0xFFF) - virt_start) as usize / crate::PAGE_SIZE;
            let page_offset = (virt & 0xFFF) as usize;
            
            if page_idx < segment_frames.len() {
                let frame = segment_frames[page_idx];
                unsafe {
                    let ptr = (frame.as_u64() as usize + hhdm + page_offset) as *mut u8;
                    core::ptr::write_volatile(ptr, byte);
                }
            }
        }
    }
    
    // BSS (mem_size > file_size) is already zero-filled since we zeroed pages
    
    Ok(())
}


/// Allocate a kernel stack for a user process
fn allocate_kernel_stack() -> Result<u64, &'static str> {
    // Allocate 4 pages (16KB) for kernel stack
    let num_pages = KERNEL_STACK_SIZE / crate::PAGE_SIZE;
    
    // We allocate frames but they're already in the direct map
    // So we use the direct-mapped (HHDM) address
    let base_frame = allocate_frame().ok_or("Out of memory for kernel stack")?;
    let hhdm = crate::get_hhdm_offset();
    
    unsafe {
        let ptr = (base_frame.as_u64() as usize + hhdm) as *mut u8;
        core::ptr::write_bytes(ptr, 0, crate::PAGE_SIZE);
    }
    
    // Allocate remaining pages (for larger stack)
    for _ in 1..num_pages {
        let frame = allocate_frame().ok_or("Out of memory for kernel stack")?;
        unsafe {
            let ptr = (frame.as_u64() as usize + hhdm) as *mut u8;
            core::ptr::write_bytes(ptr, 0, crate::PAGE_SIZE);
        }
    }
    
    // Return stack top (stack grows down)
    // Using HHDM address so it's accessible in kernel mode
    let stack_top = (base_frame.as_u64() as usize + hhdm + KERNEL_STACK_SIZE) as u64;
    Ok(stack_top)
}

/// Run a user process immediately (for testing)
/// 
/// This switches to the user address space and jumps to user mode.
/// Should only be used for single-process testing - use scheduler for real usage.
pub fn run_user_process_now(spawned: &SpawnedProcess) -> ! {
    crate::serial_println!("[SPAWN] Running user process {} now", spawned.pid.as_u64());
    
    // Get kernel stack for this process
    let kernel_stack = {
        let table = crate::process::process_table().lock();
        table.get(spawned.pid)
            .map(|p| p.kernel_stack)
            .unwrap_or(0)
    };
    
    // Update TSS with kernel stack for syscall returns
    update_kernel_stack(kernel_stack);
    
    // Switch to user page table
    unsafe {
        core::arch::asm!(
            "mov cr3, {}",
            in(reg) spawned.page_table_phys,
            options(nostack)
        );
    }
    
    // Jump to user mode (never returns)
    unsafe {
        enter_usermode(spawned.entry_point, spawned.user_stack);
    }
    // enter_usermode never returns, but Rust doesn't know that
    loop {
        core::hint::spin_loop();
    }
}
