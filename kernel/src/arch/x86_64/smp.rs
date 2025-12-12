// src/arch/x86_64/smp.rs
//! SMP (Symmetric Multi-Processing) using Limine's built-in SMP support

use super::cpu::{CpuId, register_ap, set_gs_base, get_gs_base_for_cpu};
use super::apic::ApicId;
use core::sync::atomic::{AtomicU32, AtomicBool, Ordering};
use core::arch::asm;

// AP startup synchronization
static AP_STARTED_COUNT: AtomicU32 = AtomicU32::new(0);
static AP_READY: AtomicBool = AtomicBool::new(false);

/// Initialize SMP system using Limine's SMP support
pub fn init_smp() -> Result<(), &'static str> {
    crate::serial_println!("[SMP] Initializing multiprocessor support via Limine...");
    
    // Get SMP response from Limine
    let smp_response = crate::SMP_REQUEST.get_response()
        .ok_or("Limine SMP request not supported")?;
    
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
    
    // Start all APs
    let mut started = 0u32;
    for cpu in cpus {
        if cpu.lapic_id == bsp_lapic_id {
            continue; // Skip BSP
        }
        
        crate::serial_println!("[SMP] Starting AP with LAPIC ID 0x{:x}", cpu.lapic_id);
        
        // Register the AP with our CPU tracking
        let cpu_id = register_ap(cpu.lapic_id)
            .ok_or("Failed to register AP")?;
        
        // Store CPU ID in the extra field for the AP to read
        cpu.extra.store(cpu_id.as_u8() as u64, Ordering::SeqCst);
        
        // Start the AP by writing to goto_address
        // Limine handles all the trampoline complexity!
        cpu.goto_address.write(ap_entry);
        
        started += 1;
    }
    
    // Wait for APs to start (with timeout)
    let mut timeout = 1000; // 1 second
    while AP_STARTED_COUNT.load(Ordering::Acquire) < started && timeout > 0 {
        for _ in 0..10000 { core::hint::spin_loop(); }
        timeout -= 1;
    }
    
    let actual_started = AP_STARTED_COUNT.load(Ordering::Acquire);
    crate::serial_println!("[SMP] Started {} of {} application processors", actual_started, started);
    
    Ok(())
}

/// AP entry point - called by Limine when AP starts
/// Limine provides: 64-bit mode, paging enabled, own stack, interrupts disabled
unsafe extern "C" fn ap_entry(cpu_info: &limine::mp::Cpu) -> ! {
    let lapic_id = cpu_info.lapic_id;
    let cpu_id_val = cpu_info.extra.load(Ordering::SeqCst) as u8;
    let cpu_id = CpuId::new(cpu_id_val);
    
    // Set GS base for per-CPU data
    let gs_base = get_gs_base_for_cpu(cpu_id);
    set_gs_base(gs_base);
    
    // Load GDT and IDT for this CPU (CRITICAL - must be before enabling interrupts)
    crate::interrupts::idt::init_gdt_and_tss();
    crate::interrupts::idt::init_idt();
    
    // Initialize Local APIC
    if let Err(e) = super::apic::init_apic() {
        crate::serial_println!("[SMP] CPU {} APIC init failed: {}", cpu_id_val, e);
    }
    
    // Don't setup timer on APs for now - let BSP handle scheduling
    // The timer interrupt would fire but we're not ready to handle it
    // super::apic::setup_timer(32, 1000000, true);
    
    // Signal that we're ready (before enabling interrupts)
    AP_STARTED_COUNT.fetch_add(1, Ordering::Release);
    
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
        // APs currently just idle
        // Future: integrate with scheduler for work stealing
    }
}

/// Get number of online CPUs
pub fn get_online_cpu_count() -> u32 {
    // BSP + APs started
    1 + AP_STARTED_COUNT.load(Ordering::Relaxed)
}