use linked_list_allocator::LockedHeap;
use crate::serial_println;
use limine::request::{MemoryMapRequest, HhdmRequest};
use x86_64::{VirtAddr, structures::paging::PageTableFlags as Flags};

pub mod frame_allocator;
pub mod paging;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

// Limine requests
static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

const HEAP_START: usize = 0x_4444_4444_0000;
const HEAP_SIZE: usize = 1024 * 1024; // 1 MB

/// Memory statistics
pub struct MemoryStats {
    pub total_memory: usize,
    pub usable_memory: usize,
    pub reserved_memory: usize,
}

pub fn get_memory_stats() -> MemoryStats {
    let mut stats = MemoryStats {
        total_memory: 0,
        usable_memory: 0,
        reserved_memory: 0,
    };

    if let Some(memory_map_response) = MEMORY_MAP_REQUEST.get_response() {
        for entry in memory_map_response.entries().iter() {
            let size = (entry.length) as usize;
            stats.total_memory += size;

            match entry.entry_type {
                limine::memory_map::EntryType::USABLE => {
                    stats.usable_memory += size;
                }
                _ => {
                    stats.reserved_memory += size;
                }
            }
        }
    }

    stats
}

pub fn init() {
    serial_println!("[MEM] Memory Map from Limine:");
    
    if let Some(memory_map_response) = MEMORY_MAP_REQUEST.get_response() {
        let entries = memory_map_response.entries();
        let entry_count = entries.len();
        serial_println!("[MEM] Found {} memory regions", entry_count);

        for (i, entry) in entries.iter().enumerate() {
            let base = entry.base;
            let length = entry.length;
            let end = base + length;
            let type_str = match entry.entry_type {
                limine::memory_map::EntryType::USABLE => "USABLE",
                limine::memory_map::EntryType::RESERVED => "RESERVED",
                limine::memory_map::EntryType::ACPI_RECLAIMABLE => "ACPI_RECLAIMABLE",
                limine::memory_map::EntryType::ACPI_NVS => "ACPI_NVS",
                limine::memory_map::EntryType::BAD_MEMORY => "BAD",
                limine::memory_map::EntryType::BOOTLOADER_RECLAIMABLE => "BOOTLOADER",
                limine::memory_map::EntryType::EXECUTABLE_AND_MODULES => "KERNEL",
                limine::memory_map::EntryType::FRAMEBUFFER => "FRAMEBUFFER",
                _ => "UNKNOWN",
            };

            serial_println!(
                "[MEM]   #{}: 0x{:016x} - 0x{:016x} ({} KB) [{}]",
                i,
                base,
                end,
                length / 1024,
                type_str
            );
        }

        // Initialize frame allocator
        serial_println!();
        serial_println!("[MEM] Initializing physical frame allocator...");
        frame_allocator::FRAME_ALLOCATOR.lock().init(entries);

        let (used, total) = frame_allocator::FRAME_ALLOCATOR.lock().stats();
        serial_println!("[MEM] Frame allocator ready: {}/{} frames used", used, total);
        
    } else {
        serial_println!("[MEM] ERROR: No memory map available from bootloader!");
        return;
    }

    // Get and display memory statistics
    let stats = get_memory_stats();
    serial_println!();
    serial_println!("[MEM] Memory Statistics:");
    serial_println!("[MEM]   Total:    {} MB", stats.total_memory / (1024 * 1024));
    serial_println!("[MEM]   Usable:   {} MB", stats.usable_memory / (1024 * 1024));
    serial_println!("[MEM]   Reserved: {} MB", stats.reserved_memory / (1024 * 1024));
    serial_println!();

    // Initialize paging and map heap
    if let Some(hhdm_response) = HHDM_REQUEST.get_response() {
        let physical_memory_offset = VirtAddr::new(hhdm_response.offset());
        serial_println!("[MEM] Physical memory offset: {:#x}", physical_memory_offset.as_u64());
        
        let mut mapper = unsafe { paging::init(physical_memory_offset) };
        
        // Map heap region
        let heap_start = VirtAddr::new(HEAP_START as u64);
        if let Err(e) = paging::map_heap(&mut mapper, heap_start, HEAP_SIZE as u64) {
            serial_println!("[MEM] ERROR: Failed to map heap: {}", e);
            return;
        }
        
        // Initialize heap allocator
        serial_println!("[MEM] Initializing heap allocator...");
        unsafe {
            ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
        }
        serial_println!("[MEM] Heap allocator initialized at {:#x}", HEAP_START);
        
        // Test allocation
        let test_vec = alloc::vec![1, 2, 3, 4, 5];
        serial_println!("[MEM] Test allocation successful: {:?}", test_vec);
        serial_println!("[MEM] Vec allocations are now working!");
        
    } else {
        serial_println!("[MEM] ERROR: No HHDM response from bootloader!");
    }
    
    serial_println!("[MEM] Phase 2: Memory management complete!");
}
