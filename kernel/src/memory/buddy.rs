use x86_64::{
    structures::paging::{PhysFrame, FrameAllocator, Size4KiB},
    PhysAddr,
};
use spin::Mutex;
use crate::serial_println;

const FRAME_SIZE: u64 = 4096; // 4KB
const MIN_ORDER: usize = 0;   // 4KB (2^0 * 4KB)
const MAX_ORDER: usize = 11;  // 8MB (2^11 * 4KB)
const MAX_FREE_BLOCKS: usize = 256; // Max free blocks per order

/// Simple free list node (no heap allocation required)
struct FreeList {
    blocks: [u64; MAX_FREE_BLOCKS],
    count: usize,
}

impl FreeList {
    const fn new() -> Self {
        Self {
            blocks: [0; MAX_FREE_BLOCKS],
            count: 0,
        }
    }
    
    fn push(&mut self, addr: u64) -> bool {
        if self.count < MAX_FREE_BLOCKS {
            self.blocks[self.count] = addr;
            self.count += 1;
            true
        } else {
            false // Full
        }
    }
    
    fn pop(&mut self) -> Option<u64> {
        if self.count > 0 {
            self.count -= 1;
            Some(self.blocks[self.count])
        } else {
            None
        }
    }
    
    fn remove(&mut self, addr: u64) -> bool {
        for i in 0..self.count {
            if self.blocks[i] == addr {
                // Swap with last element and decrease count
                self.blocks[i] = self.blocks[self.count - 1];
                self.count -= 1;
                return true;
            }
        }
        false
    }
    
    fn len(&self) -> usize {
        self.count
    }
}

/// Buddy allocator for physical memory frames
pub struct BuddyAllocator {
    free_lists: [FreeList; MAX_ORDER + 1],
    total_frames: usize,
    used_frames: usize,
    base_addr: u64,
}

impl BuddyAllocator {
    pub const fn new() -> Self {
        const EMPTY_LIST: FreeList = FreeList::new();
        Self {
            free_lists: [EMPTY_LIST; MAX_ORDER + 1],
            total_frames: 0,
            used_frames: 0,
            base_addr: 0,
        }
    }
    
    pub fn add_region(&mut self, start: u64, size: u64) {
        let frame_start = start / FRAME_SIZE;
        let frame_count = size / FRAME_SIZE;
        
        if self.base_addr == 0 {
            self.base_addr = start;
        }
        
        let mut current_frame = frame_start;
        let end_frame = frame_start + frame_count;
        
        while current_frame < end_frame {
            let remaining = end_frame - current_frame;
            let mut order = MAX_ORDER;
            
            // Find largest order that fits
            while order > MIN_ORDER {
                let size = 1u64 << order;
                if size <= remaining && (current_frame % size) == 0 {
                    break;
                }
                order -= 1;
            }
            
            let size = 1u64 << order;
            if !self.free_lists[order].push(current_frame) {
                serial_println!("[BUDDY] WARNING: Free list {} full, can't add block", order);
                break;
            }
            self.total_frames += size as usize;
            current_frame += size;
        }
        
        serial_println!("[BUDDY] Added region: start={:#x}, {} frames", start, frame_count);
    }
    
    pub fn allocate_order(&mut self, order: usize) -> Option<PhysFrame> {
        if order > MAX_ORDER {
            return None;
        }
        
        // Try direct allocation
        if let Some(addr) = self.free_lists[order].pop() {
            self.used_frames += 1 << order;
            return Some(PhysFrame::containing_address(PhysAddr::new(addr * FRAME_SIZE)));
        }
        
        // Split larger block
        for higher_order in (order + 1)..=MAX_ORDER {
            if let Some(addr) = self.free_lists[higher_order].pop() {
                let mut current_order = higher_order;
                let mut current_addr = addr;
                
                while current_order > order {
                    current_order -= 1;
                    let buddy_addr = current_addr + (1 << current_order);
                    self.free_lists[current_order].push(buddy_addr);
                }
                
                self.used_frames += 1 << order;
                return Some(PhysFrame::containing_address(PhysAddr::new(current_addr * FRAME_SIZE)));
            }
        }
        
        None
    }
    
    pub fn deallocate_order(&mut self, frame: PhysFrame, order: usize) {
        if order > MAX_ORDER {
            return;
        }
        
        let addr = frame.start_address().as_u64() / FRAME_SIZE;
        self.used_frames = self.used_frames.saturating_sub(1 << order);
        
        let mut current_addr = addr;
        let mut current_order = order;
        
        // Try coalescing
        while current_order < MAX_ORDER {
            let buddy_addr = current_addr ^ (1 << current_order);
            
            if self.free_lists[current_order].remove(buddy_addr) {
                current_addr = current_addr.min(buddy_addr);
                current_order += 1;
            } else {
                break;
            }
        }
        
        self.free_lists[current_order].push(current_addr);
    }
    
    pub fn stats(&self) -> (usize, usize) {
        (self.used_frames, self.total_frames)
    }
    
    pub fn print_stats(&self) {
        serial_println!("[BUDDY] Memory: {}/{} frames used ({}/{} MB)", 
            self.used_frames, self.total_frames,
            self.used_frames * 4 / 1024, self.total_frames * 4 / 1024);
        
        for order in MIN_ORDER..=MAX_ORDER {
            let count = self.free_lists[order].len();
            if count > 0 {
                serial_println!("[BUDDY]   Order {}: {} blocks ({} KB each)",
                    order, count, (1 << order) * 4);
            }
        }
    }
}

unsafe impl FrameAllocator<Size4KiB> for BuddyAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        self.allocate_order(0)
    }
}

pub static BUDDY_ALLOCATOR: Mutex<BuddyAllocator> = Mutex::new(BuddyAllocator::new());
