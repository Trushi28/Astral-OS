// src/arch/x86_64/ioapic.rs

use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

const IOAPIC_REG_ID: u8 = 0x00;
const IOAPIC_REG_VER: u8 = 0x01;
const IOAPIC_REG_ARB: u8 = 0x02;
const IOAPIC_REDTBL_BASE: u8 = 0x10;

static IOAPIC_BASE: AtomicU64 = AtomicU64::new(0);
static IOAPIC_GSI_BASE: AtomicU64 = AtomicU64::new(0);
static IOAPIC_MAX_REDIRECTS: AtomicU64 = AtomicU64::new(0);
static IOAPIC_LOCK: Mutex<()> = Mutex::new(());

pub fn init_ioapic(phys_addr: u64, gsi_base: u32) {
    let hhdm = crate::get_hhdm_offset() as u64;
    let virt_addr = phys_addr + hhdm;
    
    IOAPIC_BASE.store(virt_addr, Ordering::SeqCst);
    IOAPIC_GSI_BASE.store(gsi_base as u64, Ordering::SeqCst);
    
    unsafe {
        let version = ioapic_read(IOAPIC_REG_VER);
        let max_redirects = ((version >> 16) & 0xFF) + 1;
        IOAPIC_MAX_REDIRECTS.store(max_redirects as u64, Ordering::SeqCst);
        
        crate::serial_println!("[IOAPIC] Initialized at 0x{:x}, GSI base {}, {} redirects",
            phys_addr, gsi_base, max_redirects);
        
        // Mask all IRQs initially
        for i in 0..max_redirects {
            ioapic_mask_irq(i as u8);
        }
    }
}

pub fn ioapic_set_irq(irq: u8, vector: u8, apic_id: u32, active_low: bool, level_triggered: bool) {
    let _lock = IOAPIC_LOCK.lock();
    
    unsafe {
        let mut low: u32 = vector as u32;
        let high: u32 = apic_id << 24;
        
        if active_low {
            low |= 1 << 13;
        }
        
        if level_triggered {
            low |= 1 << 15;
        }
        
        // Unmask by clearing bit 16
        // low &= !(1 << 16);
        
        let redtbl = IOAPIC_REDTBL_BASE + (irq * 2);
        ioapic_write(redtbl, low);
        ioapic_write(redtbl + 1, high);
    }
}

pub fn ioapic_mask_irq(irq: u8) {
    let _lock = IOAPIC_LOCK.lock();
    
    unsafe {
        let redtbl = IOAPIC_REDTBL_BASE + (irq * 2);
        let mut low = ioapic_read(redtbl);
        low |= 1 << 16; // Set mask bit
        ioapic_write(redtbl, low);
    }
}

pub fn ioapic_unmask_irq(irq: u8) {
    let _lock = IOAPIC_LOCK.lock();
    
    unsafe {
        let redtbl = IOAPIC_REDTBL_BASE + (irq * 2);
        let mut low = ioapic_read(redtbl);
        low &= !(1 << 16); // Clear mask bit
        ioapic_write(redtbl, low);
    }
}

unsafe fn ioapic_read(reg: u8) -> u32 {
    let base = IOAPIC_BASE.load(Ordering::Relaxed) as *mut u32;
    write_volatile(base, reg as u32);
    read_volatile(base.add(4))
}

unsafe fn ioapic_write(reg: u8, value: u32) {
    let base = IOAPIC_BASE.load(Ordering::Relaxed) as *mut u32;
    write_volatile(base, reg as u32);
    write_volatile(base.add(4), value);
}