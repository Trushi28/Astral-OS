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
        // Set kernel stack pointer using raw pointers to avoid static_mut_refs warnings
        let stack_ptr = &raw const KERNEL_STACK;
        let stack_top = (*stack_ptr).as_ptr() as u64 + core::mem::size_of_val(&*stack_ptr) as u64;
        
        // Use raw pointer to access TSS
        let tss_ptr = &raw mut TSS;
        (*tss_ptr).set_kernel_stack(stack_top);
        
        // Set IST1 for double fault handler
        (*tss_ptr).set_ist(1, stack_top - 4096);
        
        crate::serial_println!("[TSS] Initialized with kernel stack at {:#x}", stack_top);
    }
}

pub fn get_tss_ptr() -> *const TaskStateSegment {
    // Use raw pointer syntax for Rust 2024 compatibility
    // Note: &raw const doesn't require unsafe, only dereferencing does
    &raw const TSS
}

pub fn get_tss_size() -> usize {
    size_of::<TaskStateSegment>()
}

/// Update kernel stack pointer (rsp0) for context switch
/// 
/// This must be called before switching to a user process so that
/// syscall/interrupt handlers use the correct kernel stack.
pub fn update_kernel_stack(kernel_stack_top: u64) {
    unsafe {
        let tss_ptr = &raw mut TSS;
        (*tss_ptr).rsp0 = kernel_stack_top;
    }
}

/// Get current kernel stack pointer
pub fn get_kernel_stack() -> u64 {
    unsafe { 
        let tss_ptr = &raw const TSS;
        (*tss_ptr).rsp0 
    }
}
