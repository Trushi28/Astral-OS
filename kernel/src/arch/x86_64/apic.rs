// src/arch/x86_64/apic.rs
//! Local APIC (xAPIC and x2APIC) support
#![allow(dead_code)] // Hardware register constants - intentionally defined for completeness

use super::cpu::{rdmsr, wrmsr};
use core::sync::atomic::{AtomicU64, Ordering};
use core::ptr::{read_volatile, write_volatile};

// APIC registers (xAPIC memory-mapped offsets)
const APIC_REG_ID: u32 = 0x20;
const APIC_REG_VERSION: u32 = 0x30;
const APIC_REG_TPR: u32 = 0x80;
const APIC_REG_EOI: u32 = 0xB0;
const APIC_REG_SPURIOUS: u32 = 0xF0;
const APIC_REG_ICR_LOW: u32 = 0x300;
const APIC_REG_ICR_HIGH: u32 = 0x310;
const APIC_REG_LVT_TIMER: u32 = 0x320;
const APIC_REG_LVT_LINT0: u32 = 0x350;
const APIC_REG_LVT_LINT1: u32 = 0x360;
const APIC_REG_LVT_ERROR: u32 = 0x370;
const APIC_REG_TIMER_INITIAL: u32 = 0x380;
const APIC_REG_TIMER_CURRENT: u32 = 0x390;
const APIC_REG_TIMER_DIVIDE: u32 = 0x3E0;

// x2APIC MSRs
const X2APIC_MSR_BASE: u32 = 0x800;
const X2APIC_MSR_ID: u32 = X2APIC_MSR_BASE + 0x02;
const X2APIC_MSR_VERSION: u32 = X2APIC_MSR_BASE + 0x03;
const X2APIC_MSR_TPR: u32 = X2APIC_MSR_BASE + 0x08;
const X2APIC_MSR_EOI: u32 = X2APIC_MSR_BASE + 0x0B;
const X2APIC_MSR_SPURIOUS: u32 = X2APIC_MSR_BASE + 0x0F;
const X2APIC_MSR_ICR: u32 = X2APIC_MSR_BASE + 0x30;
const X2APIC_MSR_LVT_TIMER: u32 = X2APIC_MSR_BASE + 0x32;
const X2APIC_MSR_LVT_LINT0: u32 = X2APIC_MSR_BASE + 0x35;
const X2APIC_MSR_LVT_LINT1: u32 = X2APIC_MSR_BASE + 0x36;
const X2APIC_MSR_LVT_ERROR: u32 = X2APIC_MSR_BASE + 0x37;
const X2APIC_MSR_TIMER_INITIAL: u32 = X2APIC_MSR_BASE + 0x38;
const X2APIC_MSR_TIMER_DIVIDE: u32 = X2APIC_MSR_BASE + 0x3E;

// APIC base MSR
const IA32_APIC_BASE: u32 = 0x1B;
const APIC_BASE_ENABLE: u64 = 1 << 11;
const APIC_BASE_X2APIC: u64 = 1 << 10;
const APIC_BASE_BSP: u64 = 1 << 8;

// Spurious interrupt vector
const APIC_SPURIOUS_VECTOR: u32 = 0xFF;
const APIC_SPURIOUS_ENABLE: u32 = 1 << 8;

// Timer modes
const APIC_TIMER_ONESHOT: u32 = 0;
const APIC_TIMER_PERIODIC: u32 = 1 << 17;
const APIC_TIMER_TSC_DEADLINE: u32 = 2 << 17;

// Delivery modes for IPI
const IPI_DELIVERY_FIXED: u64 = 0 << 8;
const IPI_DELIVERY_INIT: u64 = 5 << 8;
const IPI_DELIVERY_STARTUP: u64 = 6 << 8;

// Destination modes
const IPI_DEST_PHYSICAL: u64 = 0 << 11;
const IPI_DEST_LOGICAL: u64 = 1 << 11;

// Delivery status
const IPI_STATUS_PENDING: u64 = 1 << 12;

// Level
const IPI_LEVEL_DEASSERT: u64 = 0 << 14;
const IPI_LEVEL_ASSERT: u64 = 1 << 14;

// Trigger mode
const IPI_TRIGGER_EDGE: u64 = 0 << 15;
const IPI_TRIGGER_LEVEL: u64 = 1 << 15;

// Destination shorthand
const IPI_DEST_NO_SHORTHAND: u64 = 0 << 18;
const IPI_DEST_SELF: u64 = 1 << 18;
const IPI_DEST_ALL: u64 = 2 << 18;
const IPI_DEST_ALL_EXCEPT_SELF: u64 = 3 << 18;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApicMode {
    Disabled,
    XApic,
    X2Apic,
}

static APIC_MODE: AtomicU64 = AtomicU64::new(ApicMode::Disabled as u64);
static APIC_BASE_ADDR: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug)]
pub struct ApicId(u32);

impl ApicId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
    
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

pub enum IpiDestination {
    Single(ApicId),
    All,
    AllExceptSelf,
    Logical(u32),
}

/// Initialize Local APIC (prefers x2APIC if available)
pub fn init_apic() -> Result<(), &'static str> {
    // Check if APIC is supported
    let (_, _, ecx, edx) = super::cpu::cpuid(1, 0);
    if (edx & (1 << 9)) == 0 {
        return Err("APIC not supported");
    }
    
    let x2apic_supported = (ecx & (1 << 21)) != 0;
    
    unsafe {
        // Read APIC base
        let mut apic_base = rdmsr(IA32_APIC_BASE);
        let base_addr = apic_base & 0xFFFF_FFFF_FFFF_F000;
        
        APIC_BASE_ADDR.store(base_addr, Ordering::SeqCst);
        
        // Enable APIC
        apic_base |= APIC_BASE_ENABLE;
        
        // Try to enable x2APIC if supported
        if x2apic_supported {
            apic_base |= APIC_BASE_X2APIC;
            wrmsr(IA32_APIC_BASE, apic_base);
            
            // Verify x2APIC enabled
            let new_base = rdmsr(IA32_APIC_BASE);
            if (new_base & APIC_BASE_X2APIC) != 0 {
                APIC_MODE.store(ApicMode::X2Apic as u64, Ordering::SeqCst);
                init_x2apic()?;
                return Ok(());
            }
        }
        
        // Fall back to xAPIC
        wrmsr(IA32_APIC_BASE, apic_base);
        APIC_MODE.store(ApicMode::XApic as u64, Ordering::SeqCst);
        init_xapic()?;
    }
    
    Ok(())
}


/// Initialize xAPIC (memory-mapped)
unsafe fn init_xapic() -> Result<(), &'static str> {
    let base_addr = APIC_BASE_ADDR.load(Ordering::Relaxed);
    let hhdm = crate::get_hhdm_offset() as u64;
    let apic_ptr = (base_addr + hhdm) as *mut u32;
    
    // Enable APIC via spurious interrupt register
    let spurious = APIC_SPURIOUS_ENABLE | APIC_SPURIOUS_VECTOR;
    write_apic_reg(apic_ptr, APIC_REG_SPURIOUS, spurious);
    
    // Set task priority to accept all interrupts
    write_apic_reg(apic_ptr, APIC_REG_TPR, 0);
    
    // Mask all LVT entries initially
    write_apic_reg(apic_ptr, APIC_REG_LVT_TIMER, 1 << 16);
    write_apic_reg(apic_ptr, APIC_REG_LVT_LINT0, 1 << 16);
    write_apic_reg(apic_ptr, APIC_REG_LVT_LINT1, 1 << 16);
    write_apic_reg(apic_ptr, APIC_REG_LVT_ERROR, 1 << 16);
    
    Ok(())
}

/// Initialize x2APIC (MSR-based)
unsafe fn init_x2apic() -> Result<(), &'static str> {
    // Enable APIC via spurious interrupt register
    let spurious = (APIC_SPURIOUS_ENABLE | APIC_SPURIOUS_VECTOR) as u64;
    wrmsr(X2APIC_MSR_SPURIOUS, spurious);
    
    // Set task priority to accept all interrupts
    wrmsr(X2APIC_MSR_TPR, 0);
    
    // Mask all LVT entries initially
    wrmsr(X2APIC_MSR_LVT_TIMER, (1 << 16) as u64);
    wrmsr(X2APIC_MSR_LVT_LINT0, (1 << 16) as u64);
    wrmsr(X2APIC_MSR_LVT_LINT1, (1 << 16) as u64);
    wrmsr(X2APIC_MSR_LVT_ERROR, (1 << 16) as u64);
    
    Ok(())
}

/// Send End-Of-Interrupt
#[inline]
pub fn local_apic_eoi() {
    unsafe {
        match get_apic_mode() {
            ApicMode::X2Apic => {
                wrmsr(X2APIC_MSR_EOI, 0);
            }
            ApicMode::XApic => {
                let base_addr = APIC_BASE_ADDR.load(Ordering::Relaxed);
                let hhdm = crate::get_hhdm_offset() as u64;
                let apic_ptr = (base_addr + hhdm) as *mut u32;
                write_apic_reg(apic_ptr, APIC_REG_EOI, 0);
            }
            _ => {}
        }
    }
}

/// Get Local APIC ID
pub fn get_apic_id() -> ApicId {
    unsafe {
        match get_apic_mode() {
            ApicMode::X2Apic => {
                let id = rdmsr(X2APIC_MSR_ID) as u32;
                ApicId::new(id)
            }
            ApicMode::XApic => {
                let base_addr = APIC_BASE_ADDR.load(Ordering::Relaxed);
                let hhdm = crate::get_hhdm_offset() as u64;
                let apic_ptr = (base_addr + hhdm) as *mut u32;
                let id = read_apic_reg(apic_ptr, APIC_REG_ID) >> 24;
                ApicId::new(id)
            }
            _ => ApicId::new(0),
        }
    }
}

/// Send Inter-Processor Interrupt
pub fn send_ipi(dest: IpiDestination, vector: u8) {
    unsafe {
        match get_apic_mode() {
            ApicMode::X2Apic => {
                send_ipi_x2apic(dest, vector);
            }
            ApicMode::XApic => {
                send_ipi_xapic(dest, vector);
            }
            _ => {}
        }
    }
}

unsafe fn send_ipi_x2apic(dest: IpiDestination, vector: u8) {
    let mut icr: u64 = vector as u64;
    icr |= IPI_DELIVERY_FIXED;
    icr |= IPI_DEST_PHYSICAL;
    icr |= IPI_LEVEL_ASSERT;
    icr |= IPI_TRIGGER_EDGE;
    
    match dest {
        IpiDestination::Single(apic_id) => {
            icr |= IPI_DEST_NO_SHORTHAND;
            icr |= (apic_id.as_u32() as u64) << 32;
        }
        IpiDestination::All => {
            icr |= IPI_DEST_ALL;
        }
        IpiDestination::AllExceptSelf => {
            icr |= IPI_DEST_ALL_EXCEPT_SELF;
        }
        IpiDestination::Logical(dest_mask) => {
            icr |= IPI_DEST_LOGICAL;
            icr |= IPI_DEST_NO_SHORTHAND;
            icr |= (dest_mask as u64) << 32;
        }
    }
    
    wrmsr(X2APIC_MSR_ICR, icr);
}

unsafe fn send_ipi_xapic(dest: IpiDestination, vector: u8) {
    let base_addr = APIC_BASE_ADDR.load(Ordering::Relaxed);
    let hhdm = crate::get_hhdm_offset() as u64;
    let apic_ptr = (base_addr + hhdm) as *mut u32;
    
    // Wait for previous IPI to complete
    while (read_apic_reg(apic_ptr, APIC_REG_ICR_LOW) & (1 << 12)) != 0 {
        super::cpu::pause();
    }
    
    let mut icr_low: u32 = vector as u32;
    icr_low |= (IPI_DELIVERY_FIXED as u32) & 0xFFFF_FFFF;
    icr_low |= (IPI_DEST_PHYSICAL as u32) & 0xFFFF_FFFF;
    icr_low |= (IPI_LEVEL_ASSERT as u32) & 0xFFFF_FFFF;
    icr_low |= (IPI_TRIGGER_EDGE as u32) & 0xFFFF_FFFF;
    
    match dest {
        IpiDestination::Single(apic_id) => {
            let icr_high = apic_id.as_u32() << 24;
            write_apic_reg(apic_ptr, APIC_REG_ICR_HIGH, icr_high);
            write_apic_reg(apic_ptr, APIC_REG_ICR_LOW, icr_low);
        }
        IpiDestination::All => {
            icr_low |= (IPI_DEST_ALL as u32) & 0xFFFF_FFFF;
            write_apic_reg(apic_ptr, APIC_REG_ICR_LOW, icr_low);
        }
        IpiDestination::AllExceptSelf => {
            icr_low |= (IPI_DEST_ALL_EXCEPT_SELF as u32) & 0xFFFF_FFFF;
            write_apic_reg(apic_ptr, APIC_REG_ICR_LOW, icr_low);
        }
        IpiDestination::Logical(dest_mask) => {
            icr_low |= (IPI_DEST_LOGICAL as u32) & 0xFFFF_FFFF;
            write_apic_reg(apic_ptr, APIC_REG_ICR_HIGH, dest_mask);
            write_apic_reg(apic_ptr, APIC_REG_ICR_LOW, icr_low);
        }
    }
}


pub fn send_init_ipi(apic_id: ApicId) {
    unsafe {
        match get_apic_mode() {
            ApicMode::X2Apic => {
                let mut icr: u64 = 0;
                icr |= IPI_DELIVERY_INIT;
                icr |= IPI_DEST_PHYSICAL;
                icr |= IPI_LEVEL_ASSERT;
                icr |= IPI_TRIGGER_LEVEL;
                icr |= (apic_id.as_u32() as u64) << 32;
                wrmsr(X2APIC_MSR_ICR, icr);
            }
            ApicMode::XApic => {
                let base_addr = APIC_BASE_ADDR.load(Ordering::Relaxed);
                let hhdm = crate::get_hhdm_offset() as u64;
                let apic_ptr = (base_addr + hhdm) as *mut u32;
                
                let icr_high = apic_id.as_u32() << 24;
                let icr_low = (IPI_DELIVERY_INIT | IPI_LEVEL_ASSERT | IPI_TRIGGER_LEVEL) as u32;
                
                write_apic_reg(apic_ptr, APIC_REG_ICR_HIGH, icr_high);
                write_apic_reg(apic_ptr, APIC_REG_ICR_LOW, icr_low);
            }
            _ => {}
        }
    }
}

/// Send STARTUP IPI to target CPU
pub fn send_startup_ipi(apic_id: ApicId, vector: u8) {
    unsafe {
        match get_apic_mode() {
            ApicMode::X2Apic => {
                let mut icr: u64 = vector as u64;
                icr |= IPI_DELIVERY_STARTUP;
                icr |= IPI_DEST_PHYSICAL;
                icr |= IPI_LEVEL_ASSERT;
                icr |= IPI_TRIGGER_EDGE;
                icr |= (apic_id.as_u32() as u64) << 32;
                wrmsr(X2APIC_MSR_ICR, icr);
            }
            ApicMode::XApic => {
                let base_addr = APIC_BASE_ADDR.load(Ordering::Relaxed);
                let hhdm = crate::get_hhdm_offset() as u64;
                let apic_ptr = (base_addr + hhdm) as *mut u32;
                
                let icr_high = apic_id.as_u32() << 24;
                let icr_low = (vector as u32) | ((IPI_DELIVERY_STARTUP | IPI_LEVEL_ASSERT | IPI_TRIGGER_EDGE) as u32);
                
                write_apic_reg(apic_ptr, APIC_REG_ICR_HIGH, icr_high);
                write_apic_reg(apic_ptr, APIC_REG_ICR_LOW, icr_low);
            }
            _ => {}
        }
    }
}

/// Setup LAPIC timer
pub fn setup_timer(vector: u8, initial_count: u32, periodic: bool) {
    unsafe {
        let mode = if periodic { APIC_TIMER_PERIODIC } else { APIC_TIMER_ONESHOT };
        
        match get_apic_mode() {
            ApicMode::X2Apic => {
                wrmsr(X2APIC_MSR_TIMER_DIVIDE, 0x03); // Divide by 16
                wrmsr(X2APIC_MSR_LVT_TIMER, (vector as u64) | (mode as u64));
                wrmsr(X2APIC_MSR_TIMER_INITIAL, initial_count as u64);
            }
            ApicMode::XApic => {
                let base_addr = APIC_BASE_ADDR.load(Ordering::Relaxed);
                let hhdm = crate::get_hhdm_offset() as u64;
                let apic_ptr = (base_addr + hhdm) as *mut u32;
                
                write_apic_reg(apic_ptr, APIC_REG_TIMER_DIVIDE, 0x03);
                write_apic_reg(apic_ptr, APIC_REG_LVT_TIMER, (vector as u32) | mode);
                write_apic_reg(apic_ptr, APIC_REG_TIMER_INITIAL, initial_count);
            }
            _ => {}
        }
    }
}

fn get_apic_mode() -> ApicMode {
    let mode_val = APIC_MODE.load(Ordering::Relaxed);
    if mode_val == ApicMode::X2Apic as u64 {
        ApicMode::X2Apic
    } else if mode_val == ApicMode::XApic as u64 {
        ApicMode::XApic
    } else {
        ApicMode::Disabled
    }
}

#[inline]
unsafe fn read_apic_reg(base: *mut u32, offset: u32) -> u32 {
    read_volatile(base.add((offset / 4) as usize))
}

#[inline]
unsafe fn write_apic_reg(base: *mut u32, offset: u32, value: u32) {
    write_volatile(base.add((offset / 4) as usize), value);
}