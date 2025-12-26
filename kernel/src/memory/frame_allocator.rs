use x86_64::{
    structures::paging::{PhysFrame, FrameAllocator, Size4KiB},
    PhysAddr,
};
use spin::Mutex;
use limine::memory_map::EntryType;
use crate::serial_println;

const FRAME_SIZE: u64 = 4096; // 4KB
const MAX_FRAMES: usize = 1024 * 1024; // Support up to 4GB RAM

pub struct BitmapFrameAllocator {
    bitmap: [u64; MAX_FRAMES / 64],
    next_free: usize,
    total_frames: usize,
    used_frames: usize,
}

impl BitmapFrameAllocator {
    pub const fn new() -> Self {
        Self {
            bitmap: [0; MAX_FRAMES / 64],
            next_free: 0,
            total_frames: 0,
            used_frames: 0,
        }
    }

    pub fn init(&mut self, memory_map: &[&limine::memory_map::Entry]) {
        // Mark all frames as used initially
        for word in self.bitmap.iter_mut() {
            *word = u64::MAX;
        }

        // Find usable memory and mark those frames as free
        for entry in memory_map.iter() {
            if entry.entry_type == EntryType::USABLE {
                let start_frame = (entry.base / FRAME_SIZE) as usize;
                let end_frame = ((entry.base + entry.length) / FRAME_SIZE) as usize;

                for frame in start_frame..end_frame {
                    if frame < MAX_FRAMES {
                        self.mark_free(frame);
                        self.total_frames += 1;
                    }
                }
            }
        }

        serial_println!("[FRAME] Initialized: {} total frames ({} MB)",
            self.total_frames, self.total_frames * 4 / 1024);
    }

    fn mark_used(&mut self, frame: usize) {
        let word = frame / 64;
        let bit = frame % 64;
        if word < self.bitmap.len() {
            let was_free = (self.bitmap[word] & (1 << bit)) == 0;
            self.bitmap[word] |= 1 << bit;
            if was_free {
                self.used_frames += 1;
            }
        }
    }

    fn mark_free(&mut self, frame: usize) {
        let word = frame / 64;
        let bit = frame % 64;
        if word < self.bitmap.len() {
            let was_used = (self.bitmap[word] & (1 << bit)) != 0;
            self.bitmap[word] &= !(1 << bit);
            if was_used && self.used_frames > 0 {
                self.used_frames -= 1;
            }
        }
    }

    fn is_used(&self, frame: usize) -> bool {
        let word = frame / 64;
        let bit = frame % 64;
        if word < self.bitmap.len() {
            (self.bitmap[word] & (1 << bit)) != 0
        } else {
            true
        }
    }

    pub fn allocate_frame(&mut self) -> Option<PhysFrame> {
        // Start searching from next_free hint
        for frame in self.next_free..MAX_FRAMES {
            if !self.is_used(frame) {
                self.mark_used(frame);
                self.next_free = frame + 1;
                let addr = PhysAddr::new((frame as u64) * FRAME_SIZE);
                return Some(PhysFrame::containing_address(addr));
            }
        }

        // Wrap around and search from beginning
        for frame in 0..self.next_free {
            if !self.is_used(frame) {
                self.mark_used(frame);
                self.next_free = frame + 1;
                let addr = PhysAddr::new((frame as u64) * FRAME_SIZE);
                return Some(PhysFrame::containing_address(addr));
            }
        }

        None
    }

    pub fn deallocate_frame(&mut self, frame: PhysFrame) {
        let frame_num = (frame.start_address().as_u64() / FRAME_SIZE) as usize;
        self.mark_free(frame_num);
        if frame_num < self.next_free {
            self.next_free = frame_num;
        }
    }

    pub fn mark_region_used(&mut self, start: PhysAddr, size: u64) {
        let start_frame = (start.as_u64() / FRAME_SIZE) as usize;
        let end_frame = ((start.as_u64() + size + FRAME_SIZE - 1) / FRAME_SIZE) as usize;

        for frame in start_frame..end_frame {
            self.mark_used(frame);
        }
    }

    pub fn stats(&self) -> (usize, usize) {
        (self.used_frames, self.total_frames)
    }
}

unsafe impl FrameAllocator<Size4KiB> for BitmapFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        self.allocate_frame()
    }
}

pub static FRAME_ALLOCATOR: Mutex<BitmapFrameAllocator> = Mutex::new(BitmapFrameAllocator::new());
