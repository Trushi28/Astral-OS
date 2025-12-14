//! User-Mode Shell Launcher
//! 
//! Launches the user shell. Currently runs in Ring 0 but uses
//! the syscall API for all I/O, demonstrating proper separation.
//!
//! Future: Will use ELF loader to run in true Ring 3.

/// Launch the user shell (currently Ring 0 with syscall API)
/// 
/// The shell only communicates via syscalls, demonstrating
/// the proper separation even though it runs in kernel mode.
/// This avoids the complexity of remapping kernel pages as USER.
pub fn launch_user_shell() -> ! {
    crate::serial_println!("[RING3] Starting shell with syscall API...");
    
    crate::println!("\n=== Astral Shell (Syscall API Mode) ===\n");
    crate::println!("Note: Uses syscall interface for all I/O");
    crate::println!("      Ready for Ring 3 with ELF loader\n");
    
    // Run the shell - it only uses syscall wrappers
    super::shell::user_shell_main();
}
