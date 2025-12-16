//! User-Mode Shell
//! 
//! This shell uses the syscall-style API for all I/O operations.
//! Designed to run in Ring 3 user mode.

use super::usys::{
    sys_print, sys_read_char, sys_yield, sys_write,
    sys_fs_list, sys_get_process_info, sys_get_mem_info, 
    sys_get_cpu_info, sys_clear_screen, sys_exit,
    sys_getuid,
};

/// Command buffer size
const CMD_BUF_SIZE: usize = 256;

/// User-mode shell entry point
#[no_mangle]
pub extern "C" fn user_shell_main() -> ! {
    // Print welcome message
    sys_print("\n");
    sys_print("===========================================\n");
    sys_print("       Astral OS - Ring 3 User Shell\n");
    sys_print("===========================================\n");
    sys_print("Type 'help' for available commands\n\n");
    
    // Command buffer
    let mut cmd_buf: [u8; CMD_BUF_SIZE] = [0; CMD_BUF_SIZE];
    let mut cmd_len: usize;
    
    loop {
        // Print prompt
        let uid = sys_getuid();
        if uid == 0 {
            sys_print("root@astral# ");
        } else {
            sys_print("user@astral$ ");
        }
        
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
                        sys_write(&echo);
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
    let mut parts = cmd_str.split_whitespace();
    
    let command = match parts.next() {
        Some(c) => c,
        None => return,
    };
    
    match command {
        "help" => cmd_help(),
        "info" | "uname" => cmd_uname(),
        "clear" | "cls" => cmd_clear(),
        "echo" => {
            for part in parts {
                sys_print(part);
                sys_print(" ");
            }
            sys_print("\n");
        },
        "ls" => cmd_ls(),
        "pwd" => {
            let uid = sys_getuid();
            if uid == 0 {
                sys_print("/root\n");
            } else {
                sys_print("/home/user\n");
            }
        },
        "date" | "time" => sys_print("System clock not yet implemented\n"),
        "ps" => cmd_ps(),
        "whoami" => {
            let uid = sys_getuid();
            if uid == 0 {
                sys_print("root\n");
            } else {
                sys_print("user (uid=");
                // Simple number printing would be nice, but for now just "user" or naive implementation if we can't print number
                // Convert uid to string? usys doesn't have format!
                // We'll just print "user" for now, or assume 1000
                if uid == 1000 {
                    sys_print("user\n");
                } else {
                    sys_print("unknown\n");
                }
            }
        },
        "uptime" => sys_print("System running since boot\n"),
        "mem" | "free" => cmd_mem(),
        "cpu" => cmd_cpu(),
        "exit" | "logout" => cmd_exit(),
        "reboot" => cmd_reboot(),
        "version" => cmd_version(),
        _ => {
            sys_print("Command not found: ");
            sys_print(command);
            sys_print("\nType 'help' for available commands.\n");
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
    sys_print("Ring Level: 3 (User Mode)\n");
}

fn cmd_version() {
    sys_print("Astral OS version 0.3.0\n");
    sys_print("Shell API: Syscall-based\n");
    sys_print("Ring Level: 3 (User Mode)\n");
}

fn cmd_clear() {
    // Use syscall to clear screen
    sys_clear_screen();
}

fn cmd_ls() {
    // Use syscall - kernel will print the file list
    sys_fs_list();
}

fn cmd_ps() {
    // Use syscall - kernel will print the process list
    sys_get_process_info(&mut [0u8; 1]);
}

fn cmd_mem() {
    // Use syscall - kernel will print memory info
    sys_get_mem_info();
}

fn cmd_cpu() {
    // Use syscall - kernel will print CPU info
    sys_get_cpu_info();
}

fn cmd_exit() {
    sys_print("Goodbye!\n");
    sys_exit(0);
}

fn cmd_reboot() {
    sys_print("Rebooting...\n");
    // This will be handled by syscall in future
    // For now just exit
    sys_exit(0);
}
