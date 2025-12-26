use x86_64::PhysAddr;
use core::ptr::{read_volatile, write_volatile};
use crate::serial_println;

/// I/O APIC register offsets
const IOREGSEL: u32 = 0x00;
const IOWIN: u32 = 0x10;

/// I/O APIC registers
const IOAPICID: u8 = 0x00;
const IOAPICVER: u8 = 0x01;
const IOREDTBL_BASE: u8 = 0x10;

pub struct IoApic {
    base_addr: usize,
}

impl IoApic {
    /// Initialize I/O APIC
    /// 
    /// For now, we use a hardcoded address (0xFEC00000) which is the default
    /// In a proper implementation, we'd parse the ACPI MADT table
    pub unsafe fn new() -> Option<Self> {
        const DEFAULT_IOAPIC_ADDR: usize = 0xFEC00000;
        
        let ioapic = IoApic {
            base_addr: DEFAULT_IOAPIC_ADDR,
        };
        
        // Read version to verify I/O APIC exists
        let version = ioapic.read_reg(IOAPICVER);
        let max_entries = ((version >> 16) & 0xFF) + 1;
        
        serial_println!("[IOAPIC] Found at {:#x}", DEFAULT_IOAPIC_ADDR);
        serial_println!("[IOAPIC]   Version: {:#x}", version & 0xFF);
        serial_println!("[IOAPIC]   Max redirection entries: {}", max_entries);
        
        Some(ioapic)
    }
    
    /// Read from I/O APIC register
    unsafe fn read_reg(&self, reg: u8) -> u32 {
        let regsel = (self.base_addr + IOREGSEL as usize) as *mut u32;
        let win = (self.base_addr + IOWIN as usize) as *const u32;
        
        write_volatile(regsel, reg as u32);
        read_volatile(win)
    }
    
    /// Write to I/O APIC register
    unsafe fn write_reg(&mut self, reg: u8, value: u32) {
        let regsel = (self.base_addr + IOREGSEL as usize) as *mut u32;
        let win = (self.base_addr + IOWIN as usize) as *mut u32;
        
        write_volatile(regsel, reg as u32);
        write_volatile(win, value);
    }
    
    /// Configure redirection entry for an IRQ
    /// 
    /// irq: the IRQ number (0-23)
    /// vector: the interrupt vector to deliver to (e.g., 33 for keyboard)
    /// apic_id: the CPU to send the interrupt to (0 for BSP)
    pub unsafe fn set_irq(&mut self, irq: u8, vector: u8, apic_id: u8) {
        let redirection_entry_low = IOREDTBL_BASE + (irq * 2);
        let redirection_entry_high = redirection_entry_low + 1;
        
        // Low 32 bits: vector and delivery mode
        // Bit 0-7: vector
        // Bit 8-10: delivery mode (000 = fixed)
        // Bit 11: dest mode (0 = physical)
        // Bit 12: delivery status (read-only)
        // Bit 13: polarity (0 = active high)
        // Bit 14: remote IRR (read-only)
        // Bit 15: trigger (0 = edge)
        // Bit 16: mask (0 = enabled)
        let low = vector as u32; // Fixed delivery, edge-triggered, active high, unmasked
        
        // High 32 bits: destination (bits 24-27 = APIC ID)
        let high = (apic_id as u32) << 24;
        
        self.write_reg(redirection_entry_high, high);
        self.write_reg(redirection_entry_low, low);
        
        serial_println!("[IOAPIC] Configured IRQ {} -> vector {} (APIC ID {})", 
            irq, vector, apic_id);
    }
    
    /// Mask an IRQ (disable it)
    pub unsafe fn mask_irq(&mut self, irq: u8) {
        let redirection_entry_low = IOREDTBL_BASE + (irq * 2);
        let value = self.read_reg(redirection_entry_low);
        self.write_reg(redirection_entry_low, value | (1 << 16)); // Set mask bit
    }
    
    /// Unmask an IRQ (enable it)
    pub unsafe fn unmask_irq(&mut self, irq: u8) {
        let redirection_entry_low = IOREDTBL_BASE + (irq * 2);
        let value = self.read_reg(redirection_entry_low);
        self.write_reg(redirection_entry_low, value & !(1 << 16)); // Clear mask bit
    }
}

pub static mut IOAPIC: Option<IoApic> = None;

pub unsafe fn init() -> Result<(), &'static str> {
    IOAPIC = IoApic::new();
    
    if IOAPIC.is_none() {
        return Err("I/O APIC not found");
    }
    
    // Configure keyboard (IRQ 1 -> vector 33)
    if let Some(ref mut ioapic) = IOAPIC {
        ioapic.set_irq(1, 33, 0); // IRQ 1 (keyboard), vector 33, CPU 0
    }
    
    serial_println!("[IOAPIC] Initialization complete");
    Ok(())
}
