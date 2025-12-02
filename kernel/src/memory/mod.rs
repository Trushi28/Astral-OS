//src/memory/mod.rs
pub mod frame;
pub mod paging;
pub mod heap;
pub mod fractal;

pub use frame::{PhysAddr, allocate_frame, deallocate_frame};
pub use paging::{VirtAddr, PageTableEntry, PageTableManager};

use crate::{HHDM_REQUEST, MEMORY_MAP_REQUEST};

pub fn init() {
    // Get HHDM offset from Limine
    if let Some(hhdm) = HHDM_REQUEST.get_response() {
        crate::set_hhdm_offset(hhdm.offset() as usize);
    } else {
        panic!("No HHDM response from bootloader!");
    }
    
    // Initialize frame allocator
    if let Some(mmap) = MEMORY_MAP_REQUEST.get_response() {
        frame::init(mmap);
    } else {
        panic!("No memory map from bootloader!");
    }
    
    // Initialize heap
    heap::init();
    
    // Initialize fractal allocator
    fractal::init();
    
    // Print stats
    let (total, used, free) = frame::get_stats();
    crate::println!("  Frames: {} total, {} used, {} free", total, used, free);
    crate::println!("  Memory: {} MB total, {} MB free", 
        total * 4 / 1024, free * 4 / 1024);
    
    let (heap_used, heap_free) = heap::get_stats();
    crate::println!("  Heap: {} KB used, {} KB free",
        heap_used / 1024, heap_free / 1024);
    
    if let Some(stats) = fractal::get_fractal_stats() {
        crate::println!("  Fractal: Initialized");
    }
}