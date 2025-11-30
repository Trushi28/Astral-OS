//src/memory/heap.rs
use super::paging::{PageTableManager, VirtAddr};
use super::frame::allocate_frame;
use crate::{PAGE_SIZE, KERNEL_HEAP_SIZE, util::align_up};
use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};
use core::ptr::NonNull;
use spin::Mutex;

const HEAP_START: usize = 0xFFFF_8000_0000_0000;

/// Linked list node for free blocks
#[repr(C)]
struct FreeBlock {
    size: usize,
    next: Option<NonNull<FreeBlock>>,
}

unsafe impl Send for FreeBlock {}
unsafe impl Sync for FreeBlock {}

/// Proper heap allocator with free list
pub struct LinkedListAllocator {
    free_list: Option<NonNull<FreeBlock>>,
    heap_start: usize,
    heap_end: usize,
    allocated: AtomicUsize,
}

unsafe impl Send for LinkedListAllocator {}
unsafe impl Sync for LinkedListAllocator {}

impl LinkedListAllocator {
    pub const fn new() -> Self {
        Self {
            free_list: None,
            heap_start: 0,
            heap_end: 0,
            allocated: AtomicUsize::new(0),
        }
    }
    
    /// Initialize heap with page table
    pub unsafe fn init(&mut self, pt: &mut PageTableManager) {
        use super::paging::PageTableEntry;
        
        self.heap_start = HEAP_START;
        self.heap_end = HEAP_START + KERNEL_HEAP_SIZE;
        
        // Map heap pages
        for offset in (0..KERNEL_HEAP_SIZE).step_by(PAGE_SIZE) {
            let virt = VirtAddr::new((HEAP_START + offset) as u64);
            
            if let Some(frame) = allocate_frame() {
                let _ = pt.map(
                    virt,
                    frame,
                    PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::NO_EXECUTE
                );
            } else {
                panic!("Failed to allocate heap page");
            }
        }
        
        // Initialize free list with entire heap as one block
        let initial_block = HEAP_START as *mut FreeBlock;
        initial_block.write(FreeBlock {
            size: KERNEL_HEAP_SIZE,
            next: None,
        });
        
        self.free_list = NonNull::new(initial_block);
    }
    
    /// Allocate memory
    unsafe fn alloc_impl(&mut self, layout: Layout) -> *mut u8 {
        let size = align_up(layout.size().max(core::mem::size_of::<FreeBlock>()), layout.align());
        let align = layout.align();
        
        // Search free list for suitable block
        let mut current = self.free_list;
        let mut prev: Option<NonNull<FreeBlock>> = None;
        
        while let Some(node) = current {
            let node_ref = node.as_ref();
            let node_addr = node.as_ptr() as usize;
            let aligned_addr = align_up(node_addr, align);
            let padding = aligned_addr - node_addr;
            let required_size = padding + size;
            
            if node_ref.size >= required_size {
                // Found suitable block
                let alloc_start = aligned_addr;
                let alloc_end = alloc_start + size;
                
                if node_ref.size > required_size + core::mem::size_of::<FreeBlock>() {
                    // Split block
                    let remainder_size = node_ref.size - required_size;
                    let remainder_addr = alloc_end;
                    
                    let remainder = remainder_addr as *mut FreeBlock;
                    remainder.write(FreeBlock {
                        size: remainder_size,
                        next: node_ref.next,
                    });
                    
                    // Update free list
                    if let Some(mut p) = prev {
                        p.as_mut().next = NonNull::new(remainder);
                    } else {
                        self.free_list = NonNull::new(remainder);
                    }
                } else {
                    // Use entire block
                    if let Some(mut p) = prev {
                        p.as_mut().next = node_ref.next;
                    } else {
                        self.free_list = node_ref.next;
                    }
                }
                
                self.allocated.fetch_add(size, Ordering::Relaxed);
                return alloc_start as *mut u8;
            }
            
            prev = current;
            current = node_ref.next;
        }
        
        core::ptr::null_mut()
    }
    
    /// Deallocate memory
    unsafe fn dealloc_impl(&mut self, ptr: *mut u8, layout: Layout) {
        let size = align_up(layout.size().max(core::mem::size_of::<FreeBlock>()), layout.align());
        let addr = ptr as usize;
        
        // Create new free block
        let new_block = addr as *mut FreeBlock;
        
        // Insert into free list (sorted by address)
        let mut current = self.free_list;
        let mut prev: Option<NonNull<FreeBlock>> = None;
        
        while let Some(node) = current {
            if node.as_ptr() as usize > addr {
                break;
            }
            prev = current;
            current = unsafe { node.as_ref().next };
        }
        
        // Try to merge with next block
        let next_addr = addr + size;
        let _merged_next = if let Some(node) = current {
            if node.as_ptr() as usize == next_addr {
                // Merge with next
                let merged_size = size + node.as_ref().size;
                let merged_next = node.as_ref().next;
                new_block.write(FreeBlock {
                    size: merged_size,
                    next: merged_next,
                });
                true
            } else {
                new_block.write(FreeBlock {
                    size,
                    next: current,
                });
                false
            }
        } else {
            new_block.write(FreeBlock {
                size,
                next: None,
            });
            false
        };
        
        // Try to merge with previous block
        if let Some(mut p) = prev {
            let prev_ref = p.as_mut();
            let prev_addr = p.as_ptr() as usize;
            let prev_end = prev_addr + prev_ref.size;
            
            if prev_end == addr {
                // Merge with previous
                prev_ref.size += (*new_block).size;
                prev_ref.next = (*new_block).next;
            } else {
                prev_ref.next = NonNull::new(new_block);
            }
        } else {
            self.free_list = NonNull::new(new_block);
        }
        
        self.allocated.fetch_sub(size, Ordering::Relaxed);
    }
    
    pub fn allocated(&self) -> usize {
        self.allocated.load(Ordering::Relaxed)
    }
    
    pub fn available(&self) -> usize {
        KERNEL_HEAP_SIZE.saturating_sub(self.allocated())
    }
}

// Newtype wrapper to satisfy orphan rules
pub struct HeapAllocator(Mutex<LinkedListAllocator>);

impl HeapAllocator {
    pub const fn new() -> Self {
        Self(Mutex::new(LinkedListAllocator::new()))
    }
}

unsafe impl GlobalAlloc for HeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0.lock().alloc_impl(layout)
    }
    
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0.lock().dealloc_impl(ptr, layout)
    }
}

#[global_allocator]
static HEAP_ALLOCATOR: HeapAllocator = HeapAllocator::new();

pub fn init() {
    unsafe {
        let mut pt = PageTableManager::current();
        HEAP_ALLOCATOR.0.lock().init(&mut pt);
    }
}

pub fn get_stats() -> (usize, usize) {
    let alloc = HEAP_ALLOCATOR.0.lock();
    (alloc.allocated(), alloc.available())
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("Allocation error: {:?}", layout);
}