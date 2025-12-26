use crate::process::{Process, Priority};
use crate::scheduler;
use x86_64::VirtAddr;
use crate::serial_println;

/// Spawn a new kernel thread
/// 
/// # Arguments
/// * `entry_point` - Function to run in the new thread
/// * `priority` - Priority level for the thread
/// 
/// # Returns
/// ProcessId of the new thread
pub fn spawn_kernel_thread(entry_point: fn() -> !, priority: Priority) -> crate::process::ProcessId {
    // Allocate stack from pool
    let stack_top = {
        let mut pool = super::stack_pool::STACK_POOL.lock();
        pool.allocate().expect("Failed to allocate stack from pool")
    };
    
    // Convert entry point to VirtAddr
    let entry = VirtAddr::new(entry_point as *const () as usize as u64);
    
    // Create process
    let mut process = Process::new_kernel_thread(entry, stack_top);
    process.priority = priority;
    
    let pid = process.pid;
    
    serial_println!("[SPAWN] Created kernel thread PID {} at entry {:#x}", pid.as_u32(), entry.as_u64());
    
    // Add to scheduler
    if let Some(ref sched) = *scheduler::SCHEDULER.lock() {
        sched.add_process(process);
    }
    
    pid
}

/// Create idle process for a CPU
/// 
/// Idle process runs when no other process is ready
pub fn create_idle_process(cpu_id: u32) -> Process {
    extern "C" fn idle_loop() -> ! {
        loop {
            x86_64::instructions::hlt();
        }
    }
    
    let entry = VirtAddr::new(idle_loop as usize as u64);
    let stack = VirtAddr::new(0xDEAD_0000 + (cpu_id as u64 * 0x10000)); // Dummy stack for idle
    
    let mut process = Process::new_kernel_thread(entry, stack);
    process.priority = Priority::Low;
    process.cpu_affinity = Some(cpu_id);
    process.pid = crate::process::ProcessId::new(0); // PID 0 for idle
    
    serial_println!("[IDLE] Created idle process for CPU {}", cpu_id);
    
    process
}
