// src/process/scheduler.rs (SMP version with per-CPU run queues)

use super::{Pid, ProcessState, process_table, get_current_pid, set_current_pid};
use alloc::collections::VecDeque;
use crate::sync::IrqSpinlock;
use crate::arch::cpu::{get_cpu_id, get_cpu_count, CpuId};

const TIME_SLICE_MS: u64 = 10;

pub struct CpuRunQueue {
    queue: VecDeque<Pid>,
    current_time_slice: u64,
    idle_ticks: u64,
}

impl CpuRunQueue {
    pub const fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            current_time_slice: 0,
            idle_ticks: 0,
        }
    }
}

// Per-CPU run queues (fixed allocation)
static CPU_RUN_QUEUES: [IrqSpinlock<CpuRunQueue>; 256] = {
    const INIT: IrqSpinlock<CpuRunQueue> = IrqSpinlock::new(CpuRunQueue::new());
    [INIT; 256]
};

// Global ready queue for load balancing
static GLOBAL_READY_QUEUE: IrqSpinlock<VecDeque<Pid>> = IrqSpinlock::new(VecDeque::new());

/// Add process to scheduler (picks least loaded CPU)
pub fn add_to_scheduler(pid: Pid) {
    let cpu_count = get_cpu_count() as usize;
    
    // Find least loaded CPU
    let mut min_load = usize::MAX;
    let mut target_cpu = 0;
    
    for cpu in 0..cpu_count {
        let queue = CPU_RUN_QUEUES[cpu].lock();
        let load = queue.queue.len();
        if load < min_load {
            min_load = load;
            target_cpu = cpu;
        }
    }
    
    // Add to target CPU's run queue
    let mut queue = CPU_RUN_QUEUES[target_cpu].lock();
    if !queue.queue.contains(&pid) {
        queue.queue.push_back(pid);
    }
}

/// Remove process from scheduler
pub fn remove_from_scheduler(pid: Pid) {
    let cpu_count = get_cpu_count() as usize;
    
    // Remove from all CPU run queues
    for cpu in 0..cpu_count {
        let mut queue = CPU_RUN_QUEUES[cpu].lock();
        queue.queue.retain(|&p| p != pid);
    }
    
    // Remove from global queue
    let mut global = GLOBAL_READY_QUEUE.lock();
    global.retain(|&p| p != pid);
}

/// Schedule next process on current CPU
pub fn schedule() -> Option<Pid> {
    let cpu_id = get_cpu_id();
    let mut queue = CPU_RUN_QUEUES[cpu_id.as_usize()].lock();
    
    // Decrement time slice
    if queue.current_time_slice > 0 {
        queue.current_time_slice -= 1;
        
        // Current process still has time
        if queue.current_time_slice > 0 {
            if let Some(current) = get_current_pid() {
                let table = process_table().lock();
                if let Some(proc) = table.get(current) {
                    if proc.state == ProcessState::Running {
                        return Some(current);
                    }
                }
            }
        }
    }
    
    // Need to pick next process
    if let Some(next_pid) = queue.queue.pop_front() {
        let table = process_table().lock();
        if let Some(proc) = table.get(next_pid) {
            if proc.state == ProcessState::Ready {
                queue.current_time_slice = TIME_SLICE_MS;
                queue.queue.push_back(next_pid);
                drop(table);
                return Some(next_pid);
            }
        }
    }
    
    // Try to steal work from other CPUs
    let cpu_count = get_cpu_count() as usize;
    for other_cpu in 0..cpu_count {
        if other_cpu == cpu_id.as_usize() {
            continue;
        }
        
        let mut other_queue = CPU_RUN_QUEUES[other_cpu].lock();
        if other_queue.queue.len() > 1 {
            // Steal half the work
            let steal_count = other_queue.queue.len() / 2;
            for _ in 0..steal_count {
                if let Some(pid) = other_queue.queue.pop_back() {
                    queue.queue.push_back(pid);
                }
            }
            
            if let Some(stolen_pid) = queue.queue.pop_front() {
                queue.current_time_slice = TIME_SLICE_MS;
                queue.queue.push_back(stolen_pid);
                return Some(stolen_pid);
            }
        }
    }
    
    None
}

/// Yield CPU to next process
pub fn yield_cpu() {
    if let Some(current) = get_current_pid() {
        let cpu_id = get_cpu_id();
        let mut queue = CPU_RUN_QUEUES[cpu_id.as_usize()].lock();
        
        // Move current process to back of queue
        queue.queue.retain(|&p| p != current);
        queue.queue.push_back(current);
        queue.current_time_slice = 0;
    }
}

/// Get statistics for specific CPU
pub fn get_cpu_stats(cpu_id: CpuId) -> (usize, u64) {
    let queue = CPU_RUN_QUEUES[cpu_id.as_usize()].lock();
    (queue.queue.len(), queue.idle_ticks)
}

/// Load balancing (called periodically)
pub fn balance_load() {
    let cpu_count = get_cpu_count() as usize;
    
    // Calculate average load
    let mut total_load = 0;
    for cpu in 0..cpu_count {
        let queue = CPU_RUN_QUEUES[cpu].lock();
        total_load += queue.queue.len();
    }
    
    if total_load == 0 {
        return;
    }
    
    let avg_load = total_load / cpu_count;
    
    // Balance queues
    for cpu in 0..cpu_count {
        let mut queue = CPU_RUN_QUEUES[cpu].lock();
        
        while queue.queue.len() > avg_load + 1 {
            if let Some(pid) = queue.queue.pop_back() {
                // Find least loaded CPU
                let mut min_load = usize::MAX;
                let mut target_cpu = 0;
                
                for other_cpu in 0..cpu_count {
                    if other_cpu == cpu {
                        continue;
                    }
                    
                    let other_queue = CPU_RUN_QUEUES[other_cpu].lock();
                    let load = other_queue.queue.len();
                    if load < min_load {
                        min_load = load;
                        target_cpu = other_cpu;
                    }
                }
                
                // Move to target CPU
                let mut target_queue = CPU_RUN_QUEUES[target_cpu].lock();
                target_queue.queue.push_back(pid);
            }
        }
    }
}