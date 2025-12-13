//src/fs/psychicfs.rs
use crate::drivers::virtio::{disk_read_sector, disk_write_sector, disk_is_present};
use alloc::vec::Vec;
use alloc::string::String;
use spin::Mutex;
use core::sync::atomic::{AtomicBool, Ordering};

const FS_MAGIC: u32 = 0x50535946; // "PSYF"
const FS_VERSION: u16 = 2; // Bumped version for new format
const SUPERBLOCK_SECTOR: u64 = 0;
const INODE_TABLE_START: u64 = 1;
const INODE_TABLE_SECTORS: u64 = 64;
const BITMAP_SECTOR: u64 = 65;
const DATA_BLOCKS_START: u64 = 128;

const MAX_FILENAME_LEN: usize = 56;
const MAX_FILES: usize = 256;
const BLOCK_SIZE: usize = 512;
const MAX_FILE_BLOCKS: usize = 8; // Keep at 8 to fit inode in 128 bytes (4KB max file size)

// Cache configuration
const CACHE_SIZE: usize = 32; // Number of cached blocks
const CACHE_WRITEBACK_INTERVAL: u64 = 100; // Ticks between writebacks

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

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Inode {
    pub in_use: u8,
    pub file_type: u8,
    pub permissions: u16,
    pub size: u32,
    pub created_time: u64,
    pub modified_time: u64,
    pub blocks: [u32; MAX_FILE_BLOCKS],
    pub name: [u8; MAX_FILENAME_LEN],
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
            blocks: [0; MAX_FILE_BLOCKS],
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
            
            crate::serial_println!("[FS] Freed block {} (synced)", block_num);
        }
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

pub fn fs_write(name: &str, data: &[u8]) -> bool {
    crate::serial_println!("[FS] Writing {} bytes to '{}'", data.len(), name);
    
    let (inode_num, mut inode) = match find_inode_by_name(name) {
        Some(i) => {
            crate::serial_println!("[FS] Found existing file at inode {}", i.0);
            i
        },
        None => {
            crate::serial_println!("[FS] File not found, creating...");
            if !fs_create(name) {
                crate::serial_println!("[FS] Failed to create file");
                return false;
            }
            match find_inode_by_name(name) {
                Some(i) => {
                    crate::serial_println!("[FS] Created at inode {}", i.0);
                    i
                },
                None => {
                    crate::serial_println!("[FS] Critical error: file disappeared after creation");
                    return false;
                }
            }
        }
    };
    
    let blocks_needed = (data.len() + BLOCK_SIZE - 1) / BLOCK_SIZE;
    crate::serial_println!("[FS] Need {} blocks", blocks_needed);
    
    if blocks_needed > MAX_FILE_BLOCKS {
        crate::serial_println!("[FS] File too large: need {} blocks, max is {}", 
            blocks_needed, MAX_FILE_BLOCKS);
        return false;
    }
    
    // Free excess blocks
    for i in blocks_needed..MAX_FILE_BLOCKS {
        let block = inode.blocks[i];
        if block != 0 {
            crate::serial_println!("[FS] Freeing excess block {}", block);
            free_block(block);
            inode.blocks[i] = 0;
        }
    }
    
    // Allocate and write blocks
    for i in 0..blocks_needed {
        let block = if inode.blocks[i] != 0 {
            crate::serial_println!("[FS] Reusing block {} for chunk {}", inode.blocks[i], i);
            inode.blocks[i]
        } else {
            match allocate_block() {
                Some(b) => {
                    crate::serial_println!("[FS] Allocated new block {} for chunk {}", b, i);
                    inode.blocks[i] = b;
                    b
                }
                None => {
                    crate::serial_println!("[FS] FATAL: Failed to allocate block {} of {}", i, blocks_needed);
                    
                    // Cleanup: free any blocks we allocated
                    for j in 0..i {
                        if inode.blocks[j] != 0 {
                            free_block(inode.blocks[j]);
                            inode.blocks[j] = 0;
                        }
                    }
                    return false;
                }
            }
        };
        
        let start = i * BLOCK_SIZE;
        let end = ((i + 1) * BLOCK_SIZE).min(data.len());
        
        let mut buffer = [0u8; 512];
        buffer[..end - start].copy_from_slice(&data[start..end]);
        
        if !disk_write_sector(block as u64, &buffer) {
            crate::serial_println!("[FS] Failed to write block {}", block);
            return false;
        }
        
        crate::serial_println!("[FS] Wrote {} bytes to block {}", end - start, block);
    }
    
    inode.size = data.len() as u32;
    inode.modified_time = crate::get_timestamp();
    inode.dirty = 1;
    
    crate::serial_println!("[FS] Updating inode {} with size {}", inode_num, inode.size);
    
    let success = write_inode(inode_num, &inode);
    
    if success {
        // Immediately sync to disk
        fs_sync();
        crate::serial_println!("[FS] Write complete and synced");
    } else {
        crate::serial_println!("[FS] Failed to update inode");
    }
    
    success
}

pub fn fs_read(name: &str) -> Option<Vec<u8>> {
    let (inode_num, inode) = find_inode_by_name(name)?;
    
    let size = inode.size;
    if size == 0 {
        return Some(Vec::new());
    }
    
    let mut data = Vec::with_capacity(size as usize);
    let blocks_needed = (size as usize + BLOCK_SIZE - 1) / BLOCK_SIZE;
    
    for i in 0..blocks_needed {
        let block = inode.blocks[i];
        if block == 0 {
            break;
        }
        
        let mut buffer = [0u8; 512];
        if !disk_read_sector(block as u64, &mut buffer) {
            return None;
        }
        
        let remaining = size as usize - data.len();
        let to_read = remaining.min(BLOCK_SIZE);
        data.extend_from_slice(&buffer[..to_read]);
    }
    
    Some(data)
}

pub fn fs_delete(name: &str) -> bool {
    let (inode_num, mut inode) = match find_inode_by_name(name) {
        Some(i) => i,
        None => return false,
    };
    
    for i in 0..MAX_FILE_BLOCKS {
        let block = inode.blocks[i];
        if block != 0 {
            free_block(block);
            inode.blocks[i] = 0;
        }
    }
    
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