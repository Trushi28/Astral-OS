//! User-Mode Shell Launcher
//! 
//! Launches the user shell in true Ring 3 using position-independent code.

use crate::arch::x86_64::usermode::{create_user_address_space, enter_usermode, USER_CODE_BASE};
use crate::usermode::loader::load_flat_binary;
use crate::usermode::ring3_binary::build_ring3_shell;

/// Launch the user shell in Ring 3
/// 
/// Creates a user address space, loads position-independent shell code,
/// and enters Ring 3 user mode.
pub fn launch_user_shell() -> ! {
    crate::serial_println!("[RING3] Launching Ring 3 shell...");
    
    // Create user address space
    let (mut page_table, stack_top) = match create_user_address_space() {
        Ok(pt) => pt,
        Err(e) => {
            crate::serial_println!("[RING3] Failed to create address space: {}", e);
            crate::println!("Failed to create user address space!");
            loop { unsafe { core::arch::asm!("cli; hlt"); } }
        }
    };
    
    crate::serial_println!("[RING3] User address space created, stack: 0x{:x}", stack_top);
    
    // Build position-independent shell binary
    let shell_code = build_ring3_shell();
    crate::serial_println!("[RING3] Shell binary: {} bytes", shell_code.len());
    
    // Load to user space
    let entry_point = match load_flat_binary(&shell_code, &mut page_table, USER_CODE_BASE) {
        Ok(ep) => ep,
        Err(e) => {
            crate::serial_println!("[RING3] Failed to load shell: {}", e);
            crate::println!("Failed to load shell binary!");
            loop { unsafe { core::arch::asm!("cli; hlt"); } }
        }
    };
    
    crate::serial_println!("[RING3] Shell loaded at: 0x{:x}", entry_point);

    // Get page table physical address
    let pt_phys = page_table.p4_physical().as_u64();
    
    // Register this shell as a Process so syscalls work correctly
    use crate::process::{Process, Pid, ProcessState, PriorityClass, set_current_pid, process_table};
    use crate::auth::CURRENT_SESSION;

    // Create process
    let pid = Pid::new();
    let mut process = Process::new(pid);

    // Set UID from current session
    let uid = CURRENT_SESSION.lock()
        .user.as_ref()
        .map(|u| u.uid)
        .unwrap_or(0);
    process.uid = uid;
    
    // Set process properties
    process.is_user_process = true;
    process.page_table = pt_phys;
    process.kernel_stack = crate::arch::x86_64::tss::get_kernel_stack();
    process.user_stack = stack_top;
    process.state = ProcessState::Running;
    process.priority = PriorityClass::Interactive;
    process.registers.rip = entry_point;
    process.registers.rsp = stack_top;
    process.registers.cs = crate::arch::x86_64::gdt::USER_CODE_SELECTOR as u64;
    process.registers.ss = crate::arch::x86_64::gdt::USER_DATA_SELECTOR as u64;
    process.registers.rflags = 0x202;

    // Add to process table
    process_table().lock().add(process).expect("Failed to register shell process");
    
    // Set as current PID
    set_current_pid(pid);
    
    crate::serial_println!("[RING3] Registered shell process (PID: {}, UID: {})", pid.as_u64(), uid);
    
    // Update TSS for syscall returns
    crate::arch::x86_64::tss::update_kernel_stack(
        crate::arch::x86_64::tss::get_kernel_stack()
    );
    
    crate::serial_println!("[RING3] PT: 0x{:x}, entering Ring 3...", pt_phys);
    
    unsafe {
        // Switch to user page tables
        core::arch::asm!("mov cr3, {}", in(reg) pt_phys, options(nostack));
        
        // Enter Ring 3
        enter_usermode(entry_point, stack_top);
    }
    
    // Never reached
    loop { unsafe { core::arch::asm!("cli; hlt"); } }
}

/// Launch shell in Ring 0 with syscall API (fallback)
/// 
/// Uses syscall wrappers but runs in Ring 0.
/// This is a fallback if Ring 3 shell has issues.
pub fn launch_user_shell_ring0() -> ! {
    crate::serial_println!("[RING0] Starting shell with syscall API...");
    
    crate::println!("\n=== Astral Shell (Ring 0 Mode) ===\n");
    crate::println!("Note: Uses syscall interface for all I/O");
    crate::println!("      (Ring 0 fallback mode)\n");
    
    // Run the Rust shell - it only uses syscall wrappers
    super::shell::user_shell_main();
}
