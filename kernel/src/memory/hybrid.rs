/// Hybrid memory allocation interface
/// 
/// Provides both traditional allocations (via buddy) and 
/// fractal spatial allocations for advanced use cases
use x86_64::VirtAddr;
use crate::serial_println;

/// Allocation type selector
pub enum AllocationType {
    /// Traditional frame allocation via buddy allocator
    Traditional,
    
    /// Fractal spatial allocation with geometric properties
    Fractal { dimension: f32, proximity: Option<VirtAddr> },
}

/// Allocate memory with hybrid system
/// 
/// # Arguments
/// * `size` - Size in bytes
/// * `alloc_type` - Type of allocation (Traditional or Fractal)
/// 
/// # Returns
/// Virtual address of allocated region
pub fn allocate(size: usize, alloc_type: AllocationType) -> Result<VirtAddr, &'static str> {
    match alloc_type {
        AllocationType::Traditional => {
            // Use buddy allocator
            use super::buddy::BUDDY_ALLOCATOR;
            use x86_64::structures::paging::FrameAllocator;
            
            let mut allocator = BUDDY_ALLOCATOR.lock();
            let frame = allocator.allocate_frame()
                .ok_or("Buddy allocation failed")?;
            
            Ok(VirtAddr::new(frame.start_address().as_u64()))
        }
        
        AllocationType::Fractal { dimension, proximity } => {
            // Use fractal allocator
            use super::fractal_allocator::FRACTAL_ALLOCATOR;
            
            let mut allocator = FRACTAL_ALLOCATOR.lock();
            if let Some(ref mut fractal) = *allocator {
                fractal.allocate_spatial(size, dimension, proximity)
            } else {
                Err("Fractal allocator not initialized")
            }
        }
    }
}

/// Example usage for kernel modules
pub fn example_allocations() {
    serial_println!("[HYBRID] Demonstrating hybrid allocation:");
    
    // Traditional allocation
    match allocate(4096, AllocationType::Traditional) {
        Ok(addr) => serial_println!("  - Traditional: {:#x}", addr.as_u64()),
        Err(e) => serial_println!("  - Traditional failed: {}", e),
    }
    
    // Fractal allocation with high dimensionality
    match allocate(
        1024 * 1024,  // 1MB
        AllocationType::Fractal {
            dimension: 2.5,  // Between planar and volumetric
            proximity: None,
        }
    ) {
        Ok(addr) => serial_println!("  - Fractal (dim 2.5): {:#x}", addr.as_u64()),
        Err(e) => serial_println!("  - Fractal failed: {}", e),
    }
}
