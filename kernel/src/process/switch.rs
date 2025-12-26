use super::context::Context;
use core::arch::naked_asm;

/// Switch from current process context to new process context
/// 
/// CRITICAL: Uses explicit register constraints ("rax", "rbx", etc.)
/// to avoid compiler choosing wrong registers
///
/// # Safety
/// This function is unsafe because it:
/// - Directly manipulates CPU registers and stack
/// - Assumes contexts are valid and properly aligned
/// - Must only be called with interrupts disabled
#[unsafe(naked)]
pub unsafe extern "C" fn switch_context(old: *mut Context, new: *const Context) {
    naked_asm!(
        // Save current context (callee-saved registers)
        "mov [rdi + 0], r15",      // Save r15
        "mov [rdi + 8], r14",      // Save r14
        "mov [rdi + 16], r13",     // Save r13
        "mov [rdi + 24], r12",     // Save r12
        "mov [rdi + 32], rbp",     // Save rbp
        "mov [rdi + 40], rbx",     // Save rbx
        
        // Save RIP (return address from stack)
        "mov rax, [rsp]",          // Get return address
        "mov [rdi + 48], rax",     // Save as RIP
        
        // Save RSP (current stack pointer + 8 to skip return address)
        "lea rax, [rsp + 8]",      // RSP after we return
        "mov [rdi + 56], rax",     // Save RSP
        
        // Save RFLAGS
        "pushfq",
        "pop rax",
        "mov [rdi + 64], rax",     // Save RFLAGS
        
        // Load new context
        "mov r15, [rsi + 0]",      // Load r15
        "mov r14, [rsi + 8]",      // Load r14
        "mov r13, [rsi + 16]",     // Load r13
        "mov r12, [rsi + 24]",     // Load r12
        "mov rbp, [rsi + 32]",     // Load rbp
        "mov rbx, [rsi + 40]",     // Load rbx
        
        // Load new RSP
        "mov rsp, [rsi + 56]",     // Load new stack pointer
        
        // Load RFLAGS
        "mov rax, [rsi + 64]",     // Load RFLAGS
        "push rax",
        "popfq",
        
        // Jump to new RIP
        "jmp [rsi + 48]",          // Jump to saved RIP
    )
}
