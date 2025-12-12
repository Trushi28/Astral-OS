// src/arch/mod.rs
//! Architecture-specific code (x86_64)

#[cfg(target_arch = "x86_64")]
pub mod x86_64;

#[cfg(target_arch = "x86_64")]
pub use x86_64::*;