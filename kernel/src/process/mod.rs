use core::sync::atomic::{AtomicU32, Ordering};
use x86_64::{VirtAddr, structures::paging::PageTable};

pub mod context;
pub mod switch;
pub mod spawn;
pub mod stack_pool;

/// Process ID type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessId(u32);

impl ProcessId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
    
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

/// Next available PID
static NEXT_PID: AtomicU32 = AtomicU32::new(1);

/// Allocate a new PID
pub fn alloc_pid() -> ProcessId {
    ProcessId(NEXT_PID.fetch_add(1, Ordering::Relaxed))
}

/// Process state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Ready,      // Ready to run
    Running,    // Currently executing
    Blocked,    // Waiting for I/O or event
    Dead,       // Terminated
}

/// Priority level (for work-stealing scheduler)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    High = 2,      // Interactive, 5ms time slice
    Normal = 1,    // Default, 10ms time slice
    Low = 0,       // Batch/background, 20ms time slice
}

/// Task Control Block (Process descriptor)
#[derive(Clone)]
pub struct Process {
    /// Process ID
    pub pid: ProcessId,
    
    /// Current state
    pub state: ProcessState,
    
    /// Saved CPU context
    pub context: context::Context,
    
    /// Page table physical address (CR3) - for userspace processes
    pub page_table: Option<u64>,  // PhysAddr as u64 (Send-safe)
    
    /// Kernel stack pointer
    pub kernel_stack: VirtAddr,
    
    /// User stack pointer (for Ring 3)
    pub user_stack: Option<VirtAddr>,
    
    /// Priority level
    pub priority: Priority,
    
    /// CPU affinity (which CPU owns this process)
    pub cpu_affinity: Option<u32>,
    
    /// Time slice remaining (in timer ticks)
    pub time_slice: u32,
    
    /// Total CPU time used (for statistics)
    pub cpu_time: u64,
}

impl Process {
    /// Create a new kernel thread
    pub fn new_kernel_thread(entry_point: VirtAddr, stack: VirtAddr) -> Self {
        Self {
            pid: alloc_pid(),
            state: ProcessState::Ready,
            context: context::Context::new_kernel(entry_point, stack),
            page_table: None, // Kernel threads share kernel page table
            kernel_stack: stack,
            user_stack: None,
            priority: Priority::Normal,
            cpu_affinity: None,
            time_slice: 10, // Default 10 ticks
            cpu_time: 0,
        }
    }
    
    /// Get time slice for priority level
    pub fn get_time_slice(&self) -> u32 {
        match self.priority {
            Priority::High => 5,
            Priority::Normal => 10,
            Priority::Low => 20,
        }
    }
    
    /// Reset time slice
    pub fn reset_time_slice(&mut self) {
        self.time_slice = self.get_time_slice();
    }
}
