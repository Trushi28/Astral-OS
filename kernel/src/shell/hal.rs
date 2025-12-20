//! Shell Hardware Abstraction Layer (HAL)
//! 
//! This module provides an environment-agnostic interface for shell I/O.
//! The shell code uses these functions instead of direct kernel calls,
//! allowing the same shell to run in Ring 0 (kernel) or Ring 3 (usermode).

use alloc::string::String;
use alloc::vec::Vec;

/// Syscall numbers for shell operations
pub mod syscall_numbers {
    pub const SYS_WRITE: u64 = 1;
    pub const SYS_READ: u64 = 0;
    pub const SYS_EXIT: u64 = 60;
    pub const SYS_GETPID: u64 = 39;
    pub const SYS_YIELD: u64 = 24;
    pub const SYS_FS_LIST: u64 = 200;
    pub const SYS_GET_PROCESS_INFO: u64 = 201;
    pub const SYS_GET_MEM_INFO: u64 = 202;
    pub const SYS_GET_CPU_INFO: u64 = 203;
    pub const SYS_CLEAR_SCREEN: u64 = 204;
    pub const SYS_GET_TIME: u64 = 205;
    pub const SYS_GET_UPTIME: u64 = 206;
    // New HAL syscalls
    pub const SYS_PRINT: u64 = 210;
    pub const SYS_PRINT_COLORED: u64 = 211;
    pub const SYS_READ_CHAR: u64 = 212;
    pub const SYS_GET_CURSOR: u64 = 213;
    pub const SYS_SET_CURSOR: u64 = 214;
    pub const SYS_CLEAR_LINE: u64 = 215;
}

/// Environment mode - determines how HAL calls are executed
#[derive(Clone, Copy, PartialEq)]
pub enum HalMode {
    /// Direct kernel calls (Ring 0)
    Kernel,
    /// Syscall-based (Ring 3)
    User,
}

static mut HAL_MODE: HalMode = HalMode::Kernel;

/// Set the HAL mode
pub fn set_mode(mode: HalMode) {
    unsafe { HAL_MODE = mode; }
}

/// Get current HAL mode
pub fn get_mode() -> HalMode {
    unsafe { HAL_MODE }
}

/// Print a string to the console
pub fn print(s: &str) {
    match get_mode() {
        HalMode::Kernel => {
            crate::drivers::framebuffer::print(s);
        }
        HalMode::User => {
            // Use syscall
            unsafe {
                do_syscall3(syscall_numbers::SYS_WRITE, 1, s.as_ptr() as u64, s.len() as u64);
            }
        }
    }
}

/// Print a string with a specific color
pub fn print_colored(s: &str, color: u32) {
    match get_mode() {
        HalMode::Kernel => {
            crate::drivers::framebuffer::print_colored(s, color);
        }
        HalMode::User => {
            unsafe {
                do_syscall3(syscall_numbers::SYS_PRINT_COLORED, s.as_ptr() as u64, s.len() as u64, color as u64);
            }
        }
    }
}

/// Print a line (with newline)
pub fn println(s: &str) {
    print(s);
    print("\n");
}

/// Clear the screen
pub fn clear_screen() {
    match get_mode() {
        HalMode::Kernel => {
            crate::drivers::framebuffer::clear();
        }
        HalMode::User => {
            unsafe {
                do_syscall0(syscall_numbers::SYS_CLEAR_SCREEN);
            }
        }
    }
}

/// Read a character from keyboard (non-blocking)
pub fn read_char() -> Option<u8> {
    match get_mode() {
        HalMode::Kernel => {
            crate::interrupts::getchar()
        }
        HalMode::User => {
            unsafe {
                let result = do_syscall1(syscall_numbers::SYS_READ_CHAR, 0);
                if result == 0 || result == u64::MAX {
                    None
                } else {
                    Some(result as u8)
                }
            }
        }
    }
}

/// Get cursor position
pub fn get_cursor_pos() -> (usize, usize) {
    match get_mode() {
        HalMode::Kernel => {
            crate::drivers::framebuffer::get_cursor_pos()
        }
        HalMode::User => {
            unsafe {
                let result = do_syscall0(syscall_numbers::SYS_GET_CURSOR);
                let x = (result >> 32) as usize;
                let y = (result & 0xFFFFFFFF) as usize;
                (x, y)
            }
        }
    }
}

/// Set cursor position
pub fn set_cursor_pos(x: usize, y: usize) {
    match get_mode() {
        HalMode::Kernel => {
            crate::drivers::framebuffer::set_cursor_pos(x, y);
        }
        HalMode::User => {
            unsafe {
                do_syscall2(syscall_numbers::SYS_SET_CURSOR, x as u64, y as u64);
            }
        }
    }
}

/// Clear current line
pub fn clear_line() {
    match get_mode() {
        HalMode::Kernel => {
            crate::drivers::framebuffer::clear_line();
        }
        HalMode::User => {
            unsafe {
                do_syscall0(syscall_numbers::SYS_CLEAR_LINE);
            }
        }
    }
}

/// Get memory info (total_kb, free_kb)
pub fn get_mem_info() -> (u64, u64) {
    match get_mode() {
        HalMode::Kernel => {
            let (total, _used, free) = crate::memory::frame::get_stats();
            ((total * 4) as u64, (free * 4) as u64)  // Pages to KB
        }
        HalMode::User => {
            unsafe {
                let result = do_syscall0(syscall_numbers::SYS_GET_MEM_INFO);
                let total = (result >> 32) as u64 * 4;
                let free = (result & 0xFFFFFFFF) as u64 * 4;
                (total, free)
            }
        }
    }
}

/// Get CPU info (returns core count)
pub fn get_cpu_info() -> u64 {
    match get_mode() {
        HalMode::Kernel => {
            crate::arch::x86_64::cpu::get_cpu_count() as u64
        }
        HalMode::User => {
            unsafe {
                do_syscall0(syscall_numbers::SYS_GET_CPU_INFO)
            }
        }
    }
}

/// List files in filesystem
pub fn list_files() -> Vec<String> {
    match get_mode() {
        HalMode::Kernel => {
            crate::fs::psychicfs::fs_list()
        }
        HalMode::User => {
            // For now, return empty - would need proper syscall for file list
            Vec::new()
        }
    }
}

/// Yield to scheduler
pub fn yield_cpu() {
    match get_mode() {
        HalMode::Kernel => {
            // In kernel mode, just hint
            unsafe { core::arch::asm!("hlt"); }
        }
        HalMode::User => {
            unsafe {
                do_syscall0(syscall_numbers::SYS_YIELD);
            }
        }
    }
}

/// Exit the shell
pub fn exit() -> ! {
    match get_mode() {
        HalMode::Kernel => {
            // Return to kernel main loop
            crate::drivers::framebuffer::clear();
            crate::print!("Shell exited. Returning to boot menu...\n");
            loop { unsafe { core::arch::asm!("hlt"); } }
        }
        HalMode::User => {
            unsafe {
                do_syscall1(syscall_numbers::SYS_EXIT, 0);
            }
            loop { unsafe { core::arch::asm!("hlt"); } }
        }
    }
}

// Low-level syscall wrappers
#[inline(always)]
unsafe fn do_syscall0(num: u64) -> u64 {
    let result: u64;
    core::arch::asm!(
        "syscall",
        in("rax") num,
        lateout("rax") result,
        out("rcx") _,
        out("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
unsafe fn do_syscall1(num: u64, arg1: u64) -> u64 {
    let result: u64;
    core::arch::asm!(
        "syscall",
        in("rax") num,
        in("rdi") arg1,
        lateout("rax") result,
        out("rcx") _,
        out("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
unsafe fn do_syscall2(num: u64, arg1: u64, arg2: u64) -> u64 {
    let result: u64;
    core::arch::asm!(
        "syscall",
        in("rax") num,
        in("rdi") arg1,
        in("rsi") arg2,
        lateout("rax") result,
        out("rcx") _,
        out("r11") _,
        options(nostack)
    );
    result
}

#[inline(always)]
unsafe fn do_syscall3(num: u64, arg1: u64, arg2: u64, arg3: u64) -> u64 {
    let result: u64;
    core::arch::asm!(
        "syscall",
        in("rax") num,
        in("rdi") arg1,
        in("rsi") arg2,
        in("rdx") arg3,
        lateout("rax") result,
        out("rcx") _,
        out("r11") _,
        options(nostack)
    );
    result
}
