//! User-Mode Shell Launcher
//! 
//! Launches the user shell in true Ring 3 using ELF loader.

use crate::arch::x86_64::usermode::enter_usermode;
use crate::interrupts::idt::set_tss_rsp0;  // Use the correct TSS (from idt.rs, not tss.rs!)
use crate::usermode::spawn::spawn_user_process;

/// Launch the user shell in Ring 3 from embedded ELF binary
/// 
/// Creates a user address space, loads ELF shell binary,
/// and enters Ring 3 user mode.
pub fn launch_user_shell() -> ! {
    crate::serial_println!("[RING3] Launching Ring 3 shell from ELF...");
    
    // Get embedded ELF binary
    let elf_data = super::USERLAND_SHELL_ELF;
    crate::serial_println!("[RING3] Shell ELF size: {} bytes", elf_data.len());
    
    // Verify ELF magic
    if elf_data.len() >= 4 {
        crate::serial_println!("[RING3] ELF magic: {:02x} {:02x} {:02x} {:02x}",
            elf_data[0], elf_data[1], elf_data[2], elf_data[3]);
    }
    
    // Spawn the shell process using ELF loader
    let spawned = match spawn_user_process(elf_data) {
        Ok(s) => s,
        Err(e) => {
            crate::serial_println!("[RING3] Failed to spawn shell: {}", e);
            crate::println!("Failed to spawn Ring 3 shell: {}", e);
            loop { unsafe { core::arch::asm!("cli; hlt"); } }
        }
    };
    
    crate::serial_println!("[RING3] Shell process PID: {}, entry: 0x{:x}", 
        spawned.pid.as_u64(), spawned.entry_point);
    
    // Set as current process
    crate::process::set_current_pid(spawned.pid);
    
    // Update TSS RSP0 for syscall/interrupt returns
    let kernel_stack = {
        let table = crate::process::process_table().lock();
        table.get(spawned.pid).map(|p| p.kernel_stack).unwrap_or(0)
    };
    
    crate::serial_println!("[RING3] Setting TSS RSP0 to kernel_stack: 0x{:x}", kernel_stack);
    set_tss_rsp0(kernel_stack);
    
    // Verify TSS RSP0 was set
    let rsp0 = crate::interrupts::idt::get_tss_rsp0();
    crate::serial_println!("[RING3] TSS RSP0 now: 0x{:x}", rsp0);
    
    crate::serial_println!("[RING3] Entering Ring 3 at 0x{:x} with stack 0x{:x}",
        spawned.entry_point, spawned.user_stack);
    
    // Debug: Verify user code is accessible by reading first few bytes from the physical frame
    // before switching page tables
    crate::serial_println!("[RING3] Verifying loaded user code...");

        // The user code was loaded at physical frame, accessible via HHDM
        // Entry point is 0x400000, first LOAD segment starts there
        // Find the physical frame for the code

        
        // We need to walk the user page table to find the physical address
        // For now, let's verify after CR3 switch by reading from the virtual address

    
    crate::serial_println!("[RING3] Switching to user page tables (PML4=0x{:x})", spawned.page_table_phys);
    
    unsafe {
        // Switch to user page tables
        core::arch::asm!("mov cr3, {}", in(reg) spawned.page_table_phys, options(nostack));
        
        crate::serial_println!("[RING3] CR3 switched, testing user code access...");
        
        // Try to read first few bytes of user code to verify mapping
        let code_ptr = spawned.entry_point as *const u8;
        let b0 = core::ptr::read_volatile(code_ptr);
        let b1 = core::ptr::read_volatile(code_ptr.add(1));
        let b2 = core::ptr::read_volatile(code_ptr.add(2));
        let b3 = core::ptr::read_volatile(code_ptr.add(3));
        
        crate::serial_println!("[RING3] User code at 0x{:x}: {:02x} {:02x} {:02x} {:02x}",
            spawned.entry_point, b0, b1, b2, b3);
        
        // Enter Ring 3
        enter_usermode(spawned.entry_point, spawned.user_stack);
    }
    
    // Never reached
    loop { unsafe { core::arch::asm!("cli; hlt"); } }
}

/// Launch shell from filesystem (for future PsychicFS loading)
/// 
/// This function loads the shell ELF from PsychicFS rather than
/// the embedded binary. Useful for updating shell without recompiling kernel.
pub fn launch_shell_from_fs(filename: &str) -> Result<!, &'static str> {
    crate::serial_println!("[RING3] Loading shell from filesystem: {}", filename);
    
    // Read ELF from filesystem
    let elf_data = crate::fs::fs_read(filename)
        .ok_or("Shell binary not found in filesystem")?;
    
    crate::serial_println!("[RING3] Loaded {} bytes from {}", elf_data.len(), filename);
    
    // Spawn the shell process
    let spawned = spawn_user_process(&elf_data)?;
    
    // Set as current process
    crate::process::set_current_pid(spawned.pid);
    
    // Update TSS RSP0 for syscall/interrupt returns
    let kernel_stack = {
        let table = crate::process::process_table().lock();
        table.get(spawned.pid).map(|p| p.kernel_stack).unwrap_or(0)
    };
    set_tss_rsp0(kernel_stack);
    
    crate::serial_println!("[RING3] Entering Ring 3 from filesystem shell");
    
    unsafe {
        core::arch::asm!("mov cr3, {}", in(reg) spawned.page_table_phys, options(nostack));
        enter_usermode(spawned.entry_point, spawned.user_stack);
    }
    
    loop { unsafe { core::arch::asm!("cli; hlt"); } }
}
