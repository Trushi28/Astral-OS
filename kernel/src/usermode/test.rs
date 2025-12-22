// src/usermode/test.rs
//! Test userspace program using the ELF loader
//! This module provides test functions for Ring 3 execution

/// Run a test user program using the ELF loader
/// 
/// This uses the embedded shell ELF as a test program
pub fn run_test_program() -> ! {
    crate::serial_println!("[USERMODE] Running test program via ELF loader...");
    
    // Use the launcher to load and run the shell ELF
    super::launcher::launch_user_shell()
}

/// Test ELF parsing without actually running
pub fn test_elf_parse() {
    use super::elf::Elf64Header;
    
    let elf_data = super::USERLAND_SHELL_ELF;
    
    crate::serial_println!("[TEST] ELF data size: {} bytes", elf_data.len());
    
    match Elf64Header::parse(elf_data) {
        Ok(header) => {
            crate::serial_println!("[TEST] ELF parsed successfully");
            crate::serial_println!("[TEST] Entry point: 0x{:x}", header.entry_point());
            
            match header.program_headers(elf_data) {
                Ok(phdrs) => {
                    crate::serial_println!("[TEST] Program headers: {}", phdrs.len());
                    for (i, ph) in phdrs.iter().enumerate() {
                        crate::serial_println!("[TEST]   [{}] type={} vaddr=0x{:x} memsz={} flags=0x{:x}",
                            i, ph.p_type, ph.p_vaddr, ph.p_memsz, ph.p_flags);
                    }
                }
                Err(e) => {
                    crate::serial_println!("[TEST] Failed to parse program headers: {}", e);
                }
            }
        }
        Err(e) => {
            crate::serial_println!("[TEST] ELF parse failed: {}", e);
        }
    }
}
