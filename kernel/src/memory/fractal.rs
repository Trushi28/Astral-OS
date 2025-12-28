use alloc::string::String;
use alloc::vec::Vec;
use x86_64::VirtAddr;
use spin::Mutex;
use crate::serial_println;

/// Unique identifier for fractal regions
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FractalId(u64);

static NEXT_FRACTAL_ID: Mutex<u64> = Mutex::new(1);

impl FractalId {
    fn new() -> Self {
        let mut next = NEXT_FRACTAL_ID.lock();
        let id = *next;
        *next += 1;
        FractalId(id)
    }
}

/// Memory permissions for fractal regions
#[derive(Debug, Clone, Copy)]
pub struct Permissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl Permissions {
    pub const READ_WRITE: Self = Self { read: true, write: true, execute: false };
    pub const READ_ONLY: Self = Self { read: true, write: false, execute: false };
    pub const EXECUTE: Self = Self { read: true, write: false, execute: true };
}

/// A self-similar, recursive memory region
/// 
/// Fractal regions form a tree hierarchy:
/// Universe → Galaxy → Solar System → Planet → Continent
#[derive(Clone)]
pub struct FractalRegion {
    /// Unique identifier
    pub id: FractalId,
    
    /// Human-readable name
    pub name: String,
    
    /// Hierarchy level (0=universe, 1=galaxy, ...)
    pub level: u8,
    
    /// Base virtual address
    pub base: VirtAddr,
    
    /// Size in bytes
    pub size: usize,
    
    /// Fractal dimension (1.0 = linear, 2.0 = planar, 3.0 = volumetric)
    pub fractal_dimension: f32,
    
    /// Parent region (None for universe)
    pub parent: Option<FractalId>,
    
    /// Child regions
    pub children: Vec<FractalId>,
    
    /// Density: how packed is this region (0.0 = empty, 1.0 = full)
    pub density: f32,
    
    /// Cache coherence score (0.0 = scattered, 1.0 = localized)
    pub coherence: f32,
    
    /// Is this region allocated?
    pub allocated: bool,
    
    /// Access permissions
    pub permissions: Permissions,
}

impl FractalRegion {
    /// Create a new fractal region
    pub fn new(name: String, level: u8, base: VirtAddr, size: usize) -> Self {
        Self {
            id: FractalId::new(),
            name,
            level,
            base,
            size,
            fractal_dimension: 1.0, // Default: linear
            parent: None,
            children: Vec::new(),
            density: 0.0,
            coherence: 1.0,
            allocated: false,
            permissions: Permissions::READ_WRITE,
        }
    }
    
    /// Create the root "Universe" region spanning all memory
    pub fn universe(base: VirtAddr, size: usize) -> Self {
        Self::new(String::from("Universe"), 0, base, size)
    }
    
    /// Check if this region can be split into children
    pub fn can_split(&self, child_count: usize) -> bool {
        let child_size = self.size / child_count;
        child_size >= 4096 // Minimum 4KB per child
    }
    
    /// Calculate self-similarity score with children
    pub fn self_similarity(&self) -> f32 {
        if self.children.is_empty() {
            return 1.0;
        }
        
        // Perfect fractal: all children same size
        // Imperfect: varying sizes
        let avg_child_size = self.size / self.children.len();
        let variance = 0.1; // Simplified
        
        1.0 - variance
    }
}

/// Fractal coordinate in hierarchical space
/// 
/// Instead of just VirtAddr, we can address memory hierarchically:
/// Galaxy 3, Solar 2, Planet 5 = Fractal coordinate
#[derive(Debug, Clone, Copy)]
pub struct FractalCoord {
    pub level: u8,
    pub index_at_level: usize,
}

impl FractalCoord {
    pub fn new(level: u8, index: usize) -> Self {
        Self {
            level,
            index_at_level: index,
        }
    }
}
