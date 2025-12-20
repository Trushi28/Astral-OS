// src/process/scheduler.rs - AstralScheduler
#![allow(dead_code)] // Scheduler constants for future use
//! Reality-aware scheduler for Astral OS
//!
//! Features:
//! - Priority-based scheduling with 5 classes
//! - O(1) dispatch using priority bitmask
//! - Intent-aware scheduling
//! - Reality branch tracking
//! - Dream-mode background optimization
//! - Per-CPU run queues with work stealing

use super::{Pid, PriorityClass, ProcessState, ProcessIntent, process_table, get_current_pid};
use alloc::collections::VecDeque;
use crate::sync::IrqSpinlock;
use crate::arch::cpu::{get_cpu_id, get_cpu_count, CpuId};
use core::sync::atomic::{AtomicU64, AtomicBool, Ordering};

/// Number of priority levels
const NUM_PRIORITIES: usize = 5;

/// Default time slices per priority (in ticks)
const DEFAULT_TIME_SLICES: [u64; NUM_PRIORITIES] = [20, 5, 10, 15, 50];

/// Current system state (for dream mode detection)
static SYSTEM_IDLE: AtomicBool = AtomicBool::new(false);
static IDLE_TICKS: AtomicU64 = AtomicU64::new(0);

/// Per-priority run queue
pub struct PriorityQueue {
    queue: VecDeque<Pid>,
    active: bool,
}

impl PriorityQueue {
    pub const fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            active: false,
        }
    }
    
    fn push(&mut self, pid: Pid) {
        if !self.queue.contains(&pid) {
            self.queue.push_back(pid);
            self.active = true;
        }
    }
    
    fn pop(&mut self) -> Option<Pid> {
        let result = self.queue.pop_front();
        if self.queue.is_empty() {
            self.active = false;
        }
        result
    }
    
    fn remove(&mut self, pid: Pid) {
        self.queue.retain(|&p| p != pid);
        if self.queue.is_empty() {
            self.active = false;
        }
    }
    
    fn len(&self) -> usize {
        self.queue.len()
    }
    
    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

/// Per-CPU scheduler state
pub struct CpuScheduler {
    /// Priority queues (one per priority class)
    queues: [PriorityQueue; NUM_PRIORITIES],
    /// Bitmap of active priority levels (for O(1) lookup)
    active_bitmap: u8,
    /// Current running process
    current: Option<Pid>,
    /// Remaining time slice for current process
    time_slice: u64,
    /// Statistics
    context_switches: u64,
    idle_ticks: u64,
}

impl CpuScheduler {
    pub const fn new() -> Self {
        Self {
            queues: [
                PriorityQueue::new(),
                PriorityQueue::new(),
                PriorityQueue::new(),
                PriorityQueue::new(),
                PriorityQueue::new(),
            ],
            active_bitmap: 0,
            current: None,
            time_slice: 0,
            context_switches: 0,
            idle_ticks: 0,
        }
    }
    
    /// Add process to appropriate priority queue
    fn enqueue(&mut self, pid: Pid, priority: PriorityClass) {
        let idx = priority as usize;
        self.queues[idx].push(pid);
        self.active_bitmap |= 1 << idx;
    }
    
    /// Remove process from all queues
    fn dequeue(&mut self, pid: Pid) {
        for (idx, queue) in self.queues.iter_mut().enumerate() {
            queue.remove(pid);
            if queue.is_empty() {
                self.active_bitmap &= !(1 << idx);
            }
        }
    }
    
    /// O(1) pick highest priority ready process
    fn pick_next(&mut self) -> Option<Pid> {
        if self.active_bitmap == 0 {
            return None;
        }
        
        // Find highest priority (lowest bit set)
        let highest_priority = self.active_bitmap.trailing_zeros() as usize;
        
        if highest_priority >= NUM_PRIORITIES {
            return None;
        }
        
        // Check dream mode - only run dream processes if system is idle
        if highest_priority == PriorityClass::Dream as usize {
            if !SYSTEM_IDLE.load(Ordering::Relaxed) {
                // System not idle, skip dream processes
                return None;
            }
        }
        
        if let Some(pid) = self.queues[highest_priority].pop() {
            // Update bitmap if queue is now empty
            if self.queues[highest_priority].is_empty() {
                self.active_bitmap &= !(1 << highest_priority);
            }
            Some(pid)
        } else {
            None
        }
    }
    
    /// Get total queue length
    fn total_load(&self) -> usize {
        self.queues.iter().map(|q| q.len()).sum()
    }
}

// Per-CPU scheduler instances (max 256 CPUs)
static CPU_SCHEDULERS: [IrqSpinlock<CpuScheduler>; 256] = {
    const INIT: IrqSpinlock<CpuScheduler> = IrqSpinlock::new(CpuScheduler::new());
    [INIT; 256]
};

// Global statistics
static TOTAL_CONTEXT_SWITCHES: AtomicU64 = AtomicU64::new(0);

/// Add process to scheduler
pub fn add_to_scheduler(pid: Pid) {
    add_to_scheduler_with_priority(pid, PriorityClass::Normal);
}

/// Add process to scheduler with specific priority
pub fn add_to_scheduler_with_priority(pid: Pid, priority: PriorityClass) {
    let cpu_count = get_cpu_count() as usize;
    
    // Find least loaded CPU
    let mut min_load = usize::MAX;
    let mut target_cpu = 0;
    
    for cpu in 0..cpu_count {
        let sched = CPU_SCHEDULERS[cpu].lock();
        let load = sched.total_load();
        if load < min_load {
            min_load = load;
            target_cpu = cpu;
        }
    }
    
    // Check CPU affinity from process table
    {
        let table = process_table().lock();
        if let Some(proc) = table.get(pid) {
            if proc.cpu_affinity != 0 {
                // Process has CPU preference
                for cpu in 0..cpu_count {
                    if proc.cpu_affinity & (1 << cpu) != 0 {
                        target_cpu = cpu;
                        break;
                    }
                }
            }
        }
    }
    
    // Add to target CPU's scheduler
    let mut sched = CPU_SCHEDULERS[target_cpu].lock();
    sched.enqueue(pid, priority);
}

/// Remove process from scheduler
pub fn remove_from_scheduler(pid: Pid) {
    let cpu_count = get_cpu_count() as usize;
    
    for cpu in 0..cpu_count {
        let mut sched = CPU_SCHEDULERS[cpu].lock();
        sched.dequeue(pid);
    }
}

/// Main scheduling function - called on timer tick
pub fn schedule() -> Option<Pid> {
    let cpu_id = get_cpu_id();
    let mut sched = CPU_SCHEDULERS[cpu_id.as_usize()].lock();
    
    // Decrement time slice
    if sched.time_slice > 0 {
        sched.time_slice -= 1;
        
        // Current process still has time
        if sched.time_slice > 0 {
            if let Some(current) = sched.current {
                // Check process is still running
                let table = process_table().lock();
                if let Some(proc) = table.get(current) {
                    if proc.state == ProcessState::Running {
                        return Some(current);
                    }
                }
            }
        }
    }
    
    // Re-add current process to queue if still runnable
    if let Some(current) = sched.current {
        let table = process_table().lock();
        if let Some(proc) = table.get(current) {
            if proc.state == ProcessState::Running || proc.state == ProcessState::Ready {
                sched.enqueue(current, proc.priority);
            }
        }
    }
    
    // Pick next process (O(1) operation)
    if let Some(next_pid) = sched.pick_next() {
        let table = process_table().lock();
        if let Some(proc) = table.get(next_pid) {
            sched.current = Some(next_pid);
            sched.time_slice = proc.time_slice;
            sched.context_switches += 1;
            TOTAL_CONTEXT_SWITCHES.fetch_add(1, Ordering::Relaxed);
            
            // Update idle detection
            IDLE_TICKS.store(0, Ordering::Relaxed);
            SYSTEM_IDLE.store(false, Ordering::Relaxed);
            
            drop(table);
            return Some(next_pid);
        }
    }
    
    // No process to run - system is idle
    sched.idle_ticks += 1;
    sched.current = None;
    
    // Update global idle tracking
    let idle = IDLE_TICKS.fetch_add(1, Ordering::Relaxed);
    if idle > 100 {
        SYSTEM_IDLE.store(true, Ordering::Relaxed);
    }
    
    None
}

/// Yield CPU to next process
pub fn yield_cpu() {
    if let Some(current) = get_current_pid() {
        let cpu_id = get_cpu_id();
        let mut sched = CPU_SCHEDULERS[cpu_id.as_usize()].lock();
        
        // Reset time slice to force reschedule
        sched.time_slice = 0;
        
        // Re-add to queue
        let table = process_table().lock();
        if let Some(proc) = table.get(current) {
            sched.enqueue(current, proc.priority);
        }
    }
}

/// Set process priority
pub fn set_priority(pid: Pid, priority: PriorityClass) {
    // Remove from current queue and re-add with new priority
    remove_from_scheduler(pid);
    
    // Update process priority
    {
        let mut table = process_table().lock();
        if let Some(proc) = table.get_mut(pid) {
            proc.priority = priority;
            proc.time_slice = super::Process::time_slice_for_priority(priority);
        }
    }
    
    // Re-add with new priority
    add_to_scheduler_with_priority(pid, priority);
}

/// Set process intent (for intent-aware scheduling)
pub fn set_intent(pid: Pid, intent: ProcessIntent) {
    let mut table = process_table().lock();
    if let Some(proc) = table.get_mut(pid) {
        proc.set_intent(intent);
    }
}

/// Work stealing for load balancing
pub fn balance_load() {
    let cpu_count = get_cpu_count() as usize;
    if cpu_count <= 1 {
        return;
    }
    
    // Calculate average load
    let mut total_load = 0;
    for cpu in 0..cpu_count {
        let sched = CPU_SCHEDULERS[cpu].lock();
        total_load += sched.total_load();
    }
    
    if total_load == 0 {
        return;
    }
    
    let avg_load = total_load / cpu_count;
    
    // Balance queues
    for cpu in 0..cpu_count {
        let load = {
            let sched = CPU_SCHEDULERS[cpu].lock();
            sched.total_load()
        };
        
        if load > avg_load + 1 {
            // This CPU is overloaded, migrate some processes
            let mut sched = CPU_SCHEDULERS[cpu].lock();
            
            // Steal from lowest priority queues first
            for priority_idx in (0..NUM_PRIORITIES).rev() {
                while sched.queues[priority_idx].len() > 0 && load > avg_load {
                    if let Some(pid) = sched.queues[priority_idx].pop() {
                        if sched.queues[priority_idx].is_empty() {
                            sched.active_bitmap &= !(1 << priority_idx);
                        }
                        
                        // Find least loaded CPU
                        drop(sched);
                        
                        let mut min_load = usize::MAX;
                        let mut target_cpu = 0;
                        for other_cpu in 0..cpu_count {
                            if other_cpu == cpu {
                                continue;
                            }
                            let other_sched = CPU_SCHEDULERS[other_cpu].lock();
                            let other_load = other_sched.total_load();
                            if other_load < min_load {
                                min_load = other_load;
                                target_cpu = other_cpu;
                            }
                        }
                        
                        // Add to target CPU
                        let priority = PriorityClass::try_from(priority_idx as u8)
                            .unwrap_or(PriorityClass::Normal);
                        let mut target_sched = CPU_SCHEDULERS[target_cpu].lock();
                        target_sched.enqueue(pid, priority);
                        
                        sched = CPU_SCHEDULERS[cpu].lock();
                    }
                }
            }
        }
    }
}

/// Get CPU statistics
pub fn get_cpu_stats(cpu_id: CpuId) -> (usize, u64, u64) {
    let sched = CPU_SCHEDULERS[cpu_id.as_usize()].lock();
    (sched.total_load(), sched.context_switches, sched.idle_ticks)
}

/// Get global scheduler statistics
pub fn get_global_stats() -> SchedulerStats {
    let cpu_count = get_cpu_count() as usize;
    
    let mut total_processes = 0;
    let mut total_switches = 0;
    let mut total_idle = 0;
    
    for cpu in 0..cpu_count {
        let sched = CPU_SCHEDULERS[cpu].lock();
        total_processes += sched.total_load();
        total_switches += sched.context_switches;
        total_idle += sched.idle_ticks;
    }
    
    SchedulerStats {
        total_processes,
        context_switches: total_switches,
        idle_ticks: total_idle,
        is_idle: SYSTEM_IDLE.load(Ordering::Relaxed),
    }
}

/// Check if system is in idle/dream mode
pub fn is_dream_mode() -> bool {
    SYSTEM_IDLE.load(Ordering::Relaxed)
}

/// Scheduler statistics
#[derive(Debug, Clone, Copy)]
pub struct SchedulerStats {
    pub total_processes: usize,
    pub context_switches: u64,
    pub idle_ticks: u64,
    pub is_idle: bool,
}

// Helper for priority conversion
impl TryFrom<u8> for PriorityClass {
    type Error = ();
    
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(PriorityClass::RealTime),
            1 => Ok(PriorityClass::Interactive),
            2 => Ok(PriorityClass::Normal),
            3 => Ok(PriorityClass::Background),
            4 => Ok(PriorityClass::Dream),
            _ => Err(()),
        }
    }
}