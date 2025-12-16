//! User-mode Syscall Wrappers
//! 
//! These functions provide the syscall interface for user-mode programs.
//! They use inline assembly to invoke the syscall instruction.
//! 
//! IMPORTANT: These functions are designed to be called from Ring 3 code.
//! They use the syscall instruction for all I/O operations.

use core::arch::asm;

/// System call numbers (must match kernel's syscall.rs)
pub const SYS_READ: u64 = 0;
pub const SYS_WRITE: u64 = 1;
pub const SYS_EXIT: u64 = 60;
pub const SYS_YIELD: u64 = 158;
pub const SYS_GETPID: u64 = 39;
pub const SYS_GETUID: u64 = 102;

// Shell-specific syscalls
pub const SYS_FS_LIST: u64 = 200;
pub const SYS_GET_PROCESS_INFO: u64 = 201;
pub const SYS_GET_MEM_INFO: u64 = 202;
pub const SYS_GET_CPU_INFO: u64 = 203;
pub const SYS_CLEAR_SCREEN: u64 = 204;

/// Raw syscall with up to 3 arguments
#[inline(always)]
pub fn syscall3(num: u64, arg1: u64, arg2: u64, arg3: u64) -> u64 {
    let ret: u64;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") num => ret,
            in("rdi") arg1,
            in("rsi") arg2,
            in("rdx") arg3,
            out("rcx") _,
            out("r11") _,
            options(nostack)
        );
    }
    ret
}

/// Raw syscall with 1 argument
#[inline(always)]
pub fn syscall1(num: u64, arg1: u64) -> u64 {
    let ret: u64;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") num => ret,
            in("rdi") arg1,
            out("rcx") _,
            out("r11") _,
            options(nostack)
        );
    }
    ret
}

/// Raw syscall with no arguments
#[inline(always)]
pub fn syscall0(num: u64) -> u64 {
    let ret: u64;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") num => ret,
            out("rcx") _,
            out("r11") _,
            options(nostack)
        );
    }
    ret
}

// ============ High-Level Syscall Wrappers ============
// These all use the syscall instruction for Ring 3 compatibility

/// Write bytes to stdout via syscall
pub fn sys_write(buf: &[u8]) -> usize {
    // syscall: write(fd=1, buf, len)
    syscall3(SYS_WRITE, 1, buf.as_ptr() as u64, buf.len() as u64) as usize
}

/// Write a string to stdout
pub fn sys_print(s: &str) {
    sys_write(s.as_bytes());
}

/// Read a character from stdin via syscall
/// Returns the character, or 0 if no input available
pub fn sys_read_char() -> u8 {
    // syscall: read(fd=0, buf=ptr, len=1)
    let mut buf = [0u8; 1];
    let res = syscall3(SYS_READ, 0, buf.as_mut_ptr() as u64, 1);
    
    // Check if we read 1 byte successfully
    if res == 1 {
        return buf[0];
    }
    
    // Error or no input
    0
}

/// Yield the CPU to other processes via syscall
pub fn sys_yield() {
    syscall0(SYS_YIELD);
}

/// Exit the current process via syscall
pub fn sys_exit(code: i32) -> ! {
    syscall1(SYS_EXIT, code as u64);
    // Never returns - syscall halts the CPU
    loop {
        unsafe { asm!("hlt"); }
    }
}

/// Get current process ID via syscall
pub fn sys_getpid() -> u64 {
    syscall0(SYS_GETPID)
}

/// Get current user ID via syscall
pub fn sys_getuid() -> u64 {
    syscall0(SYS_GETUID)
}

// ============ Shell-specific syscalls ============

/// File system list syscall - returns pointer to file list or 0
pub fn sys_fs_list() -> u64 {
    syscall0(SYS_FS_LIST)
}

/// Get process info syscall
/// Returns number of processes, writes info to buffer
pub fn sys_get_process_info(buf: &mut [u8]) -> u64 {
    syscall3(SYS_GET_PROCESS_INFO, buf.as_mut_ptr() as u64, buf.len() as u64, 0)
}

/// Get memory info syscall
/// Returns: (total << 32) | free (in KB)
pub fn sys_get_mem_info() -> u64 {
    syscall0(SYS_GET_MEM_INFO)
}

/// Get CPU info syscall
/// Returns: cpu_count
pub fn sys_get_cpu_info() -> u64 {
    syscall0(SYS_GET_CPU_INFO)
}

/// Clear screen syscall
pub fn sys_clear_screen() {
    syscall0(SYS_CLEAR_SCREEN);
}
