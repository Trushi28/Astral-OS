//src/process/context.rs
use super::{Pid, Registers, ProcessState, process_table, get_current_pid, set_current_pid};
use core::arch::asm;

/// Save current context and switch to new process
#[unsafe(naked)]
pub unsafe extern "C" fn context_switch(old_regs: *mut Registers, new_regs: *const Registers) {
    core::arch::naked_asm!(
        // Save old context
        "mov [rdi + 0x00], r15",
        "mov [rdi + 0x08], r14",
        "mov [rdi + 0x10], r13",
        "mov [rdi + 0x18], r12",
        "mov [rdi + 0x20], rbx",
        "mov [rdi + 0x28], rbp",
        "mov [rdi + 0x30], r11",
        "mov [rdi + 0x38], r10",
        "mov [rdi + 0x40], r9",
        "mov [rdi + 0x48], r8",
        "mov [rdi + 0x50], rsi",
        "mov [rdi + 0x60], rdx",
        "mov [rdi + 0x68], rcx",
        "mov [rdi + 0x70], rax",
        
        // Save RIP (return address)
        "mov rax, [rsp]",
        "mov [rdi + 0x78], rax",
        
        // Save RSP
        "lea rax, [rsp + 8]",
        "mov [rdi + 0x90], rax",
        
        // Save RFLAGS
        "pushfq",
        "pop rax",
        "mov [rdi + 0x88], rax",
        
        // Save RDI last
        "mov [rdi + 0x58], rdi",
        
        // Restore new context
        "mov r15, [rsi + 0x00]",
        "mov r14, [rsi + 0x08]",
        "mov r13, [rsi + 0x10]",
        "mov r12, [rsi + 0x18]",
        "mov rbx, [rsi + 0x20]",
        "mov rbp, [rsi + 0x28]",
        "mov r11, [rsi + 0x30]",
        "mov r10, [rsi + 0x38]",
        "mov r9,  [rsi + 0x40]",
        "mov r8,  [rsi + 0x48]",
        "mov rdi, [rsi + 0x58]",
        "mov rdx, [rsi + 0x60]",
        "mov rcx, [rsi + 0x68]",
        "mov rax, [rsi + 0x70]",
        
        // Restore RSP
        "mov rsp, [rsi + 0x90]",
        
        // Push return address and RFLAGS
        "push qword ptr [rsi + 0x78]",
        "push qword ptr [rsi + 0x88]",
        "popfq",
        
        // Restore RSI last and return
        "mov rsi, [rsi + 0x50]",
        "ret",
    )
}

/// High-level context switch function
pub fn switch_to_process(new_pid: Pid) {
    let current_pid = get_current_pid();
    
    // Don't switch if already running
    if let Some(curr) = current_pid {
        if curr == new_pid {
            return;
        }
    }
    
    // Lock process table and get pointers
    let mut table = process_table().lock();
    
    // Get old process registers pointer
    let old_regs_ptr = current_pid
        .and_then(|pid| {
            table.processes.iter_mut().flatten()
                .find(|p| p.pid == pid)
                .map(|proc| {
                    proc.state = ProcessState::Ready;
                    &mut proc.registers as *mut Registers
                })
        });
    
    // Get new process info
    let new_proc = table.processes.iter_mut().flatten()
        .find(|p| p.pid == new_pid)
        .expect("Process not found");
    
    new_proc.state = ProcessState::Running;
    let new_regs_ptr = &new_proc.registers as *const Registers;
    let new_page_table = new_proc.page_table;
    
    // Release lock before switching
    drop(table);
    
    // Load new page table if different
    if new_page_table != 0 {
        unsafe {
            asm!("mov cr3, {}", in(reg) new_page_table, options(nostack, preserves_flags));
        }
    }
    
    // Update current PID
    set_current_pid(new_pid);
    
    // Perform context switch
    if let Some(old_ptr) = old_regs_ptr {
        unsafe {
            context_switch(old_ptr, new_regs_ptr);
        }
    } else {
        // First process - jump directly
        unsafe {
            let regs = &*new_regs_ptr;
            asm!(
                "mov rsp, {rsp}",
                "push {ss}",
                "push {rsp}",
                "push {rflags}",
                "push {cs}",
                "push {rip}",
                "mov rax, {rax}",
                "iretq",
                rsp = in(reg) regs.rsp,
                ss = in(reg) regs.ss,
                rflags = in(reg) regs.rflags,
                cs = in(reg) regs.cs,
                rip = in(reg) regs.rip,
                rax = in(reg) regs.rax,
                options(noreturn)
            );
        }
    }
}