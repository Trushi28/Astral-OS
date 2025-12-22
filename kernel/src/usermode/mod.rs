//! Usermode execution module
//! 
//! Provides Ring 3 user-mode process support with syscall-based API.

pub mod elf;
pub mod loader;
pub mod spawn;
pub mod syscall;
pub mod test;
pub mod launcher;

/// Embedded userland shell ELF binary (built separately)
/// This is the TRUE Ring 3 shell that uses syscalls for I/O
pub static USERLAND_SHELL_ELF: &[u8] = include_bytes!("../../../target/x86_64-userland/release/userland");