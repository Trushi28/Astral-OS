//src/process/mod.rs
pub mod scheduler;
pub mod context;

use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pid(u64);

static NEXT_PID: AtomicU64 = AtomicU64::new(1);

impl Pid {
    pub fn new() -> Self {
        Self(NEXT_PID.fetch_add(1, Ordering::SeqCst))
    }
    
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessState {
    Ready = 0,
    Running = 1,
    Blocked = 2,
    Zombie = 3,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Registers {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbx: u64,
    pub rbp: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rax: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

impl Registers {
    pub const fn new() -> Self {
        Self {
            r15: 0, r14: 0, r13: 0, r12: 0, rbx: 0, rbp: 0,
            r11: 0, r10: 0, r9: 0, r8: 0,
            rsi: 0, rdi: 0, rdx: 0, rcx: 0, rax: 0,
            rip: 0, cs: 0x08, rflags: 0x202, rsp: 0, ss: 0x10,
        }
    }
}

/// Process priority class for AstralScheduler
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum PriorityClass {
    /// Real-time priority - immediate execution, minimal preemption
    RealTime = 0,
    /// High priority - interactive processes, short time slices
    Interactive = 1,
    /// Normal priority - batch processes
    Normal = 2,
    /// Low priority - background tasks
    Background = 3,
    /// Dream priority - only runs during idle/dream state
    Dream = 4,
}

impl Default for PriorityClass {
    fn default() -> Self {
        PriorityClass::Normal
    }
}

/// Process intent declaration for intent-based scheduling
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessIntent {
    /// No specific intent
    None,
    /// CPU-bound computation
    Compute,
    /// I/O bound operation
    IoWait,
    /// Memory-intensive operation
    MemoryIntensive,
    /// Network communication
    Network,
    /// Graphics/UI operation
    Graphics,
    /// File system operation
    FileSystem,
    /// Reality branching operation
    RealityBranch,
    /// Interactive user-facing operation
    Interactive,
}

impl Default for ProcessIntent {
    fn default() -> Self {
        ProcessIntent::None
    }
}

#[derive(Clone, Copy)]
pub struct Process {
    pub pid: Pid,
    pub state: ProcessState,
    pub registers: Registers,
    pub page_table: u64,
    pub kernel_stack: u64,
    // Ring 3 userspace fields
    pub user_stack: u64,        // User-mode stack top
    pub is_user_process: bool,  // True if this runs in Ring 3
    // AstralScheduler fields
    pub priority: PriorityClass,
    pub intent: ProcessIntent,
    pub reality_id: u64,       // Which reality branch this process belongs to
    pub cpu_affinity: u64,     // Bitmask of preferred CPUs (0 = any)
    pub time_slice: u64,       // Remaining time slice in ticks
    pub total_runtime: u64,    // Total runtime in ticks
    pub last_scheduled: u64,   // Timestamp of last schedule
    pub wait_reason: u64,      // Why process is blocked (if blocked)
}

impl Process {
    pub fn new(pid: Pid) -> Self {
        Self {
            pid,
            state: ProcessState::Ready,
            registers: Registers::new(),
            page_table: 0,
            kernel_stack: 0,
            user_stack: 0,
            is_user_process: false,
            priority: PriorityClass::Normal,
            intent: ProcessIntent::None,
            reality_id: 0,
            cpu_affinity: 0,
            time_slice: 10, // Default time slice
            total_runtime: 0,
            last_scheduled: 0,
            wait_reason: 0,
        }
    }
    
    /// Create a new process with specific priority
    pub fn with_priority(pid: Pid, priority: PriorityClass) -> Self {
        let mut proc = Self::new(pid);
        proc.priority = priority;
        proc.time_slice = Self::time_slice_for_priority(priority);
        proc
    }
    
    /// Get default time slice for priority class
    pub fn time_slice_for_priority(priority: PriorityClass) -> u64 {
        match priority {
            PriorityClass::RealTime => 20,
            PriorityClass::Interactive => 5,
            PriorityClass::Normal => 10,
            PriorityClass::Background => 15,
            PriorityClass::Dream => 50, // Long slices during dream mode
        }
    }
    
    /// Set process intent and adjust scheduling hints
    pub fn set_intent(&mut self, intent: ProcessIntent) {
        self.intent = intent;
        // Adjust priority based on intent (optional optimization)
        match intent {
            ProcessIntent::Graphics | ProcessIntent::Interactive => {
                if self.priority > PriorityClass::Interactive {
                    self.priority = PriorityClass::Interactive;
                }
            }
            ProcessIntent::IoWait | ProcessIntent::FileSystem => {
                // I/O bound processes get boosted when they become ready
            }
            _ => {}
        }
    }
}

pub struct ProcessTable {
    pub processes: [Option<Process>; crate::MAX_PROCESSES],
    pub count: usize,
}

impl ProcessTable {
    pub const fn new() -> Self {
        Self {
            processes: [None; crate::MAX_PROCESSES],
            count: 0,
        }
    }
    
    pub fn add(&mut self, process: Process) -> Result<(), &'static str> {
        for slot in &mut self.processes {
            if slot.is_none() {
                *slot = Some(process);
                self.count += 1;
                return Ok(());
            }
        }
        Err("Process table full")
    }
    
    pub fn remove(&mut self, pid: Pid) -> Option<Process> {
        for slot in &mut self.processes {
            if let Some(proc) = slot {
                if proc.pid == pid {
                    let removed = *proc;
                    *slot = None;
                    self.count = self.count.saturating_sub(1);
                    return Some(removed);
                }
            }
        }
        None
    }
    
    pub fn get(&self, pid: Pid) -> Option<&Process> {
        self.processes.iter()
            .flatten()
            .find(|p| p.pid == pid)
    }
    
    pub fn get_mut(&mut self, pid: Pid) -> Option<&mut Process> {
        self.processes.iter_mut()
            .flatten()
            .find(|p| p.pid == pid)
    }
    
    pub fn iter(&self) -> impl Iterator<Item = &Process> {
        self.processes.iter().flatten()
    }
    
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Process> {
        self.processes.iter_mut().flatten()
    }
    
    pub fn count(&self) -> usize {
        self.count
    }
}

static PROCESS_TABLE: Mutex<ProcessTable> = Mutex::new(ProcessTable::new());
static CURRENT_PID: AtomicU64 = AtomicU64::new(0);

pub fn get_current_pid() -> Option<Pid> {
    let val = CURRENT_PID.load(Ordering::Relaxed);
    if val == 0 { None } else { Some(Pid(val)) }
}

pub fn set_current_pid(pid: Pid) {
    CURRENT_PID.store(pid.as_u64(), Ordering::Relaxed);
}

pub fn process_table() -> &'static Mutex<ProcessTable> {
    &PROCESS_TABLE
}

pub fn exit_process(_exit_code: i32) {
    if let Some(current) = get_current_pid() {
        let mut table = PROCESS_TABLE.lock();
        
        if let Some(proc) = table.get_mut(current) {
            proc.state = ProcessState::Zombie;
            scheduler::remove_from_scheduler(current);
        }
        
        drop(table);
        scheduler::yield_cpu();
    }
}