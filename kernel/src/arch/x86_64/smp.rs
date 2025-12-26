use limine::request::SmpRequest;
use limine::smp::Cpu;
use crate::serial_println;
use core::sync::atomic::{AtomicU32, Ordering};

/// Limine SMP request
static SMP_REQUEST: SmpRequest = SmpRequest::new();

/// Number of CPUs detected
static CPU_COUNT: AtomicU32 = AtomicU32::new(0);

/// Initialize SMP (Symmetric Multiprocessing)
pub unsafe fn init() {
    serial_println!("[SMP] Initializing Symmetric Multiprocessing...");
    
    if let Some(smp_response) = SMP_REQUEST.get_response() {
        let cpu_count = smp_response.cpus().len();
        CPU_COUNT.store(cpu_count as u32, Ordering::Relaxed);
        
        serial_println!("[SMP] Detected {} CPU(s)", cpu_count);
        serial_println!("[SMP] BSP (Bootstrap Processor): CPU 0");
        
        // List all CPUs
        for (i, cpu) in smp_response.cpus().iter().enumerate() {
            let lapic_id = cpu.lapic_id;
            serial_println!("[SMP]   CPU #{}: LAPIC ID {}", i, lapic_id);
        }
        
        // Start Application Processors (APs)
        let ap_count = cpu_count - 1; // Subtract BSP
        if ap_count > 0 {
            serial_println!("[SMP] Starting {} Application Processor(s)...", ap_count);
            
            for (i, cpu) in smp_response.cpus().iter().enumerate() {
                if i == 0 {
                    // Skip BSP (we're already running on it)
                    continue;
                }
                
                // Set the goto address for the AP
                // The AP will jump to this function when it wakes up
                let entry_fn: extern "C" fn(&Cpu) -> ! = ap_entry;
                cpu.goto_address.write(entry_fn);
                
                serial_println!("[SMP] CPU #{} started", i);
            }
            
            serial_println!("[SMP] All APs initialized");
        } else {
            serial_println!("[SMP] Single-core system (no APs to start)");
        }
    } else {
        serial_println!("[SMP] WARNING: No SMP response from bootloader");
        serial_println!("[SMP] Running in single-core mode");
        CPU_COUNT.store(1, Ordering::Relaxed);
    }
}

/// Entry point for Application Processors (APs)
/// 
/// This function is called by each AP when it wakes up
extern "C" fn ap_entry(cpu_info: &Cpu) -> ! {
    let cpu_id = cpu_info.lapic_id;
    
    serial_println!("[SMP] AP {} online! LAPIC ID: {}", cpu_id, cpu_id);
    
    // Initialize Local APIC for this CPU
    unsafe {
        super::apic::local::init_ap(cpu_id);
    }
    
    serial_println!("[SMP] AP {} initialized", cpu_id);
    
    // AP idle loop
    // In Phase 4, this will be replaced with the scheduler
    loop {
        x86_64::instructions::hlt();
    }
}

/// Get the number of CPUs
pub fn cpu_count() -> u32 {
    CPU_COUNT.load(Ordering::Relaxed)
}

/// Check if we're running on the BSP
pub fn is_bsp() -> bool {
    // For now, assume CPU 0 is always BSP
    // In a more complete implementation, we'd check the APIC ID
    true
}
