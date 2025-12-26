// Memory safety features: guard pages, heap canaries, bounds checking

use x86_64::{VirtAddr, structures::paging::{PageTableFlags as Flags, Page, Size4KiB}};
use crate::serial_println;

/// Magic canary value to detect heap corruption
pub const HEAP_CANARY: u64 = 0xDEADBEEFCAFEBABE;

/// Guard page configuration
pub struct GuardPageConfig {
    pub enabled: bool,
    pub size: usize, // In pages
}

impl GuardPageConfig {
    pub const fn new() -> Self {
        Self {
            enabled: true,
            size: 1, // 1 guard page (4KB)
        }
    }
}

/// Allocation metadata with canary
#[repr(C)]
pub struct AllocationHeader {
    pub canary_start: u64,
    pub size: usize,
    pub magic: u32,
}

impl AllocationHeader {
    pub const MAGIC: u32 = 0x414C4C4F; // "ALLO" in hex
    
    pub fn new(size: usize) -> Self {
        Self {
            canary_start: HEAP_CANARY,
            size,
            magic: Self::MAGIC,
        }
    }
    
    pub fn verify(&self) -> bool {
        self.canary_start == HEAP_CANARY && self.magic == Self::MAGIC
    }
}

/// Create guard pages around a memory region
pub unsafe fn create_guard_pages(
    mapper: &mut impl x86_64::structures::paging::Mapper<Size4KiB>,
    region_start: VirtAddr,
    region_size: usize,
) -> Result<(), &'static str> {
    let guard_page_before = Page::containing_address(region_start - 4096u64);
    let guard_page_after = Page::containing_address(region_start + region_size as u64);
    
    // Unmap guard pages to cause page fault on access
    if let Ok(_) = mapper.unmap(guard_page_before) {
        serial_println!("[GUARD] Created guard page before region at {:#x}", guard_page_before.start_address().as_u64());
    }
    
    if let Ok(_) = mapper.unmap(guard_page_after) {
        serial_println!("[GUARD] Created guard page after region at {:#x}", guard_page_after.start_address().as_u64());
    }
    
    Ok(())
}

/// Dynamic heap expansion
pub struct HeapExpansion {
    pub current_size: usize,
    pub max_size: usize,
    pub growth_step: usize,
}

impl HeapExpansion {
    pub const fn new(initial_size: usize, max_size: usize) -> Self {
        Self {
            current_size: initial_size,
            max_size,
            growth_step: 1024 * 1024, // Grow by 1MB at a time
        }
    }
    
    pub fn can_expand(&self) -> bool {
        self.current_size < self.max_size
    }
    
    pub fn next_size(&self) -> usize {
        (self.current_size + self.growth_step).min(self.max_size)
    }
}

pub static GUARD_CONFIG: GuardPageConfig = GuardPageConfig::new();
