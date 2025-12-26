use x86_64::VirtAddr;

/// CPU context for context switching
/// 
/// CRITICAL: This matches the exact layout expected by our inline assembly
/// context switch code. DO NOT reorder fields!
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Context {
    // General purpose registers (callee-saved)
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbp: u64,
    pub rbx: u64,
    
    // Instruction pointer and stack pointer
    pub rip: u64,
    pub rsp: u64,
    
    // RFLAGS
    pub rflags: u64,
    
    // Segment registers (for userspace)
    pub cs: u64,
    pub ss: u64,
}

impl Context {
    /// Create a new context for a kernel thread
    pub fn new_kernel(entry_point: VirtAddr, stack: VirtAddr) -> Self {
        Self {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            rbp: 0,
            rbx: 0,
            rip: entry_point.as_u64(),
            rsp: stack.as_u64(),
            rflags: 0x202, // IF (interrupts enabled) + reserved bit 1
            cs: 0x08,      // Kernel code segment
            ss: 0x10,      // Kernel data segment
        }
    }
    
    /// Create a new context for a userspace process
    #[allow(dead_code)]
    pub fn new_user(entry_point: VirtAddr, stack: VirtAddr) -> Self {
        Self {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            rbp: 0,
            rbx: 0,
            rip: entry_point.as_u64(),
            rsp: stack.as_u64(),
            rflags: 0x202, // IF + reserved bit
            cs: 0x1B,      // User code segment (GDT selector 3, RPL=3)
            ss: 0x23,      // User data segment (GDT selector 4, RPL=3)
        }
    }
}
