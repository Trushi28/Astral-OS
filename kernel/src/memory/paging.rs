use x86_64::{
    structures::paging::{
        PageTable, OffsetPageTable, Page, PhysFrame, Mapper, Size4KiB,
        FrameAllocator, PageTableFlags as Flags,
    },
    VirtAddr, PhysAddr,
};
use crate::serial_println;
use super::frame_allocator::FRAME_ALLOCATOR;

/// Initialize a new OffsetPageTable from the level 4 table
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

/// Returns a mutable reference to the active level 4 table
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr
}

/// Map a virtual page to a physical frame
pub fn map_page(
    mapper: &mut OffsetPageTable,
    page: Page,
    frame: PhysFrame,
    flags: Flags,
) -> Result<(), &'static str> {
    let mut frame_allocator = FRAME_ALLOCATOR.lock();
    
    unsafe {
        mapper
            .map_to(page, frame, flags, &mut *frame_allocator)
            .map_err(|_| "map_to failed")?
            .flush();
    }
    
    Ok(())
}

/// Map a range of virtual pages to physical frames
pub fn map_range(
    mapper: &mut OffsetPageTable,
    start_virt: VirtAddr,
    start_phys: PhysAddr,
    size: u64,
    flags: Flags,
) -> Result<(), &'static str> {
    let page_count = (size + 4095) / 4096;
    
    for i in 0..page_count {
        let virt = start_virt + i * 4096;
        let phys = start_phys + i * 4096;
        
        let page = Page::containing_address(virt);
        let frame = PhysFrame::containing_address(phys);
        
        map_page(mapper, page, frame, flags)?;
    }
    
    Ok(())
}

/// Allocate and map a new page
pub fn allocate_and_map(
    mapper: &mut OffsetPageTable,
    page: Page,
    flags: Flags,
) -> Result<(), &'static str> {
    let mut frame_allocator = FRAME_ALLOCATOR.lock();
    
    let frame = frame_allocator
        .allocate_frame()
        .ok_or("out of memory")?;
    
    drop(frame_allocator);
    
    map_page(mapper, page, frame, flags)
}

/// Map heap region
pub fn map_heap(
    mapper: &mut OffsetPageTable,
    heap_start: VirtAddr,
    heap_size: u64,
) -> Result<(), &'static str> {
    serial_println!("[PAGING] Mapping heap region at {:#x} ({} KB)", 
        heap_start.as_u64(), heap_size / 1024);
    
    let page_range = {
        let heap_end = heap_start + heap_size - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    let flags = Flags::PRESENT | Flags::WRITABLE;

    for page in page_range {
        allocate_and_map(mapper, page, flags)?;
    }

    serial_println!("[PAGING] Heap mapped successfully");
    Ok(())
}
