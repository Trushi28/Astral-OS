//! User Memory Management
//! 
//! Handles creation and management of user address spaces:
//! - Per-process page tables (PML4)
//! - User-accessible memory mappings
//! - User stack allocation with guard pages

use x86_64::{
    structures::paging::{
        PageTable, Page, PhysFrame, Mapper, Size4KiB,
        FrameAllocator, PageTableFlags as Flags, OffsetPageTable,
    },
    VirtAddr, PhysAddr,
    registers::control::Cr3,
};
use crate::serial_println;
use super::buddy::BUDDY_ALLOCATOR as FRAME_ALLOCATOR;

/// User address space layout constants
/// 
/// Layout:
/// ```text
/// 0x0000_0000_0000_0000 - 0x0000_0000_0040_0000  : Reserved (null guard)
/// 0x0000_0000_0040_0000 - 0x0000_0000_0080_0000  : User code (.text)
/// 0x0000_0000_0080_0000 - 0x0000_0000_00C0_0000  : User data (.data, .bss)
/// 0x0000_7FFF_FFFF_0000 - 0x0000_7FFF_FFFF_F000  : User stack (grows down)
/// 0x0000_7FFF_FFFF_F000 - 0x0000_8000_0000_0000  : Stack guard page
/// ```

/// Start of user code region
pub const USER_CODE_START: u64 = 0x0000_0000_0040_0000;

/// Start of user data region  
pub const USER_DATA_START: u64 = 0x0000_0000_0080_0000;

/// Top of user stack (stack grows down from here)
pub const USER_STACK_TOP: u64 = 0x0000_7FFF_FFFF_F000;

/// Size of user stack (64KB)
pub const USER_STACK_SIZE: u64 = 64 * 1024;

/// Stack guard page (unmapped to catch overflow)
pub const USER_STACK_GUARD: u64 = USER_STACK_TOP - USER_STACK_SIZE - 4096;

/// Physical memory offset from bootloader (HHDM)
static mut PHYSICAL_MEMORY_OFFSET: Option<VirtAddr> = None;

/// Initialize user memory module with physical memory offset
pub fn init(phys_offset: VirtAddr) {
    unsafe {
        PHYSICAL_MEMORY_OFFSET = Some(phys_offset);
    }
    serial_println!("[USER_MEMORY] Initialized with offset: {:#x}", phys_offset.as_u64());
}

/// Get physical memory offset
fn get_phys_offset() -> VirtAddr {
    unsafe {
        PHYSICAL_MEMORY_OFFSET.expect("User memory not initialized")
    }
}

/// Create a new user page table (PML4)
/// 
/// Returns: (CR3 value, virtual address of PML4)
pub fn create_user_page_table() -> Result<(u64, VirtAddr), &'static str> {
    let phys_offset = get_phys_offset();
    
    // Allocate a physical frame for the new PML4
    let pml4_frame = {
        let mut allocator = FRAME_ALLOCATOR.lock();
        allocator.allocate_frame().ok_or("Failed to allocate PML4 frame")?
    };
    
    let pml4_phys = pml4_frame.start_address();
    let pml4_virt = phys_offset + pml4_phys.as_u64();
    
    // Initialize PML4 to zeros
    let pml4_ptr: *mut PageTable = pml4_virt.as_mut_ptr();
    unsafe {
        core::ptr::write_bytes(pml4_ptr, 0, 1);
    }
    
    // Clone kernel mappings (upper half: entries 256-511)
    // These map kernel code, heap, and the higher half direct map
    let kernel_pml4 = get_kernel_pml4();
    let new_pml4 = unsafe { &mut *pml4_ptr };
    
    for i in 256..512 {
        new_pml4[i] = kernel_pml4[i].clone();
    }
    
    serial_println!("[USER_MEMORY] Created user PML4 at phys {:#x}", pml4_phys.as_u64());
    
    Ok((pml4_phys.as_u64(), pml4_virt))
}

/// Get reference to kernel's PML4
fn get_kernel_pml4() -> &'static PageTable {
    let phys_offset = get_phys_offset();
    let (pml4_frame, _) = Cr3::read();
    let pml4_phys = pml4_frame.start_address();
    let pml4_virt = phys_offset + pml4_phys.as_u64();
    
    unsafe { &*(pml4_virt.as_ptr() as *const PageTable) }
}

/// Allocate user stack with guard page
/// 
/// Returns the top of the stack (stack grows DOWN from this address)
pub fn allocate_user_stack() -> Result<VirtAddr, &'static str> {
    let phys_offset = get_phys_offset();
    
    // Get current page table
    let (pml4_frame, _) = Cr3::read();
    let pml4_virt = phys_offset + pml4_frame.start_address().as_u64();
    let mut mapper = unsafe { 
        OffsetPageTable::new(&mut *(pml4_virt.as_mut_ptr()), phys_offset) 
    };
    
    // Map stack pages (USER_STACK_TOP - USER_STACK_SIZE to USER_STACK_TOP)
    let stack_bottom = USER_STACK_TOP - USER_STACK_SIZE;
    let page_count = USER_STACK_SIZE / 4096;
    
    let flags = Flags::PRESENT | Flags::WRITABLE | Flags::USER_ACCESSIBLE | Flags::NO_EXECUTE;
    
    for i in 0..page_count {
        let page_addr = VirtAddr::new(stack_bottom + i * 4096);
        let page = Page::<Size4KiB>::containing_address(page_addr);
        
        let frame = {
            let mut allocator = FRAME_ALLOCATOR.lock();
            allocator.allocate_frame().ok_or("Out of memory for user stack")?
        };
        
        unsafe {
            mapper.map_to(page, frame, flags, &mut *FRAME_ALLOCATOR.lock())
                .map_err(|_| "Failed to map user stack page")?
                .flush();
        }
    }
    
    // Guard page is simply not mapped - any access will cause page fault
    serial_println!("[USER_MEMORY] Allocated user stack: {:#x} - {:#x}", 
        stack_bottom, USER_STACK_TOP);
    serial_println!("[USER_MEMORY] Stack guard at: {:#x}", USER_STACK_GUARD);
    
    Ok(VirtAddr::new(USER_STACK_TOP))
}

/// Allocate and map a user code page
pub fn allocate_user_code_page(addr: VirtAddr) -> Result<(), &'static str> {
    let phys_offset = get_phys_offset();
    
    let (pml4_frame, _) = Cr3::read();
    let pml4_virt = phys_offset + pml4_frame.start_address().as_u64();
    let mut mapper = unsafe { 
        OffsetPageTable::new(&mut *(pml4_virt.as_mut_ptr()), phys_offset) 
    };
    
    let page = Page::<Size4KiB>::containing_address(addr);
    
    // Code pages: readable, executable, user accessible (NOT writable for W^X)
    // But for testing, we need to write first, so we make it writable temporarily
    let flags = Flags::PRESENT | Flags::WRITABLE | Flags::USER_ACCESSIBLE;
    
    let frame = {
        let mut allocator = FRAME_ALLOCATOR.lock();
        allocator.allocate_frame().ok_or("Out of memory for user code")?
    };
    
    unsafe {
        mapper.map_to(page, frame, flags, &mut *FRAME_ALLOCATOR.lock())
            .map_err(|_| "Failed to map user code page")?
            .flush();
    }
    
    serial_println!("[USER_MEMORY] Mapped user code page at {:#x}", addr.as_u64());
    
    Ok(())
}

/// Map a region for user access
pub fn map_user_region(
    start: VirtAddr,
    size: u64,
    writable: bool,
    executable: bool,
) -> Result<(), &'static str> {
    let phys_offset = get_phys_offset();
    
    let (pml4_frame, _) = Cr3::read();
    let pml4_virt = phys_offset + pml4_frame.start_address().as_u64();
    let mut mapper = unsafe { 
        OffsetPageTable::new(&mut *(pml4_virt.as_mut_ptr()), phys_offset) 
    };
    
    let page_count = (size + 4095) / 4096;
    
    let mut flags = Flags::PRESENT | Flags::USER_ACCESSIBLE;
    if writable {
        flags |= Flags::WRITABLE;
    }
    if !executable {
        flags |= Flags::NO_EXECUTE;
    }
    
    for i in 0..page_count {
        let page_addr = start + i * 4096;
        let page = Page::<Size4KiB>::containing_address(page_addr);
        
        let frame = {
            let mut allocator = FRAME_ALLOCATOR.lock();
            allocator.allocate_frame().ok_or("Out of memory")?
        };
        
        unsafe {
            mapper.map_to(page, frame, flags, &mut *FRAME_ALLOCATOR.lock())
                .map_err(|_| "Failed to map user region")?
                .flush();
        }
    }
    
    serial_println!("[USER_MEMORY] Mapped user region {:#x} - {:#x} (W={}, X={})",
        start.as_u64(), start.as_u64() + size, writable, executable);
    
    Ok(())
}
