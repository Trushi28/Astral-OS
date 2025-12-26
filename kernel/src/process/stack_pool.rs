use spin::Mutex;
use x86_64::VirtAddr;
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};
use crate::memory::FRAME_ALLOCATOR;
use crate::serial_println;

const STACK_SIZE: usize = 16384; // 16KB per thread
const MAX_STACKS: usize = 256;

struct StackEntry {
    base: VirtAddr,
    in_use: bool,
}

pub struct StackPool {
    stacks: [StackEntry; MAX_STACKS],
    allocated_count: usize,
}

impl StackPool {
    pub const fn new() -> Self {
        const EMPTY: StackEntry = StackEntry {
            base: VirtAddr::new(0),
            in_use: false,
        };
        
        Self {
            stacks: [EMPTY; MAX_STACKS],
            allocated_count: 0,
        }
    }
    
    /// Allocate a new stack
    pub fn allocate(&mut self) -> Option<VirtAddr> {
        // Try to reuse freed stack
        for entry in &mut self.stacks {
            if entry.base.as_u64() != 0 && !entry.in_use {
                entry.in_use = true;
                serial_println!("[STACK] Reused stack at {:#x}", entry.base.as_u64());
                return Some(entry.base + STACK_SIZE as u64);
            }
        }
        
        // Allocate new stack
        if self.allocated_count >= MAX_STACKS {
            serial_println!("[STACK] ERROR: Stack pool exhausted!");
            return None;
        }
        
        // Allocate frames for stack
        let frame_count = STACK_SIZE / 4096;
        let mut base = None;
        
        unsafe {
            let mut allocator = FRAME_ALLOCATOR.lock();
            for i in 0..frame_count {
                if let Some(frame) = allocator.allocate_frame() {
                    if i == 0 {
                        base = Some(VirtAddr::new(frame.start_address().as_u64()));
                    }
                } else {
                    serial_println!("[STACK] ERROR: Failed to allocate stack frames");
                    return None;
                }
            }
        }
        
        let base = base?;
        
        // Store in pool
        for entry in &mut self.stacks {
            if entry.base.as_u64() == 0 {
                entry.base = base;
                entry.in_use = true;
                self.allocated_count += 1;
                
                serial_println!("[STACK] Allocated new stack at {:#x}", base.as_u64());
                return Some(base + STACK_SIZE as u64); // Return top of stack
            }
        }
        
        None
    }
    
    /// Free a stack (mark as available for reuse)
    pub fn free(&mut self, stack_top: VirtAddr) {
        let stack_base = stack_top - STACK_SIZE as u64;
        
        for entry in &mut self.stacks {
            if entry.base == stack_base && entry.in_use {
                entry.in_use = false;
                serial_println!("[STACK] Freed stack at {:#x}", stack_base.as_u64());
                return;
            }
        }
        
        serial_println!("[STACK] WARNING: Attempted to free unknown stack at {:#x}", stack_top.as_u64());
    }
}

pub static STACK_POOL: Mutex<StackPool> = Mutex::new(StackPool::new());
