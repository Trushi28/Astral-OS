//! Syscall Wrappers
//!
//! Low-level syscall interface for userspace programs.

/// Exit the current process
pub fn exit(code: i32) -> ! {
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
pub fn write(fd: i32, buf: &[u8]) -> isize {
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
pub fn print(s: &str) {
    write(1, s.as_bytes());
}

/// Print string to stdout with newline
pub fn println(s: &str) {
    print(s);
    print("\n");
}

/// Get current process ID
pub fn getpid() -> i64 {
    let ret: i64;
    unsafe {
        core::arch::asm!(
            "mov rax, 39",      // sys_getpid
            "syscall",
            lateout("rax") ret,
            out("rcx") _,
            out("r11") _,
        );
    }
    ret
}

/// Yield CPU to scheduler
pub fn yield_now() {
    unsafe {
        core::arch::asm!(
            "mov rax, 24",      // sys_yield
            "syscall",
            out("rax") _,
            out("rcx") _,
            out("r11") _,
        );
    }
}
