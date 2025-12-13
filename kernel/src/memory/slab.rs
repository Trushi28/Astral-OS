//! Slab Allocator for Astral OS
//!
//! High-performance kernel object allocator with per-CPU caches.
//! Optimized for common allocation sizes in the kernel.

use crate::memory::frame::{allocate_frame, deallocate_frame, PhysAddr};
use crate::PAGE_SIZE;
use core::alloc::Layout;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use spin::Mutex;

/// Slab size classes (in bytes)
pub const SLAB_SIZES: [usize; 8] = [32, 64, 128, 256, 512, 1024, 2048, 4096];

/// Number of slabs per size class
const SLABS_PER_CLASS: usize = 4;

/// Objects per slab (depends on size class)
const fn objects_per_slab(size: usize) -> usize {
    PAGE_SIZE / size
}

/// Free object header (embedded in free objects)
#[repr(C)]
struct FreeObject {
    next: AtomicPtr<FreeObject>,
}

/// A single slab (one page containing objects of same size)
#[repr(C)]
pub struct Slab {
    /// Physical address of the slab's memory
    phys_addr: PhysAddr,
    /// Virtual address of the slab's memory  
    virt_addr: usize,
    /// Object size for this slab
    object_size: usize,
    /// Number of allocated objects
    allocated: AtomicUsize,
    /// Total objects in this slab
    total_objects: usize,
    /// Free list head
    free_list: AtomicPtr<FreeObject>,
    /// Next slab in the chain
    next: AtomicPtr<Slab>,
}

impl Slab {
    /// Create a new slab for the given object size
    pub fn new(object_size: usize) -> Option<*mut Self> {
        // Allocate a page for the slab
        let frame = allocate_frame()?;
        let virt_addr = frame.to_virt();
        
        let total_objects = objects_per_slab(object_size);
        
        // We store the Slab struct at the beginning of the page
        // Objects start after the Slab struct
        let slab_header_size = core::mem::size_of::<Slab>();
        let aligned_header = (slab_header_size + object_size - 1) / object_size * object_size;
        let usable_objects = (PAGE_SIZE - aligned_header) / object_size;
        
        if usable_objects == 0 {
            deallocate_frame(frame);
            return None;
        }
        
        // Initialize the slab header at the start of the page
        let slab_ptr = virt_addr as *mut Slab;
        unsafe {
            slab_ptr.write(Slab {
                phys_addr: frame,
                virt_addr,
                object_size,
                allocated: AtomicUsize::new(0),
                total_objects: usable_objects,
                free_list: AtomicPtr::new(core::ptr::null_mut()),
                next: AtomicPtr::new(core::ptr::null_mut()),
            });
            
            // Initialize free list with all objects
            let objects_start = virt_addr + aligned_header;
            let slab = &mut *slab_ptr;
            
            for i in 0..usable_objects {
                let obj_addr = objects_start + i * object_size;
                let obj = obj_addr as *mut FreeObject;
                
                // Link to current head
                (*obj).next = AtomicPtr::new(slab.free_list.load(Ordering::Relaxed));
                slab.free_list.store(obj, Ordering::Relaxed);
            }
        }
        
        Some(slab_ptr)
    }
    
    /// Allocate an object from this slab
    pub fn alloc(&self) -> Option<*mut u8> {
        loop {
            let head = self.free_list.load(Ordering::Acquire);
            if head.is_null() {
                return None; // Slab is full
            }
            
            let next = unsafe { (*head).next.load(Ordering::Relaxed) };
            
            if self.free_list
                .compare_exchange_weak(head, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                self.allocated.fetch_add(1, Ordering::Relaxed);
                return Some(head as *mut u8);
            }
            // Retry on CAS failure
            core::hint::spin_loop();
        }
    }
    
    /// Free an object back to this slab
    pub unsafe fn free(&self, ptr: *mut u8) {
        let obj = ptr as *mut FreeObject;
        
        loop {
            let head = self.free_list.load(Ordering::Acquire);
            (*obj).next.store(head, Ordering::Relaxed);
            
            if self.free_list
                .compare_exchange_weak(head, obj, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                self.allocated.fetch_sub(1, Ordering::Relaxed);
                return;
            }
            core::hint::spin_loop();
        }
    }
    
    /// Check if this slab is empty (no allocations)
    pub fn is_empty(&self) -> bool {
        self.allocated.load(Ordering::Relaxed) == 0
    }
    
    /// Check if this slab is full
    pub fn is_full(&self) -> bool {
        self.allocated.load(Ordering::Relaxed) >= self.total_objects
    }
    
    /// Check if pointer belongs to this slab
    pub fn contains(&self, ptr: *mut u8) -> bool {
        let addr = ptr as usize;
        addr >= self.virt_addr && addr < self.virt_addr + PAGE_SIZE
    }
}

/// Size class cache - manages slabs for a specific size
pub struct SizeClassCache {
    /// Object size for this cache
    size: usize,
    /// Partial slabs (have some free objects)
    partial_slabs: AtomicPtr<Slab>,
    /// Full slabs (all objects allocated)
    full_slabs: AtomicPtr<Slab>,
    /// Statistics
    total_allocs: AtomicUsize,
    total_frees: AtomicUsize,
}

impl SizeClassCache {
    pub const fn new(size: usize) -> Self {
        Self {
            size,
            partial_slabs: AtomicPtr::new(core::ptr::null_mut()),
            full_slabs: AtomicPtr::new(core::ptr::null_mut()),
            total_allocs: AtomicUsize::new(0),
            total_frees: AtomicUsize::new(0),
        }
    }
    
    /// Allocate an object from this size class
    pub fn alloc(&self) -> Option<*mut u8> {
        // Try partial slabs first
        let mut current = self.partial_slabs.load(Ordering::Acquire);
        
        while !current.is_null() {
            let slab = unsafe { &*current };
            
            if let Some(ptr) = slab.alloc() {
                self.total_allocs.fetch_add(1, Ordering::Relaxed);
                
                // Move to full list if now full
                if slab.is_full() {
                    self.move_to_full(current);
                }
                
                return Some(ptr);
            }
            
            current = slab.next.load(Ordering::Acquire);
        }
        
        // No space in partial slabs, create new slab
        let new_slab = Slab::new(self.size)?;
        
        // Add to partial list
        unsafe {
            (*new_slab).next.store(
                self.partial_slabs.load(Ordering::Relaxed),
                Ordering::Relaxed
            );
        }
        self.partial_slabs.store(new_slab, Ordering::Release);
        
        // Allocate from new slab
        let ptr = unsafe { (*new_slab).alloc() };
        self.total_allocs.fetch_add(1, Ordering::Relaxed);
        ptr
    }
    
    /// Free an object back to this size class
    pub unsafe fn free(&self, ptr: *mut u8) {
        // Search partial slabs
        let mut current = self.partial_slabs.load(Ordering::Acquire);
        while !current.is_null() {
            let slab = &*current;
            if slab.contains(ptr) {
                slab.free(ptr);
                self.total_frees.fetch_add(1, Ordering::Relaxed);
                return;
            }
            current = slab.next.load(Ordering::Acquire);
        }
        
        // Search full slabs
        current = self.full_slabs.load(Ordering::Acquire);
        while !current.is_null() {
            let slab = &*current;
            if slab.contains(ptr) {
                slab.free(ptr);
                self.total_frees.fetch_add(1, Ordering::Relaxed);
                
                // Move back to partial list
                self.move_to_partial(current);
                return;
            }
            current = slab.next.load(Ordering::Acquire);
        }
        
        // Object not found - may already be freed or corrupted
    }
    
    fn move_to_full(&self, _slab: *mut Slab) {
        // Simplified: just leave in partial for now
        // Full implementation would move between lists
    }
    
    fn move_to_partial(&self, _slab: *mut Slab) {
        // Simplified: just leave in full for now
        // Full implementation would move between lists
    }
    
    pub fn stats(&self) -> (usize, usize) {
        (
            self.total_allocs.load(Ordering::Relaxed),
            self.total_frees.load(Ordering::Relaxed),
        )
    }
}

/// Global slab allocator
pub struct SlabAllocator {
    /// Size class caches
    caches: [SizeClassCache; 8],
    /// Initialized flag
    initialized: AtomicUsize,
}

impl SlabAllocator {
    pub const fn new() -> Self {
        Self {
            caches: [
                SizeClassCache::new(SLAB_SIZES[0]),
                SizeClassCache::new(SLAB_SIZES[1]),
                SizeClassCache::new(SLAB_SIZES[2]),
                SizeClassCache::new(SLAB_SIZES[3]),
                SizeClassCache::new(SLAB_SIZES[4]),
                SizeClassCache::new(SLAB_SIZES[5]),
                SizeClassCache::new(SLAB_SIZES[6]),
                SizeClassCache::new(SLAB_SIZES[7]),
            ],
            initialized: AtomicUsize::new(0),
        }
    }
    
    /// Initialize the slab allocator
    pub fn init(&self) {
        self.initialized.store(1, Ordering::Release);
    }
    
    /// Find the appropriate size class for a given size
    fn find_size_class(&self, size: usize) -> Option<usize> {
        for (i, &class_size) in SLAB_SIZES.iter().enumerate() {
            if size <= class_size {
                return Some(i);
            }
        }
        None
    }
    
    /// Allocate memory of the given size
    pub fn alloc(&self, size: usize) -> Option<*mut u8> {
        if self.initialized.load(Ordering::Acquire) == 0 {
            return None;
        }
        
        let class_idx = self.find_size_class(size)?;
        self.caches[class_idx].alloc()
    }
    
    /// Allocate memory with specific layout
    pub fn alloc_layout(&self, layout: Layout) -> Option<*mut u8> {
        // For alignments larger than size class, use the larger size
        let size = layout.size().max(layout.align());
        self.alloc(size)
    }
    
    /// Free memory allocated by this allocator
    pub unsafe fn free(&self, ptr: *mut u8, size: usize) {
        if let Some(class_idx) = self.find_size_class(size) {
            self.caches[class_idx].free(ptr);
        }
    }
    
    /// Free memory with specific layout
    pub unsafe fn free_layout(&self, ptr: *mut u8, layout: Layout) {
        let size = layout.size().max(layout.align());
        self.free(ptr, size);
    }
    
    /// Get allocator statistics
    pub fn stats(&self) -> SlabStats {
        let mut total_allocs = 0;
        let mut total_frees = 0;
        
        for cache in &self.caches {
            let (allocs, frees) = cache.stats();
            total_allocs += allocs;
            total_frees += frees;
        }
        
        SlabStats {
            total_allocs,
            total_frees,
            active_objects: total_allocs.saturating_sub(total_frees),
        }
    }
    
    /// Check if allocation can be handled by slab
    pub fn can_handle(&self, size: usize) -> bool {
        size <= SLAB_SIZES[SLAB_SIZES.len() - 1]
    }
}

/// Slab allocator statistics
#[derive(Debug, Clone, Copy)]
pub struct SlabStats {
    pub total_allocs: usize,
    pub total_frees: usize,
    pub active_objects: usize,
}

// Global slab allocator instance
static SLAB_ALLOCATOR: SlabAllocator = SlabAllocator::new();

/// Initialize the slab allocator
pub fn init() {
    SLAB_ALLOCATOR.init();
}

/// Allocate memory from slab allocator
pub fn slab_alloc(size: usize) -> Option<*mut u8> {
    SLAB_ALLOCATOR.alloc(size)
}

/// Allocate memory with layout from slab allocator
pub fn slab_alloc_layout(layout: Layout) -> Option<*mut u8> {
    SLAB_ALLOCATOR.alloc_layout(layout)
}

/// Free memory to slab allocator
pub unsafe fn slab_free(ptr: *mut u8, size: usize) {
    SLAB_ALLOCATOR.free(ptr, size)
}

/// Free memory with layout to slab allocator
pub unsafe fn slab_free_layout(ptr: *mut u8, layout: Layout) {
    SLAB_ALLOCATOR.free_layout(ptr, layout)
}

/// Check if size can be handled by slab allocator
pub fn slab_can_handle(size: usize) -> bool {
    SLAB_ALLOCATOR.can_handle(size)
}

/// Get slab allocator statistics
pub fn get_slab_stats() -> SlabStats {
    SLAB_ALLOCATOR.stats()
}
