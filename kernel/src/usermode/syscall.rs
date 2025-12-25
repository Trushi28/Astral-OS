// src/usermode/syscall.rs
//! System call interface and handlers

use crate::process::get_current_pid;
use core::slice;

// Syscall numbers (extended from basic set)
pub const SYS_READ: u64 = 0;
pub const SYS_WRITE: u64 = 1;
pub const SYS_OPEN: u64 = 2;
pub const SYS_CLOSE: u64 = 3;
pub const SYS_STAT: u64 = 4;
pub const SYS_FSTAT: u64 = 5;
pub const SYS_LSEEK: u64 = 8;
pub const SYS_MMAP: u64 = 9;
pub const SYS_MUNMAP: u64 = 11;
pub const SYS_EXIT: u64 = 60;
pub const SYS_GETPID: u64 = 39;
pub const SYS_FORK: u64 = 57;
pub const SYS_EXECVE: u64 = 59;
pub const SYS_WAIT4: u64 = 61;
pub const SYS_KILL: u64 = 62;
pub const SYS_UNAME: u64 = 63;
pub const SYS_YIELD: u64 = 158;
pub const SYS_GETUID: u64 = 102;
pub const SYS_BRK: u64 = 12;
pub const SYS_SBRK: u64 = 45;

// Astral OS specific syscalls (starting from 1000)
pub const SYS_REALITY_FORK: u64 = 1000;
pub const SYS_REALITY_MERGE: u64 = 1001;
pub const SYS_REALITY_STATUS: u64 = 1002;
pub const SYS_INTENT_REQUEST: u64 = 1010;
pub const SYS_CAPABILITY_CHECK: u64 = 1020;
pub const SYS_CAPABILITY_GRANT: u64 = 1021;
pub const SYS_GRAPHICS_CREATE_SURFACE: u64 = 2000;
pub const SYS_GRAPHICS_DESTROY_SURFACE: u64 = 2001;
pub const SYS_GRAPHICS_BLIT: u64 = 2002;
pub const SYS_GRAPHICS_PRESENT: u64 = 2003;
pub const SYS_GRAPHICS_FILL_RECT: u64 = 2004;
pub const SYS_GRAPHICS_GET_SCREEN_SIZE: u64 = 2005;

// Shell syscalls
pub const SYS_FS_LIST: u64 = 200;
pub const SYS_GET_PROCESS_INFO: u64 = 201;
pub const SYS_GET_MEM_INFO: u64 = 202;
pub const SYS_GET_CPU_INFO: u64 = 203;
pub const SYS_CLEAR_SCREEN: u64 = 204;
pub const SYS_PRINT_COLORED: u64 = 211;

// File system extended syscalls
pub const SYS_FS_CREATE: u64 = 205;
pub const SYS_FS_WRITE_FILE: u64 = 206;
pub const SYS_FS_DELETE: u64 = 207;
pub const SYS_FS_READ_FILE: u64 = 208;
pub const SYS_FS_FORMAT: u64 = 213;
pub const SYS_FS_MOUNT: u64 = 214;
pub const SYS_FS_SYNC: u64 = 215;

// Power syscalls
pub const SYS_SHUTDOWN: u64 = 220;
pub const SYS_REBOOT: u64 = 221;

/// System call result
pub type SyscallResult = Result<u64, SyscallError>;

#[derive(Debug, Clone, Copy)]
#[repr(u64)]
pub enum SyscallError {
    InvalidSyscall = 1,
    InvalidArgument = 2,
    PermissionDenied = 3,
    NotFound = 4,
    AlreadyExists = 5,
    OutOfMemory = 6,
    Interrupted = 7,
    IoError = 8,
    NotImplemented = 9,
}

impl SyscallError {
    pub fn as_errno(self) -> u64 {
        -(self as i64) as u64
    }
}

/// Main syscall dispatcher
pub fn handle_syscall(
    syscall_num: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
) -> u64 {
    let result = match syscall_num {
        SYS_READ => sys_read(arg1 as i32, arg2 as *mut u8, arg3 as usize),
        SYS_WRITE => sys_write(arg1 as i32, arg2 as *const u8, arg3 as usize),
        SYS_OPEN => sys_open(arg1 as *const u8, arg2 as u32, arg3 as u32),
        SYS_CLOSE => sys_close(arg1 as i32),
        SYS_EXIT => sys_exit(arg1 as i32),
        SYS_GETPID => sys_getpid(),
        SYS_FORK => sys_fork(),
        SYS_YIELD => sys_yield(),
        SYS_GETUID => sys_getuid(),
        
        // Memory management
        SYS_BRK => sys_brk(arg1),
        SYS_SBRK => sys_sbrk(arg1 as i64),
        
        // Astral OS specific
        SYS_REALITY_FORK => sys_reality_fork(),
        SYS_REALITY_MERGE => sys_reality_merge(arg1),
        SYS_REALITY_STATUS => sys_reality_status(arg1 as *mut u64),
        SYS_INTENT_REQUEST => sys_intent_request(arg1 as *const u8, arg2 as usize),
        SYS_CAPABILITY_CHECK => sys_capability_check(arg1),
        
        // Graphics
        SYS_GRAPHICS_CREATE_SURFACE => sys_graphics_create_surface(arg1 as u32, arg2 as u32),
        SYS_GRAPHICS_BLIT => sys_graphics_blit(arg1, arg2 as *const u8, arg3 as usize),
        SYS_GRAPHICS_PRESENT => sys_graphics_present(arg1),
        SYS_GRAPHICS_FILL_RECT => sys_graphics_fill_rect(arg1, arg2 as u32, arg3 as u32, arg4 as u32, arg5 as u32, 0xFFFFFFFF),
        SYS_GRAPHICS_GET_SCREEN_SIZE => sys_graphics_get_screen_size(),
        
        // Shell commands
        SYS_FS_LIST => sys_fs_list(),
        SYS_GET_PROCESS_INFO => sys_get_process_info(),
        SYS_GET_MEM_INFO => sys_get_mem_info(),
        SYS_GET_CPU_INFO => sys_get_cpu_info(),
        SYS_CLEAR_SCREEN => sys_clear_screen(),
        SYS_PRINT_COLORED => sys_print_colored(arg1 as *const u8, arg2 as usize, arg3 as u32),
        
        // File system extended
        SYS_FS_CREATE => sys_fs_create(arg1 as *const u8, arg2 as usize),
        SYS_FS_WRITE_FILE => sys_fs_write_file(arg1 as *const u8, arg2 as usize, arg3 as *const u8, arg4 as usize),
        SYS_FS_DELETE => sys_fs_delete(arg1 as *const u8, arg2 as usize),
        SYS_FS_READ_FILE => sys_fs_read_file(arg1 as *const u8, arg2 as usize, arg3 as *mut u8, arg4 as usize),
        SYS_FS_FORMAT => sys_fs_format(),
        SYS_FS_MOUNT => sys_fs_mount(),
        SYS_FS_SYNC => sys_fs_sync(),
        
        // Power
        SYS_SHUTDOWN => sys_shutdown(),
        SYS_REBOOT => sys_reboot(),
        
        _ => Err(SyscallError::InvalidSyscall),
    };
    
    match result {
        Ok(val) => val,
        Err(e) => e.as_errno(),
    }
}

// File I/O syscalls
fn sys_read(fd: i32, buf: *mut u8, count: usize) -> SyscallResult {
    if fd < 0 || buf.is_null() || count == 0 {
        return Err(SyscallError::InvalidArgument);
    }
    
    // Validate user pointer
    if !is_user_pointer_valid(buf, count) {
        return Err(SyscallError::InvalidArgument);
    }
    
    // stdin (fd=0) - read from keyboard
    if fd == 0 {
        // Try to read from keyboard buffer
        let mut bytes_read = 0;
        
        unsafe {
            while bytes_read < count {
                if let Some(key) = crate::interrupts::getchar() {
                    // Skip non-printable keys (special keys)
                    if key < 0x80 && key >= 0x20 || key == b'\n' || key == 8 {
                        *buf.add(bytes_read) = key;
                        bytes_read += 1;
                        
                        // For interactive input, return after newline  
                        if key == b'\n' {
                            break;
                        }
                    }
                } else {
                    // No more characters available
                    break;
                }
            }
        }
        
        if bytes_read > 0 {
            return Ok(bytes_read as u64);
        }
        
        // No input available, return 0 (non-blocking)
        return Ok(0);
    }
    
    // Other file descriptors not implemented
    Err(SyscallError::NotImplemented)
}

fn sys_write(fd: i32, buf: *const u8, count: usize) -> SyscallResult {
    if fd < 0 || buf.is_null() || count == 0 {
        return Err(SyscallError::InvalidArgument);
    }
    
    // Validate user pointer
    if !is_user_pointer_valid(buf, count) {
        return Err(SyscallError::InvalidArgument);
    }
    
    unsafe {
        let data = slice::from_raw_parts(buf, count);
        
        // Write to stdout (fd=1) or stderr (fd=2)
        if fd == 1 || fd == 2 {
            if let Ok(s) = core::str::from_utf8(data) {
                crate::print!("{}", s);
                return Ok(count as u64);
            }
        }
    }
    
    Err(SyscallError::InvalidArgument)
}

fn sys_open(path: *const u8, _flags: u32, _mode: u32) -> SyscallResult {
    if path.is_null() {
        return Err(SyscallError::InvalidArgument);
    }
    
    // Read path from userspace
    let _path_str = unsafe { read_user_string(path, 4096)? };
    
    // For now, stub implementation
    Err(SyscallError::NotImplemented)
}

fn sys_close(fd: i32) -> SyscallResult {
    if fd < 0 {
        return Err(SyscallError::InvalidArgument);
    }
    
    // For now, stub implementation
    Err(SyscallError::NotImplemented)
}

// Process syscalls
fn sys_exit(code: i32) -> SyscallResult {
    crate::process::exit_process(code);
    Ok(0)
}

fn sys_getpid() -> SyscallResult {
    if let Some(pid) = get_current_pid() {
        Ok(pid.as_u64())
    } else {
        Err(SyscallError::InvalidArgument)
    }
}

fn sys_fork() -> SyscallResult {
    // Fork current process
    // This would create a copy of the current process
    Err(SyscallError::NotImplemented)
}

fn sys_yield() -> SyscallResult {
    crate::process::scheduler::yield_cpu();
    Ok(0)
}

fn sys_getuid() -> SyscallResult {
    if let Some(pid) = get_current_pid() {
        let table = crate::process::process_table().lock();
        if let Some(proc) = table.get(pid) {
            return Ok(proc.uid as u64);
        }
    }
    // Default to strict failure if process not found
    Err(SyscallError::PermissionDenied)
}

fn sys_fs_list() -> SyscallResult {
    let files = crate::fs::psychicfs::fs_list();
    crate::println!("Files:");
    for file in files {
        crate::println!("  {}", file);
    }
    Ok(0)
}

fn sys_get_process_info() -> SyscallResult {
    crate::println!("PID | UID | State   | Priority    | Intent");
    crate::println!("----|-----|---------|-------------|-------");
    
    let table = crate::process::process_table().lock();
    for proc in table.iter() {
        crate::println!("{:<3} | {:<3} | {:<7?} | {:<11?} | {:<11?}", 
            proc.pid.as_u64(),
            proc.uid,
            proc.state,
            proc.priority,
            proc.intent
        );
    }
    Ok(0)
}

fn sys_get_mem_info() -> SyscallResult {
    let (total, used, free) = crate::memory::frame::get_stats();
    crate::println!("Memory System:");
    crate::println!("  Total: {} KB", total / 1024);
    crate::println!("  Used:  {} KB", used / 1024);
    crate::println!("  Free:  {} KB", free / 1024);
    Ok(0)
}

fn sys_get_cpu_info() -> SyscallResult {
    let count = crate::get_cpu_count();
    crate::println!("CPU Information:");
    crate::println!("  Cores: {}", count);
    crate::println!("  Arch:  x86_64");
    Ok(0)
}

fn sys_clear_screen() -> SyscallResult {
    crate::drivers::framebuffer::clear();
    Ok(0)
}

// Reality Engine syscalls
fn sys_reality_fork() -> SyscallResult {
    use crate::reality::causality::{RealityId, set_current_reality};
    
    let new_reality = RealityId::new();
    set_current_reality(new_reality);
    
    Ok(new_reality.as_u64())
}

fn sys_reality_merge(_target_reality_id: u64) -> SyscallResult {
    use crate::reality::causality::{RealityId, set_current_reality};
    
    let target = RealityId::new(); // Would lookup by ID
    set_current_reality(target);
    
    Ok(0)
}

fn sys_reality_status(out_ptr: *mut u64) -> SyscallResult {
    if out_ptr.is_null() || !is_user_pointer_valid(out_ptr, 8) {
        return Err(SyscallError::InvalidArgument);
    }
    
    use crate::reality::causality::RealityId;
    
    let current = RealityId::current();
    
    unsafe {
        *out_ptr = current.as_u64();
    }
    
    Ok(0)
}

// Intent-based syscalls
fn sys_intent_request(intent_str: *const u8, len: usize) -> SyscallResult {
    if intent_str.is_null() || !is_user_pointer_valid(intent_str, len) {
        return Err(SyscallError::InvalidArgument);
    }
    
    unsafe {
        let _intent_data = slice::from_raw_parts(intent_str, len);
        
        // Parse intent and route to appropriate handler
        // For now, stub
    }
    
    Err(SyscallError::NotImplemented)
}

fn sys_capability_check(_capability_id: u64) -> SyscallResult {
    // Check if current process has capability
    // For now, grant all capabilities
    Ok(1)
}

// Graphics syscalls
fn sys_graphics_create_surface(width: u32, height: u32) -> SyscallResult {
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return Err(SyscallError::InvalidArgument);
    }
    
    // Create surface in graphics server
    match crate::graphics::create_surface(width, height) {
        Ok(id) => Ok(id),
        Err(_) => Err(SyscallError::OutOfMemory),
    }
}

fn sys_graphics_blit(surface_id: u64, pixels: *const u8, size: usize) -> SyscallResult {
    if pixels.is_null() || !is_user_pointer_valid(pixels, size) {
        return Err(SyscallError::InvalidArgument);
    }
    
    // Copy pixel data to surface
    unsafe {
        let data = slice::from_raw_parts(pixels, size);
        match crate::graphics::update_surface(surface_id, data) {
            Ok(()) => Ok(0),
            Err(_) => Err(SyscallError::InvalidArgument),
        }
    }
}

fn sys_graphics_present(surface_id: u64) -> SyscallResult {
    // Present surface to screen
    match crate::graphics::present_surface(surface_id) {
        Ok(()) => {
            // Trigger composition
            crate::graphics::composite_frame();
            Ok(0)
        },
        Err(_) => Err(SyscallError::InvalidArgument),
    }
}

// Additional graphics syscalls for drawing primitives
fn sys_graphics_fill_rect(surface_id: u64, x: u32, y: u32, w: u32, h: u32, color: u32) -> SyscallResult {
    let result = crate::graphics::with_server(|server| {
        if let Some(surface) = server.get_surface_mut(surface_id) {
            // Fill rectangle on surface
            let bytes_per_pixel = surface.format.bytes_per_pixel();
            for dy in 0..h {
                for dx in 0..w {
                    let px = x + dx;
                    let py = y + dy;
                    if px < surface.width && py < surface.height {
                        let offset = (py * surface.width + px) as usize * bytes_per_pixel;
                        if offset + 3 < surface.pixels.len() {
                            // RGBA format
                            surface.pixels[offset] = ((color >> 24) & 0xFF) as u8;
                            surface.pixels[offset + 1] = ((color >> 16) & 0xFF) as u8;
                            surface.pixels[offset + 2] = ((color >> 8) & 0xFF) as u8;
                            surface.pixels[offset + 3] = (color & 0xFF) as u8;
                        }
                    }
                }
            }
            surface.mark_dirty();
            true
        } else {
            false
        }
    });
    
    match result {
        Some(true) => Ok(0),
        _ => Err(SyscallError::InvalidArgument),
    }
}

fn sys_graphics_get_screen_size() -> SyscallResult {
    let (width, height) = crate::drivers::framebuffer::get_dimensions();
    // Pack width and height into return value (width in high 32 bits, height in low 32 bits)
    Ok(((width as u64) << 32) | (height as u64))
}

// Utility functions
fn is_user_pointer_valid<T>(ptr: *const T, count: usize) -> bool {
    let addr = ptr as usize;
    let size = count * core::mem::size_of::<T>();
    
    // Check if address is in user space (< 0x0000_8000_0000_0000)
    if addr >= 0x0000_8000_0000_0000 {
        return false;
    }
    
    // Check for overflow
    if addr.checked_add(size).is_none() {
        return false;
    }
    
    // Would also check if pages are mapped in process page table
    true
}

unsafe fn read_user_string(ptr: *const u8, max_len: usize) -> Result<&'static str, SyscallError> {
    if !is_user_pointer_valid(ptr, max_len) {
        return Err(SyscallError::InvalidArgument);
    }
    
    let mut len = 0;
    while len < max_len {
        if *ptr.add(len) == 0 {
            break;
        }
        len += 1;
    }
    
    let slice = slice::from_raw_parts(ptr, len);
    core::str::from_utf8(slice)
        .map_err(|_| SyscallError::InvalidArgument)
}

// ============================================================================
// Memory Management Syscalls
// ============================================================================

/// Get or set the program break (heap end)
/// If addr is 0, returns current break
/// Otherwise, sets new break and returns new break address
fn sys_brk(addr: u64) -> SyscallResult {
    let pid = get_current_pid().ok_or(SyscallError::InvalidArgument)?;
    let mut table = crate::process::process_table().lock();
    let proc = table.get_mut(pid).ok_or(SyscallError::InvalidArgument)?;
    
    if addr == 0 {
        // Query current break
        return Ok(proc.heap_end);
    }
    
    // Validate new break is in user space and not below heap start
    if addr < proc.heap_start || addr >= 0x0000_7000_0000_0000 {
        return Err(SyscallError::InvalidArgument);
    }
    
    let old_end = proc.heap_end;
    let new_end = crate::util::align_up(addr as usize, crate::PAGE_SIZE) as u64;
    
    if new_end > old_end {
        // Growing heap - need to map new pages
        let page_table_phys = proc.page_table;
        drop(table); // Release lock before memory operations
        
        let mut page_table = crate::memory::PageTableManager::from_phys(
            crate::memory::PhysAddr::new(page_table_phys)
        );
        
        let start_page = crate::util::align_up(old_end as usize, crate::PAGE_SIZE) as u64;
        let num_pages = ((new_end - start_page) as usize + crate::PAGE_SIZE - 1) / crate::PAGE_SIZE;
        
        let hhdm = crate::get_hhdm_offset();
        let flags = crate::memory::PageTableEntry::PRESENT 
            | crate::memory::PageTableEntry::WRITABLE 
            | crate::memory::PageTableEntry::USER
            | crate::memory::PageTableEntry::NO_EXECUTE;
        
        for i in 0..num_pages {
            let page_virt = start_page + (i * crate::PAGE_SIZE) as u64;
            
            // Check if page already mapped
            if page_table.translate(crate::memory::VirtAddr::new(page_virt)).is_some() {
                continue;
            }
            
            let frame = crate::memory::frame::allocate_frame()
                .ok_or(SyscallError::OutOfMemory)?;
            
            // Zero the new page
            unsafe {
                let ptr = (frame.as_u64() as usize + hhdm) as *mut u8;
                core::ptr::write_bytes(ptr, 0, crate::PAGE_SIZE);
            }
            
            page_table.map(crate::memory::VirtAddr::new(page_virt), frame, flags)
                .map_err(|_| SyscallError::OutOfMemory)?;
        }
        
        // Update heap_end in process
        let mut table = crate::process::process_table().lock();
        if let Some(proc) = table.get_mut(pid) {
            proc.heap_end = new_end;
        }
    } else if new_end < old_end {
        // Shrinking heap - could unmap pages (optional, leave mapped for now)
        proc.heap_end = new_end;
    }
    
    Ok(new_end)
}

/// Increment/decrement the program break by given amount
/// Returns the OLD break address (before change)
fn sys_sbrk(increment: i64) -> SyscallResult {
    let pid = get_current_pid().ok_or(SyscallError::InvalidArgument)?;
    
    let (old_break, new_break) = {
        let table = crate::process::process_table().lock();
        let proc = table.get(pid).ok_or(SyscallError::InvalidArgument)?;
        let old = proc.heap_end;
        let new = if increment >= 0 {
            old.checked_add(increment as u64).ok_or(SyscallError::InvalidArgument)?
        } else {
            old.checked_sub((-increment) as u64).ok_or(SyscallError::InvalidArgument)?
        };
        (old, new)
    };
    
    if increment != 0 {
        // Set the new break
        sys_brk(new_break)?;
    }
    
    Ok(old_break)
}

// ============================================================================
// File System Extended Syscalls
// ============================================================================

fn sys_print_colored(buf: *const u8, len: usize, color: u32) -> SyscallResult {
    if buf.is_null() || len == 0 || !is_user_pointer_valid(buf, len) {
        return Err(SyscallError::InvalidArgument);
    }
    
    unsafe {
        let data = slice::from_raw_parts(buf, len);
        if let Ok(s) = core::str::from_utf8(data) {
            crate::drivers::framebuffer::print_colored(s, color);
            return Ok(len as u64);
        }
    }
    
    Err(SyscallError::InvalidArgument)
}

fn sys_fs_create(path_ptr: *const u8, path_len: usize) -> SyscallResult {
    crate::serial_println!("[SYSCALL] sys_fs_create: path_ptr={:?}, path_len={}", path_ptr, path_len);
    
    if path_ptr.is_null() || path_len == 0 {
        crate::serial_println!("[SYSCALL] sys_fs_create: invalid path");
        return Err(SyscallError::InvalidArgument);
    }
    
    unsafe {
        let path = read_user_string(path_ptr, path_len)?;
        crate::serial_println!("[SYSCALL] sys_fs_create: creating file '{}'", path);
        if crate::fs::fs_create(path) {
            crate::serial_println!("[SYSCALL] sys_fs_create: success");
            Ok(0)
        } else {
            crate::serial_println!("[SYSCALL] sys_fs_create: fs_create failed");
            Err(SyscallError::IoError)
        }
    }
}

fn sys_fs_write_file(path_ptr: *const u8, path_len: usize, data_ptr: *const u8, data_len: usize) -> SyscallResult {
    crate::serial_println!("[SYSCALL] sys_fs_write_file: path_ptr={:?}, path_len={}, data_ptr={:?}, data_len={}", 
        path_ptr, path_len, data_ptr, data_len);
    
    if path_ptr.is_null() || path_len == 0 {
        crate::serial_println!("[SYSCALL] sys_fs_write_file: invalid path");
        return Err(SyscallError::InvalidArgument);
    }
    if data_ptr.is_null() {
        crate::serial_println!("[SYSCALL] sys_fs_write_file: null data pointer");
        return Err(SyscallError::InvalidArgument);
    }
    if !is_user_pointer_valid(data_ptr, data_len) {
        crate::serial_println!("[SYSCALL] sys_fs_write_file: invalid data pointer");
        return Err(SyscallError::InvalidArgument);
    }
    
    unsafe {
        let path = read_user_string(path_ptr, path_len)?;
        crate::serial_println!("[SYSCALL] sys_fs_write_file: writing {} bytes to '{}'", data_len, path);
        let data = slice::from_raw_parts(data_ptr, data_len);
        if crate::fs::fs_write(path, data) {
            crate::serial_println!("[SYSCALL] sys_fs_write_file: success");
            Ok(data_len as u64)
        } else {
            crate::serial_println!("[SYSCALL] sys_fs_write_file: fs_write failed");
            Err(SyscallError::IoError)
        }
    }
}

fn sys_fs_delete(path_ptr: *const u8, path_len: usize) -> SyscallResult {
    if path_ptr.is_null() || path_len == 0 {
        return Err(SyscallError::InvalidArgument);
    }
    
    unsafe {
        let path = read_user_string(path_ptr, path_len)?;
        if crate::fs::fs_delete(path) {
            Ok(0)
        } else {
            Err(SyscallError::NotFound)
        }
    }
}

fn sys_fs_read_file(path_ptr: *const u8, path_len: usize, buf: *mut u8, buf_len: usize) -> SyscallResult {
    if path_ptr.is_null() || path_len == 0 {
        return Err(SyscallError::InvalidArgument);
    }
    if buf.is_null() || !is_user_pointer_valid(buf, buf_len) {
        return Err(SyscallError::InvalidArgument);
    }
    
    unsafe {
        let path = read_user_string(path_ptr, path_len)?;
        if let Some(data) = crate::fs::fs_read(path) {
            let copy_len = core::cmp::min(data.len(), buf_len);
            core::ptr::copy_nonoverlapping(data.as_ptr(), buf, copy_len);
            Ok(copy_len as u64)
        } else {
            Err(SyscallError::NotFound)
        }
    }
}

fn sys_fs_format() -> SyscallResult {
    // Check if user is root
    if let Some(pid) = get_current_pid() {
        let table = crate::process::process_table().lock();
        if let Some(proc) = table.get(pid) {
            if proc.uid != 0 {
                return Err(SyscallError::PermissionDenied);
            }
        }
    }
    
    if crate::fs::fs_format() {
        Ok(0)
    } else {
        Err(SyscallError::IoError)
    }
}

fn sys_fs_mount() -> SyscallResult {
    if crate::fs::fs_mount() {
        Ok(0)
    } else {
        Err(SyscallError::IoError)
    }
}

fn sys_fs_sync() -> SyscallResult {
    crate::fs::fs_sync();
    Ok(0)
}

// ============================================================================
// Power Syscalls
// ============================================================================

fn sys_shutdown() -> SyscallResult {
    // Check if user is root
    if let Some(pid) = get_current_pid() {
        let table = crate::process::process_table().lock();
        if let Some(proc) = table.get(pid) {
            if proc.uid != 0 {
                return Err(SyscallError::PermissionDenied);
            }
        }
    }
    
    crate::power::shutdown();
    Ok(0)
}

fn sys_reboot() -> SyscallResult {
    // Check if user is root
    if let Some(pid) = get_current_pid() {
        let table = crate::process::process_table().lock();
        if let Some(proc) = table.get(pid) {
            if proc.uid != 0 {
                return Err(SyscallError::PermissionDenied);
            }
        }
    }
    
    crate::power::reboot();
    Ok(0)
}