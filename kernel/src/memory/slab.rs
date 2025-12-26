use spin::Mutex;
use crate::serial_println;

// Slab sizes: 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096 bytes
const SLAB_SIZES: [usize; 10] = [8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];

/// Slab allocator for efficient small object allocation
/// 
/// This is a framework for future optimization. Currently uses
/// the underlying linked-list allocator but tracks statistics
/// and provides infrastructure for per-size free lists.
pub struct SlabAllocator {
    /// Allocation statistics per size class
    stats: [SlabStats; 10],
}

#[derive(Copy, Clone)]
struct SlabStats {
    allocations: usize,
    deallocations: usize,
    active: usize,
}

impl SlabStats {
    const fn new() -> Self {
        Self {
            allocations: 0,
            deallocations: 0,
            active: 0,
        }
    }
}

impl SlabAllocator {
    pub const fn new() -> Self {
        const EMPTY_STATS: SlabStats = SlabStats::new();
        Self {
            stats: [EMPTY_STATS; 10],
        }
    }
    
    /// Get size class for allocation
    fn size_class(&self, size: usize) -> Option<usize> {
        SLAB_SIZES.iter().position(|&s| s >= size)
    }
    
    /// Track allocation in statistics
    pub fn track_alloc(&mut self, size: usize) {
        if let Some(class) = self.size_class(size) {
            self.stats[class].allocations += 1;
            self.stats[class].active += 1;
        }
    }
    
    /// Track deallocation in statistics
    pub fn track_dealloc(&mut self, size: usize) {
        if let Some(class) = self.size_class(size) {
            self.stats[class].deallocations += 1;
            self.stats[class].active = self.stats[class].active.saturating_sub(1);
        }
    }
    
    /// Print allocation statistics
    pub fn print_stats(&self) {
        serial_println!("[SLAB] Allocation Statistics:");
        for (i, stats) in self.stats.iter().enumerate() {
            if stats.allocations > 0 {
                serial_println!("[SLAB]   {} bytes: {} allocs, {} active",
                    SLAB_SIZES[i], stats.allocations, stats.active);
            }
        }
    }
}

pub static SLAB_STATS: Mutex<SlabAllocator> = Mutex::new(SlabAllocator::new());
