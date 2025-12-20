//! Usermode execution module
//! 
//! Provides Ring 3 user-mode process support with syscall-based API.

pub mod elf;
pub mod loader;
pub mod spawn;
pub mod ring3_binary;
pub mod syscall;
pub mod test;
pub mod usys;
pub mod shell;
pub mod launcher;

/// Embedded userland shell binary (built separately, converted to flat binary)
/// This is the TRUE Ring 3 shell that uses syscalls for I/O
pub static USERLAND_SHELL_BIN: &[u8] = include_bytes!("../../../target/x86_64-userland/release/userland.bin");