//src/fs/psychicfs.rs
use crate::drivers::virtio::{disk_read_sector, disk_write_sector, disk_is_present};
use alloc::vec::Vec;
use alloc::string::String;
use spin::Mutex;
use core::sync::atomic::{AtomicBool, Ordering};

const FS_MAGIC: u32 = 0x50535946; // "PSYF"
const FS_VERSION: u16 = 3; // Version 3: extent-based
const SUPERBLOCK_SECTOR: u64 = 0;
const INODE_TABLE_START: u64 = 1;
const INODE_TABLE_SECTORS: u64 = 64;
const BITMAP_SECTOR: u64 = 65;
const DATA_BLOCKS_START: u64 = 128;

const MAX_FILENAME_LEN: usize = 48;  // Reduced to fit extents
const MAX_FILES: usize = 256;
const BLOCK_SIZE: usize = 512;

// Extent-based allocation
const INLINE_EXTENTS: usize = 4;     // 4 inline extents in inode
const EXTENTS_PER_BLOCK: usize = 64; // 512 bytes / 8 bytes per extent
// Max: 4 inline + 64 indirect = 68 extents
// Each extent: up to 65535 contiguous blocks
// Theoretical max: 68 * 65535 * 512 bytes = ~2.1 GB
// Practical max: disk size (67 MB)

// Extent flags
const EXTENT_ALLOCATED: u16 = 0x0001;
const EXTENT_HOLE: u16 = 0x0002;      // Sparse file hole
const EXTENT_PREALLOC: u16 = 0x0004;  // Preallocated, not yet written

// Cache configuration
const CACHE_SIZE: usize = 32;
const CACHE_WRITEBACK_INTERVAL: u64 = 100;

/// Extent: describes a contiguous range of blocks
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Extent {
    pub logical_block: u16,   // Logical offset in file (in blocks)
    pub physical_block: u32,  // Physical disk block number
    pub count: u16,           // Number of contiguous blocks (0 = unused)
}

impl Extent {
    pub const fn empty() -> Self {
        Self { logical_block: 0, physical_block: 0, count: 0 }
    }
    
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    
    /// Check if logical block falls within this extent
    pub fn contains(&self, logical: u16) -> bool {
        !self.is_empty() && 
        logical >= self.logical_block && 
        logical < self.logical_block + self.count
    }
    
    /// Get physical block for a logical block within this extent
    pub fn translate(&self, logical: u16) -> Option<u32> {
        if self.contains(logical) {
            Some(self.physical_block + (logical - self.logical_block) as u32)
        } else {
            None
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Superblock {
    pub magic: u32,
    pub version: u16,
    pub total_blocks: u32,
    pub free_blocks: u32,
    pub total_inodes: u32,
    pub free_inodes: u32,
    pub first_data_block: u32,
    pub block_size: u16,
    pub dirty: u8,
    pub _reserved: [u8; 485],
}

/// Inode with extent-based allocation (128 bytes)
/// Layout: 1+1+2+4+8+8 = 24 bytes header
///         4*8 = 32 bytes inline extents
///         4 bytes extent indirect pointer
///         2 bytes last_extent_idx (append fast path)
///         2 bytes reserved
///         48 bytes name
///         1+7 = 8 bytes dirty + reserved
///         Total = 24+32+4+2+2+48+8 = 120 bytes (8 bytes spare)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Inode {
    pub in_use: u8,
    pub file_type: u8,
    pub permissions: u16,
    pub size: u32,                         // File size in bytes
    pub created_time: u64,
    pub modified_time: u64,
    pub extents: [Extent; INLINE_EXTENTS], // 4 inline extents (32 bytes)
    pub extent_indirect: u32,              // Indirect extent block pointer
    pub last_extent_idx: u16,              // Last used extent (append fast path)
    pub extent_count: u16,                 // Total extents used
    pub name: [u8; MAX_FILENAME_LEN],      // 48 bytes
    pub dirty: u8,
    pub _reserved: [u8; 7],
}

impl Inode {
    pub fn new() -> Self {
        Self {
            in_use: 0,
            file_type: 0,
            permissions: 0o644,
            size: 0,
            created_time: 0,
            modified_time: 0,
            extents: [Extent::empty(); INLINE_EXTENTS],
            extent_indirect: 0,
            last_extent_idx: 0,
            extent_count: 0,
            name: [0; MAX_FILENAME_LEN],
            dirty: 0,
            _reserved: [0; 7],
        }
    }
    
    pub fn get_name(&self) -> &str {
        let name_slice = &self.name;
        let len = name_slice.iter().position(|&c| c == 0).unwrap_or(MAX_FILENAME_LEN);
        core::str::from_utf8(&name_slice[..len]).unwrap_or("")
    }
    
    pub fn set_name(&mut self, name: &str) {
        let bytes = name.as_bytes();
        let len = bytes.len().min(MAX_FILENAME_LEN - 1);
        self.name[..len].copy_from_slice(&bytes[..len]);
        self.name[len] = 0;
    }
    
    /// Get physical block for a logical block number
    pub fn get_physical_block(&self, logical: u16) -> Option<u32> {
        // Check inline extents first
        for ext in &self.extents {
            if let Some(phys) = ext.translate(logical) {
                return Some(phys);
            }
        }
        // Would check indirect extents here if extent_indirect != 0
        None
    }
    
    /// Add a new extent (append fast path)
    pub fn add_extent(&mut self, physical_start: u32, count: u16) -> bool {
        // Find logical position
        let logical_start = if self.extent_count == 0 {
            0u16
        } else {
            // Get end of last extent
            let last_idx = self.last_extent_idx as usize;
            if last_idx < INLINE_EXTENTS {
                let last = &self.extents[last_idx];
                last.logical_block + last.count
            } else {
                return false; // Would need indirect
            }
        };
        
        // Try to extend last extent if contiguous
        if self.extent_count > 0 {
            let last_idx = self.last_extent_idx as usize;
            if last_idx < INLINE_EXTENTS {
                let last = &mut self.extents[last_idx];
                if last.physical_block + last.count as u32 == physical_start {
                    // Contiguous - extend in place
                    last.count += count;
                    return true;
                }
            }
        }
        
        // Find free inline extent slot
        for (i, ext) in self.extents.iter_mut().enumerate() {
            if ext.is_empty() {
                *ext = Extent {
                    logical_block: logical_start,
                    physical_block: physical_start,
                    count,
                };
                self.last_extent_idx = i as u16;
                self.extent_count += 1;
                return true;
            }
        }
        
        // Would allocate indirect extent block here
        false
    }
    
    /// Clear all extents
    pub fn clear_extents(&mut self) {
        for ext in &mut self.extents {
            *ext = Extent::empty();
        }
        self.extent_indirect = 0;
        self.last_extent_idx = 0;
        self.extent_count = 0;
    }
}

/// Block cache entry
#[derive(Clone, Copy)]
pub struct CacheEntry {
    pub block_num: u32,
    pub data: [u8; BLOCK_SIZE],
    pub dirty: bool,
    pub access_count: u32,
    pub last_access: u64,
}

impl CacheEntry {
    pub const fn empty() -> Self {
        Self {
            block_num: 0,
            data: [0; BLOCK_SIZE],
            dirty: false,
            access_count: 0,
            last_access: 0,
        }
    }
}

/// Access pattern for predictive loading
#[derive(Clone, Copy)]
pub struct AccessPattern {
    pub inode: u32,
    pub last_block: u32,
    pub access_count: u32,
    pub sequential_count: u32,
}

impl AccessPattern {
    pub const fn empty() -> Self {
        Self {
            inode: 0,
            last_block: 0,
            access_count: 0,
            sequential_count: 0,
        }
    }
}

pub struct PsychicFs {
    pub mounted: bool,
    pub superblock: Superblock,
    pub block_bitmap: [u8; 1024],
    pub bitmap_dirty: AtomicBool,
    pub superblock_dirty: AtomicBool,
    // Block cache for read/write optimization
    pub cache: [CacheEntry; CACHE_SIZE],
    pub cache_hits: u64,
    pub cache_misses: u64,
    // Access pattern tracking for predictive loading
    pub patterns: [AccessPattern; 16],
}

impl PsychicFs {
    pub fn new() -> Self {
        const EMPTY_CACHE: CacheEntry = CacheEntry::empty();
        const EMPTY_PATTERN: AccessPattern = AccessPattern::empty();
        Self {
            mounted: false,
            superblock: unsafe { core::mem::zeroed() },
            block_bitmap: [0; 1024],
            bitmap_dirty: AtomicBool::new(false),
            superblock_dirty: AtomicBool::new(false),
            cache: [EMPTY_CACHE; CACHE_SIZE],
            cache_hits: 0,
            cache_misses: 0,
            patterns: [EMPTY_PATTERN; 16],
        }
    }
    
    /// Read block through cache
    pub fn cached_read(&mut self, block_num: u32, buffer: &mut [u8; BLOCK_SIZE]) -> bool {
        // Check cache first
        for entry in &mut self.cache {
            if entry.block_num == block_num && entry.access_count > 0 {
                buffer.copy_from_slice(&entry.data);
                entry.access_count += 1;
                entry.last_access = crate::get_timestamp();
                self.cache_hits += 1;
                return true;
            }
        }
        
        self.cache_misses += 1;
        
        // Cache miss - read from disk
        if !disk_read_sector(block_num as u64, buffer) {
            return false;
        }
        
        // Add to cache (LRU replacement)
        self.cache_insert(block_num, buffer, false);
        true
    }
    
    /// Write block through cache
    pub fn cached_write(&mut self, block_num: u32, data: &[u8; BLOCK_SIZE]) -> bool {
        // Update cache if present
        for entry in &mut self.cache {
            if entry.block_num == block_num && entry.access_count > 0 {
                entry.data.copy_from_slice(data);
                entry.dirty = true;
                entry.access_count += 1;
                entry.last_access = crate::get_timestamp();
                return true;
            }
        }
        
        // Add to cache
        self.cache_insert(block_num, data, true);
        true
    }
    
    /// Insert block into cache with LRU replacement
    fn cache_insert(&mut self, block_num: u32, data: &[u8; BLOCK_SIZE], dirty: bool) {
        // Find empty slot or LRU entry
        let mut min_access = u64::MAX;
        let mut min_idx = 0;
        
        for (i, entry) in self.cache.iter().enumerate() {
            if entry.access_count == 0 {
                min_idx = i;
                break;
            }
            if entry.last_access < min_access {
                min_access = entry.last_access;
                min_idx = i;
            }
        }
        
        // Write back dirty entry if being evicted
        if self.cache[min_idx].dirty && self.cache[min_idx].access_count > 0 {
            disk_write_sector(self.cache[min_idx].block_num as u64, &self.cache[min_idx].data);
        }
        
        // Insert new entry
        self.cache[min_idx] = CacheEntry {
            block_num,
            data: *data,
            dirty,
            access_count: 1,
            last_access: crate::get_timestamp(),
        };
    }
    
    /// Flush all dirty cache entries to disk
    pub fn flush_cache(&mut self) -> bool {
        let mut success = true;
        for entry in &mut self.cache {
            if entry.dirty && entry.access_count > 0 {
                if disk_write_sector(entry.block_num as u64, &entry.data) {
                    entry.dirty = false;
                } else {
                    success = false;
                }
            }
        }
        success
    }
    
    /// Record access pattern for predictive loading
    pub fn record_access(&mut self, inode: u32, block: u32) {
        for pattern in &mut self.patterns {
            if pattern.inode == inode {
                if block == pattern.last_block + 1 {
                    pattern.sequential_count += 1;
                }
                pattern.last_block = block;
                pattern.access_count += 1;
                return;
            }
        }
        
        // New inode - find empty or least accessed slot
        let mut min_access = u32::MAX;
        let mut min_idx = 0;
        for (i, pattern) in self.patterns.iter().enumerate() {
            if pattern.access_count < min_access {
                min_access = pattern.access_count;
                min_idx = i;
            }
        }
        
        self.patterns[min_idx] = AccessPattern {
            inode,
            last_block: block,
            access_count: 1,
            sequential_count: 0,
        };
    }
    
    /// Predict next blocks to prefetch based on access patterns
    pub fn predict_prefetch(&self, inode: u32) -> Option<u32> {
        for pattern in &self.patterns {
            if pattern.inode == inode && pattern.sequential_count > 2 {
                // Sequential access detected, prefetch next block
                return Some(pattern.last_block + 1);
            }
        }
        None
    }
    
    pub fn mark_dirty(&mut self) {
        self.bitmap_dirty.store(true, Ordering::Release);
        self.superblock_dirty.store(true, Ordering::Release);
    }
    
    pub fn sync(&mut self) -> bool {
        if !self.mounted {
            return true;
        }
        
        let mut success = true;
        
        if self.bitmap_dirty.load(Ordering::Acquire) {
            if self.write_bitmap_to_disk() {
                self.bitmap_dirty.store(false, Ordering::Release);
            } else {
                success = false;
            }
        }
        
        if self.superblock_dirty.load(Ordering::Acquire) {
            self.superblock.dirty = 0;
            let sb_bytes = superblock_to_bytes(&self.superblock);
            if disk_write_sector(SUPERBLOCK_SECTOR, &sb_bytes) {
                self.superblock_dirty.store(false, Ordering::Release);
            } else {
                success = false;
            }
        }
        
        success
    }
    
    pub fn write_bitmap_to_disk(&self) -> bool {
        let mut sector1 = [0u8; 512];
        sector1.copy_from_slice(&self.block_bitmap[..512]);
        if !disk_write_sector(BITMAP_SECTOR, &sector1) {
            return false;
        }
        
        let mut sector2 = [0u8; 512];
        sector2.copy_from_slice(&self.block_bitmap[512..1024]);
        disk_write_sector(BITMAP_SECTOR + 1, &sector2)
    }
    
    pub fn read_bitmap_from_disk(&mut self) -> bool {
        let mut sector1 = [0u8; 512];
        if !disk_read_sector(BITMAP_SECTOR, &mut sector1) {
            return false;
        }
        self.block_bitmap[..512].copy_from_slice(&sector1);
        
        let mut sector2 = [0u8; 512];
        if !disk_read_sector(BITMAP_SECTOR + 1, &mut sector2) {
            return false;
        }
        self.block_bitmap[512..1024].copy_from_slice(&sector2);
        
        true
    }
    
    /// Check if a block is allocated
    pub fn is_block_allocated(&self, block_num: u32) -> bool {
        if block_num < DATA_BLOCKS_START as u32 {
            return true; // Reserved blocks always "allocated"
        }
        
        let idx = (block_num - DATA_BLOCKS_START as u32) as usize;
        let byte_idx = idx / 8;
        let bit_idx = idx % 8;
        
        if byte_idx >= 1024 {
            return true;
        }
        
        (self.block_bitmap[byte_idx] & (1 << bit_idx)) != 0
    }
    
    /// Get free block count
    pub fn count_free_blocks(&self) -> u32 {
        let mut count = 0;
        let max_blocks = 8192 - DATA_BLOCKS_START as usize;
        
        for i in 0..max_blocks.min(1024 * 8) {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            
            if (self.block_bitmap[byte_idx] & (1 << bit_idx)) == 0 {
                count += 1;
            }
        }
        
        count
    }
}

pub static PSYCHIC_FS: Mutex<Option<PsychicFs>> = Mutex::new(None);

fn superblock_to_bytes(sb: &Superblock) -> [u8; 512] {
    let mut buffer = [0u8; 512];
    unsafe {
        core::ptr::copy_nonoverlapping(
            sb as *const Superblock as *const u8,
            buffer.as_mut_ptr(),
            core::mem::size_of::<Superblock>()
        );
    }
    buffer
}

fn bytes_to_superblock(buffer: &[u8; 512]) -> Superblock {
    unsafe {
        core::ptr::read_unaligned(buffer.as_ptr() as *const Superblock)
    }
}

fn inode_to_bytes(inode: &Inode) -> [u8; 128] {
    let mut buffer = [0u8; 128];
    unsafe {
        core::ptr::copy_nonoverlapping(
            inode as *const Inode as *const u8,
            buffer.as_mut_ptr(),
            core::mem::size_of::<Inode>()
        );
    }
    buffer
}

fn bytes_to_inode(buffer: &[u8]) -> Inode {
    unsafe {
        core::ptr::read_unaligned(buffer.as_ptr() as *const Inode)
    }
}

pub fn fs_format() -> bool {
    if !disk_is_present() {
        crate::serial_println!("[FS] Format failed: No disk present");
        return false;
    }
    
    crate::serial_println!("[FS] Formatting PsychicFS...");
    
    let superblock = Superblock {
        magic: FS_MAGIC,
        version: FS_VERSION,
        total_blocks: 8192,
        free_blocks: 8192 - DATA_BLOCKS_START as u32,
        total_inodes: MAX_FILES as u32,
        free_inodes: MAX_FILES as u32,
        first_data_block: DATA_BLOCKS_START as u32,
        block_size: BLOCK_SIZE as u16,
        dirty: 0,
        _reserved: [0; 485],
    };
    
    let sb_bytes = superblock_to_bytes(&superblock);
    if !disk_write_sector(SUPERBLOCK_SECTOR, &sb_bytes) {
        crate::serial_println!("[FS] Failed to write superblock");
        return false;
    }
    
    // Clear inode table
    let empty_sector = [0u8; 512];
    for i in 0..INODE_TABLE_SECTORS {
        if !disk_write_sector(INODE_TABLE_START + i, &empty_sector) {
            crate::serial_println!("[FS] Failed to clear inode table at sector {}", i);
            return false;
        }
    }
    
    // Clear bitmap (all blocks free)
    if !disk_write_sector(BITMAP_SECTOR, &empty_sector) {
        crate::serial_println!("[FS] Failed to write bitmap sector 1");
        return false;
    }
    if !disk_write_sector(BITMAP_SECTOR + 1, &empty_sector) {
        crate::serial_println!("[FS] Failed to write bitmap sector 2");
        return false;
    }
    
    crate::serial_println!("[FS] Format complete");
    true
}

pub fn fs_mount() -> bool {
    if !disk_is_present() {
        crate::serial_println!("[FS] Mount failed: No disk present");
        return false;
    }
    
    let mut sb_buffer = [0u8; 512];
    if !disk_read_sector(SUPERBLOCK_SECTOR, &mut sb_buffer) {
        crate::serial_println!("[FS] Failed to read superblock");
        return false;
    }
    
    let superblock = bytes_to_superblock(&sb_buffer);
    
    if superblock.magic != FS_MAGIC {
        crate::serial_println!("[FS] Invalid magic: 0x{:08x}", superblock.magic);
        return false;
    }
    
    let mut pfs = PsychicFs::new();
    if !pfs.read_bitmap_from_disk() {
        crate::serial_println!("[FS] Failed to read bitmap");
        return false;
    }
    
    pfs.superblock = superblock;
    pfs.mounted = true;
    
    if superblock.dirty != 0 {
        crate::println!("Warning: Filesystem was not cleanly unmounted");
    }
    
    pfs.superblock.dirty = 1;
    pfs.superblock_dirty.store(true, Ordering::Release);
    
    let free_blocks = pfs.count_free_blocks();
    crate::serial_println!("[FS] Mounted: {} free blocks", free_blocks);
    
    *PSYCHIC_FS.lock() = Some(pfs);
    
    true
}

pub fn fs_unmount() {
    fs_sync();
    *PSYCHIC_FS.lock() = None;
}

pub fn fs_sync() {
    let mut fs = PSYCHIC_FS.lock();
    if let Some(ref mut pfs) = *fs {
        pfs.sync();
    }
}

fn find_inode_by_name(name: &str) -> Option<(u32, Inode)> {
    let mut buffer = [0u8; 512];
    
    crate::serial_println!("[FS] find_inode_by_name: looking for '{}'", name);
    
    for sector in 0..INODE_TABLE_SECTORS {
        if !disk_read_sector(INODE_TABLE_START + sector, &mut buffer) {
            crate::serial_println!("[FS] find: failed to read sector {}", sector);
            continue;
        }
        
        for i in 0..4 {
            let offset = i * 128;
            let inode = bytes_to_inode(&buffer[offset..offset + 128]);
            
            if inode.in_use != 0 {
                let inode_name = inode.get_name();
                crate::serial_println!("[FS] find: sector {} slot {} in_use={} name='{}' (looking for '{}')", 
                    sector, i, inode.in_use, inode_name, name);
                
                if inode_name == name {
                    let inode_num = (sector * 4 + i as u64) as u32;
                    crate::serial_println!("[FS] find: FOUND at inode {}", inode_num);
                    return Some((inode_num, inode));
                }
            }
        }
    }
    
    crate::serial_println!("[FS] find: NOT FOUND");
    None
}

fn find_free_inode() -> Option<u32> {
    let mut buffer = [0u8; 512];
    
    for sector in 0..INODE_TABLE_SECTORS {
        if !disk_read_sector(INODE_TABLE_START + sector, &mut buffer) {
            continue;
        }
        
        for i in 0..4 {
            let offset = i * 128;
            let inode = bytes_to_inode(&buffer[offset..offset + 128]);
            
            if inode.in_use == 0 {
                return Some((sector * 4 + i as u64) as u32);
            }
        }
    }
    
    None
}

fn write_inode(inode_num: u32, inode: &Inode) -> bool {
    let sector = (inode_num / 4) as u64;
    let offset = (inode_num % 4) as usize * 128;
    
    crate::serial_println!("[FS] write_inode: inode={} sector={} offset={}", inode_num, sector, offset);
    
    let mut buffer = [0u8; 512];
    if !disk_read_sector(INODE_TABLE_START + sector, &mut buffer) {
        crate::serial_println!("[FS] Failed to read inode sector for write");
        return false;
    }
    
    let inode_bytes = inode_to_bytes(inode);
    buffer[offset..offset + 128].copy_from_slice(&inode_bytes);
    
    crate::serial_println!("[FS] write_inode: writing to sector {} (abs: {})", sector, INODE_TABLE_START + sector);
    
    if !disk_write_sector(INODE_TABLE_START + sector, &buffer) {
        crate::serial_println!("[FS] Failed to write inode sector");
        return false;
    }
    
    // Verify write succeeded by reading back
    let mut verify_buffer = [0u8; 512];
    if !disk_read_sector(INODE_TABLE_START + sector, &mut verify_buffer) {
        crate::serial_println!("[FS] Failed to verify inode write");
        return false;
    }
    
    let written_inode = bytes_to_inode(&verify_buffer[offset..offset + 128]);
    if written_inode.in_use != inode.in_use {
        crate::serial_println!("[FS] VERIFY FAILED: in_use mismatch {} vs {}", written_inode.in_use, inode.in_use);
        return false;
    }
    
    crate::serial_println!("[FS] write_inode: verified OK, in_use={}", written_inode.in_use);
    true
}

/// THE FIX: Improved block allocation with better debugging
fn allocate_block() -> Option<u32> {
    let mut fs = PSYCHIC_FS.lock();
    if let Some(ref mut pfs) = *fs {
        let max_blocks = 8192 - DATA_BLOCKS_START as usize;
        
        // Search for free block
        for i in 0..max_blocks.min(1024 * 8) {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            
            if byte_idx >= 1024 {
                break;
            }
            
            // Check if block is free
            if pfs.block_bitmap[byte_idx] & (1 << bit_idx) == 0 {
                // Mark as allocated
                pfs.block_bitmap[byte_idx] |= 1 << bit_idx;
                
                let block_num = DATA_BLOCKS_START as u32 + i as u32;
                
                // Update superblock
                if pfs.superblock.free_blocks > 0 {
                    pfs.superblock.free_blocks -= 1;
                }
                
                // CRITICAL FIX: Immediate sync to disk
                pfs.mark_dirty();
                let sync_result = pfs.sync();
                
                if !sync_result {
                    // Rollback on sync failure
                    pfs.block_bitmap[byte_idx] &= !(1 << bit_idx);
                    pfs.superblock.free_blocks += 1;
                    crate::serial_println!("[FS] CRITICAL: Failed to sync block allocation!");
                    return None;
                }
                
                crate::serial_println!("[FS] Allocated block {} (synced)", block_num);
                return Some(block_num);
            }
        }
        
        crate::serial_println!("[FS] No free blocks available!");
        crate::serial_println!("[FS] Free blocks according to superblock: {}", pfs.superblock.free_blocks);
        
        // Debug: Count actual free blocks
        let actual_free = pfs.count_free_blocks();
        crate::serial_println!("[FS] Actual free blocks in bitmap: {}", actual_free);
    } else {
        crate::serial_println!("[FS] Filesystem not mounted!");
    }
    
    None
}

fn free_block(block_num: u32) {
    let mut fs = PSYCHIC_FS.lock();
    if let Some(ref mut pfs) = *fs {
        if block_num < DATA_BLOCKS_START as u32 {
            return;
        }
        
        let idx = (block_num - DATA_BLOCKS_START as u32) as usize;
        let byte_idx = idx / 8;
        let bit_idx = idx % 8;
        
        if byte_idx < 1024 {
            // Check if block was actually allocated
            if pfs.block_bitmap[byte_idx] & (1 << bit_idx) == 0 {
                crate::serial_println!("[FS] WARNING: Freeing already free block {}", block_num);
                return;
            }
            
            pfs.block_bitmap[byte_idx] &= !(1 << bit_idx);
            pfs.superblock.free_blocks += 1;
            pfs.mark_dirty();
            
            // Sync after every free (safety over performance)
            pfs.sync();
        }
    }
}

/// Free a contiguous range of blocks
fn free_block_range(start: u32, count: u16) {
    for i in 0..count as u32 {
        free_block(start + i);
    }
}

pub fn fs_create(name: &str) -> bool {
    if name.len() >= MAX_FILENAME_LEN {
        crate::serial_println!("[FS] Create failed: name too long");
        return false;
    }
    
    if find_inode_by_name(name).is_some() {
        crate::serial_println!("[FS] Create failed: file already exists");
        return false;
    }
    
    let inode_num = match find_free_inode() {
        Some(n) => n,
        None => {
            crate::serial_println!("[FS] Create failed: no free inodes");
            return false;
        }
    };
    
    let mut inode = Inode::new();
    inode.in_use = 1;
    inode.set_name(name);
    inode.created_time = crate::get_timestamp();
    inode.modified_time = crate::get_timestamp();
    
    crate::serial_println!("[FS] Creating file '{}' at inode {}", name, inode_num);
    
    write_inode(inode_num, &inode)
}

/// Find contiguous free blocks for extent allocation
fn find_contiguous_blocks(count: usize) -> Option<u32> {
    let mut fs = PSYCHIC_FS.lock();
    let pfs = fs.as_mut()?;
    
    let mut run_start = 0u32;
    let mut run_len = 0usize;
    
    // Scan bitmap for contiguous free blocks
    for block in DATA_BLOCKS_START as u32..pfs.superblock.total_blocks {
        if !pfs.is_block_allocated(block) {
            if run_len == 0 {
                run_start = block;
            }
            run_len += 1;
            if run_len >= count {
                return Some(run_start);
            }
        } else {
            run_len = 0;
        }
    }
    
    // Try single-block allocations if no contiguous run found
    if count == 1 {
        for block in DATA_BLOCKS_START as u32..pfs.superblock.total_blocks {
            if !pfs.is_block_allocated(block) {
                return Some(block);
            }
        }
    }
    
    None
}

/// Allocate contiguous blocks and mark them as used
fn allocate_contiguous_blocks(start: u32, count: u16) -> bool {
    let mut fs = PSYCHIC_FS.lock();
    if let Some(pfs) = fs.as_mut() {
        for i in 0..count as u32 {
            let block = start + i;
            let byte_idx = (block / 8) as usize;
            let bit_idx = block % 8;
            if byte_idx < pfs.block_bitmap.len() {
                pfs.block_bitmap[byte_idx] |= 1 << bit_idx;
            }
        }
        pfs.superblock.free_blocks = pfs.superblock.free_blocks.saturating_sub(count as u32);
        true
    } else {
        false
    }
}

pub fn fs_write(name: &str, data: &[u8]) -> bool {
    let (inode_num, mut inode) = match find_inode_by_name(name) {
        Some(i) => i,
        None => {
            if !fs_create(name) {
                return false;
            }
            match find_inode_by_name(name) {
                Some(i) => i,
                None => return false,
            }
        }
    };
    
    let blocks_needed = (data.len() + BLOCK_SIZE - 1) / BLOCK_SIZE;
    if blocks_needed == 0 {
        inode.size = 0;
        inode.clear_extents();
        return write_inode(inode_num, &inode);
    }
    
    // Clear existing extents for overwrite
    // Free old blocks first
    for ext in &inode.extents {
        if !ext.is_empty() {
            free_block_range(ext.physical_block, ext.count);
        }
    }
    inode.clear_extents();
    
    // Try to allocate contiguous blocks for the entire file
    let mut blocks_allocated = 0;
    while blocks_allocated < blocks_needed {
        let remaining = blocks_needed - blocks_allocated;
        let chunk_size = remaining.min(65535); // Max extent size
        
        // Find contiguous run
        if let Some(start_block) = find_contiguous_blocks(chunk_size) {
            // Allocate and add extent
            allocate_contiguous_blocks(start_block, chunk_size as u16);
            if !inode.add_extent(start_block, chunk_size as u16) {
                return false; // Out of extent slots
            }
            blocks_allocated += chunk_size;
        } else if chunk_size > 1 {
            // Try smaller chunks
            if let Some(start_block) = find_contiguous_blocks(1) {
                allocate_contiguous_blocks(start_block, 1);
                if !inode.add_extent(start_block, 1) {
                    return false;
                }
                blocks_allocated += 1;
            } else {
                return false; // Out of disk space
            }
        } else {
            return false; // Out of disk space
        }
    }
    
    // Write data to allocated blocks
    let mut written = 0;
    for logical_block in 0..blocks_needed {
        if let Some(phys_block) = inode.get_physical_block(logical_block as u16) {
            let start = logical_block * BLOCK_SIZE;
            let end = (start + BLOCK_SIZE).min(data.len());
            
            let mut buffer = [0u8; 512];
            buffer[..end - start].copy_from_slice(&data[start..end]);
            
            if !disk_write_sector(phys_block as u64, &buffer) {
                return false;
            }
            written += 1;
        } else {
            return false;
        }
    }
    
    inode.size = data.len() as u32;
    inode.modified_time = crate::get_timestamp();
    inode.dirty = 1;
    
    write_inode(inode_num, &inode) && { fs_sync(); true }
}

pub fn fs_read(name: &str) -> Option<Vec<u8>> {
    let (_inode_num, inode) = find_inode_by_name(name)?;
    
    let size = inode.size;
    if size == 0 {
        return Some(Vec::new());
    }
    
    let mut data = Vec::with_capacity(size as usize);
    let blocks_needed = (size as usize + BLOCK_SIZE - 1) / BLOCK_SIZE;
    
    for logical_block in 0..blocks_needed {
        // Translate logical block to physical using extents
        if let Some(phys_block) = inode.get_physical_block(logical_block as u16) {
            let mut buffer = [0u8; 512];
            if !disk_read_sector(phys_block as u64, &mut buffer) {
                return None;
            }
            
            let remaining = size as usize - data.len();
            let to_read = remaining.min(BLOCK_SIZE);
            data.extend_from_slice(&buffer[..to_read]);
        } else {
            // Sparse file hole - return zeros
            let remaining = size as usize - data.len();
            let to_read = remaining.min(BLOCK_SIZE);
            data.extend((0..to_read).map(|_| 0u8));
        }
    }
    
    Some(data)
}

pub fn fs_delete(name: &str) -> bool {
    let (inode_num, mut inode) = match find_inode_by_name(name) {
        Some(i) => i,
        None => return false,
    };
    
    // Free all extent blocks
    for ext in &inode.extents {
        if !ext.is_empty() {
            free_block_range(ext.physical_block, ext.count);
        }
    }
    
    // Free indirect extent block if present
    if inode.extent_indirect != 0 {
        // Would read indirect extent block and free all referenced blocks
        free_block(inode.extent_indirect);
    }
    
    inode.clear_extents();
    inode.in_use = 0;
    inode.size = 0;
    
    write_inode(inode_num, &inode)
}

pub fn fs_list() -> Vec<String> {
    let mut files = Vec::new();
    let mut buffer = [0u8; 512];
    
    for sector in 0..INODE_TABLE_SECTORS {
        if !disk_read_sector(INODE_TABLE_START + sector, &mut buffer) {
            continue;
        }
        
        for i in 0..4 {
            let offset = i * 128;
            let inode = bytes_to_inode(&buffer[offset..offset + 128]);
            
            if inode.in_use != 0 {
                files.push(String::from(inode.get_name()));
            }
        }
    }
    
    files
}