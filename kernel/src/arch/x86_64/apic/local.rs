use x86_64::VirtAddr;
use core::ptr::{read_volatile, write_volatile};
use crate::serial_println;
use super::super::msr;

// Local APIC register offsets
const APIC_ID: u32 = 0x20;
const APIC_VERSION: u32 = 0x30;
const APIC_TPR: u32 = 0x80;
const APIC_EOI: u32 = 0xB0;
const APIC_SIVR: u32 = 0xF0;
const APIC_ESR: u32 = 0x280;
const APIC_LVT_TIMER: u32 = 0x320;
const APIC_TIMER_INITIAL: u32 = 0x380;
const APIC_TIMER_CURRENT: u32 = 0x390;
const APIC_TIMER_DIVIDE: u32 = 0x3E0;

// Flags
const APIC_SIVR_ENABLE: u32 = 1 << 8;
const TIMER_PERIODIC: u32 = 1 << 17;
const TIMER_MASKED: u32 = 1 << 16;

pub struct LocalApic {
    base_addr: VirtAddr,
}

impl LocalApic {
    /// Initialize Local APIC
    pub unsafe fn new() -> Option<Self> {
        // Get APIC base address from MSR
        let apic_base_msr = msr::rdmsr(msr::IA32_APIC_BASE_MSR);
        
        // Check if APIC is enabled (bit 11)
        if (apic_base_msr & (1 << 11)) == 0 {
            serial_println!("[APIC] ERROR: APIC not enabled in MSR");
            return None;
        }
        
        // Extract base address (bits 12-35, page-aligned)
        let base_addr = VirtAddr::new(apic_base_msr & 0xFFFF_FFFF_F000);
        
        serial_println!("[APIC] Local APIC base address: {:#x}", base_addr.as_u64());
        
        let mut apic = LocalApic { base_addr };
        
        // Enable APIC via Spurious Interrupt Vector Register
        let sivr = apic.read(APIC_SIVR);
        apic.write(APIC_SIVR, sivr | APIC_SIVR_ENABLE | 0xFF); // 0xFF = spurious vector
        
        let apic_id = apic.read(APIC_ID) >> 24;
        let version = apic.read(APIC_VERSION) & 0xFF;
        
        serial_println!("[APIC] Local APIC initialized");
        serial_println!("[APIC]   ID: {}", apic_id);
        serial_println!("[APIC]   Version: {:#x}", version);
        
        Some(apic)
    }
    
    /// Read from APIC register
    #[inline]
    unsafe fn read(&self, offset: u32) -> u32 {
        let addr = (self.base_addr.as_u64() + offset as u64) as *const u32;
        read_volatile(addr)
    }
    
    /// Write to APIC register
    #[inline]
    unsafe fn write(&mut self, offset: u32, value: u32) {
        let addr = (self.base_addr.as_u64() + offset as u64) as *mut u32;
        write_volatile(addr, value);
    }
    
    /// Send End-Of-Interrupt signal
    #[inline]
    pub unsafe fn end_of_interrupt(&mut self) {
        self.write(APIC_EOI, 0);
    }
    
    /// Setup APIC timer
    pub unsafe fn setup_timer(&mut self, vector: u8, divide: u8, initial_count: u32) {
        // Set timer divide configuration
        self.write(APIC_TIMER_DIVIDE, divide as u32);
        
        // Set timer mode to periodic and unmask
        self.write(APIC_LVT_TIMER, (vector as u32) | TIMER_PERIODIC);
        
        // Set initial count to start timer
        self.write(APIC_TIMER_INITIAL, initial_count);
        
        serial_println!("[APIC] Timer configured: vector={}, divide={}, count={}", 
            vector, divide, initial_count);
    }
    
    /// Setup APIC timer (masked - won't fire until unmasked)
    pub unsafe fn setup_timer_masked(&mut self, vector: u8, divide: u8, initial_count: u32) {
        // Set timer divide configuration
        self.write(APIC_TIMER_DIVIDE, divide as u32);
        
        // Set timer mode to periodic AND MASKED
        self.write(APIC_LVT_TIMER, (vector as u32) | TIMER_PERIODIC | TIMER_MASKED);
        
        // Set initial count (won't start until unmasked)
        self.write(APIC_TIMER_INITIAL, initial_count);
        
        serial_println!("[APIC] Timer configured: vector={}, divide={}, count={}", 
            vector, divide, initial_count);
    }
    
    /// Unmask timer to start firing interrupts
    pub unsafe fn unmask_timer(&mut self, vector: u8) {
        self.write(APIC_LVT_TIMER, (vector as u32) | TIMER_PERIODIC);
        serial_println!("[APIC] Timer unmasked and active");
    }
    
    /// Get current timer count
    pub unsafe fn get_timer_count(&self) -> u32 {
        self.read(APIC_TIMER_CURRENT)
    }
    
    /// Calibrate timer using PIT or TSC
    pub unsafe fn calibrate_timer(&mut self) -> u32 {
        // Set timer to max value
        self.write(APIC_TIMER_DIVIDE, 0x03); // Divide by 16
        self.write(APIC_LVT_TIMER, TIMER_MASKED); // Masked during calibration
        self.write(APIC_TIMER_INITIAL, 0xFFFFFFFF);
        
        // Wait a known amount of time (we'll use a busy loop for now)
        // In a real implementation, you'd use PIT or TSC
        for _ in 0..10000000 {
            core::hint::spin_loop();
        }
        
        // Read how many ticks passed
        let ticks = 0xFFFFFFFF - self.read(APIC_TIMER_CURRENT);
        
        serial_println!("[APIC] Timer calibrated: {} ticks", ticks);
        ticks
    }
}

pub static mut LOCAL_APIC: Option<LocalApic> = None;

pub unsafe fn init() -> Result<(), &'static str> {
    serial_println!("[APIC] Initializing Local APIC...");
    
    // Enable APIC in MSR if not already enabled
    let mut apic_base = msr::rdmsr(msr::IA32_APIC_BASE_MSR);
    if (apic_base & (1 << 11)) == 0 {
        apic_base |= 1 << 11; // Set APIC Global Enable
        msr::wrmsr(msr::IA32_APIC_BASE_MSR, apic_base);
    }
    
    // Create Local APIC instance
    LOCAL_APIC = LocalApic::new();
    
    if LOCAL_APIC.is_none() {
        return Err("Failed to initialize Local APIC");
    }
    
    Ok(())
}

/// Initialize Local APIC on an Application Processor
pub unsafe fn init_ap(cpu_id: u32) {
    serial_println!("[APIC] Initializing Local APIC on CPU {}...", cpu_id);
    
    // Enable APIC in MSR
    let mut apic_base = msr::rdmsr(msr::IA32_APIC_BASE_MSR);
    apic_base |= 1 << 11; // Set APIC Global Enable
    msr::wrmsr(msr::IA32_APIC_BASE_MSR, apic_base);
    
    // Get base address and enable APIC
    let apic_base_msr = msr::rdmsr(msr::IA32_APIC_BASE_MSR);
    let base_addr = x86_64::VirtAddr::new(apic_base_msr & 0xFFFF_FFFF_F000);
    
    // Read APIC ID register to verify
    let addr = (base_addr.as_u64() + 0x20) as *const u32;
    let apic_id = read_volatile(addr) >> 24;
    
    serial_println!("[APIC] CPU {} Local APIC ready (ID: {})", cpu_id, apic_id);
}

pub unsafe fn end_of_interrupt() {
    if let Some(ref mut apic) = LOCAL_APIC {
        apic.end_of_interrupt();
    }
}
