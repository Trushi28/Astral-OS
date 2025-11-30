//src/memory/frame.rs
use crate::PAGE_SIZE;
use limine::memory_map::EntryType;
use spin::Mutex;
use core::sync::atomic::{AtomicUsize, Ordering};

const MAX_FRAMES: usize = 1024 * 1024; // Support up to 4GB RAM
const BITMAP_SIZE: usize = MAX_FRAMES / 8;

/// Physical address wrapper
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysAddr(u64);

impl PhysAddr {
    pub fn new(addr: u64) -> Self {
        Self(addr & 0x000F_FFFF_FFFF_F000) // Mask to 52-bit page boundary
    }
    
    pub fn as_u64(self) -> u64 {
        self.0
    }
    
    pub fn to_virt(self) -> usize {
        let hhdm = crate::get_hhdm_offset();
        (self.0 as usize).wrapping_add(hhdm)
    }
}

/// Physical frame allocator using bitmap
pub struct FrameAllocator {
    bitmap: [AtomicUsize; BITMAP_SIZE / core::mem::size_of::<usize>()],
    total_frames: usize,
    used_frames: AtomicUsize,
    start_search: AtomicUsize,
}

impl FrameAllocator {
    pub const fn new() -> Self {
        const ATOMIC_INIT: AtomicUsize = AtomicUsize::new(!0); // All allocated initially
        Self {
            bitmap: [ATOMIC_INIT; BITMAP_SIZE / core::mem::size_of::<usize>()],
            total_frames: 0,
            used_frames: AtomicUsize::new(0),
            start_search: AtomicUsize::new(0),
        }
    }
    
    /// Initialize with memory map from Limine
    pub fn init(&mut self, memory_map: &limine::response::MemoryMapResponse) {
        // Mark all as allocated first
        for entry in &self.bitmap {
            entry.store(!0, Ordering::Relaxed);
        }
        
        let mut max_addr: u64 = 0;
        
        // Free usable regions
        for entry in memory_map.entries() {
            if entry.entry_type == EntryType::USABLE {
                let start_frame = (entry.base as usize) / PAGE_SIZE;
                let frame_count = (entry.length as usize) / PAGE_SIZE;
                
                for frame in start_frame..(start_frame + frame_count) {
                    if frame < MAX_FRAMES {
                        self.free_frame_initial(frame);
                    }
                }
                
                let end = entry.base + entry.length;
                if end > max_addr {
                    max_addr = end;
                }
            }
        }
        
        self.total_frames = ((max_addr as usize) / PAGE_SIZE).min(MAX_FRAMES);
        
        // Reserve first 1MB (legacy devices, kernel code)
        for frame in 0..(1024 * 1024 / PAGE_SIZE) {
            self.allocate_frame_at(frame);
        }
    }
    
    /// Allocate a physical frame
    pub fn allocate(&self) -> Option<PhysAddr> {
        let start = self.start_search.load(Ordering::Relaxed);
        let bitmap_len = self.bitmap.len();
        
        // Search from start_search
        for i in start..bitmap_len {
            let mut word = self.bitmap[i].load(Ordering::Acquire);
            
            if word != !0 {
                // Find first zero bit
                for bit in 0..usize::BITS {
                    let mask = 1usize << bit;
                    if word & mask == 0 {
                        // Try to set the bit atomically
                        loop {
                            match self.bitmap[i].compare_exchange_weak(
                                word,
                                word | mask,
                                Ordering::AcqRel,
                                Ordering::Acquire
                            ) {
                                Ok(_) => {
                                    self.used_frames.fetch_add(1, Ordering::Relaxed);
                                    self.start_search.store(i, Ordering::Relaxed);
                                    
                                    let frame = i * usize::BITS as usize + bit as usize;
                                    return Some(PhysAddr::new((frame * PAGE_SIZE) as u64));
                                }
                                Err(current) => {
                                    word = current;
                                    // Retry if bit is still free
                                    if word & mask != 0 {
                                        break; // Bit was taken by another CPU
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Wrap around
        if start > 0 {
            self.start_search.store(0, Ordering::Relaxed);
            return self.allocate();
        }
        
        None
    }
    
    /// Allocate specific frame (for reserving)
    pub fn allocate_frame_at(&self, frame: usize) {
        if frame >= MAX_FRAMES {
            return;
        }
        
        let word_idx = frame / usize::BITS as usize;
        let bit_idx = frame % usize::BITS as usize;
        let mask = 1usize << bit_idx;
        
        let old = self.bitmap[word_idx].fetch_or(mask, Ordering::AcqRel);
        if old & mask == 0 {
            self.used_frames.fetch_add(1, Ordering::Relaxed);
        }
    }
    
    /// Deallocate a frame
    pub fn deallocate(&self, addr: PhysAddr) {
        let frame = (addr.as_u64() as usize) / PAGE_SIZE;
        if frame >= MAX_FRAMES {
            return;
        }
        
        let word_idx = frame / usize::BITS as usize;
        let bit_idx = frame % usize::BITS as usize;
        let mask = 1usize << bit_idx;
        
        let old = self.bitmap[word_idx].fetch_and(!mask, Ordering::AcqRel);
        if old & mask != 0 {
            self.used_frames.fetch_sub(1, Ordering::Relaxed);
        }
    }
    
    /// Free frame during init (non-atomic)
    fn free_frame_initial(&mut self, frame: usize) {
        if frame >= MAX_FRAMES {
            return;
        }
        
        let word_idx = frame / usize::BITS as usize;
        let bit_idx = frame % usize::BITS as usize;
        let mask = 1usize << bit_idx;
        
        let old = self.bitmap[word_idx].load(Ordering::Relaxed);
        self.bitmap[word_idx].store(old & !mask, Ordering::Relaxed);
    }
    
    pub fn used_frames(&self) -> usize {
        self.used_frames.load(Ordering::Relaxed)
    }
    
    pub fn total_frames(&self) -> usize {
        self.total_frames
    }
    
    pub fn free_frames(&self) -> usize {
        self.total_frames.saturating_sub(self.used_frames())
    }
}

static FRAME_ALLOCATOR: Mutex<FrameAllocator> = Mutex::new(FrameAllocator::new());

pub fn init(memory_map: &limine::response::MemoryMapResponse) {
    FRAME_ALLOCATOR.lock().init(memory_map);
}

pub fn allocate_frame() -> Option<PhysAddr> {
    FRAME_ALLOCATOR.lock().allocate()
}

pub fn deallocate_frame(addr: PhysAddr) {
    FRAME_ALLOCATOR.lock().deallocate(addr)
}

pub fn get_stats() -> (usize, usize, usize) {
    let alloc = FRAME_ALLOCATOR.lock();
    (alloc.total_frames(), alloc.used_frames(), alloc.free_frames())
}