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