//! Hello World - First Userspace Program
//!
//! A minimal program that prints and exits.

#![no_std]
#![no_main]

use core::panic::PanicInfo;

/// Panic handler - just exit with error code
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    exit(1);
}

/// Exit the current process
fn exit(code: i32) -> ! {
    unsafe {
        core::arch::asm!(
            "mov rax, 60",      // sys_exit
            "syscall",
            in("rdi") code,
            options(noreturn)
        );
    }
}

/// Write to a file descriptor
fn write(fd: i32, buf: &[u8]) -> isize {
    let ret: isize;
    unsafe {
        core::arch::asm!(
            "mov rax, 1",       // sys_write
            "syscall",
            in("rdi") fd,
            in("rsi") buf.as_ptr(),
            in("rdx") buf.len(),
            lateout("rax") ret,
            out("rcx") _,
            out("r11") _,
        );
    }
    ret
}

/// Print string to stdout
fn print(s: &str) {
    write(1, s.as_bytes());
}

/// Print string to stdout with newline
fn println(s: &str) {
    print(s);
    print("\n");
}

/// Entry point for userspace program
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Print hello message
    println("Hello from Ring 3 userspace!");
    println("Astral OS user program running!");
    
    // Exit cleanly
    exit(0);
}
