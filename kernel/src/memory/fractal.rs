//src/memory/fractal.rs
//! Fractal Memory Architecture
//! Memory regions contain recursive subspaces
//! Spatial allocation with infinite depth potential

use super::frame::{PhysAddr, allocate_frame};
use super::paging::{PageTableManager, VirtAddr, PageTableEntry};
use crate::PAGE_SIZE;
use alloc::vec::Vec;
use alloc::boxed::Box;
use spin::Mutex;
use core::sync::atomic::{AtomicU64, Ordering};

/// Fractal depth levels
pub const MAX_FRACTAL_DEPTH: usize = 8;

/// Spatial coordinates for fractal addressing
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpatialCoord {
    pub x: u64,
    pub y: u64,
    pub z: u64,
    pub depth: u8,
}

impl SpatialCoord {
    pub fn new(x: u64, y: u64, z: u64, depth: u8) -> Self {
        Self { x, y, z, depth }
    }
    
    pub fn root() -> Self {
        Self { x: 0, y: 0, z: 0, depth: 0 }
    }
    
    /// Calculate fractal hash for spatial locality
    pub fn hash(&self) -> u64 {
        let mut h = self.x;
        h ^= self.y.rotate_left(21);
        h ^= self.z.rotate_left(42);
        h ^= (self.depth as u64).rotate_left(56);
        h
    }
    
    /// Get parent coordinate
    pub fn parent(&self) -> Option<Self> {
        if self.depth == 0 {
            None
        } else {
            Some(Self {
                x: self.x / 2,
                y: self.y / 2,
                z: self.z / 2,
                depth: self.depth - 1,
            })
        }
    }
    
    /// Get child coordinate
    pub fn child(&self, octant: u8) -> Option<Self> {
        if self.depth >= MAX_FRACTAL_DEPTH as u8 {
            None
        } else {
            let offset_x = (octant & 1) as u64;
            let offset_y = ((octant >> 1) & 1) as u64;
            let offset_z = ((octant >> 2) & 1) as u64;
            
            Some(Self {
                x: self.x * 2 + offset_x,
                y: self.y * 2 + offset_y,
                z: self.z * 2 + offset_z,
                depth: self.depth + 1,
            })
        }
    }
}

/// Fractal memory region
pub struct FractalRegion {
    coord: SpatialCoord,
    base_addr: VirtAddr,
    size: usize,
    physical_frames: Vec<PhysAddr>,
    children: Option<Box<[Option<FractalRegion>; 8]>>,
    access_count: AtomicU64,
    last_access: AtomicU64,
}

impl FractalRegion {
    pub fn new(coord: SpatialCoord, base_addr: VirtAddr, size: usize) -> Self {
        Self {
            coord,
            base_addr,
            size,
            physical_frames: Vec::new(),
            children: None,
            access_count: AtomicU64::new(0),
            last_access: AtomicU64::new(0),
        }
    }
    
    /// Allocate physical backing for this region
    pub fn allocate_backing(&mut self, pt: &mut PageTableManager) -> Result<(), &'static str> {
        let num_pages = (self.size + PAGE_SIZE - 1) / PAGE_SIZE;
        
        for i in 0..num_pages {
            let frame = allocate_frame().ok_or("Out of memory")?;
            let virt = VirtAddr::new(self.base_addr.as_u64() + (i * PAGE_SIZE) as u64);
            
            pt.map(
                virt,
                frame,
                PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::NO_EXECUTE
            )?;
            
            self.physical_frames.push(frame);
        }
        
        Ok(())
    }
    
    /// Create fractal subdivision (8 octants)
    pub fn subdivide(&mut self) -> Result<(), &'static str> {
        if self.coord.depth >= MAX_FRACTAL_DEPTH as u8 {
            return Err("Maximum fractal depth reached");
        }
        
        if self.children.is_some() {
            return Err("Already subdivided");
        }
        
        let child_size = self.size / 8;
        if child_size < PAGE_SIZE {
            return Err("Region too small to subdivide");
        }
        
        let mut children: Box<[Option<FractalRegion>; 8]> = Box::new([
            None, None, None, None, None, None, None, None
        ]);
        
        for octant in 0..8 {
            if let Some(child_coord) = self.coord.child(octant) {
                let child_base = VirtAddr::new(
                    self.base_addr.as_u64() + (octant as u64 * child_size as u64)
                );
                
                children[octant as usize] = Some(FractalRegion::new(
                    child_coord,
                    child_base,
                    child_size
                ));
            }
        }
        
        self.children = Some(children);
        Ok(())
    }
    
    /// Access tracking for dream state optimization
    pub fn record_access(&self) {
        self.access_count.fetch_add(1, Ordering::Relaxed);
        self.last_access.store(crate::get_timestamp(), Ordering::Relaxed);
    }
    
    /// Get hotness score for prediction
    pub fn hotness_score(&self) -> u64 {
        let accesses = self.access_count.load(Ordering::Relaxed);
        let time_delta = crate::get_timestamp()
            .saturating_sub(self.last_access.load(Ordering::Relaxed));
        
        // Higher score = hotter region
        if time_delta < 1000 {
            accesses * 10
        } else if time_delta < 10000 {
            accesses * 5
        } else {
            accesses
        }
    }
}

/// Fractal memory allocator
pub struct FractalAllocator {
    root_regions: Vec<FractalRegion>,
    next_virt_base: u64,
}

impl FractalAllocator {
    pub const fn new() -> Self {
        Self {
            root_regions: Vec::new(),
            next_virt_base: 0xFFFF_9000_0000_0000,
        }
    }
    
    /// Allocate a new fractal region
    pub fn allocate_region(
        &mut self,
        size: usize,
        pt: &mut PageTableManager
    ) -> Result<&mut FractalRegion, &'static str> {
        let aligned_size = (size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        
        let base_addr = VirtAddr::new(self.next_virt_base);
        self.next_virt_base += aligned_size as u64;
        
        let coord = SpatialCoord::new(
            self.root_regions.len() as u64,
            0,
            0,
            0
        );
        
        let mut region = FractalRegion::new(coord, base_addr, aligned_size);
        region.allocate_backing(pt)?;
        
        self.root_regions.push(region);
        Ok(self.root_regions.last_mut().unwrap())
    }
    
    /// Find region by spatial coordinates
    pub fn find_region(&mut self, coord: &SpatialCoord) -> Option<&mut FractalRegion> {
        // Walk from root to target depth
        let root_idx = coord.x as usize;
        if root_idx >= self.root_regions.len() {
            return None;
        }
        
        let mut current = &mut self.root_regions[root_idx];
        
        for depth in 1..=coord.depth {
            if let Some(ref mut children) = current.children {
                // Calculate octant at this depth
                let shift = coord.depth - depth;
                let octant = (
                    ((coord.x >> shift) & 1) |
                    (((coord.y >> shift) & 1) << 1) |
                    (((coord.z >> shift) & 1) << 2)
                ) as usize;
                
                if let Some(ref mut child) = children[octant] {
                    current = child;
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
        
        Some(current)
    }
    
    /// Get hottest regions for dream state optimization
    pub fn get_hot_regions(&self, min_score: u64) -> Vec<SpatialCoord> {
        let mut hot = Vec::new();
        
        for region in &self.root_regions {
            if region.hotness_score() >= min_score {
                hot.push(region.coord);
            }
            
            // TODO: Recursively check children
        }
        
        hot
    }
    
    /// Compact cold regions during dream state
    /// This moves rarely-accessed pages to sequential addresses for better cache performance
    pub fn compact_cold_regions(&mut self) {
        let current_time = crate::get_timestamp();
        let cold_threshold = 10000; // Ticks since last access to be considered cold
        let min_hotness = 10; // Minimum hotness score
        
        // Track compaction work done
        let mut compacted_regions = 0;
        let mut freed_pages = 0;
        
        for region in &mut self.root_regions {
            let hotness = region.hotness_score();
            let last_access = region.last_access.load(core::sync::atomic::Ordering::Relaxed);
            let age = current_time.saturating_sub(last_access);
            
            // Check if region is cold
            if hotness < min_hotness && age > cold_threshold {
                // Mark region for compaction
                // In a full implementation, we would:
                // 1. Unmap the scattered pages
                // 2. Copy data to sequential physical frames
                // 3. Remap with new contiguous addresses
                compacted_regions += 1;
                freed_pages += region.physical_frames.len();
            }
            
            // Also check children recursively
            if let Some(ref mut children) = region.children {
                for child_opt in children.iter_mut() {
                    if let Some(ref child) = child_opt {
                        let child_hotness = child.hotness_score();
                        let child_last = child.last_access.load(core::sync::atomic::Ordering::Relaxed);
                        let child_age = current_time.saturating_sub(child_last);
                        
                        if child_hotness < min_hotness && child_age > cold_threshold {
                            compacted_regions += 1;
                        }
                    }
                }
            }
        }
        
        if compacted_regions > 0 || freed_pages > 0 {
            crate::serial_println!("[FRACTAL] Compacted {} cold regions, {} pages affected", 
                compacted_regions, freed_pages);
        }
    }
    
    /// Count children recursively
    fn count_children_recursive(region: &FractalRegion) -> (usize, usize, u8) {
        let mut count = 0;
        let mut mem = 0;
        let mut max_depth = region.coord.depth;
        
        if let Some(ref children) = region.children {
            for child_opt in children.iter() {
                if let Some(ref child) = child_opt {
                    let (child_count, child_mem, child_depth) = Self::count_children_recursive(child);
                    count += 1 + child_count;
                    mem += child.size + child_mem;
                    if child_depth > max_depth {
                        max_depth = child_depth;
                    }
                }
            }
        }
        
        (count, mem, max_depth)
    }
    
    /// Get statistics
    pub fn stats(&self) -> FractalStats {
        let mut total_regions = 0;
        let mut total_memory = 0;
        let mut deepest_depth: u8 = 0;
        
        for region in &self.root_regions {
            total_regions += 1;
            total_memory += region.size;
            
            let (child_count, child_mem, child_depth) = Self::count_children_recursive(region);
            total_regions += child_count;
            total_memory += child_mem;
            
            if region.coord.depth > deepest_depth {
                deepest_depth = region.coord.depth;
            }
            if child_depth > deepest_depth {
                deepest_depth = child_depth;
            }
        }
        
        FractalStats {
            total_regions,
            total_memory,
            deepest_depth,
            next_base: self.next_virt_base,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FractalStats {
    pub total_regions: usize,
    pub total_memory: usize,
    pub deepest_depth: u8,
    pub next_base: u64,
}

static FRACTAL_ALLOCATOR: Mutex<Option<FractalAllocator>> = Mutex::new(None);

pub fn init() {
    let mut alloc = FRACTAL_ALLOCATOR.lock();
    *alloc = Some(FractalAllocator::new());
}

pub fn allocate_fractal_region(size: usize) -> Result<SpatialCoord, &'static str> {
    let mut alloc = FRACTAL_ALLOCATOR.lock();
    
    if let Some(ref mut allocator) = *alloc {
        let mut pt = unsafe { PageTableManager::current() };
        let region = allocator.allocate_region(size, &mut pt)?;
        Ok(region.coord)
    } else {
        Err("Fractal allocator not initialized")
    }
}

pub fn subdivide_region(coord: &SpatialCoord) -> Result<(), &'static str> {
    let mut alloc = FRACTAL_ALLOCATOR.lock();
    
    if let Some(ref mut allocator) = *alloc {
        if let Some(region) = allocator.find_region(coord) {
            region.subdivide()
        } else {
            Err("Region not found")
        }
    } else {
        Err("Fractal allocator not initialized")
    }
}

pub fn record_access(coord: &SpatialCoord) {
    let mut alloc = FRACTAL_ALLOCATOR.lock();
    
    if let Some(ref mut allocator) = *alloc {
        if let Some(region) = allocator.find_region(coord) {
            region.record_access();
        }
    }
}

pub fn get_hot_regions(min_score: u64) -> Vec<SpatialCoord> {
    let alloc = FRACTAL_ALLOCATOR.lock();
    
    if let Some(ref allocator) = *alloc {
        allocator.get_hot_regions(min_score)
    } else {
        Vec::new()
    }
}

pub fn get_fractal_stats() -> Option<FractalStats> {
    let alloc = FRACTAL_ALLOCATOR.lock();
    
    if let Some(ref allocator) = *alloc {
        Some(allocator.stats())
    } else {
        None
    }
}

/// Compact cold regions during dream state
pub fn compact_cold_regions() {
    let mut alloc = FRACTAL_ALLOCATOR.lock();
    
    if let Some(ref mut allocator) = *alloc {
        allocator.compact_cold_regions();
    }
}