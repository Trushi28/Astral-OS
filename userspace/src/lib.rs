//! Userspace Runtime Library
//!
//! Provides syscall wrappers and runtime support for user programs.

#![no_std]

pub mod syscall;

use core::panic::PanicInfo;

/// Panic handler - just exit with error code
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    syscall::exit(1);
}
