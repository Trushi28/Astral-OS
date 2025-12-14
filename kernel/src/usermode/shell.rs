//! User-Mode Shell
//! 
//! This shell uses the syscall-style API for all I/O operations.
//! Currently runs in Ring 0 but the API is designed for Ring 3.

use super::usys::{sys_print, sys_read_char, sys_yield};

/// Command buffer size
const CMD_BUF_SIZE: usize = 256;

/// User-mode shell entry point
#[no_mangle]
pub extern "C" fn user_shell_main() -> ! {
    // Print welcome message
    sys_print("\n");
    sys_print("===========================================\n");
    sys_print("       Astral OS - User Mode Shell\n");
    sys_print("===========================================\n");
    sys_print("Type 'help' for available commands\n\n");
    
    // Command buffer
    let mut cmd_buf: [u8; CMD_BUF_SIZE] = [0; CMD_BUF_SIZE];
    let mut cmd_len: usize;
    
    loop {
        // Print prompt
        sys_print("root@astral:~$ ");
        
        // Read command
        cmd_len = 0;
        loop {
            let c = sys_read_char();
            
            if c == 0 {
                sys_yield();
                continue;
            }
            
            match c {
                b'\n' | b'\r' => {
                    sys_print("\n");
                    break;
                }
                8 | 127 => {
                    // Backspace
                    if cmd_len > 0 {
                        cmd_len -= 1;
                        sys_print("\x08 \x08");
                    }
                }
                _ => {
                    if cmd_len < CMD_BUF_SIZE - 1 {
                        cmd_buf[cmd_len] = c;
                        cmd_len += 1;
                        let echo = [c];
                        super::usys::sys_write(&echo);
                    }
                }
            }
        }
        
        // Process command
        if cmd_len > 0 {
            process_command(&cmd_buf[..cmd_len]);
        }
    }
}

/// Process a command
fn process_command(cmd: &[u8]) {
    // Convert to string for easier matching
    let cmd_str = core::str::from_utf8(cmd).unwrap_or("");
    let parts: alloc::vec::Vec<&str> = cmd_str.split_whitespace().collect();
    
    if parts.is_empty() {
        return;
    }
    
    match parts[0] {
        "help" => cmd_help(),
        "info" | "uname" => cmd_uname(),
        "clear" | "cls" => cmd_clear(),
        "echo" => cmd_echo(&parts[1..]),
        "ls" => cmd_ls(),
        "pwd" => cmd_pwd(),
        "date" | "time" => cmd_date(),
        "ps" => cmd_ps(),
        "whoami" => cmd_whoami(),
        "uptime" => cmd_uptime(),
        "mem" | "free" => cmd_mem(),
        "cpu" => cmd_cpu(),
        "exit" | "logout" => cmd_exit(),
        "reboot" => cmd_reboot(),
        "version" => cmd_version(),
        _ => {
            sys_print("Command not found: ");
            sys_print(parts[0]);
            sys_print("\n");
            sys_print("Type 'help' for available commands.\n");
        }
    }
}

fn cmd_help() {
    sys_print("=== Astral Shell Commands ===\n\n");
    sys_print("System:\n");
    sys_print("  help      - Show this help\n");
    sys_print("  info      - System information\n");
    sys_print("  version   - OS version\n");
    sys_print("  uptime    - System uptime\n");
    sys_print("  reboot    - Reboot system\n");
    sys_print("  exit      - Exit shell\n");
    sys_print("\nFiles:\n");
    sys_print("  ls        - List files\n");
    sys_print("  pwd       - Show current directory\n");
    sys_print("\nMonitor:\n");
    sys_print("  ps        - List processes\n");
    sys_print("  mem       - Memory usage\n");
    sys_print("  cpu       - CPU info\n");
    sys_print("\nUtility:\n");
    sys_print("  echo      - Print text\n");
    sys_print("  clear     - Clear screen\n");
    sys_print("  whoami    - Current user\n");
    sys_print("  date      - Current time\n");
}

fn cmd_uname() {
    sys_print("Astral OS v0.3.0\n");
    sys_print("Architecture: x86_64\n");
    sys_print("Kernel: Astral Microkernel\n");
    sys_print("Build: Rust Edition 2024\n");
}

fn cmd_version() {
    sys_print("Astral OS version 0.3.0\n");
    sys_print("Shell API: Syscall-based\n");
    sys_print("Ring Level: 0 (Kernel Mode)\n");
}

fn cmd_clear() {
    // Clear framebuffer properly
    crate::drivers::framebuffer::clear();
}

fn cmd_echo(args: &[&str]) {
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            sys_print(" ");
        }
        sys_print(arg);
    }
    sys_print("\n");
}

fn cmd_ls() {
    // Get real file list from filesystem
    let files = crate::fs::psychicfs::fs_list();
    if files.is_empty() {
        sys_print("(empty)\n");
    } else {
        for file in files {
            sys_print(&file);
            sys_print("  ");
        }
        sys_print("\n");
    }
}

fn cmd_pwd() {
    sys_print("/home/root\n");
}

fn cmd_date() {
    sys_print("System clock not yet implemented\n");
    sys_print("Use 'uptime' for system runtime\n");
}

fn cmd_ps() {
    sys_print("PID  STATE     NAME\n");
    sys_print("---  --------  ----\n");
    
    let table = crate::process::process_table().lock();
    for proc in table.iter() {
        let state = match proc.state {
            crate::process::ProcessState::Ready => "READY   ",
            crate::process::ProcessState::Running => "RUNNING ",
            crate::process::ProcessState::Blocked => "BLOCKED ",
            crate::process::ProcessState::Zombie => "ZOMBIE  ",
        };
        sys_print("  ");
        print_num(proc.pid.as_u64());
        sys_print("  ");
        sys_print(state);
        sys_print("\n");
    }
    
    sys_print("\nTotal: ");
    print_num(table.count() as u64);
    sys_print(" processes\n");
}

fn cmd_whoami() {
    sys_print("root\n");
}

fn cmd_uptime() {
    // Get scheduler stats which has idle_ticks
    let stats = crate::process::scheduler::get_global_stats();
    let seconds = stats.context_switches / 100;  // Approximate
    sys_print("System running since boot\n");
    sys_print("Context switches: ");
    print_num(stats.context_switches);
    sys_print("\n");
}

fn cmd_mem() {
    // Get real memory stats
    let (total, used, free) = crate::memory::frame::get_stats();
    
    sys_print("Physical Memory:\n");
    sys_print("  Total frames: ");
    print_num(total as u64);
    sys_print("\n");
    sys_print("  Used frames:  ");
    print_num(used as u64);
    sys_print("\n");
    sys_print("  Free frames:  ");
    print_num(free as u64);
    sys_print("\n");
    sys_print("  Total: ");
    print_num((total * 4 / 1024) as u64);
    sys_print(" MB\n");
    sys_print("  Free:  ");
    print_num((free * 4 / 1024) as u64);
    sys_print(" MB\n");
    
    // Heap stats
    let (heap_used, heap_free) = crate::memory::heap::get_stats();
    sys_print("\nKernel Heap:\n");
    sys_print("  Used: ");
    print_num((heap_used / 1024) as u64);
    sys_print(" KB\n");
    sys_print("  Free: ");
    print_num((heap_free / 1024) as u64);
    sys_print(" KB\n");
}

fn cmd_cpu() {
    let cpu_count = crate::arch::x86_64::cpu::get_cpu_count();
    
    sys_print("CPU Information:\n");
    sys_print("  Architecture: x86_64\n");
    sys_print("  Cores: ");
    print_num(cpu_count as u64);
    sys_print("\n");
    sys_print("  Mode: Long Mode (64-bit)\n");
    
    // Scheduler stats
    let stats = crate::process::scheduler::get_global_stats();
    sys_print("  Processes: ");
    print_num(stats.total_processes as u64);
    sys_print("\n");
}

fn cmd_exit() {
    sys_print("Goodbye!\n");
    super::usys::sys_exit(0);
}

fn cmd_reboot() {
    sys_print("Rebooting...\n");
    unsafe {
        // PS/2 keyboard controller reset
        core::arch::asm!(
            "mov al, 0xFE",
            "out 0x64, al",
            options(nostack)
        );
    }
}

/// Helper to print a number
fn print_num(n: u64) {
    if n >= 10 {
        print_num(n / 10);
    }
    let digit = (n % 10) as u8 + b'0';
    super::usys::sys_write(&[digit]);
}

// Needed for Vec
extern crate alloc;
