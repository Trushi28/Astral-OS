//! Virtual Filesystem (VFS) Layer for Astral OS
//!
//! Provides a unified interface for all filesystem operations with:
//! - Mountable filesystem abstraction
//! - Intent-based file resolution
//! - Path caching for performance
//! - Support for fractal addressing

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use crate::sync::RwLock;

/// File types supported by VFS
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Regular,
    Directory,
    SymLink,
    Device,
    Socket,
    Pipe,
}

/// File permissions
#[derive(Debug, Clone, Copy)]
pub struct Permissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl Permissions {
    pub fn new(read: bool, write: bool, execute: bool) -> Self {
        Self { read, write, execute }
    }
    
    pub fn from_mode(mode: u16) -> Self {
        Self {
            read: (mode & 0o400) != 0,
            write: (mode & 0o200) != 0,
            execute: (mode & 0o100) != 0,
        }
    }
    
    pub fn to_mode(&self) -> u16 {
        let mut mode = 0u16;
        if self.read { mode |= 0o400; }
        if self.write { mode |= 0o200; }
        if self.execute { mode |= 0o100; }
        mode
    }
}

impl Default for Permissions {
    fn default() -> Self {
        Self::new(true, true, false)
    }
}

/// File metadata/attributes
#[derive(Debug, Clone)]
pub struct FileAttr {
    pub file_type: FileType,
    pub size: u64,
    pub permissions: Permissions,
    pub created: u64,
    pub modified: u64,
    pub accessed: u64,
    pub inode: u64,
}

impl Default for FileAttr {
    fn default() -> Self {
        Self {
            file_type: FileType::Regular,
            size: 0,
            permissions: Permissions::default(),
            created: 0,
            modified: 0,
            accessed: 0,
            inode: 0,
        }
    }
}

/// Directory entry
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub file_type: FileType,
    pub inode: u64,
}

/// Open file flags
#[derive(Debug, Clone, Copy)]
pub struct OpenFlags {
    pub read: bool,
    pub write: bool,
    pub create: bool,
    pub truncate: bool,
    pub append: bool,
}

impl OpenFlags {
    pub fn read_only() -> Self {
        Self { read: true, write: false, create: false, truncate: false, append: false }
    }
    
    pub fn write_only() -> Self {
        Self { read: false, write: true, create: true, truncate: true, append: false }
    }
    
    pub fn read_write() -> Self {
        Self { read: true, write: true, create: true, truncate: false, append: false }
    }
    
    pub fn append() -> Self {
        Self { read: false, write: true, create: true, truncate: false, append: true }
    }
}

impl Default for OpenFlags {
    fn default() -> Self {
        Self::read_only()
    }
}

/// Seek position for file operations
#[derive(Debug, Clone, Copy)]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}

/// VFS Error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfsError {
    NotFound,
    AlreadyExists,
    NotDirectory,
    IsDirectory,
    PermissionDenied,
    InvalidPath,
    NoSpace,
    IoError,
    NotMounted,
    Busy,
    InvalidOperation,
    TooLarge,
    NotEmpty,
}

impl VfsError {
    pub fn as_str(&self) -> &'static str {
        match self {
            VfsError::NotFound => "File not found",
            VfsError::AlreadyExists => "File already exists",
            VfsError::NotDirectory => "Not a directory",
            VfsError::IsDirectory => "Is a directory",
            VfsError::PermissionDenied => "Permission denied",
            VfsError::InvalidPath => "Invalid path",
            VfsError::NoSpace => "No space left on device",
            VfsError::IoError => "I/O error",
            VfsError::NotMounted => "Filesystem not mounted",
            VfsError::Busy => "Resource busy",
            VfsError::InvalidOperation => "Invalid operation",
            VfsError::TooLarge => "File too large",
            VfsError::NotEmpty => "Directory not empty",
        }
    }
}

pub type VfsResult<T> = Result<T, VfsError>;

/// File handle for open files
pub struct FileHandle {
    pub inode: u64,
    pub position: AtomicU64,
    pub flags: OpenFlags,
    pub fs_id: usize,
}

impl FileHandle {
    pub fn new(inode: u64, flags: OpenFlags, fs_id: usize) -> Self {
        Self {
            inode,
            position: AtomicU64::new(0),
            flags,
            fs_id,
        }
    }
    
    pub fn position(&self) -> u64 {
        self.position.load(Ordering::Relaxed)
    }
    
    pub fn set_position(&self, pos: u64) {
        self.position.store(pos, Ordering::Relaxed);
    }
    
    pub fn advance(&self, bytes: u64) {
        self.position.fetch_add(bytes, Ordering::Relaxed);
    }
}

/// Filesystem trait - implement this for each filesystem type
pub trait Filesystem: Send + Sync {
    /// Get filesystem name (e.g., "psychicfs", "tmpfs")
    fn name(&self) -> &'static str;
    
    /// Check if filesystem is mounted
    fn is_mounted(&self) -> bool;
    
    /// Mount the filesystem
    fn mount(&mut self) -> VfsResult<()>;
    
    /// Unmount the filesystem
    fn unmount(&mut self) -> VfsResult<()>;
    
    /// Sync all pending changes to disk
    fn sync(&mut self) -> VfsResult<()>;
    
    /// Get file attributes by path
    fn getattr(&self, path: &str) -> VfsResult<FileAttr>;
    
    /// Read directory contents
    fn readdir(&self, path: &str) -> VfsResult<Vec<DirEntry>>;
    
    /// Create a regular file
    fn create(&mut self, path: &str, permissions: Permissions) -> VfsResult<u64>;
    
    /// Create a directory
    fn mkdir(&mut self, path: &str, permissions: Permissions) -> VfsResult<()>;
    
    /// Remove a file
    fn unlink(&mut self, path: &str) -> VfsResult<()>;
    
    /// Remove a directory
    fn rmdir(&mut self, path: &str) -> VfsResult<()>;
    
    /// Rename a file or directory
    fn rename(&mut self, from: &str, to: &str) -> VfsResult<()>;
    
    /// Read from a file
    fn read(&self, inode: u64, offset: u64, buffer: &mut [u8]) -> VfsResult<usize>;
    
    /// Write to a file
    fn write(&mut self, inode: u64, offset: u64, data: &[u8]) -> VfsResult<usize>;
    
    /// Truncate a file to a specific size
    fn truncate(&mut self, path: &str, size: u64) -> VfsResult<()>;
    
    /// Get filesystem statistics
    fn statfs(&self) -> VfsResult<FsStats>;
}

/// Filesystem statistics
#[derive(Debug, Clone, Copy)]
pub struct FsStats {
    pub total_blocks: u64,
    pub free_blocks: u64,
    pub block_size: u32,
    pub total_inodes: u64,
    pub free_inodes: u64,
    pub fs_type: u32,
}

/// Mount point entry
struct MountPoint {
    path: String,
    fs: Box<dyn Filesystem>,
    fs_id: usize,
}

/// Maximum path cache entries before LRU eviction
const MAX_PATH_CACHE_SIZE: usize = 256;

/// Virtual Filesystem Manager
pub struct VfsManager {
    /// Mount points sorted by path length (longest first for lookup)
    mounts: RwLock<Vec<MountPoint>>,
    /// Next filesystem ID
    next_fs_id: AtomicU64,
    /// Path resolution cache
    path_cache: RwLock<BTreeMap<String, (usize, String)>>,
    /// LRU order for cache eviction (most recent at end)
    cache_lru: RwLock<Vec<String>>,
}

impl VfsManager {
    pub const fn new() -> Self {
        Self {
            mounts: RwLock::new(Vec::new()),
            next_fs_id: AtomicU64::new(1),
            path_cache: RwLock::new(BTreeMap::new()),
            cache_lru: RwLock::new(Vec::new()),
        }
    }
    
    /// Mount a filesystem at the given path
    pub fn mount(&self, mount_path: &str, mut fs: Box<dyn Filesystem>) -> VfsResult<()> {
        let path = normalize_path(mount_path);
        
        fs.mount()?;
        
        let fs_id = self.next_fs_id.fetch_add(1, Ordering::Relaxed) as usize;
        
        let mut mounts = self.mounts.write();
        
        // Check for existing mount
        if mounts.iter().any(|m| m.path == path) {
            fs.unmount().ok();
            return Err(VfsError::AlreadyExists);
        }
        
        mounts.push(MountPoint { path, fs, fs_id });
        
        // Sort by path length (longest first) for proper lookup
        mounts.sort_by(|a, b| b.path.len().cmp(&a.path.len()));
        
        // Clear path cache and LRU
        self.path_cache.write().clear();
        self.cache_lru.write().clear();
        
        Ok(())
    }
    
    /// Unmount filesystem at the given path
    pub fn unmount(&self, mount_path: &str) -> VfsResult<()> {
        let path = normalize_path(mount_path);
        
        let mut mounts = self.mounts.write();
        
        if let Some(pos) = mounts.iter().position(|m| m.path == path) {
            let mut mount = mounts.remove(pos);
            mount.fs.unmount()?;
            self.path_cache.write().clear();
            self.cache_lru.write().clear();
            Ok(())
        } else {
            Err(VfsError::NotMounted)
        }
    }
    
    /// Resolve a path to (fs_id, relative_path)
    fn resolve_path(&self, path: &str) -> VfsResult<(usize, String)> {
        let normalized = normalize_path(path);
        
        // Check cache first
        if let Some(cached) = self.path_cache.read().get(&normalized) {
            let result: (usize, String) = cached.clone();
            // Update LRU order (move to end)
            let mut lru = self.cache_lru.write();
            if let Some(pos) = lru.iter().position(|p| p == &normalized) {
                lru.remove(pos);
            }
            lru.push(normalized);
            return Ok(result);
        }
        
        let mounts = self.mounts.read();
        
        for mount in mounts.iter() {
            if normalized == mount.path || normalized.starts_with(&format!("{}/", mount.path)) {
                let relative = if normalized == mount.path {
                    String::from("/")
                } else {
                    String::from(&normalized[mount.path.len()..])
                };
                
                let result = (mount.fs_id, relative);
                
                // Cache the result with LRU tracking
                drop(mounts);
                self.cache_insert(normalized, result.clone());
                
                return Ok(result);
            }
        }
        
        // Check root mount
        for mount in mounts.iter() {
            if mount.path == "/" {
                let norm_clone = normalized.clone();
                let result = (mount.fs_id, normalized);
                drop(mounts);
                self.cache_insert(norm_clone, result.clone());
                return Ok(result);
            }
        }
        
        Err(VfsError::NotMounted)
    }
    
    /// Insert into cache with LRU eviction
    fn cache_insert(&self, path: String, value: (usize, String)) {
        let mut cache = self.path_cache.write();
        let mut lru = self.cache_lru.write();
        
        // Evict oldest entries if at capacity
        while lru.len() >= MAX_PATH_CACHE_SIZE {
            if let Some(oldest) = lru.first().cloned() {
                cache.remove(&oldest);
                lru.remove(0);
            } else {
                break;
            }
        }
        
        // Insert new entry
        cache.insert(path.clone(), value);
        lru.push(path);
    }
    
    /// Get filesystem by ID
    fn get_fs(&self, fs_id: usize) -> VfsResult<&dyn Filesystem> {
        let mounts = self.mounts.read();
        for mount in mounts.iter() {
            if mount.fs_id == fs_id {
                // SAFETY: We're returning a reference that's valid as long as mounts is borrowed
                return Ok(unsafe { &*(&*mount.fs as *const dyn Filesystem) });
            }
        }
        Err(VfsError::NotMounted)
    }
    
    /// Get mutable filesystem by ID
    fn get_fs_mut(&self, fs_id: usize) -> VfsResult<&mut dyn Filesystem> {
        let mut mounts = self.mounts.write();
        for mount in mounts.iter_mut() {
            if mount.fs_id == fs_id {
                // SAFETY: We're returning a reference that's valid as long as mounts is borrowed
                return Ok(unsafe { &mut *(&mut *mount.fs as *mut dyn Filesystem) });
            }
        }
        Err(VfsError::NotMounted)
    }
    
    /// Open a file
    pub fn open(&self, path: &str, flags: OpenFlags) -> VfsResult<FileHandle> {
        let (fs_id, rel_path) = self.resolve_path(path)?;
        
        let mounts = self.mounts.read();
        let mount = mounts.iter().find(|m| m.fs_id == fs_id)
            .ok_or(VfsError::NotMounted)?;
        
        // Check if file exists
        match mount.fs.getattr(&rel_path) {
            Ok(attr) => {
                if attr.file_type == FileType::Directory {
                    return Err(VfsError::IsDirectory);
                }
                Ok(FileHandle::new(attr.inode, flags, fs_id))
            }
            Err(VfsError::NotFound) if flags.create => {
                // Need to drop read lock and get write lock
                drop(mounts);
                let mut mounts = self.mounts.write();
                let mount = mounts.iter_mut().find(|m| m.fs_id == fs_id)
                    .ok_or(VfsError::NotMounted)?;
                
                let inode = mount.fs.create(&rel_path, Permissions::default())?;
                Ok(FileHandle::new(inode, flags, fs_id))
            }
            Err(e) => Err(e),
        }
    }
    
    /// Read from an open file
    pub fn read(&self, handle: &FileHandle, buffer: &mut [u8]) -> VfsResult<usize> {
        if !handle.flags.read {
            return Err(VfsError::PermissionDenied);
        }
        
        let mounts = self.mounts.read();
        let mount = mounts.iter().find(|m| m.fs_id == handle.fs_id)
            .ok_or(VfsError::NotMounted)?;
        
        let pos = handle.position();
        let bytes_read = mount.fs.read(handle.inode, pos, buffer)?;
        handle.advance(bytes_read as u64);
        
        Ok(bytes_read)
    }
    
    /// Write to an open file
    pub fn write(&self, handle: &FileHandle, data: &[u8]) -> VfsResult<usize> {
        if !handle.flags.write {
            return Err(VfsError::PermissionDenied);
        }
        
        let mut mounts = self.mounts.write();
        let mount = mounts.iter_mut().find(|m| m.fs_id == handle.fs_id)
            .ok_or(VfsError::NotMounted)?;
        
        let pos = if handle.flags.append {
            // Get current file size for append mode
            // Get root attributes for mount point
            let attr = mount.fs.getattr("/")?;
            attr.size
        } else {
            handle.position()
        };
        
        let bytes_written = mount.fs.write(handle.inode, pos, data)?;
        handle.advance(bytes_written as u64);
        
        Ok(bytes_written)
    }
    
    /// Create a file
    pub fn create(&self, path: &str, permissions: Permissions) -> VfsResult<u64> {
        let (fs_id, rel_path) = self.resolve_path(path)?;
        
        let mut mounts = self.mounts.write();
        let mount = mounts.iter_mut().find(|m| m.fs_id == fs_id)
            .ok_or(VfsError::NotMounted)?;
        
        mount.fs.create(&rel_path, permissions)
    }
    
    /// Delete a file
    pub fn unlink(&self, path: &str) -> VfsResult<()> {
        let (fs_id, rel_path) = self.resolve_path(path)?;
        
        let mut mounts = self.mounts.write();
        let mount = mounts.iter_mut().find(|m| m.fs_id == fs_id)
            .ok_or(VfsError::NotMounted)?;
        
        mount.fs.unlink(&rel_path)
    }
    
    /// Create a directory
    pub fn mkdir(&self, path: &str, permissions: Permissions) -> VfsResult<()> {
        let (fs_id, rel_path) = self.resolve_path(path)?;
        
        let mut mounts = self.mounts.write();
        let mount = mounts.iter_mut().find(|m| m.fs_id == fs_id)
            .ok_or(VfsError::NotMounted)?;
        
        mount.fs.mkdir(&rel_path, permissions)
    }
    
    /// Read directory
    pub fn readdir(&self, path: &str) -> VfsResult<Vec<DirEntry>> {
        let (fs_id, rel_path) = self.resolve_path(path)?;
        
        let mounts = self.mounts.read();
        let mount = mounts.iter().find(|m| m.fs_id == fs_id)
            .ok_or(VfsError::NotMounted)?;
        
        mount.fs.readdir(&rel_path)
    }
    
    /// Get file attributes
    pub fn stat(&self, path: &str) -> VfsResult<FileAttr> {
        let (fs_id, rel_path) = self.resolve_path(path)?;
        
        let mounts = self.mounts.read();
        let mount = mounts.iter().find(|m| m.fs_id == fs_id)
            .ok_or(VfsError::NotMounted)?;
        
        mount.fs.getattr(&rel_path)
    }
    
    /// Sync all filesystems
    pub fn sync_all(&self) -> VfsResult<()> {
        let mut mounts = self.mounts.write();
        for mount in mounts.iter_mut() {
            mount.fs.sync()?;
        }
        Ok(())
    }
    
    /// List all mount points
    pub fn list_mounts(&self) -> Vec<(String, &'static str)> {
        let mounts = self.mounts.read();
        mounts.iter().map(|m| (m.path.clone(), m.fs.name())).collect()
    }
}

/// Normalize a path (remove trailing slashes, handle . and ..)
pub fn normalize_path(path: &str) -> String {
    if path.is_empty() || path == "/" {
        return String::from("/");
    }
    
    let mut components: Vec<&str> = Vec::new();
    
    for component in path.split('/') {
        match component {
            "" | "." => continue,
            ".." => { components.pop(); }
            c => components.push(c),
        }
    }
    
    if components.is_empty() {
        String::from("/")
    } else {
        let mut result = String::new();
        for c in components {
            result.push('/');
            result.push_str(c);
        }
        result
    }
}

/// Extract parent directory from path
pub fn parent_path(path: &str) -> Option<String> {
    let normalized = normalize_path(path);
    if normalized == "/" {
        return None;
    }
    
    if let Some(pos) = normalized.rfind('/') {
        if pos == 0 {
            Some(String::from("/"))
        } else {
            Some(String::from(&normalized[..pos]))
        }
    } else {
        Some(String::from("/"))
    }
}

/// Extract filename from path
pub fn file_name(path: &str) -> Option<&str> {
    let normalized = normalize_path(path);
    if normalized == "/" {
        return None;
    }
    
    path.rsplit('/').next().filter(|s| !s.is_empty())
}

// Global VFS manager
static VFS: VfsManager = VfsManager::new();

/// Initialize VFS
pub fn init() {
    // VFS is initialized, filesystems will be mounted separately
}

/// Mount a filesystem
pub fn mount(path: &str, fs: Box<dyn Filesystem>) -> VfsResult<()> {
    VFS.mount(path, fs)
}

/// Unmount a filesystem
pub fn unmount(path: &str) -> VfsResult<()> {
    VFS.unmount(path)
}

/// Open a file
pub fn open(path: &str, flags: OpenFlags) -> VfsResult<FileHandle> {
    VFS.open(path, flags)
}

/// Read from a file handle
pub fn read(handle: &FileHandle, buffer: &mut [u8]) -> VfsResult<usize> {
    VFS.read(handle, buffer)
}

/// Write to a file handle  
pub fn write(handle: &FileHandle, data: &[u8]) -> VfsResult<usize> {
    VFS.write(handle, data)
}

/// Create a file
pub fn create(path: &str) -> VfsResult<u64> {
    VFS.create(path, Permissions::default())
}

/// Delete a file
pub fn unlink(path: &str) -> VfsResult<()> {
    VFS.unlink(path)
}

/// Create a directory
pub fn mkdir(path: &str) -> VfsResult<()> {
    VFS.mkdir(path, Permissions::default())
}

/// Read directory contents
pub fn readdir(path: &str) -> VfsResult<Vec<DirEntry>> {
    VFS.readdir(path)
}

/// Get file info
pub fn stat(path: &str) -> VfsResult<FileAttr> {
    VFS.stat(path)
}

/// Sync all filesystems
pub fn sync_all() -> VfsResult<()> {
    VFS.sync_all()
}

/// List mount points
pub fn list_mounts() -> Vec<(String, &'static str)> {
    VFS.list_mounts()
}
