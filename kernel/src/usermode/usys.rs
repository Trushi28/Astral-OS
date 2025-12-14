//! User-mode Syscall Wrappers
//! 
//! These functions provide the syscall interface for user-mode programs.
//! They use inline assembly to invoke the syscall instruction.
//! 
//! IMPORTANT: These functions are designed to be called from Ring 3 code.

use core::arch::asm;

/// System call numbers
pub const SYS_READ: u64 = 0;
pub const SYS_WRITE: u64 = 1;
pub const SYS_EXIT: u64 = 60;
pub const SYS_YIELD: u64 = 24;

/// Raw syscall with up to 3 arguments
#[inline(always)]
pub unsafe fn syscall3(num: u64, arg1: u64, arg2: u64, arg3: u64) -> u64 {
    let ret: u64;
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
    ret
}

/// Raw syscall with 1 argument
#[inline(always)]
pub unsafe fn syscall1(num: u64, arg1: u64) -> u64 {
    let ret: u64;
    asm!(
        "syscall",
        inlateout("rax") num => ret,
        in("rdi") arg1,
        out("rcx") _,
        out("r11") _,
        options(nostack)
    );
    ret
}

/// Raw syscall with no arguments
#[inline(always)]
pub unsafe fn syscall0(num: u64) -> u64 {
    let ret: u64;
    asm!(
        "syscall",
        inlateout("rax") num => ret,
        out("rcx") _,
        out("r11") _,
        options(nostack)
    );
    ret
}

// ============ High-Level Syscall Wrappers ============
// These call kernel functions directly when in Ring 0 mode.
// When running in true Ring 3, they use the syscall instruction.

/// Write bytes to stdout
pub fn sys_write(buf: &[u8]) -> usize {
    // Call kernel directly (Ring 0 mode)
    for &b in buf {
        crate::drivers::framebuffer::print_char(b as char, 0xFFFFFF);
    }
    buf.len()
}

/// Write a string to stdout
pub fn sys_print(s: &str) {
    sys_write(s.as_bytes());
}

/// Read a character from stdin
/// Returns the character, or 0 if no input available
pub fn sys_read_char() -> u8 {
    // Call kernel directly (Ring 0 mode)
    if let Some(ch) = crate::drivers::keyboard::try_read_char() {
        ch as u8
    } else {
        0
    }
}

/// Yield the CPU to other processes
pub fn sys_yield() {
    // Just pause in Ring 0 mode
    unsafe { core::arch::asm!("pause"); }
}

/// Exit the current process
pub fn sys_exit(_code: i32) -> ! {
    crate::serial_println!("[SHELL] Exit called");
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}
