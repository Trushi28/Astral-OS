//src/lib.rs
#![no_std]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(naked_functions)]
#![feature(const_mut_refs)]
#![feature(core_intrinsics)]

extern crate alloc;

pub mod memory;
pub mod process;
pub mod interrupts;
pub mod drivers;
pub mod fs;
pub mod reality;
pub mod shell;
pub mod util;

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

// ============================================================================
// GLOBAL CONSTANTS
// ============================================================================

pub const PAGE_SIZE: usize = 4096;
pub const MAX_PROCESSES: usize = 256;
pub const KERNEL_HEAP_SIZE: usize = 100 * 1024 * 1024; // 100MB

// Virtual memory layout
pub const KERNEL_VIRT_BASE: u64 = 0xFFFFFFFF80000000;
pub const USER_VIRT_BASE: u64 = 0x400000;

// ============================================================================
// GLOBAL STATE
// ============================================================================

static HHDM_OFFSET: AtomicUsize = AtomicUsize::new(0);
static SYSTEM_TICKS: AtomicU64 = AtomicU64::new(0);

pub fn set_hhdm_offset(offset: usize) {
    HHDM_OFFSET.store(offset, Ordering::SeqCst);
}

pub fn get_hhdm_offset() -> usize {
    HHDM_OFFSET.load(Ordering::Relaxed)
}

pub fn get_timestamp() -> u64 {
    SYSTEM_TICKS.load(Ordering::Relaxed)
}

pub fn increment_timestamp() {
    SYSTEM_TICKS.fetch_add(1, Ordering::Relaxed);
}

// ============================================================================
// LIMINE REQUESTS - CRITICAL: Must be in .requests section
// ============================================================================

use limine::request::{
    FramebufferRequest, HhdmRequest, MemoryMapRequest,
    StackSizeRequest, RequestsEndMarker, RequestsStartMarker
};

// Start marker - MUST be first
#[used]
#[link_section = ".requests_start_marker"]
static REQUESTS_START: RequestsStartMarker = RequestsStartMarker::new();

// Framebuffer request
#[used]
#[link_section = ".requests"]
pub static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

// Memory map request
#[used]
#[link_section = ".requests"]
pub static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

// HHDM request
#[used]
#[link_section = ".requests"]
pub static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

// Stack size request
#[used]
#[link_section = ".requests"]
static STACK_SIZE_REQUEST: StackSizeRequest = StackSizeRequest::new()
    .with_size(0x10000); // 64KB stack

// End marker - MUST be last
#[used]
#[link_section = ".requests_end_marker"]
static REQUESTS_END: RequestsEndMarker = RequestsEndMarker::new();