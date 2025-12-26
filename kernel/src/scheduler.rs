use alloc::collections::VecDeque;
use alloc::vec::Vec;
use spin::Mutex;
use crate::process::{Process, ProcessId, ProcessState, Priority};
use crate::serial_println;

/// Per-CPU run queue
struct CpuQueue {
    high: VecDeque<Process>,      // High priority (5ms slice)
    normal: VecDeque<Process>,    // Normal priority (10ms slice)
    low: VecDeque<Process>,       // Low priority (20ms slice)
    total_count: usize,
}

impl CpuQueue {
    fn new() -> Self {
        Self {
            high: VecDeque::new(),
            normal: VecDeque::new(),
            low: VecDeque::new(),
            total_count: 0,
        }
    }
    
    fn push(&mut self, mut process: Process) {
        process.state = ProcessState::Ready;
        match process.priority {
            Priority::High => self.high.push_back(process),
            Priority::Normal => self.normal.push_back(process),
            Priority::Low => self.low.push_back(process),
        }
        self.total_count += 1;
    }
    
    fn pop(&mut self) -> Option<Process> {
        // Try high priority first
        if let Some(mut proc) = self.high.pop_front() {
            proc.state = ProcessState::Running;
            proc.reset_time_slice();
            self.total_count -= 1;
            return Some(proc);
        }
        
        // Then normal priority
        if let Some(mut proc) = self.normal.pop_front() {
            proc.state = ProcessState::Running;
            proc.reset_time_slice();
            self.total_count -= 1;
            return Some(proc);
        }
        
        // Finally low priority
        if let Some(mut proc) = self.low.pop_front() {
            proc.state = ProcessState::Running;
            proc.reset_time_slice();
            self.total_count -= 1;
            return Some(proc);
        }
        
        None
    }
    
    fn len(&self) -> usize {
        self.total_count
    }
    
    fn is_empty(&self) -> bool {
        self.total_count == 0
    }
}

/// Global scheduler state
pub struct Scheduler {
    /// Per-CPU queues
    cpu_queues: Vec<Mutex<CpuQueue>>,
    
    /// Number of CPUs
    cpu_count: usize,
    
    /// Currently running process per CPU (None if idle)
    current_process: Vec<Mutex<Option<Process>>>,
}

impl Scheduler {
    /// Create a new scheduler for the given number of CPUs
    pub fn new(cpu_count: usize) -> Self {
        let mut cpu_queues = Vec::new();
        let mut current_process = Vec::new();
        
        for _ in 0..cpu_count {
            cpu_queues.push(Mutex::new(CpuQueue::new()));
            current_process.push(Mutex::new(None));
        }
        
        Self {
            cpu_queues,
            cpu_count,
            current_process,
        }
    }
    
    /// Add a process to the scheduler
    pub fn add_process(&self, process: Process) {
        // Simple load balancing: add to least loaded CPU
        let mut min_cpu = 0;
        let mut min_count = usize::MAX;
        
        for (i, queue) in self.cpu_queues.iter().enumerate() {
            let count = queue.lock().len();
            if count < min_count {
                min_count = count;
                min_cpu = i;
            }
        }
        
        self.cpu_queues[min_cpu].lock().push(process);
    }
    
    /// Schedule next process on given CPU
    /// Returns None if no process is ready (CPU should idle)
    pub fn schedule(&self, cpu_id: usize) -> Option<Process> {
        if cpu_id >= self.cpu_count {
            return None;
        }
        
        // Try local queue first
        if let Some(process) = self.cpu_queues[cpu_id].lock().pop() {
            return Some(process);
        }
        
        // Work stealing: find busiest CPU and steal from it
        let mut max_cpu = None;
        let mut max_count = 0;
        
        for (i, queue) in self.cpu_queues.iter().enumerate() {
            if i == cpu_id {
                continue; // Skip self
            }
            let count = queue.lock().len();
            if count > max_count {
                max_count = count;
                max_cpu = Some(i);
            }
        }
        
        // Steal if busiest CPU has > 1 process
        if let Some(steal_from) = max_cpu {
            if max_count > 1 {
                // Steal one process
                if let Some(process) = self.cpu_queues[steal_from].lock().pop() {
                    serial_println!("[SCHED] CPU {} stole process from CPU {}", cpu_id, steal_from);
                    return Some(process);
                }
            }
        }
        
        None
    }
    
    /// Save current process back to queue (for preemption)
    pub fn reschedule(&self, cpu_id: usize, mut process: Process) {
        // Demote priority if time slice exhausted
        if process.time_slice == 0 {
            process.priority = match process.priority {
                Priority::High => Priority::Normal,  // Demote high to normal
                Priority::Normal => Priority::Low,   // Demote normal to low
                Priority::Low => Priority::Low,      // Keep low at low
            };
        }
        
        self.cpu_queues[cpu_id].lock().push(process);
    }
    
    /// Voluntary yield - boost priority
    pub fn yield_cpu(&self, cpu_id: usize, mut process: Process) {
        // Boost priority for being cooperative
        process.priority = match process.priority {
            Priority::High => Priority::High,      // Keep high
            Priority::Normal => Priority::High,    // Boost to high
            Priority::Low => Priority::Normal,     // Boost to normal
        };
        
        self.cpu_queues[cpu_id].lock().push(process);
    }
    
    /// Get current process on CPU
    pub fn current(&self, cpu_id: usize) -> Option<Process> {
        self.current_process[cpu_id].lock().clone()
    }
    
    /// Set current process on CPU
    pub fn set_current(&self, cpu_id: usize, process: Option<Process>) {
        *self.current_process[cpu_id].lock() = process;
    }
}

/// Global scheduler instance
pub static SCHEDULER: Mutex<Option<Scheduler>> = Mutex::new(None);

/// Initialize scheduler with CPU count
pub fn init(cpu_count: usize) {
    *SCHEDULER.lock() = Some(Scheduler::new(cpu_count));
    serial_println!("[SCHED] Work-stealing scheduler initialized for {} CPUs", cpu_count);
}

/// Get the scheduler
pub fn get() -> &'static Mutex<Option<Scheduler>> {
    &SCHEDULER
}

/// Timer tick - called from timer interrupt
/// Decrements time slice and triggers rescheduling if needed
pub fn timer_tick() {
    // Single-CPU tracking (multicore requires APIC ID lookup)
    let cpu_id = 0;
    
    if let Some(ref sched) = *SCHEDULER.lock() {
        // Get current process
        if let Some(mut current) = sched.current(cpu_id) {
            // Decrement time slice
            if current.time_slice > 0 {
                current.time_slice -= 1;
            }
            
            // If time slice expired, reschedule
            if current.time_slice == 0 {
                // Put back in queue (will demote if CPU hog)
                sched.reschedule(cpu_id, current);
                
                // Rescheduling tracked; context switch on next schedule() call
            } else {
                // Update current process
                sched.set_current(cpu_id, Some(current));
            }
        }
    }
}
