use crate::process::{ProcessId, Priority};
use crate::serial_println;
use crate::arch::x86_64::msr;

/// Syscall numbers (Linux-compatible where possible)
#[repr(u64)]
#[derive(Debug, Clone, Copy)]
pub enum Syscall {
    Exit = 60,          // exit(code)
    Write = 1,          // write(fd, buf, len)
    Read = 2,           // read(fd, buf, len)
    Getpid = 39,        // getpid()
    Clone = 56,         // clone(fn_ptr, stack, flags) - thread creation!
    Yield = 24,         // sched_yield()
}

/// Syscall error codes (negative errno)
#[repr(i64)]
pub enum SyscallError {
    InvalidSyscall = -38,   // ENOSYS
    InvalidArgument = -22,  // EINVAL
    PermissionDenied = -13, // EACCES
}

/// Syscall entry point (called from assembly)
/// 
/// ABI: RAX=syscall#, RDI=arg1, RSI=arg2, RDX=arg3, R10=arg4, R8=arg5, R9=arg6
#[no_mangle]
pub extern "C" fn syscall_entry(
    syscall_num: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
) -> i64 {
    handle_syscall(syscall_num, arg1, arg2, arg3, arg4, arg5, 0)
}

/// Initialize syscall interface
/// Sets up MSRs for syscall/sysret
pub unsafe fn init() {
    // Kernel CS = 0x08 (index 1)
    // Kernel SS = 0x10 (index 2)  
    // User CS = 0x23 (index 4, RPL=3)
    // User SS = 0x1b (index 3, RPL=3)
    //
    // STAR MSR layout:
    // bits 63:48 = SYSRET CS and SS (add 16 to get SS, add 8 to get 64-bit CS)
    //              For user: base selector should be 0x1B (user data), then:
    //              64-bit mode: CS = base + 16 = 0x2B... wait, that's wrong
    //              Actually SYSRET: CS = STAR[63:48] + 16, SS = STAR[63:48] + 8
    // bits 47:32 = SYSCALL CS and SS (SS = CS + 8)
    //
    // For SYSRET to work correctly:
    // STAR[63:48] should be (USER_DATA_SELECTOR - 8) for 64-bit
    // But x86-64 SYSRET: CS = STAR[63:48] + 16, SS = STAR[63:48] + 8
    // So: STAR[63:48] = 0x1B - 8 = 0x13... but actually need:
    //   User CS = 0x23, so STAR[63:48] + 16 = 0x23, means STAR[63:48] = 0x13
    //   User SS = STAR[63:48] + 8 = 0x1B ✓
    //
    // For SYSCALL:
    // STAR[47:32] = Kernel CS = 0x08
    // Kernel SS = STAR[47:32] + 8 = 0x10 ✓
    
    let star_value: u64 = 
        (0x13u64 << 48) |  // SYSRET base (user): CS=0x23=base+16, SS=0x1B=base+8
        (0x08u64 << 32);   // SYSCALL target (kernel): CS=0x08, SS=0x10
    
    msr::wrmsr(msr::IA32_STAR, star_value);
    
    // LSTAR = 64-bit SYSCALL entry point
    msr::wrmsr(msr::IA32_LSTAR, syscall_handler as u64);
    
    // FMASK = RFLAGS mask (clear TF, IF, DF on syscall)
    // Clear: TF(8), IF(9), DF(10) = 0x700
    msr::wrmsr(msr::IA32_FMASK, 0x700);
    
    // Enable SYSCALL instruction (already enabled on most CPUs)
    // IA32_EFER.SCE (bit 0) should be set
    const IA32_EFER: u32 = 0xC0000080;
    let efer = msr::rdmsr(IA32_EFER);
    msr::wrmsr(IA32_EFER, efer | 1); // Set SCE bit
    
    serial_println!("[SYSCALL] STAR={:#x}, LSTAR={:#x}", star_value, syscall_handler as u64);
    serial_println!("[SYSCALL] Interface initialized (SYSCALL instruction method)");
}

/// Syscall handler (naked function for assembly entry)
#[unsafe(naked)]
unsafe extern "C" fn syscall_handler() -> ! {
    core::arch::naked_asm!(
        // On entry from SYSCALL:
        // - RCX = user RIP (return address)
        // - R11 = user RFLAGS
        // - RSP = still user stack (we must switch!)
        // - Interrupts are disabled (IF cleared by FMASK)
        
        // Save user stack pointer
        "mov r15, rsp",
        
        // Switch to kernel stack
        // Compute stack top = SYSCALL_STACK base + 16384 (16KB)
        "lea rsp, [rip + SYSCALL_STACK + 16384]",
        
        // Save user state
        "push r15",         // User RSP
        "push r11",         // User RFLAGS  
        "push rcx",         // User RIP
        
        // Save callee-saved registers used by syscall
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        
        // Arguments already in correct registers for C ABI:
        // RDI = arg1 ✓
        // RSI = arg2 ✓
        // RDX = arg3 ✓
        // R10 = arg4 (need to move to RCX for C ABI)
        // R8 = arg5 ✓
        // R9 = arg6 ✓
        // RAX = syscall number (need to move to first arg)
        
        // Rearrange for C call: fn(syscall_num, arg1, arg2, arg3, arg4, arg5)
        "mov r9, r8",       // arg5 -> r9
        "mov r8, r10",      // arg4 -> r8
        "mov rcx, rdx",     // arg3 -> rcx
        "mov rdx, rsi",     // arg2 -> rdx
        "mov rsi, rdi",     // arg1 -> rsi
        "mov rdi, rax",     // syscall_num -> rdi
        
        // Call Rust handler
        "call syscall_entry",
        
        // Return value in RAX
        
        // Restore registers
        "pop r14",
        "pop r13", 
        "pop r12",
        "pop rbx",
        "pop rbp",
        
        // Restore user state
        "pop rcx",          // User RIP -> RCX for SYSRET
        "pop r11",          // User RFLAGS -> R11 for SYSRET
        "pop rsp",          // User RSP
        
        // Return to userspace
        "sysretq",
    )
}

// Kernel stack for syscall handler (16KB, 16-byte aligned)
#[repr(C, align(16))]
pub struct SyscallStack {
    data: [u8; 4096 * 4],
}

#[no_mangle]
pub static mut SYSCALL_STACK: SyscallStack = SyscallStack { data: [0; 4096 * 4] };

/// Handle syscall from userspace
pub fn handle_syscall(
    syscall_num: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    _arg4: u64,
    _arg5: u64,
    _arg6: u64,
) -> i64 {
    match syscall_num {
        // sys_exit(code)
        60 => sys_exit(arg1 as i32),
        
        // sys_write(fd, buf, len)
        1 => sys_write(arg1 as i32, arg2 as usize, arg3 as usize),
        
        // sys_getpid()
        39 => sys_getpid(),
        
        // sys_clone(fn_ptr, stack_ptr, flags)
        56 => sys_clone(arg1, arg2, arg3),
        
        // sys_yield()
        24 => sys_yield(),
        
        _ => {
            serial_println!("[SYSCALL] Unknown syscall: {}", syscall_num);
            SyscallError::InvalidSyscall as i64
        }
    }
}

/// sys_exit - Terminate current process
fn sys_exit(code: i32) -> i64 {
    serial_println!("[SYSCALL] exit({}) - Process terminating", code);
    serial_println!("[EXIT] User process exited cleanly! Halting...");
    serial_println!("[SYSTEM] User execution complete. Safe to shutdown.");
    
    // NOTE: We're still in user page table context, so we can't access
    // kernel heap structures like SCHEDULER or STACK_POOL safely.
    // For now, just halt. Proper implementation would switch CR3 first
    // or have cleanup code in a context that has access to kernel memory.
    
    loop {
        x86_64::instructions::hlt();
    }
}

/// sys_write - Write to file descriptor
fn sys_write(fd: i32, buf_ptr: usize, len: usize) -> i64 {
    if fd != 1 && fd != 2 {
        return SyscallError::InvalidArgument as i64;
    }
    
    // TODO: Validate user pointer
    // For now, just print to serial
    let buf = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, len) };
    if let Ok(s) = core::str::from_utf8(buf) {
        crate::serial_print!("{}", s);
    }
    
    len as i64
}

/// sys_getpid - Get current process ID
fn sys_getpid() -> i64 {
    serial_println!("[SYSCALL] getpid()");
    1
}

/// sys_clone - Create a new thread/process
fn sys_clone(fn_ptr: u64, stack_ptr: u64, flags: u64) -> i64 {
    serial_println!("[SYSCALL] clone(fn={:#x}, stack={:#x}, flags={:#x})", fn_ptr, stack_ptr, flags);
    
    fn thread_wrapper() -> ! {
        loop {
            x86_64::instructions::hlt();
        }
    }
    
    let pid = crate::process::spawn::spawn_kernel_thread(thread_wrapper, Priority::Normal);
    
    serial_println!("[SYSCALL] Created thread with PID {}", pid.as_u32());
    
    pid.as_u32() as i64
}

/// sys_yield - Voluntarily yield CPU
fn sys_yield() -> i64 {
    serial_println!("[SYSCALL] yield()");
    0
}
