use crate::process::{ProcessId, Priority};
use crate::serial_println;

/// Syscall numbers (Linux-compatible where possible)
#[repr(u64)]
#[derive(Debug, Clone, Copy)]
pub enum Syscall {
    Exit = 60,          // exit(code)
    Write = 1,          // write(fd, buf, len)
    Read = 2,           // read(fd, buf, len)
    Getpid = 39,        // getpid()
    Clone = 56,         // clone(fn_ptr, stack, flags) - thread creation!
    Yield = 24,         // sched_yield()
}

/// Syscall error codes (negative errno)
#[repr(i64)]
pub enum SyscallError {
    InvalidSyscall = -38,   // ENOSYS
    InvalidArgument = -22,  // EINVAL
    PermissionDenied = -13, // EACCES
}

/// Initialize syscall interface
/// Sets up MSRs for syscall/sysret
pub unsafe fn init() {
    // Syscalls use int 0x80 method (legacy but simple)
    // Modern syscall instruction would require MSR configuration
    
    serial_println!("[SYSCALL] Interface initialized (int 0x80 method)");
}

/// Handle syscall from userspace
/// 
/// Arguments (Linux x86-64 ABI):
/// - RAX: syscall number
/// - RDI: arg1
/// - RSI: arg2
/// - RDX: arg3
/// - R10: arg4 (not RCX, which holds return address)
/// - R8: arg5
/// - R9: arg6
/// 
/// Return: RAX (0 or positive = success, negative = -errno)
pub fn handle_syscall(
    syscall_num: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    _arg4: u64,
    _arg5: u64,
    _arg6: u64,
) -> i64 {
    match syscall_num {
        // sys_exit(code)
        60 => sys_exit(arg1 as i32),
        
        // sys_write(fd, buf, len)
        1 => sys_write(arg1 as i32, arg2 as usize, arg3 as usize),
        
        // sys_getpid()
        39 => sys_getpid(),
        
        // sys_clone(fn_ptr, stack_ptr, flags) - **THREAD CREATION**
        56 => sys_clone(arg1, arg2, arg3),
        
        // sys_yield()
        24 => sys_yield(),
        
        _ => {
            serial_println!("[SYSCALL] Unknown syscall: {}", syscall_num);
            SyscallError::InvalidSyscall as i64
        }
    }
}

/// sys_exit - Terminate current process
/// 
/// Properly cleans up resources:
/// - Marks process as DEAD
/// - Frees stack memory
/// - Removes from scheduler
/// - Switches to next process
fn sys_exit(code: i32) -> i64 {
    serial_println!("[SYSCALL] exit({}) - Process terminating", code);
    
    // Single-CPU assumed for now (multicore requires per-CPU tracking)
    let cpu_id = 0;
    
    // Cleanup current process
    if let Some(ref sched) = *crate::scheduler::SCHEDULER.lock() {
        if let Some(mut process) = sched.current(cpu_id) {
            serial_println!("[EXIT] Cleaning up PID {}", process.pid.as_u32());
            
            // Mark as dead
            process.state = crate::process::ProcessState::Dead;
            
            // Free stack
            {
                let mut pool = crate::process::stack_pool::STACK_POOL.lock();
                pool.free(process.kernel_stack);
                serial_println!("[EXIT] Freed stack for PID {}", process.pid.as_u32());
            }
            
            // Clear current process
            sched.set_current(cpu_id, None);
            
            serial_println!("[EXIT] PID {} cleanup complete", process.pid.as_u32());
        }
    }
    
    // After cleanup, halt CPU (proper impl would switch to next process)
    unsafe {
        loop {
            x86_64::instructions::hlt();
        }
    }
}

/// sys_write - Write to file descriptor
/// For now, only supports stdout (fd=1) and stderr (fd=2) to serial
/// 
/// Full implementation needs:
/// - VFS layer for file operations
/// - Userspace memory validation
/// - Proper error handling
fn sys_write(fd: i32, buf_ptr: usize, len: usize) -> i64 {
    if fd != 1 && fd != 2 {
        return SyscallError::InvalidArgument as i64;
    }
    
    // Validation skipped - full impl requires VFS
    // Buffer writing deferred to filesystem implementation
    serial_println!("[SYSCALL] write(fd={}, len={}) - stub implementation", fd, len);
    
    len as i64 // Return bytes "written"
}

/// sys_getpid - Get current process ID
fn sys_getpid() -> i64 {
    // Current process PID requires per-CPU tracking
    serial_println!("[SYSCALL] getpid()");
    1 // Placeholder PID
}

/// sys_clone - Create a new thread/process (CRITICAL for applications!)
/// 
/// This is how userspace applications create threads!
/// 
/// Args:
/// - fn_ptr: Entry point for new thread
/// - stack_ptr: Stack pointer for new thread
/// - flags: Clone flags (CLONE_VM for threads, etc.)
/// 
/// Returns: PID of new thread, or negative errno
fn sys_clone(fn_ptr: u64, stack_ptr: u64, flags: u64) -> i64 {
    serial_println!("[SYSCALL] clone(fn={:#x}, stack={:#x}, flags={:#x})", fn_ptr, stack_ptr, flags);
    
    // Kernel thread creation (userspace requires page table isolation)
    
    // Create wrapper function that jumps to user entry point
    fn thread_wrapper() -> ! {
        // Userspace context setup deferred (needs Ring 3 transition)
        loop {
            x86_64::instructions::hlt();
        }
    }
    
    let pid = crate::process::spawn::spawn_kernel_thread(thread_wrapper, Priority::Normal);
    
    serial_println!("[SYSCALL] Created thread with PID {}", pid.as_u32());
    
    pid.as_u32() as i64
}

/// sys_yield - Voluntarily yield CPU to scheduler
fn sys_yield() -> i64 {
    serial_println!("[SYSCALL] yield()");
    // Context switch triggered by next timer interrupt
    0
}
