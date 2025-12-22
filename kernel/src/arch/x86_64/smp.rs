// src/arch/x86_64/smp.rs
//! SMP (Symmetric Multi-Processing) using Limine's built-in SMP support

use super::cpu::{CpuId, register_ap, set_gs_base, get_gs_base_for_cpu};
use core::sync::atomic::{AtomicU32, AtomicBool, Ordering};
use core::arch::asm;

// AP startup synchronization
static AP_STARTED_COUNT: AtomicU32 = AtomicU32::new(0);
static AP_INIT_LOCK: AtomicBool = AtomicBool::new(false);
static CURRENT_AP_READY: AtomicBool = AtomicBool::new(false);

/// Initialize SMP system using Limine's SMP support
pub fn init_smp() -> Result<(), &'static str> {
    crate::serial_println!("[SMP] Initializing multiprocessor support via Limine...");
    
    // Get SMP response from Limine
    let smp_response = crate::MP_REQUEST.get_response()
        .ok_or("Limine MP request not supported")?;
    
    let cpus = smp_response.cpus();
    let cpu_count = cpus.len();
    
    crate::serial_println!("[SMP] Limine reports {} CPUs", cpu_count);
    
    if cpu_count <= 1 {
        crate::serial_println!("[SMP] Only BSP found, no APs to start");
        return Ok(());
    }
    
    // Initialize IOAPIC using MADT
    unsafe {
        if let Some(madt) = crate::acpi::madt::parse_madt() {
            super::ioapic::init_ioapic(madt.ioapic_addr, madt.ioapic_gsi_base);
        }
    }
    
    // Get BSP LAPIC ID
    let bsp_lapic_id = smp_response.bsp_lapic_id();
    crate::serial_println!("[SMP] BSP LAPIC ID: 0x{:x}", bsp_lapic_id);
    
    // Start APs ONE AT A TIME with synchronization to avoid race conditions
    let mut started = 0u32;
    for cpu in cpus {
        if cpu.lapic_id == bsp_lapic_id {
            continue; // Skip BSP
        }
        
        crate::serial_println!("[SMP] Starting AP with LAPIC ID 0x{:x}", cpu.lapic_id);
        
        // Register the AP with our CPU tracking
        let cpu_id = register_ap(cpu.lapic_id)
            .ok_or("Failed to register AP")?;
        
        // Clear ready flag before starting this AP
        CURRENT_AP_READY.store(false, Ordering::SeqCst);
        
        // Store CPU ID in the extra field for the AP to read
        cpu.extra.store(cpu_id.as_u8() as u64, Ordering::SeqCst);
        
        // Memory fence to ensure all stores are visible
        core::sync::atomic::fence(Ordering::SeqCst);
        
        // Start the AP by writing to goto_address
        cpu.goto_address.write(ap_entry);
        
        // Wait for THIS AP to signal ready before starting next
        let mut timeout = 10000;
        while !CURRENT_AP_READY.load(Ordering::Acquire) && timeout > 0 {
            for _ in 0..100 { core::hint::spin_loop(); }
            timeout -= 1;
        }
        
        if timeout == 0 {
            crate::serial_println!("[SMP] WARNING: AP 0x{:x} startup timeout", cpu.lapic_id);
        }
        
        started += 1;
    }
    
    let actual_started = AP_STARTED_COUNT.load(Ordering::Acquire);
    crate::serial_println!("[SMP] Started {} of {} application processors", actual_started, started);
    
    Ok(())
}

/// AP entry point - called by Limine when AP starts
/// Limine provides: 64-bit mode, paging enabled, own stack, interrupts disabled
unsafe extern "C" fn ap_entry(cpu_info: &limine::mp::Cpu) -> ! {
    // Acquire init lock - only one AP initializes at a time
    while AP_INIT_LOCK.compare_exchange(
        false, true, Ordering::Acquire, Ordering::Relaxed
    ).is_err() {
        core::hint::spin_loop();
    }
    
    let lapic_id = cpu_info.lapic_id;
    let cpu_id_val = cpu_info.extra.load(Ordering::SeqCst) as u8;
    let cpu_id = CpuId::new(cpu_id_val);
    
    // Set GS base for per-CPU data
    let gs_base = get_gs_base_for_cpu(cpu_id);
    set_gs_base(gs_base);
    
    // Load GDT and IDT for this CPU (CRITICAL - must be before enabling interrupts)
    crate::interrupts::idt::init_gdt_and_tss();
    crate::interrupts::idt::init_idt();
    
    // Initialize syscall MSRs for this CPU (per-CPU syscall stack)
    crate::arch::x86_64::syscall::init_for_cpu(cpu_id_val as usize);
    
    // Initialize Local APIC (DON'T print here - could cause deadlock!)
    let apic_result = super::apic::init_apic();
    
    // Signal that we're ready FIRST (before any serial output to avoid deadlock!)
    AP_STARTED_COUNT.fetch_add(1, Ordering::Release);
    CURRENT_AP_READY.store(true, Ordering::Release);
    
    // Release init lock so next AP can initialize
    AP_INIT_LOCK.store(false, Ordering::Release);
    
    // NOW safe to print - BSP is no longer blocked waiting for us
    if let Err(e) = apic_result {
        crate::serial_println!("[SMP] CPU {} APIC init failed: {}", cpu_id_val, e);
    }
    crate::serial_println!("[SMP] CPU {} online (LAPIC 0x{:x})", cpu_id_val, lapic_id);
    
    // Enable interrupts now that IDT is loaded
    asm!("sti");
    
    // Enter idle loop
    ap_idle_loop()
}

/// AP idle loop - waits for IPIs 
fn ap_idle_loop() -> ! {
    loop {
        unsafe { asm!("hlt"); }
        // APs idle until scheduler sends them work via IPI
    }
}

/// Get number of online CPUs
pub fn get_online_cpu_count() -> u32 {
    1 + AP_STARTED_COUNT.load(Ordering::Relaxed)
}