//src/reality/dream.rs
//! Dream-State OS Mode - System optimization during idle time
//! - Predictive loading of files
//! - Memory reorganization and compaction
//! - Pattern learning and model building

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use spin::Mutex;
use alloc::vec::Vec;
use super::causality::{log_event, CausalEventType, CausalEventData};

/// System operating states
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum SystemState {
    Active = 0,      // Normal operation
    Dreaming = 1,    // Light optimization mode
    DeepDream = 2,   // Heavy optimization mode  
    Awakening = 3,   // Transitioning back to active
}

static SYSTEM_STATE: AtomicUsize = AtomicUsize::new(SystemState::Active as usize);
static IDLE_CYCLES: AtomicU64 = AtomicU64::new(0);
static DREAM_CYCLES: AtomicU64 = AtomicU64::new(0);
static DREAM_COMPACTIONS: AtomicU64 = AtomicU64::new(0);
static DREAM_PREDICTIONS: AtomicU64 = AtomicU64::new(0);
static PREFETCHED_FILES: AtomicU64 = AtomicU64::new(0);

// Thresholds for state transitions
const DREAM_THRESHOLD: u64 = 1000;       // Enter dream after 1000 idle ticks
const DEEP_DREAM_THRESHOLD: u64 = 5000;  // Enter deep dream after 5000 idle
const AWAKENING_GRACE: u64 = 100;        // Grace period before full wake

// Work intervals during dream cycles
const DREAM_COMPACT_INTERVAL: u64 = 100;
const DREAM_PREDICT_INTERVAL: u64 = 50;
const DEEP_DREAM_GC_INTERVAL: u64 = 50;

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
    
    crate::serial_println!("[DREAM] Entering dream state...");
}

pub fn exit_dream_state() {
    SYSTEM_STATE.store(SystemState::Active as usize, Ordering::SeqCst);
    IDLE_CYCLES.store(0, Ordering::SeqCst);
    log_event(CausalEventType::DreamExit, None, CausalEventData { raw: [0; 32] });
    
    crate::serial_println!("[DREAM] Awakening...");
}

/// Called during idle time - may trigger state transitions
pub fn increment_idle() {
    let idle = IDLE_CYCLES.fetch_add(1, Ordering::Relaxed);
    
    match get_system_state() {
        SystemState::Active => {
            if idle >= DREAM_THRESHOLD {
                enter_dream_state();
            }
        }
        SystemState::Dreaming => {
            if idle >= DEEP_DREAM_THRESHOLD {
                SYSTEM_STATE.store(SystemState::DeepDream as usize, Ordering::SeqCst);
                crate::serial_println!("[DREAM] Entering deep dream...");
            }
        }
        _ => {}
    }
}

/// Reset idle counter and potentially wake the system
pub fn reset_idle() {
    let prev_idle = IDLE_CYCLES.swap(0, Ordering::SeqCst);
    
    let state = get_system_state();
    if state != SystemState::Active {
        if prev_idle > 100 {
            // Start awakening process
            SYSTEM_STATE.store(SystemState::Awakening as usize, Ordering::SeqCst);
        } else {
            exit_dream_state();
        }
    }
}

// ============================================================================
// ACCESS PATTERN TRACKING
// ============================================================================

const MAX_ACCESS_PATTERNS: usize = 128;

/// File access pattern for prediction
#[derive(Clone, Copy)]
pub struct AccessPattern {
    pub file_id: u32,
    pub access_count: u32,
    pub last_access: u64,
    pub predicted_next: u64,
    pub avg_interval: u64,
    pub variance: u64,
}

impl AccessPattern {
    fn new(file_id: u32, timestamp: u64) -> Self {
        Self {
            file_id,
            access_count: 1,
            last_access: timestamp,
            predicted_next: 0,
            avg_interval: 0,
            variance: 0,
        }
    }
    
    /// Update pattern with new access
    fn update(&mut self, timestamp: u64) {
        let interval = timestamp.saturating_sub(self.last_access);
        
        // Update running average
        let n = self.access_count as u64;
        let new_avg = (self.avg_interval * n + interval) / (n + 1);
        
        // Update variance estimate
        let diff = if interval > self.avg_interval {
            interval - self.avg_interval
        } else {
            self.avg_interval - interval
        };
        self.variance = (self.variance * n + diff) / (n + 1);
        
        self.avg_interval = new_avg;
        self.access_count += 1;
        self.last_access = timestamp;
        self.predicted_next = timestamp + self.avg_interval;
    }
    
    /// Confidence score for predictions (higher = more reliable)
    fn prediction_confidence(&self) -> u32 {
        if self.access_count < 3 {
            return 0; // Not enough data
        }
        
        // Lower variance relative to average = higher confidence
        if self.avg_interval == 0 {
            return 0;
        }
        
        let stability = 100 - (self.variance * 100 / self.avg_interval).min(100) as u32;
        let recency = 100 - (self.access_count.min(100));
        
        (stability * 2 + recency) / 3
    }
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
    
    /// Record a file access
    pub fn record_access(&mut self, file_id: u32, timestamp: u64) {
        // Look for existing pattern
        for i in 0..self.count {
            if let Some(ref mut pattern) = self.patterns[i] {
                if pattern.file_id == file_id {
                    pattern.update(timestamp);
                    return;
                }
            }
        }
        
        // Add new pattern
        if self.count < MAX_ACCESS_PATTERNS {
            self.patterns[self.count] = Some(AccessPattern::new(file_id, timestamp));
            self.count += 1;
        } else {
            // Replace least accessed pattern
            let mut min_idx = 0;
            let mut min_count = u32::MAX;
            for i in 0..self.count {
                if let Some(ref p) = self.patterns[i] {
                    if p.access_count < min_count {
                        min_count = p.access_count;
                        min_idx = i;
                    }
                }
            }
            self.patterns[min_idx] = Some(AccessPattern::new(file_id, timestamp));
        }
    }
    
    /// Get hot files (frequently accessed)
    pub fn get_hot_files(&self, min_accesses: u32) -> Vec<u32> {
        let mut hot = Vec::new();
        
        for i in 0..self.count {
            if let Some(ref pattern) = self.patterns[i] {
                if pattern.access_count >= min_accesses {
                    hot.push(pattern.file_id);
                }
            }
        }
        
        // Sort by access count (descending)
        hot.sort_by(|a, b| {
            let count_a = self.patterns.iter()
                .filter_map(|p| p.as_ref())
                .find(|p| p.file_id == *a)
                .map(|p| p.access_count)
                .unwrap_or(0);
            let count_b = self.patterns.iter()
                .filter_map(|p| p.as_ref())
                .find(|p| p.file_id == *b)
                .map(|p| p.access_count)
                .unwrap_or(0);
            count_b.cmp(&count_a)
        });
        
        hot
    }
    
    /// Predict next file access based on patterns
    pub fn predict_next_access(&self, current_time: u64) -> Vec<u32> {
        let mut predictions = Vec::new();
        
        for i in 0..self.count {
            if let Some(ref pattern) = self.patterns[i] {
                // Must have enough history
                if pattern.access_count < 3 {
                    continue;
                }
                
                // Check if prediction is within time window
                let time_diff = if pattern.predicted_next > current_time {
                    pattern.predicted_next - current_time
                } else {
                    current_time - pattern.predicted_next
                };
                
                // If predicted within next 1000 ticks, add to prefetch list
                if time_diff < 1000 && pattern.prediction_confidence() > 50 {
                    predictions.push(pattern.file_id);
                }
            }
        }
        
        predictions
    }
    
    /// Get cold files (rarely accessed, candidates for compaction)
    pub fn get_cold_files(&self, max_accesses: u32, age_threshold: u64) -> Vec<u32> {
        let current_time = crate::get_timestamp();
        let mut cold = Vec::new();
        
        for i in 0..self.count {
            if let Some(ref pattern) = self.patterns[i] {
                let age = current_time.saturating_sub(pattern.last_access);
                
                if pattern.access_count <= max_accesses && age > age_threshold {
                    cold.push(pattern.file_id);
                }
            }
        }
        
        cold
    }
}

static ACCESS_PATTERNS: Mutex<Option<AccessPatternTracker>> = Mutex::new(None);

pub fn init() {
    let mut patterns = ACCESS_PATTERNS.lock();
    *patterns = Some(AccessPatternTracker::new());
    
    crate::serial_println!("[DREAM] Dream state initialized");
}

pub fn record_file_access(file_id: u32) {
    if let Some(ref mut tracker) = *ACCESS_PATTERNS.lock() {
        tracker.record_access(file_id, crate::get_timestamp());
    }
}

pub fn get_hot_files(min_accesses: u32) -> Vec<u32> {
    if let Some(ref tracker) = *ACCESS_PATTERNS.lock() {
        tracker.get_hot_files(min_accesses)
    } else {
        Vec::new()
    }
}

pub fn predict_next_file() -> Option<u32> {
    if let Some(ref tracker) = *ACCESS_PATTERNS.lock() {
        let predictions = tracker.predict_next_access(crate::get_timestamp());
        predictions.first().copied()
    } else {
        None
    }
}

// ============================================================================
// DREAM CYCLE WORK
// ============================================================================

/// Main dream cycle function - called during idle
pub fn dream_cycle() {
    let cycles = DREAM_CYCLES.fetch_add(1, Ordering::Relaxed);
    
    match get_system_state() {
        SystemState::Dreaming => {
            // Light optimization work
            if cycles % DREAM_PREDICT_INTERVAL == 0 {
                do_prediction_work();
            }
            
            if cycles % DREAM_COMPACT_INTERVAL == 0 {
                do_light_compaction();
            }
        }
        
        SystemState::DeepDream => {
            // Heavy optimization work
            if cycles % DEEP_DREAM_GC_INTERVAL == 0 {
                do_garbage_collection();
            }
            
            if cycles % 100 == 0 {
                do_file_reorganization();
            }
            
            if cycles % 200 == 0 {
                do_memory_defragmentation();
            }
        }
        
        SystemState::Awakening => {
            // Transition work - stop optimization, prepare for active
            let idle = IDLE_CYCLES.load(Ordering::Relaxed);
            if idle > AWAKENING_GRACE {
                // Grace period over, return to active
                exit_dream_state();
            }
        }
        
        _ => {}
    }
}

/// Predictive file loading based on access patterns
fn do_prediction_work() {
    DREAM_PREDICTIONS.fetch_add(1, Ordering::Relaxed);
    
    if let Some(file_id) = predict_next_file() {
        // In a real implementation, we would prefetch this file
        PREFETCHED_FILES.fetch_add(1, Ordering::Relaxed);
        
        crate::serial_println!("[DREAM] Predicted file {} for prefetch", file_id);
    }
}

/// Light memory compaction during dream state
fn do_light_compaction() {
    DREAM_COMPACTIONS.fetch_add(1, Ordering::Relaxed);
    
    // Get hot memory regions and ensure they're in fast-access areas
    let hot_regions = crate::memory::fractal::get_hot_regions(100);
    
    if !hot_regions.is_empty() {
        crate::serial_println!("[DREAM] Light compaction: {} hot regions", hot_regions.len());
    }
}

/// Garbage collection during deep dream
fn do_garbage_collection() {
    // Identify and reclaim unused memory
    // This would integrate with the allocator to find unreferenced pages
    crate::serial_println!("[DREAM] GC cycle");
}

/// File reorganization based on access patterns
fn do_file_reorganization() {
    // Get hot files and cold files
    let hot_files = get_hot_files(10);
    
    if let Some(ref tracker) = *ACCESS_PATTERNS.lock() {
        let cold_files = tracker.get_cold_files(2, 10000);
        
        if !hot_files.is_empty() || !cold_files.is_empty() {
            crate::serial_println!("[DREAM] File reorg: {} hot, {} cold", 
                hot_files.len(), cold_files.len());
        }
    }
}

/// Memory defragmentation during deep dream
fn do_memory_defragmentation() {
    // Compact cold memory regions
    crate::memory::fractal::compact_cold_regions();
    
    crate::serial_println!("[DREAM] Memory defrag cycle");
}

// ============================================================================
// STATISTICS
// ============================================================================

#[derive(Debug, Clone, Copy)]
pub struct DreamStats {
    pub total_cycles: u64,
    pub compactions: u64,
    pub predictions: u64,
    pub prefetched_files: u64,
    pub state: SystemState,
    pub idle_cycles: u64,
}

pub fn get_dream_stats() -> DreamStats {
    DreamStats {
        total_cycles: DREAM_CYCLES.load(Ordering::Relaxed),
        compactions: DREAM_COMPACTIONS.load(Ordering::Relaxed),
        predictions: DREAM_PREDICTIONS.load(Ordering::Relaxed),
        prefetched_files: PREFETCHED_FILES.load(Ordering::Relaxed),
        state: get_system_state(),
        idle_cycles: IDLE_CYCLES.load(Ordering::Relaxed),
    }
}