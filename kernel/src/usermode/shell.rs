//! User-Mode Shell
//! 
//! This shell runs entirely in Ring 3 and communicates with the kernel
//! only through syscalls. It provides a command-line interface for users.

use super::usys::{sys_print, sys_read_char, sys_yield};

/// Command buffer size
const CMD_BUF_SIZE: usize = 256;

/// User-mode shell entry point
/// This function is called after switching to Ring 3
#[no_mangle]
pub extern "C" fn user_shell_main() -> ! {
    // Print welcome message
    sys_print("\n");
    sys_print("=================================\n");
    sys_print("  Astral OS - User Mode Shell\n");
    sys_print("  Running in Ring 3\n");
    sys_print("=================================\n");
    sys_print("\n");
    
    // Command buffer
    let mut cmd_buf: [u8; CMD_BUF_SIZE] = [0; CMD_BUF_SIZE];
    let mut cmd_len: usize = 0;
    
    loop {
        // Print prompt
        sys_print("root@astral> ");
        
        // Read command
        cmd_len = 0;
        loop {
            // Try to read a character
            let c = sys_read_char();
            
            if c == 0 {
                // No input available, yield
                sys_yield();
                continue;
            }
            
            // Handle special characters
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
                        // Echo the character
                        let echo = [c];
                        super::usys::sys_write(&echo);
                    }
                }
            }
        }
        
        // Process command
        if cmd_len > 0 {
            let cmd = &cmd_buf[..cmd_len];
            process_command(cmd);
        }
    }
}

/// Process a command
fn process_command(cmd: &[u8]) {
    // Simple command matching
    if cmd == b"help" {
        sys_print("Available commands:\n");
        sys_print("  help  - Show this help\n");
        sys_print("  echo  - Echo text\n");
        sys_print("  clear - Clear screen\n");
        sys_print("  info  - System info\n");
    } else if cmd == b"info" {
        sys_print("Astral OS v0.3.0\n");
        sys_print("Running in Ring 3 (User Mode)\n");
        sys_print("Syscall API active\n");
    } else if cmd == b"clear" {
        // Print many newlines to "clear"
        for _ in 0..25 {
            sys_print("\n");
        }
    } else if cmd.starts_with(b"echo ") {
        // Echo the rest
        if let Ok(text) = core::str::from_utf8(&cmd[5..]) {
            sys_print(text);
            sys_print("\n");
        }
    } else {
        sys_print("Unknown command: ");
        if let Ok(s) = core::str::from_utf8(cmd) {
            sys_print(s);
        }
        sys_print("\nType 'help' for available commands.\n");
    }
}
