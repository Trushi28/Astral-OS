use super::fractal::{FractalRegion, FractalId, FractalCoord, Permissions};
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::format;
use x86_64::VirtAddr;
use spin::Mutex;
use crate::serial_println;

/// Spatial memory allocator using fractal regions
pub struct FractalAllocator {
    /// The root region (Universe)
    root: FractalRegion,
    
    /// All regions indexed by ID
    regions: BTreeMap<FractalId, FractalRegion>,
}

impl FractalAllocator {
    /// Create a new fractal allocator
    pub fn new(base: VirtAddr, size: usize) -> Self {
        let mut root = FractalRegion::universe(base, size);
        let root_id = root.id;
        
        let mut regions = BTreeMap::new();
        regions.insert(root_id, root.clone());
        
        serial_println!("[FRACTAL] Initialized universe at {:#x}, size {:#x}", 
                       base.as_u64(), size);
        
        Self {
            root,
            regions,
        }
    }
    
    /// Split a region into self-similar children
    /// 
    /// Creates `count` child regions, each with size = parent.size / count
    pub fn fractal_split(&mut self, region_id: FractalId, count: usize) -> Result<Vec<FractalId>, &'static str> {
        let region = self.regions.get(&region_id)
            .ok_or("Region not found")?;
        
        if !region.can_split(count) {
            return Err("Region too small to split");
        }
        
        let child_size = region.size / count;
        let parent_base = region.base;
        let parent_level = region.level;
        let parent_name = region.name.clone(); // Clone to avoid borrow issues
        let parent_dimension = region.fractal_dimension;
        
        let mut child_ids = Vec::new();
        
        for i in 0..count {
            let child_base = parent_base + (i * child_size) as u64;
            let child_name = format!("{}-Child{}", parent_name, i);
            
            let mut child = FractalRegion::new(
                child_name,
                parent_level + 1,
                child_base,
                child_size
            );
            
            child.parent = Some(region_id);
            child.fractal_dimension = parent_dimension;
            
            let child_id = child.id;
            self.regions.insert(child_id, child);
            child_ids.push(child_id);
        }
        
        // Update parent
        if let Some(parent) = self.regions.get_mut(&region_id) {
            parent.children = child_ids.clone();
        }
        
        serial_println!("[FRACTAL] Split region {} into {} children", 
                       parent_name, count);
        
        Ok(child_ids)
    }
    
    /// Merge children back into parent
    pub fn fractal_merge(&mut self, parent_id: FractalId) -> Result<(), &'static str> {
        let parent = self.regions.get(&parent_id)
            .ok_or("Parent not found")?;
        
        let child_ids = parent.children.clone();
        
        if child_ids.is_empty() {
            return Err("No children to merge");
        }
        
        // Remove all children
        for child_id in &child_ids {
            self.regions.remove(child_id);
        }
        
        // Clear parent's children list
        if let Some(parent) = self.regions.get_mut(&parent_id) {
            parent.children.clear();
            parent.allocated = false;
            serial_println!("[FRACTAL] Merged {} children back into {}", 
                           child_ids.len(), parent.name);
        }
        
        Ok(())
    }
    
    /// Allocate memory with spatial properties
    /// 
    /// # Arguments
    /// * `size` - Size in bytes
    /// * `dimension` - Fractal dimension (1.0-3.0)
    /// * `proximity` - Optional address to allocate near
    pub fn allocate_spatial(
        &mut self,
        size: usize,
        dimension: f32,
        proximity: Option<VirtAddr>,
    ) -> Result<VirtAddr, &'static str> {
        // Find suitable region
        let region_id = self.find_free_region(size, proximity)?;
        
        // Mark as allocated
        if let Some(region) = self.regions.get_mut(&region_id) {
            region.allocated = true;
            region.fractal_dimension = dimension;
            region.density = 1.0;
            
            serial_println!("[FRACTAL] Allocated {:#x} bytes at {:#x} (dim={:.1})",
                           size, region.base.as_u64(), dimension);
            
            Ok(region.base)
        } else {
            Err("Failed to allocate")
        }
    }
    
    /// Find a free region of at least `size` bytes
    fn find_free_region(&self, size: usize, proximity: Option<VirtAddr>) -> Result<FractalId, &'static str> {
        // Simple first-fit for now
        for (id, region) in &self.regions {
            if !region.allocated && region.size >= size {
                // If proximity specified, prefer closer regions
                if let Some(target) = proximity {
                    let distance = if region.base >= target {
                        region.base - target
                    } else {
                        target - region.base
                    };
                    
                    // Accept if within 1MB
                    if distance < 1024 * 1024 {
                        return Ok(*id);
                    }
                } else {
                    return Ok(*id);
                }
            }
        }
        
        Err("No suitable region found")
    }
    
    /// Translate fractal coordinates to virtual address
    pub fn fractal_to_virtual(&self, coord: FractalCoord) -> Option<VirtAddr> {
        // Walk down the tree to the specified level and index
        // Simplified implementation
        Some(self.root.base)
    }
    
    /// Get region by ID
    pub fn get_region(&self, id: FractalId) -> Option<&FractalRegion> {
        self.regions.get(&id)
    }
    
    /// Get root region
    pub fn root(&self) -> &FractalRegion {
        &self.root
    }
}

/// Global fractal allocator instance
pub static FRACTAL_ALLOCATOR: Mutex<Option<FractalAllocator>> = Mutex::new(None);

/// Initialize fractal allocator
pub fn init(base: VirtAddr, size: usize) {
    *FRACTAL_ALLOCATOR.lock() = Some(FractalAllocator::new(base, size));
    serial_println!("[FRACTAL] Allocator initialized");
}
