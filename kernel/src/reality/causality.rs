//src/reality/causality.rs
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

static REALITY_COUNTER: AtomicU64 = AtomicU64::new(0);
static CURRENT_REALITY_ID: AtomicU64 = AtomicU64::new(0);
static CAUSAL_EVENT_COUNTER: AtomicU64 = AtomicU64::new(0);

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RealityId(u64);

impl RealityId {
    pub fn new() -> Self {
        Self(REALITY_COUNTER.fetch_add(1, Ordering::SeqCst))
    }
    
    pub fn root() -> Self {
        Self(0)
    }
    
    pub fn current() -> Self {
        Self(CURRENT_REALITY_ID.load(Ordering::SeqCst))
    }
    
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

pub fn set_current_reality(id: RealityId) {
    CURRENT_REALITY_ID.store(id.as_u64(), Ordering::SeqCst);
}

pub fn get_reality_count() -> u64 {
    REALITY_COUNTER.load(Ordering::Relaxed)
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub enum CausalEventType {
    Boot,
    Interrupt,
    Syscall,
    Allocation,
    Deallocation,
    DiskRead,
    DiskWrite,
    FileOpen,
    FileClose,
    TaskSpawn,
    TaskExit,
    DreamEnter,
    DreamExit,
    Checkpoint,
    Rollback,
    UserCommand,
    Custom,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union CausalEventData {
    pub interrupt: InterruptEventData,
    pub allocation: AllocationEventData,
    pub disk: DiskEventData,
    pub file: FileEventData,
    pub command: CommandEventData,
    pub raw: [u8; 32],
}

impl core::fmt::Debug for CausalEventData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "CausalEventData {{ ... }}")
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct InterruptEventData {
    pub vector: u8,
    pub error_code: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct AllocationEventData {
    pub address: usize,
    pub size: usize,
    pub spatial_x: u64,
    pub spatial_y: u64,
    pub spatial_z: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DiskEventData {
    pub sector: u64,
    pub count: u16,
    pub success: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FileEventData {
    pub inode: u32,
    pub operation: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CommandEventData {
    pub cmd_hash: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CausalEvent {
    pub id: u64,
    pub timestamp: u64,
    pub event_type: CausalEventType,
    pub cause_id: Option<u64>,
    pub reality_id: u64,
    pub data: CausalEventData,
}

const MAX_CAUSAL_EVENTS: usize = 1024;

pub struct CausalLog {
    events: VecDeque<CausalEvent>,
    next_id: u64,
}

impl CausalLog {
    pub fn new() -> Self {
        Self {
            events: VecDeque::with_capacity(MAX_CAUSAL_EVENTS),
            next_id: 0,
        }
    }
    
    pub fn log(&mut self, event_type: CausalEventType, cause: Option<u64>, data: CausalEventData) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        
        let event = CausalEvent {
            id,
            timestamp: crate::get_timestamp(),
            event_type,
            cause_id: cause,
            reality_id: CURRENT_REALITY_ID.load(Ordering::Relaxed),
            data,
        };
        
        if self.events.len() >= MAX_CAUSAL_EVENTS {
            self.events.pop_front();
        }
        
        self.events.push_back(event);
        CAUSAL_EVENT_COUNTER.fetch_add(1, Ordering::Relaxed);
        
        id
    }
    
    pub fn get_event(&self, id: u64) -> Option<&CausalEvent> {
        self.events.iter().find(|e| e.id == id)
    }
    
    pub fn get_recent(&self, count: usize) -> impl Iterator<Item = &CausalEvent> {
        self.events.iter().rev().take(count)
    }
    
    pub fn get_causal_chain(&self, event_id: u64) -> Vec<&CausalEvent> {
        let mut chain = Vec::new();
        let mut current_id = Some(event_id);
        
        while let Some(id) = current_id {
            if let Some(event) = self.get_event(id) {
                chain.push(event);
                current_id = event.cause_id;
            } else {
                break;
            }
        }
        
        chain
    }
    
    pub fn len(&self) -> usize {
        self.events.len()
    }
    
    pub fn total_events(&self) -> u64 {
        CAUSAL_EVENT_COUNTER.load(Ordering::Relaxed)
    }
}

static CAUSAL_LOG: Mutex<Option<CausalLog>> = Mutex::new(None);

pub fn init() {
    let mut log = CAUSAL_LOG.lock();
    *log = Some(CausalLog::new());
    
    if let Some(ref mut l) = *log {
        l.log(
            CausalEventType::Boot,
            None,
            CausalEventData { raw: [0; 32] }
        );
    }
}

pub fn log_event(event_type: CausalEventType, cause: Option<u64>, data: CausalEventData) -> u64 {
    if let Some(ref mut log) = *CAUSAL_LOG.lock() {
        log.log(event_type, cause, data)
    } else {
        0
    }
}

pub fn get_causal_chain(event_id: u64) -> Vec<CausalEvent> {
    if let Some(ref log) = *CAUSAL_LOG.lock() {
        log.get_causal_chain(event_id).into_iter().cloned().collect()
    } else {
        Vec::new()
    }
}

pub fn get_recent_events(count: usize) -> Vec<CausalEvent> {
    if let Some(ref log) = *CAUSAL_LOG.lock() {
        log.get_recent(count).cloned().collect()
    } else {
        Vec::new()
    }
}

pub fn get_event_count() -> usize {
    if let Some(ref log) = *CAUSAL_LOG.lock() {
        log.len()
    } else {
        0
    }
}

pub fn get_total_events() -> u64 {
    if let Some(ref log) = *CAUSAL_LOG.lock() {
        log.total_events()
    } else {
        0
    }
}