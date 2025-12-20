//! Minimal Ring 3 shell to prove loader works
//! This will be converted to flat binary and embedded in kernel

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::arch::asm;

/// Syscall numbers (must match kernel)
const SYS_WRITE: u64 = 1;
const SYS_EXIT: u64 = 60;
const SYS_PRINT_COLORED: u64 = 211;

/// Entry point - must be at start of binary
#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start() -> ! {
    // Print hello message via syscall
    let msg = "Hello from TRUE Ring 3!\n";
    syscall_print_colored(msg, 0x00FF00); // Green
    
    let msg2 = "Userland shell working!\n";
    syscall_print_colored(msg2, 0x00AAFF); // Cyan
    
    // Exit cleanly
    syscall_exit(0);
}

/// Print colored text via syscall 211
fn syscall_print_colored(s: &str, color: u32) {
    unsafe {
        asm!(
            "syscall",
            in("rax") SYS_PRINT_COLORED,
            in("rdi") s.as_ptr() as u64,
            in("rsi") s.len() as u64,
            in("rdx") color as u64,
            out("rcx") _,
            out("r11") _,
            options(nostack)
        );
    }
}

/// Exit via syscall 60
fn syscall_exit(code: u64) -> ! {
    unsafe {
        asm!(
            "syscall",
            in("rax") SYS_EXIT,
            in("rdi") code,
            options(nostack, noreturn)
        );
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    // On panic, just exit with error code
    syscall_exit(1);
}
