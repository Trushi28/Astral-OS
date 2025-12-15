// src/usermode/spawn.rs
//! User process spawning - main API for launching Ring 3 processes

use crate::process::{Process, Pid, ProcessState, PriorityClass};
use crate::arch::x86_64::usermode::{
    create_user_address_space, 
    enter_usermode,
    USER_CODE_BASE,
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
    
    // Load ELF segments
    for ph in header.program_headers(elf_data)? {
        if ph.p_type == PT_LOAD {
            load_segment_to_user(ph, elf_data, &mut user_pt)?;
        }
    }
    
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

/// Load a single ELF segment into user page table
fn load_segment_to_user(
    ph: &super::elf::Elf64ProgramHeader,
    elf_data: &[u8],
    page_table: &mut PageTableManager,
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
    
    // Allocate and map pages
    let mut allocated_frames: alloc::vec::Vec<PhysAddr> = alloc::vec::Vec::new();
    
    for i in 0..num_pages {
        let page_virt = VirtAddr::new(virt_start + (i * crate::PAGE_SIZE) as u64);
        let frame = allocate_frame().ok_or("Out of memory loading ELF")?;
        
        // Zero the page
        unsafe {
            let ptr = (frame.as_u64() as usize + hhdm) as *mut u8;
            core::ptr::write_bytes(ptr, 0, crate::PAGE_SIZE);
        }
        
        // Use map_executable for code segments to ensure NX is not set on intermediate entries
        if is_executable {
            page_table.map_executable(page_virt, frame, flags)?;
        } else {
            page_table.map(page_virt, frame, flags)?;
        }
        
        allocated_frames.push(frame);
    }
    
    // Copy segment data
    if file_size > 0 {
        let segment_data = ph.get_data(elf_data)
            .ok_or("Invalid segment offset")?;
        
        for (offset, &byte) in segment_data.iter().enumerate() {
            let virt = virt_addr + offset as u64;
            let page_idx = ((virt & !0xFFF) - virt_start) as usize / crate::PAGE_SIZE;
            let page_offset = (virt & 0xFFF) as usize;
            
            if page_idx < allocated_frames.len() {
                let frame = allocated_frames[page_idx];
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

/// Embedded test binary - a minimal user program that calls write syscall
/// This is position-independent code that prints "Hello from Ring 3!\n"
pub fn get_test_user_binary() -> &'static [u8] {
    // Minimal user program (raw x86-64 machine code):
    // mov rax, 1        ; syscall number for write
    // mov rdi, 1        ; fd = stdout
    // lea rsi, [rip+msg] ; buffer address
    // mov rdx, 18       ; length
    // syscall
    // mov rax, 60       ; syscall number for exit  
    // xor rdi, rdi      ; exit code 0
    // syscall
    // msg: "Hello from Ring 3!\n"
    
    static TEST_CODE: [u8; 61] = [
        // mov rax, 1
        0x48, 0xc7, 0xc0, 0x01, 0x00, 0x00, 0x00,
        // mov rdi, 1
        0x48, 0xc7, 0xc7, 0x01, 0x00, 0x00, 0x00,
        // lea rsi, [rip+21] (message is 21 bytes after this instruction ends)
        0x48, 0x8d, 0x35, 0x15, 0x00, 0x00, 0x00,
        // mov rdx, 18
        0x48, 0xc7, 0xc2, 0x12, 0x00, 0x00, 0x00,
        // syscall
        0x0f, 0x05,
        // mov rax, 60 (exit)
        0x48, 0xc7, 0xc0, 0x3c, 0x00, 0x00, 0x00,
        // xor rdi, rdi
        0x48, 0x31, 0xff,
        // syscall
        0x0f, 0x05,
        // Message: "Hello from Ring 3!\n" (19 bytes)
        b'H', b'e', b'l', b'l', b'o', b' ', b'f', b'r',
        b'o', b'm', b' ', b'R', b'i', b'n', b'g', b' ',
        b'3', b'!', b'\n', 
    ];
    
    &TEST_CODE
}
