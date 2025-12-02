//src/power/mod.rs
//! Power management and shutdown for Astral OS

use crate::util::{outw, outl};
use core::arch::asm;

/// ACPI power management ports
const PM1a_CNT: u16 = 0x604;  // QEMU default
const PM1b_CNT: u16 = 0x0;    // Usually not present

/// Sleep type values (chipset-specific)
const SLP_TYPa: u16 = 0x2000; // Sleep type for shutdown
const SLP_EN: u16 = 0x2000;   // Sleep enable bit

pub enum PowerState {
    Shutdown,
    Reboot,
    Suspend,
}

/// Attempt ACPI shutdown
fn acpi_shutdown() -> bool {
    unsafe {
        // Try ACPI shutdown via PM1 control registers
        let sleep_command = SLP_TYPa | SLP_EN;
        
        if PM1a_CNT != 0 {
            outw(PM1a_CNT, sleep_command);
        }
        
        if PM1b_CNT != 0 {
            outw(PM1b_CNT, sleep_command);
        }
        
        // Wait a bit
        for _ in 0..1000000 {
            asm!("pause", options(nomem, nostack));
        }
    }
    
    false // If we're still here, it didn't work
}

/// Attempt APM shutdown (legacy)
fn apm_shutdown() -> bool {
    unsafe {
        // APM 1.1+ interface
        asm!(
            "mov ax, 0x5301",
            "xor bx, bx",
            "int 0x15",
            out("ax") _,
            options(nomem, nostack)
        );

        asm!(
            "mov ax, 0x530E",
            "xor bx, bx",
            "mov cx, 0x0101",
            "int 0x15",
            out("ax") _, out("cx") _,
            options(nomem, nostack)
        );

        asm!(
            "mov ax, 0x5308",
            "mov bx, 0x0001",
            "mov cx, 0x0001",
            "int 0x15",
            out("ax") _, out("cx") _,
            options(nomem, nostack)
        );

        asm!(
            "mov ax, 0x5307",
            "mov bx, 0x0001",
            "mov cx, 0x0003",
            "int 0x15",
            out("ax") _, out("cx") _,
            options(nomem, nostack)
        );
    }

    false
}


/// Try QEMU/Bochs specific shutdown
fn qemu_shutdown() -> bool {
    unsafe {
        // QEMU isa-debug-exit device
        outl(0x604, 0x2000);
        
        // Bochs/QEMU shutdown port
        outw(0xB004, 0x2000);
        
        // Old QEMU versions
        outw(0x600, 0x34);
        
        // Wait
        for _ in 0..100000 {
            asm!("pause", options(nomem, nostack));
        }
    }
    
    false
}

/// Triple fault shutdown (last resort)
fn triple_fault() -> ! {
    unsafe {
        // Load invalid IDT to cause triple fault
        asm!(
            "lidt [{}]",
            in(reg) &0u64,
            options(nostack, noreturn)
        );
    }
}

/// Keyboard controller reboot
fn keyboard_reboot() -> ! {
    unsafe {
        // Pulse reset line via keyboard controller
        asm!(
            "2:",
            "in al, 0x64",
            "test al, 0x02",
            "jnz 2b",
            "mov al, 0xFE",
            "out 0x64, al",
            "3:",
            "hlt",
            "jmp 3b",
            options(nostack, noreturn)
        );
    }
}

/// Main power management function
pub fn set_power_state(state: PowerState) -> ! {
    match state {
        PowerState::Shutdown => {
            crate::println!("Initiating shutdown...");
            
            // Sync filesystem
            crate::fs::fs_sync();
            
            crate::println!("Filesystem synced");
            crate::println!("Attempting ACPI shutdown...");
            
            if !acpi_shutdown() {
                crate::println!("ACPI failed, trying QEMU ports...");
                if !qemu_shutdown() {
                    crate::println!("QEMU ports failed, trying APM...");
                    if !apm_shutdown() {
                        crate::println!("All methods failed, halting CPU");
                    }
                }
            }
            
            // If all else fails, halt
            loop {
                unsafe {
                    asm!("cli", "hlt", options(nostack, nomem));
                }
            }
        }
        
        PowerState::Reboot => {
            crate::println!("Rebooting...");
            
            // Sync filesystem
            crate::fs::fs_sync();
            
            crate::println!("Filesystem synced");
            
            // Try keyboard controller reboot
            keyboard_reboot()
        }
        
        PowerState::Suspend => {
            crate::println!("Suspend not yet implemented");
            loop {
                unsafe {
                    asm!("hlt", options(nostack, nomem));
                }
            }
        }
    }
}

pub fn shutdown() -> ! {
    set_power_state(PowerState::Shutdown)
}

pub fn reboot() -> ! {
    set_power_state(PowerState::Reboot)
}