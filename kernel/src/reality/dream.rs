//src/reality/dream.rs
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;
use super::causality::{log_event, CausalEventType, CausalEventData};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum SystemState {
    Active = 0,
    Dreaming = 1,
    DeepDream = 2,
    Awakening = 3,
}

static SYSTEM_STATE: AtomicUsize = AtomicUsize::new(SystemState::Active as usize);
static IDLE_CYCLES: AtomicU64 = AtomicU64::new(0);
static DREAM_CYCLES: AtomicU64 = AtomicU64::new(0);
static DREAM_COMPACTIONS: AtomicU64 = AtomicU64::new(0);
static DREAM_PREDICTIONS: AtomicU64 = AtomicU64::new(0);

const DREAM_THRESHOLD: u64 = 1000;
const DEEP_DREAM_THRESHOLD: u64 = 5000;
const DREAM_COMPACT_INTERVAL: u64 = 100;
const DREAM_PREDICT_INTERVAL: u64 = 50;

pub fn get_system_state() -> SystemState {
    match SYSTEM_STATE.load(Ordering::Relaxed) {
        0 => SystemState::Active,
        1 => SystemState::Dreaming,
        2 => SystemState::DeepDream,
        3 => SystemState::Awakening,
        _ => SystemState::Active,
    }
}

pub fn enter_dream_state() {
    SYSTEM_STATE.store(SystemState::Dreaming as usize, Ordering::SeqCst);
    log_event(CausalEventType::DreamEnter, None, CausalEventData { raw: [0; 32] });
}

pub fn exit_dream_state() {
    SYSTEM_STATE.store(SystemState::Active as usize, Ordering::SeqCst);
    IDLE_CYCLES.store(0, Ordering::SeqCst);
    log_event(CausalEventType::DreamExit, None, CausalEventData { raw: [0; 32] });
}

pub fn increment_idle() {
    let idle = IDLE_CYCLES.fetch_add(1, Ordering::Relaxed);
    
    if idle == DREAM_THRESHOLD && get_system_state() == SystemState::Active {
        enter_dream_state();
    }
}

pub fn reset_idle() {
    IDLE_CYCLES.store(0, Ordering::Relaxed);
    
    if get_system_state() != SystemState::Active {
        exit_dream_state();
    }
}

const MAX_ACCESS_PATTERNS: usize = 64;

#[derive(Clone, Copy)]
pub struct AccessPattern {
    pub file_id: u32,
    pub access_count: u32,
    pub last_access: u64,
    pub predicted_next: u64,
    pub avg_interval: u64,
}

pub struct AccessPatternTracker {
    patterns: [Option<AccessPattern>; MAX_ACCESS_PATTERNS],
    count: usize,
}

impl AccessPatternTracker {
    pub fn new() -> Self {
        Self {
            patterns: [None; MAX_ACCESS_PATTERNS],
            count: 0,
        }
    }
    
    pub fn record_access(&mut self, file_id: u32, timestamp: u64) {
        for i in 0..self.count {
            if let Some(ref mut pattern) = self.patterns[i] {
                if pattern.file_id == file_id {
                    let interval = timestamp.saturating_sub(pattern.last_access);
                    pattern.avg_interval = (pattern.avg_interval * pattern.access_count as u64 + interval) 
                                          / (pattern.access_count as u64 + 1);
                    pattern.access_count += 1;
                    pattern.last_access = timestamp;
                    pattern.predicted_next = timestamp + pattern.avg_interval;
                    return;
                }
            }
        }
        
        if self.count < MAX_ACCESS_PATTERNS {
            self.patterns[self.count] = Some(AccessPattern {
                file_id,
                access_count: 1,
                last_access: timestamp,
                predicted_next: 0,
                avg_interval: 0,
            });
            self.count += 1;
        }
    }
    
    pub fn get_hot_files(&self, min_accesses: u32) -> alloc::vec::Vec<u32> {
        let mut hot = alloc::vec::Vec::new();
        
        for i in 0..self.count {
            if let Some(ref pattern) = self.patterns[i] {
                if pattern.access_count >= min_accesses {
                    hot.push(pattern.file_id);
                }
            }
        }
        
        hot
    }
    
    pub fn predict_next_access(&self, current_time: u64) -> Option<u32> {
        let mut best_file: Option<u32> = None;
        let mut min_time_diff = u64::MAX;
        
        for i in 0..self.count {
            if let Some(ref pattern) = self.patterns[i] {
                if pattern.access_count >= 3 {
                    let time_diff = pattern.predicted_next.saturating_sub(current_time);
                    if time_diff < min_time_diff && time_diff < 1000 {
                        min_time_diff = time_diff;
                        best_file = Some(pattern.file_id);
                    }
                }
            }
        }
        
        best_file
    }
}

static ACCESS_PATTERNS: Mutex<Option<AccessPatternTracker>> = Mutex::new(None);

pub fn init() {
    let mut patterns = ACCESS_PATTERNS.lock();
    *patterns = Some(AccessPatternTracker::new());
}

pub fn record_file_access(file_id: u32) {
    if let Some(ref mut tracker) = *ACCESS_PATTERNS.lock() {
        tracker.record_access(file_id, crate::get_timestamp());
    }
}

pub fn get_hot_files(min_accesses: u32) -> alloc::vec::Vec<u32> {
    if let Some(ref tracker) = *ACCESS_PATTERNS.lock() {
        tracker.get_hot_files(min_accesses)
    } else {
        alloc::vec::Vec::new()
    }
}

pub fn predict_next_file() -> Option<u32> {
    if let Some(ref tracker) = *ACCESS_PATTERNS.lock() {
        tracker.predict_next_access(crate::get_timestamp())
    } else {
        None
    }
}

pub fn dream_cycle() {
    let cycles = DREAM_CYCLES.fetch_add(1, Ordering::Relaxed);
    
    match get_system_state() {
        SystemState::Dreaming => {
            if cycles % DREAM_COMPACT_INTERVAL == 0 {
                DREAM_COMPACTIONS.fetch_add(1, Ordering::Relaxed);
            }
            
            if cycles % DREAM_PREDICT_INTERVAL == 0 {
                DREAM_PREDICTIONS.fetch_add(1, Ordering::Relaxed);
            }
            
            let idle = IDLE_CYCLES.load(Ordering::Relaxed);
            if idle > DEEP_DREAM_THRESHOLD {
                SYSTEM_STATE.store(SystemState::DeepDream as usize, Ordering::SeqCst);
            }
        }
        
        SystemState::DeepDream => {
            if cycles % 100 == 0 {
                DREAM_COMPACTIONS.fetch_add(1, Ordering::Relaxed);
            }
        }
        
        _ => {}
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DreamStats {
    pub total_cycles: u64,
    pub compactions: u64,
    pub predictions: u64,
    pub state: SystemState,
    pub idle_cycles: u64,
}

pub fn get_dream_stats() -> DreamStats {
    DreamStats {
        total_cycles: DREAM_CYCLES.load(Ordering::Relaxed),
        compactions: DREAM_COMPACTIONS.load(Ordering::Relaxed),
        predictions: DREAM_PREDICTIONS.load(Ordering::Relaxed),
        state: get_system_state(),
        idle_cycles: IDLE_CYCLES.load(Ordering::Relaxed),
    }
}