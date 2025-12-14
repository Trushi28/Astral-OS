//! Task State Segment (TSS)
//! Required for privilege level switching (Ring 0 ↔ Ring 3)

use core::mem::size_of;

#[repr(C, packed)]
pub struct TaskStateSegment {
    reserved1: u32,
    /// Stack pointers for privilege levels 0-2
    pub rsp0: u64,  // Ring 0 stack (kernel)
    pub rsp1: u64,  // Ring 1 stack (unused)
    pub rsp2: u64,  // Ring 2 stack (unused)
    reserved2: u64,
    /// Interrupt stack table
    pub ist1: u64,
    pub ist2: u64,
    pub ist3: u64,
    pub ist4: u64,
    pub ist5: u64,
    pub ist6: u64,
    pub ist7: u64,
    reserved3: u64,
    reserved4: u16,
    /// I/O permission bitmap offset
    pub iomap_base: u16,
}

impl TaskStateSegment {
    pub const fn new() -> Self {
        Self {
            reserved1: 0,
            rsp0: 0,
            rsp1: 0,
            rsp2: 0,
            reserved2: 0,
            ist1: 0,
            ist2: 0,
            ist3: 0,
            ist4: 0,
            ist5: 0,
            ist6: 0,
            ist7: 0,
            reserved3: 0,
            reserved4: 0,
            iomap_base: size_of::<Self>() as u16,
        }
    }
    
    pub fn set_kernel_stack(&mut self, stack: u64) {
        self.rsp0 = stack;
    }
    
    pub fn set_ist(&mut self, index: usize, stack: u64) {
        match index {
            1 => self.ist1 = stack,
            2 => self.ist2 = stack,
            3 => self.ist3 = stack,
            4 => self.ist4 = stack,
            5 => self.ist5 = stack,
            6 => self.ist6 = stack,
            7 => self.ist7 = stack,
            _ => {}
        }
    }
}

static mut TSS: TaskStateSegment = TaskStateSegment::new();
static mut KERNEL_STACK: [u8; 16384] = [0; 16384];  // 16KB kernel stack

pub fn init() {
    unsafe {
        // Set kernel stack pointer
        let stack_top = KERNEL_STACK.as_ptr() as u64 + KERNEL_STACK.len() as u64;
        TSS.set_kernel_stack(stack_top);
        
        // Set IST1 for double fault handler
        TSS.set_ist(1, stack_top - 4096);
        
        crate::serial_println!("[TSS] Initialized with kernel stack at {:#x}", stack_top);
    }
}

pub fn get_tss_ptr() -> *const TaskStateSegment {
    unsafe { &TSS as *const _ }
}

pub fn get_tss_size() -> usize {
    size_of::<TaskStateSegment>()
}
