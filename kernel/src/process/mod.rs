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

#[derive(Clone, Copy)]
pub struct Process {
    pub pid: Pid,
    pub state: ProcessState,
    pub registers: Registers,
    pub page_table: u64,
    pub kernel_stack: u64,
}

impl Process {
    pub fn new(pid: Pid) -> Self {
        Self {
            pid,
            state: ProcessState::Ready,
            registers: Registers::new(),
            page_table: 0,
            kernel_stack: 0,
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

pub fn exit_process(exit_code: i32) {
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