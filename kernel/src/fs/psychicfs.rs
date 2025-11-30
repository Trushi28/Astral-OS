//src/mod/psychicfs.rs
use crate::drivers::virtio::{disk_read_sector, disk_write_sector, disk_is_present};
use alloc::vec::Vec;
use alloc::string::String;
use spin::Mutex;

const FS_MAGIC: u32 = 0x50535946;
const FS_VERSION: u16 = 1;
const SUPERBLOCK_SECTOR: u64 = 0;
const INODE_TABLE_START: u64 = 1;
const INODE_TABLE_SECTORS: u64 = 64;
const BITMAP_SECTOR: u64 = 65;
const DATA_BLOCKS_START: u64 = 128;

const MAX_FILENAME_LEN: usize = 56;
const MAX_FILES: usize = 256;
const BLOCK_SIZE: usize = 512;
const MAX_FILE_BLOCKS: usize = 8;

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
    pub _reserved: [u8; 486],
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
    pub _reserved: [u8; 8],
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
            _reserved: [0; 8],
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

pub struct PsychicFs {
    pub mounted: bool,
    pub superblock: Superblock,
    pub block_bitmap: [u8; 1024],
}

impl PsychicFs {
    pub fn new() -> Self {
        Self {
            mounted: false,
            superblock: unsafe { core::mem::zeroed() },
            block_bitmap: [0; 1024],
        }
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
        return false;
    }
    
    let superblock = Superblock {
        magic: FS_MAGIC,
        version: FS_VERSION,
        total_blocks: 8192,
        free_blocks: 8192 - DATA_BLOCKS_START as u32,
        total_inodes: MAX_FILES as u32,
        free_inodes: MAX_FILES as u32,
        first_data_block: DATA_BLOCKS_START as u32,
        block_size: BLOCK_SIZE as u16,
        _reserved: [0; 486],
    };
    
    let sb_bytes = superblock_to_bytes(&superblock);
    if !disk_write_sector(SUPERBLOCK_SECTOR, &sb_bytes) {
        return false;
    }
    
    let empty_sector = [0u8; 512];
    for i in 0..INODE_TABLE_SECTORS {
        if !disk_write_sector(INODE_TABLE_START + i, &empty_sector) {
            return false;
        }
    }
    
    if !disk_write_sector(BITMAP_SECTOR, &empty_sector) {
        return false;
    }
    if !disk_write_sector(BITMAP_SECTOR + 1, &empty_sector) {
        return false;
    }
    
    true
}

pub fn fs_mount() -> bool {
    if !disk_is_present() {
        return false;
    }
    
    let mut sb_buffer = [0u8; 512];
    if !disk_read_sector(SUPERBLOCK_SECTOR, &mut sb_buffer) {
        return false;
    }
    
    let superblock = bytes_to_superblock(&sb_buffer);
    
    if superblock.magic != FS_MAGIC {
        return false;
    }
    
    let mut pfs = PsychicFs::new();
    if !pfs.read_bitmap_from_disk() {
        return false;
    }
    
    pfs.superblock = superblock;
    pfs.mounted = true;
    
    *PSYCHIC_FS.lock() = Some(pfs);
    
    true
}

pub fn fs_unmount() {
    *PSYCHIC_FS.lock() = None;
}

fn find_inode_by_name(name: &str) -> Option<(u32, Inode)> {
    let mut buffer = [0u8; 512];
    
    for sector in 0..INODE_TABLE_SECTORS {
        if !disk_read_sector(INODE_TABLE_START + sector, &mut buffer) {
            continue;
        }
        
        for i in 0..4 {
            let offset = i * 128;
            let inode = bytes_to_inode(&buffer[offset..offset + 128]);
            
            if inode.in_use != 0 && inode.get_name() == name {
                let inode_num = (sector * 4 + i as u64) as u32;
                return Some((inode_num, inode));
            }
        }
    }
    
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
    
    let mut buffer = [0u8; 512];
    if !disk_read_sector(INODE_TABLE_START + sector, &mut buffer) {
        return false;
    }
    
    let inode_bytes = inode_to_bytes(inode);
    buffer[offset..offset + 128].copy_from_slice(&inode_bytes);
    
    disk_write_sector(INODE_TABLE_START + sector, &buffer)
}

fn allocate_block() -> Option<u32> {
    let mut fs = PSYCHIC_FS.lock();
    if let Some(ref mut pfs) = *fs {
        let max_blocks = 8192 - DATA_BLOCKS_START as usize;
        
        for i in 0..max_blocks {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            
            if byte_idx >= 1024 {
                break;
            }
            
            if pfs.block_bitmap[byte_idx] & (1 << bit_idx) == 0 {
                pfs.block_bitmap[byte_idx] |= 1 << bit_idx;
                let _ = pfs.write_bitmap_to_disk();
                return Some(DATA_BLOCKS_START as u32 + i as u32);
            }
        }
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
            pfs.block_bitmap[byte_idx] &= !(1 << bit_idx);
            let _ = pfs.write_bitmap_to_disk();
        }
    }
}

pub fn fs_create(name: &str) -> bool {
    if name.len() >= MAX_FILENAME_LEN {
        return false;
    }
    
    if find_inode_by_name(name).is_some() {
        return false;
    }
    
    let inode_num = match find_free_inode() {
        Some(n) => n,
        None => return false,
    };
    
    let mut inode = Inode::new();
    inode.in_use = 1;
    inode.set_name(name);
    inode.created_time = crate::get_timestamp();
    inode.modified_time = crate::get_timestamp();
    
    write_inode(inode_num, &inode)
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
    if blocks_needed > MAX_FILE_BLOCKS {
        return false;
    }
    
    for i in blocks_needed..MAX_FILE_BLOCKS {
        let block = inode.blocks[i];
        if block != 0 {
            free_block(block);
            inode.blocks[i] = 0;
        }
    }
    
    for i in 0..blocks_needed {
        let block = if inode.blocks[i] != 0 {
            inode.blocks[i]
        } else {
            match allocate_block() {
                Some(b) => {
                    inode.blocks[i] = b;
                    b
                }
                None => return false,
            }
        };
        
        let start = i * BLOCK_SIZE;
        let end = ((i + 1) * BLOCK_SIZE).min(data.len());
        
        let mut buffer = [0u8; 512];
        buffer[..end - start].copy_from_slice(&data[start..end]);
        
        if !disk_write_sector(block as u64, &buffer) {
            return false;
        }
    }
    
    inode.size = data.len() as u32;
    inode.modified_time = crate::get_timestamp();
    
    write_inode(inode_num, &inode)
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