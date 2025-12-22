//! Astral OS Ring 3 Shell
//! 
//! A full-featured usermode shell that uses syscalls for all I/O.
//! This binary runs in Ring 3 and communicates with the kernel via syscalls.

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::arch::asm;

// ============================================================================
// SYSCALL NUMBERS (must match kernel)
// ============================================================================

const SYS_READ: u64 = 0;
const SYS_WRITE: u64 = 1;
const SYS_EXIT: u64 = 60;
const SYS_YIELD: u64 = 158;
const SYS_GETPID: u64 = 39;
const SYS_GETUID: u64 = 102;

// Shell-specific syscalls
const SYS_FS_LIST: u64 = 200;
const SYS_GET_PROCESS_INFO: u64 = 201;
const SYS_GET_MEM_INFO: u64 = 202;
const SYS_GET_CPU_INFO: u64 = 203;
const SYS_CLEAR_SCREEN: u64 = 204;
const SYS_PRINT_COLORED: u64 = 211;

// ============================================================================
// SYSCALL WRAPPERS
// ============================================================================

#[inline(always)]
fn syscall0(num: u64) -> u64 {
    let ret: u64;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") num => ret,
            out("rcx") _,
            out("r11") _,
            // Kernel handler may clobber these caller-saved regs
            lateout("rdi") _,
            lateout("rsi") _,
            lateout("rdx") _,
            lateout("r8") _,
            lateout("r9") _,
            lateout("r10") _,
            options(nostack)
        );
    }
    ret
}

#[inline(always)]
fn syscall1(num: u64, arg1: u64) -> u64 {
    let ret: u64;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") num => ret,
            in("rdi") arg1,
            out("rcx") _,
            out("r11") _,
            // Kernel handler may clobber these caller-saved regs
            lateout("rsi") _,
            lateout("rdx") _,
            lateout("r8") _,
            lateout("r9") _,
            lateout("r10") _,
            options(nostack)
        );
    }
    ret
}

#[inline(always)]
fn syscall3(num: u64, arg1: u64, arg2: u64, arg3: u64) -> u64 {
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

// ============================================================================
// I/O FUNCTIONS
// ============================================================================

fn print(s: &str) {
    syscall3(SYS_WRITE, 1, s.as_ptr() as u64, s.len() as u64);
}

fn print_colored(s: &str, color: u32) {
    syscall3(SYS_PRINT_COLORED, s.as_ptr() as u64, s.len() as u64, color as u64);
}

fn read_char() -> u8 {
    let mut buf = [0u8; 1];
    let res = syscall3(SYS_READ, 0, buf.as_mut_ptr() as u64, 1);
    if res == 1 {
        buf[0]
    } else {
        0
    }
}

fn yield_cpu() {
    syscall0(SYS_YIELD);
}

fn exit(code: i32) -> ! {
    syscall1(SYS_EXIT, code as u64);
    loop { unsafe { asm!("hlt"); } }
}

fn clear_screen() {
    syscall0(SYS_CLEAR_SCREEN);
}

fn getuid() -> u64 {
    syscall0(SYS_GETUID)
}

fn getpid() -> u64 {
    syscall0(SYS_GETPID)
}

fn fs_list() {
    syscall0(SYS_FS_LIST);
}

fn get_process_info() {
    syscall0(SYS_GET_PROCESS_INFO);
}

fn get_mem_info() -> u64 {
    syscall0(SYS_GET_MEM_INFO)
}

fn get_cpu_info() -> u64 {
    syscall0(SYS_GET_CPU_INFO)
}

// ============================================================================
// NUMBER PRINTING
// ============================================================================

fn print_num(mut n: u64) {
    if n == 0 {
        print("0");
        return;
    }
    
    let mut buf = [0u8; 20];
    let mut i = 0;
    
    while n > 0 {
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    
    // Print in reverse
    while i > 0 {
        i -= 1;
        let c = [buf[i]];
        syscall3(SYS_WRITE, 1, c.as_ptr() as u64, 1);
    }
}

fn print_hex(mut n: u64) {
    print("0x");
    if n == 0 {
        print("0");
        return;
    }
    
    let hex = b"0123456789abcdef";
    let mut buf = [0u8; 16];
    let mut i = 0;
    
    while n > 0 {
        buf[i] = hex[(n & 0xF) as usize];
        n >>= 4;
        i += 1;
    }
    
    while i > 0 {
        i -= 1;
        let c = [buf[i]];
        syscall3(SYS_WRITE, 1, c.as_ptr() as u64, 1);
    }
}

// ============================================================================
// SHELL CONSTANTS
// ============================================================================

const CMD_BUF_SIZE: usize = 256;

// Colors
const COLOR_CYAN: u32 = 0x00AAFF;
const COLOR_GREEN: u32 = 0x00FF00;
const COLOR_WHITE: u32 = 0xFFFFFF;
const COLOR_YELLOW: u32 = 0xFFFF00;
const COLOR_RED: u32 = 0xFF6666;
const COLOR_GRAY: u32 = 0x888888;

// ============================================================================
// ENTRY POINT
// Entry point - must align stack to 16 bytes before calling Rust code
#[unsafe(naked)]
#[no_mangle]
#[link_section = ".text._start"]
pub unsafe extern "C" fn _start() -> ! {
    core::arch::naked_asm!(
        // Align the stack to 16 bytes (required for SSE instructions like movaps)
        "and rsp, ~0xF",
        // Call our main function
        "call {shell_main}",
        // If shell_main returns (shouldn't), exit
        "mov rdi, 0",
        "call {exit}",
        // Should never reach here
        "ud2",
        shell_main = sym shell_main,
        exit = sym exit,
    );
}

/// Main shell function, called with aligned stack
extern "C" fn shell_main() {
    // Print banner
    print_colored("╔═══════════════════════════════════════════╗\n", COLOR_CYAN);
    print_colored("║      ", COLOR_CYAN);
    print_colored("ASTRAL OS", COLOR_WHITE);
    print_colored(" Ring 3 Shell            ║\n", COLOR_CYAN);
    print_colored("╚═══════════════════════════════════════════╝\n", COLOR_CYAN);
    print("\n");
    print_colored("Type 'help' for available commands\n\n", COLOR_GRAY);
    
    // Command buffer
    let mut cmd_buf: [u8; CMD_BUF_SIZE] = [0; CMD_BUF_SIZE];
    let mut cmd_len: usize;
    
    loop {
        // Print prompt
        let uid = getuid();
        if uid == 0 {
            print_colored("root", COLOR_RED);
        } else {
            print_colored("user", COLOR_GREEN);
        }
        print_colored("@", COLOR_GRAY);
        print_colored("astral", COLOR_CYAN);
        print_colored("> ", COLOR_GREEN);
        
        // Read command
        cmd_len = 0;
        loop {
            let c = read_char();
            
            if c == 0 {
                yield_cpu();
                continue;
            }
            
            match c {
                b'\n' | b'\r' => {
                    print("\n");
                    break;
                }
                8 | 127 => {
                    // Backspace
                    if cmd_len > 0 {
                        cmd_len -= 1;
                        print("\x08 \x08");
                    }
                }
                32..=126 => {
                    if cmd_len < CMD_BUF_SIZE - 1 {
                        cmd_buf[cmd_len] = c;
                        cmd_len += 1;
                        let echo = [c];
                        syscall3(SYS_WRITE, 1, echo.as_ptr() as u64, 1);
                    }
                }
                _ => {}
            }
        }
        
        // Process command
        if cmd_len > 0 {
            // DEBUG: Show command length to trace panic
            print("[CMD:");
            let digit = b'0' + (cmd_len as u8 % 10);
            let digit_str = [digit];
            syscall3(SYS_WRITE, 1, digit_str.as_ptr() as u64, 1);
            print("]");
            process_command(&cmd_buf[..cmd_len]);
        }
    }
}

// ============================================================================
// COMMAND PROCESSING
// ============================================================================

fn process_command(cmd: &[u8]) {
    // Simple command matching
    if starts_with(cmd, b"help") {
        cmd_help();
    } else if starts_with(cmd, b"info") || starts_with(cmd, b"uname") {
        cmd_info();
    } else if starts_with(cmd, b"clear") || starts_with(cmd, b"cls") {
        clear_screen();
    } else if starts_with(cmd, b"ls") {
        cmd_ls();
    } else if starts_with(cmd, b"ps") {
        cmd_ps();
    } else if starts_with(cmd, b"mem") || starts_with(cmd, b"free") {
        cmd_mem();
    } else if starts_with(cmd, b"cpu") {
        cmd_cpu();
    } else if starts_with(cmd, b"echo ") {
        cmd_echo(cmd);
    } else if starts_with(cmd, b"whoami") {
        cmd_whoami();
    } else if starts_with(cmd, b"pwd") {
        cmd_pwd();
    } else if starts_with(cmd, b"pid") {
        cmd_pid();
    } else if starts_with(cmd, b"date") || starts_with(cmd, b"time") {
        print("System clock not yet implemented\n");
    } else if starts_with(cmd, b"uptime") {
        print("System running since boot\n");
    } else if starts_with(cmd, b"version") {
        cmd_version();
    } else if starts_with(cmd, b"exit") || starts_with(cmd, b"logout") || starts_with(cmd, b"quit") {
        cmd_exit();
    } else {
        print_colored("Unknown command: ", COLOR_RED);
        // Print the command
        syscall3(SYS_WRITE, 1, cmd.as_ptr() as u64, cmd.len() as u64);
        print("\nType 'help' for available commands.\n");
    }
}

fn starts_with(haystack: &[u8], needle: &[u8]) -> bool {
    if haystack.len() < needle.len() {
        return false;
    }
    &haystack[..needle.len()] == needle
}

// ============================================================================
// COMMANDS
// ============================================================================

fn cmd_help() {
    print_colored("=== Astral Ring 3 Shell Commands ===\n\n", COLOR_YELLOW);
    
    print_colored("System:\n", COLOR_CYAN);
    print("  help      - Show this help\n");
    print("  info      - System information\n");
    print("  version   - OS version\n");
    print("  uptime    - System uptime\n");
    print("  exit      - Exit shell\n");
    print("\n");
    
    print_colored("Files:\n", COLOR_CYAN);
    print("  ls        - List files\n");
    print("  pwd       - Show current directory\n");
    print("\n");
    
    print_colored("Monitor:\n", COLOR_CYAN);
    print("  ps        - List processes\n");
    print("  mem       - Memory usage\n");
    print("  cpu       - CPU info\n");
    print("  pid       - Show process ID\n");
    print("\n");
    
    print_colored("Utility:\n", COLOR_CYAN);
    print("  echo      - Print text\n");
    print("  clear     - Clear screen\n");
    print("  whoami    - Current user\n");
    print("  date      - Current time\n");
}

fn cmd_info() {
    print_colored("Astral OS v0.3.0\n", COLOR_CYAN);
    print("Architecture: x86_64\n");
    print("Kernel: Astral Microkernel\n");
    print_colored("Ring Level: 3 (User Mode)\n", COLOR_GREEN);
}

fn cmd_version() {
    print_colored("Astral OS version 0.3.0\n", COLOR_CYAN);
    print("Shell: Ring 3 Userspace Shell\n");
    print("API: Syscall-based\n");
}

fn cmd_ls() {
    fs_list();
}

fn cmd_ps() {
    get_process_info();
}

fn cmd_mem() {
    let info = get_mem_info();
    let total_pages = (info >> 32) as u64;
    let free_pages = (info & 0xFFFFFFFF) as u64;
    
    print_colored("Physical Memory:\n", COLOR_YELLOW);
    print("  Total: ");
    print_num(total_pages * 4 / 1024);
    print(" MB (");
    print_num(total_pages);
    print(" pages)\n");
    print("  Free:  ");
    print_num(free_pages * 4 / 1024);
    print(" MB (");
    print_num(free_pages);
    print(" pages)\n");
}

fn cmd_cpu() {
    let cpu_count = get_cpu_info();
    print_colored("CPU Information:\n", COLOR_YELLOW);
    print("  Architecture: x86_64\n");
    print("  CPU Cores: ");
    print_num(cpu_count);
    print("\n");
}

fn cmd_echo(cmd: &[u8]) {
    // Skip "echo "
    if cmd.len() > 5 {
        syscall3(SYS_WRITE, 1, cmd[5..].as_ptr() as u64, (cmd.len() - 5) as u64);
    }
    print("\n");
}

fn cmd_whoami() {
    let uid = getuid();
    if uid == 0 {
        print("root\n");
    } else {
        print("user (uid=");
        print_num(uid);
        print(")\n");
    }
}

fn cmd_pwd() {
    let uid = getuid();
    if uid == 0 {
        print("/root\n");
    } else {
        print("/home/user\n");
    }
}

fn cmd_pid() {
    let pid = getpid();
    print("PID: ");
    print_num(pid);
    print("\n");
}

fn cmd_exit() {
    print_colored("Goodbye!\n", COLOR_GREEN);
    exit(0);
}

// ============================================================================
// PANIC HANDLER
// ============================================================================

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    print_colored("\n!!! PANIC in Ring 3 shell !!!\n", COLOR_RED);
    exit(1);
}
