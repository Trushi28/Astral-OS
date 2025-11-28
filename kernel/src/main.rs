// ============================================================================
// ASTRAL OS - SECTION 1: HEADER & CORE TYPES
// ============================================================================

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(naked_functions)]
#![feature(const_mut_refs)]

extern crate alloc;

use core::panic::PanicInfo;
use core::arch::asm;
use core::ptr::{write_volatile, read_volatile};
use core::sync::atomic::{AtomicU64, AtomicUsize, AtomicBool, AtomicU8, Ordering};
use alloc::vec::Vec;
use alloc::string::String;
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use spin::Mutex;
use core::alloc::{GlobalAlloc, Layout};
use alloc::string::ToString;
use alloc::format;
// Limine requests
use limine::request::{
    FramebufferRequest, HhdmRequest, MemoryMapRequest, 
    RsdpRequest, StackSizeRequest, ExecutableAddressRequest
};
use limine::memory_map::EntryType;

// ============================================================================
// GLOBAL CONSTANTS
// ============================================================================

const PAGE_SIZE: usize = 4096;
const MAX_PROCESSES: usize = 256;
const MAX_THREADS_PER_PROCESS: usize = 16;
const KERNEL_STACK_SIZE: usize = 0x10000; // 64KB
const USER_STACK_SIZE: usize = 0x100000;  // 1MB
const KERNEL_HEAP_SIZE: usize = 100 * 1024 * 1024; // 100MB
const MAX_OPEN_FILES: usize = 256;

// Virtual memory layout
const KERNEL_VIRT_BASE: u64 = 0xFFFFFFFF80000000;
const USER_VIRT_BASE: u64 = 0x400000;
const USER_STACK_TOP: u64 = 0x7FFFFFFFFFFF;

// Syscall numbers
const SYS_EXIT: u64 = 0;
const SYS_READ: u64 = 1;
const SYS_WRITE: u64 = 2;
const SYS_OPEN: u64 = 3;
const SYS_CLOSE: u64 = 4;
const SYS_FORK: u64 = 5;
const SYS_EXEC: u64 = 6;
const SYS_GETPID: u64 = 7;
const SYS_SLEEP: u64 = 8;
const SYS_YIELD: u64 = 9;

// Process states
const PROCESS_READY: u8 = 0;
const PROCESS_RUNNING: u8 = 1;
const PROCESS_BLOCKED: u8 = 2;
const PROCESS_ZOMBIE: u8 = 3;

// ============================================================================
// LIMINE REQUESTS (STATIC)
// ============================================================================

static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();
static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();
static RSDP_REQUEST: RsdpRequest = RsdpRequest::new();
static STACK_SIZE_REQUEST: StackSizeRequest = StackSizeRequest::new().with_size(0x10000);
static KERNEL_ADDRESS_REQUEST: ExecutableAddressRequest = ExecutableAddressRequest::new();

// ============================================================================
// GLOBAL STATE
// ============================================================================

static HHDM_OFFSET: AtomicUsize = AtomicUsize::new(0);
static SYSTEM_TICKS: AtomicU64 = AtomicU64::new(0);
static NEXT_PID: AtomicU64 = AtomicU64::new(1);

// ============================================================================
// CORE DATA STRUCTURES
// ============================================================================

/// CPU Register state for context switching
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Registers {
    // Preserved by callee
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbx: u64,
    pub rbp: u64,
    
    // Scratch registers
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rax: u64,
    
    // Interrupt frame (pushed by CPU or us)
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

impl Registers {
    pub const fn new() -> Self {
        Self {
            r15: 0, r14: 0, r13: 0, r12: 0, rbx: 0, rbp: 0,
            r11: 0, r10: 0, r9: 0, r8: 0,
            rsi: 0, rdi: 0, rdx: 0, rcx: 0, rax: 0,
            rip: 0, cs: 0x08, rflags: 0x202, rsp: 0, ss: 0x10,
        }
    }
}

/// Page table entry (x86_64)
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    pub const PRESENT: u64 = 1 << 0;
    pub const WRITABLE: u64 = 1 << 1;
    pub const USER: u64 = 1 << 2;
    pub const WRITE_THROUGH: u64 = 1 << 3;
    pub const NO_CACHE: u64 = 1 << 4;
    pub const ACCESSED: u64 = 1 << 5;
    pub const DIRTY: u64 = 1 << 6;
    pub const HUGE: u64 = 1 << 7;
    pub const NO_EXECUTE: u64 = 1 << 63;
    
    pub const fn new() -> Self {
        Self(0)
    }
    
    pub fn is_present(&self) -> bool {
        self.0 & Self::PRESENT != 0
    }
    
    pub fn set_present(&mut self, present: bool) {
        if present {
            self.0 |= Self::PRESENT;
        } else {
            self.0 &= !Self::PRESENT;
        }
    }
    
    pub fn physical_address(&self) -> u64 {
        self.0 & 0x000FFFFFFFFFF000
    }
    
    pub fn set_address(&mut self, addr: u64, flags: u64) {
        self.0 = (addr & 0x000FFFFFFFFFF000) | flags;
    }
    
    pub fn flags(&self) -> u64 {
        self.0 & 0xFFF
    }
}

/// Page table (512 entries)
#[repr(C, align(4096))]
pub struct PageTable {
    pub entries: [PageTableEntry; 512],
}

impl PageTable {
    pub const fn new() -> Self {
        Self {
            entries: [PageTableEntry::new(); 512],
        }
    }
}

/// Virtual address components
#[derive(Clone, Copy, Debug)]
pub struct VirtAddr(u64);

impl VirtAddr {
    pub fn new(addr: u64) -> Self {
        Self(addr)
    }
    
    pub fn as_u64(&self) -> u64 {
        self.0
    }
    
    pub fn p4_index(&self) -> usize {
        ((self.0 >> 39) & 0x1FF) as usize
    }
    
    pub fn p3_index(&self) -> usize {
        ((self.0 >> 30) & 0x1FF) as usize
    }
    
    pub fn p2_index(&self) -> usize {
        ((self.0 >> 21) & 0x1FF) as usize
    }
    
    pub fn p1_index(&self) -> usize {
        ((self.0 >> 12) & 0x1FF) as usize
    }
    
    pub fn page_offset(&self) -> usize {
        (self.0 & 0xFFF) as usize
    }
}

/// Physical address
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysAddr(u64);

impl PhysAddr {
    pub fn new(addr: u64) -> Self {
        Self(addr & 0x000FFFFFFFFFF000) // Mask to page boundary
    }
    
    pub fn as_u64(&self) -> u64 {
        self.0
    }
    
    pub fn to_virt(&self) -> VirtAddr {
        let hhdm = HHDM_OFFSET.load(Ordering::Relaxed) as u64;
        VirtAddr::new(self.0 + hhdm)
    }
}

/// Process ID
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Pid(u64);

impl Pid {
    pub fn new() -> Self {
        Self(NEXT_PID.fetch_add(1, Ordering::SeqCst))
    }
    
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// File descriptor
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileDescriptor(usize);

impl FileDescriptor {
    pub fn new(fd: usize) -> Self {
        Self(fd)
    }
    
    pub fn as_usize(&self) -> usize {
        self.0
    }
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/// Align address up to alignment
pub const fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

/// Align address down to alignment
pub const fn align_down(addr: usize, align: usize) -> usize {
    addr & !(align - 1)
}

/// Check if address is aligned
pub const fn is_aligned(addr: usize, align: usize) -> bool {
    addr & (align - 1) == 0
}

/// Get current timestamp
pub fn get_timestamp() -> u64 {
    SYSTEM_TICKS.load(Ordering::Relaxed)
}

/// Increment system timer
pub fn increment_timestamp() {
    SYSTEM_TICKS.fetch_add(1, Ordering::Relaxed);
}
// ============================================================================
// ASTRAL OS - SECTION 2: MEMORY MANAGEMENT
// ============================================================================

// ============================================================================
// PHYSICAL FRAME ALLOCATOR (Bitmap-based)
// ============================================================================

const MAX_FRAMES: usize = 1024 * 1024;
const BITMAP_SIZE: usize = MAX_FRAMES / 8;

pub struct FrameAllocator {
    bitmap: [u8; BITMAP_SIZE],
    total_frames: usize,
    used_frames: usize,
    start_frame: usize,
}

impl FrameAllocator {
    pub const fn new() -> Self {
        Self {
            bitmap: [0; BITMAP_SIZE],
            total_frames: 0,
            used_frames: 0,
            start_frame: 0,
        }
    }
    
    /// Initialize the frame allocator with memory map
    pub fn init(&mut self, memory_map: &limine::response::MemoryMapResponse) {
        self.bitmap.fill(0xFF);
        
        let mut max_addr: u64 = 0;
        
        for entry in memory_map.entries() {
            if entry.entry_type == EntryType::USABLE {
                let start = entry.base as usize / PAGE_SIZE;
                let count = entry.length as usize / PAGE_SIZE;
                
                for frame in start..(start + count) {
                    if frame < MAX_FRAMES {
                        self.free_frame(frame);
                    }
                }
                
                let end = entry.base + entry.length;
                if end > max_addr {
                    max_addr = end;
                }
            }
        }
        
        self.total_frames = (max_addr as usize / PAGE_SIZE).min(MAX_FRAMES);
        
        for frame in 0..(1024 * 1024 / PAGE_SIZE) {
            self.allocate_frame_at(frame);
        }
    }
    
    pub fn allocate(&mut self) -> Option<PhysAddr> {
        for i in self.start_frame..self.total_frames {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            
            if byte_idx >= BITMAP_SIZE {
                break;
            }
            
            if self.bitmap[byte_idx] & (1 << bit_idx) == 0 {
                self.bitmap[byte_idx] |= 1 << bit_idx;
                self.used_frames += 1;
                self.start_frame = i + 1;
                return Some(PhysAddr::new((i * PAGE_SIZE) as u64));
            }
        }
        
        self.start_frame = 0;
        None
    }
    
    pub fn allocate_frame_at(&mut self, frame: usize) {
        if frame >= MAX_FRAMES {
            return;
        }
        
        let byte_idx = frame / 8;
        let bit_idx = frame % 8;
        
        if byte_idx < BITMAP_SIZE {
            if self.bitmap[byte_idx] & (1 << bit_idx) == 0 {
                self.used_frames += 1;
            }
            self.bitmap[byte_idx] |= 1 << bit_idx;
        }
    }
    
    pub fn deallocate(&mut self, addr: PhysAddr) {
        let frame = addr.as_u64() as usize / PAGE_SIZE;
        self.free_frame(frame);
    }
    
    fn free_frame(&mut self, frame: usize) {
        if frame >= MAX_FRAMES {
            return;
        }
        
        let byte_idx = frame / 8;
        let bit_idx = frame % 8;
        
        if byte_idx < BITMAP_SIZE {
            if self.bitmap[byte_idx] & (1 << bit_idx) != 0 {
                self.used_frames = self.used_frames.saturating_sub(1);
            }
            self.bitmap[byte_idx] &= !(1 << bit_idx);
        }
    }
    
    pub fn used_frames(&self) -> usize {
        self.used_frames
    }
    
    pub fn total_frames(&self) -> usize {
        self.total_frames
    }
    
    pub fn free_frames(&self) -> usize {
        self.total_frames.saturating_sub(self.used_frames)
    }
}

static FRAME_ALLOCATOR: Mutex<FrameAllocator> = Mutex::new(FrameAllocator::new());

pub fn allocate_frame() -> Option<PhysAddr> {
    FRAME_ALLOCATOR.lock().allocate()
}

pub fn deallocate_frame(addr: PhysAddr) {
    FRAME_ALLOCATOR.lock().deallocate(addr)
}

// ============================================================================
// PAGE TABLE MANAGER (FIXED - No borrow checker issues)
// ============================================================================

pub struct PageTableManager {
    p4_table: &'static mut PageTable,
}

impl PageTableManager {
    pub unsafe fn current() -> Self {
        let p4_addr = Self::read_cr3();
        let hhdm = HHDM_OFFSET.load(Ordering::Relaxed);
        let p4_ptr = (p4_addr as usize + hhdm) as *mut PageTable;
        
        Self {
            p4_table: &mut *p4_ptr,
        }
    }
    
    pub fn new() -> Option<Self> {
        let frame = allocate_frame()?;
        let hhdm = HHDM_OFFSET.load(Ordering::Relaxed);
        let p4_ptr = (frame.as_u64() as usize + hhdm) as *mut PageTable;
        
        unsafe {
            core::ptr::write_bytes(p4_ptr, 0, 1);
            
            Some(Self {
                p4_table: &mut *p4_ptr,
            })
        }
    }
    
    pub fn p4_physical(&self) -> PhysAddr {
        let hhdm = HHDM_OFFSET.load(Ordering::Relaxed);
        let virt = self.p4_table as *const _ as usize;
        PhysAddr::new((virt - hhdm) as u64)
    }
    
    /// Map a virtual page to a physical frame (FIXED)
    pub fn map(&mut self, virt: VirtAddr, phys: PhysAddr, flags: u64) -> Result<(), &'static str> {
        let p4_index = virt.p4_index();
        let p3_index = virt.p3_index();
        let p2_index = virt.p2_index();
        let p1_index = virt.p1_index();
        
        let hhdm = HHDM_OFFSET.load(Ordering::Relaxed);
        
        // Get or create P3
        let p3_entry = &mut self.p4_table.entries[p4_index];
        let p3_phys = if p3_entry.is_present() {
            p3_entry.physical_address()
        } else {
            let frame = allocate_frame().ok_or("Out of memory")?;
            let ptr = (frame.as_u64() as usize + hhdm) as *mut PageTable;
            unsafe { core::ptr::write_bytes(ptr, 0, 1); }
            p3_entry.set_address(
                frame.as_u64(),
                PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER
            );
            frame.as_u64()
        };
        
        let p3 = unsafe { &mut *((p3_phys as usize + hhdm) as *mut PageTable) };
        
        // Get or create P2
        let p2_entry = &mut p3.entries[p3_index];
        let p2_phys = if p2_entry.is_present() {
            p2_entry.physical_address()
        } else {
            let frame = allocate_frame().ok_or("Out of memory")?;
            let ptr = (frame.as_u64() as usize + hhdm) as *mut PageTable;
            unsafe { core::ptr::write_bytes(ptr, 0, 1); }
            p2_entry.set_address(
                frame.as_u64(),
                PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER
            );
            frame.as_u64()
        };
        
        let p2 = unsafe { &mut *((p2_phys as usize + hhdm) as *mut PageTable) };
        
        // Get or create P1
        let p1_entry = &mut p2.entries[p2_index];
        let p1_phys = if p1_entry.is_present() {
            p1_entry.physical_address()
        } else {
            let frame = allocate_frame().ok_or("Out of memory")?;
            let ptr = (frame.as_u64() as usize + hhdm) as *mut PageTable;
            unsafe { core::ptr::write_bytes(ptr, 0, 1); }
            p1_entry.set_address(
                frame.as_u64(),
                PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER
            );
            frame.as_u64()
        };
        
        let p1 = unsafe { &mut *((p1_phys as usize + hhdm) as *mut PageTable) };
        
        // Map the page
        let entry = &mut p1.entries[p1_index];
        if entry.is_present() {
            return Err("Page already mapped");
        }
        
        entry.set_address(phys.as_u64(), flags);
        
        unsafe {
            asm!("invlpg [{}]", in(reg) virt.as_u64(), options(nostack, preserves_flags));
        }
        
        Ok(())
    }
    
    /// Unmap a virtual page (FIXED)
    pub fn unmap(&mut self, virt: VirtAddr) -> Result<PhysAddr, &'static str> {
        let p4_index = virt.p4_index();
        let p3_index = virt.p3_index();
        let p2_index = virt.p2_index();
        let p1_index = virt.p1_index();
        
        let hhdm = HHDM_OFFSET.load(Ordering::Relaxed);
        
        let p3_entry = &self.p4_table.entries[p4_index];
        if !p3_entry.is_present() {
            return Err("P3 not present");
        }
        let p3 = unsafe { &*((p3_entry.physical_address() as usize + hhdm) as *const PageTable) };
        
        let p2_entry = &p3.entries[p3_index];
        if !p2_entry.is_present() {
            return Err("P2 not present");
        }
        let p2 = unsafe { &*((p2_entry.physical_address() as usize + hhdm) as *const PageTable) };
        
        let p1_entry = &p2.entries[p2_index];
        if !p1_entry.is_present() {
            return Err("P1 not present");
        }
        let p1 = unsafe { &mut *((p1_entry.physical_address() as usize + hhdm) as *mut PageTable) };
        
        let entry = &mut p1.entries[p1_index];
        if !entry.is_present() {
            return Err("Page not mapped");
        }
        
        let phys = PhysAddr::new(entry.physical_address());
        entry.set_present(false);
        
        unsafe {
            asm!("invlpg [{}]", in(reg) virt.as_u64(), options(nostack, preserves_flags));
        }
        
        Ok(phys)
    }
    
    pub fn translate(&self, virt: VirtAddr) -> Option<PhysAddr> {
        let hhdm = HHDM_OFFSET.load(Ordering::Relaxed);
        
        let p3_entry = &self.p4_table.entries[virt.p4_index()];
        if !p3_entry.is_present() {
            return None;
        }
        let p3 = unsafe { &*((p3_entry.physical_address() as usize + hhdm) as *const PageTable) };
        
        let p2_entry = &p3.entries[virt.p3_index()];
        if !p2_entry.is_present() {
            return None;
        }
        let p2 = unsafe { &*((p2_entry.physical_address() as usize + hhdm) as *const PageTable) };
        
        let p1_entry = &p2.entries[virt.p2_index()];
        if !p1_entry.is_present() {
            return None;
        }
        let p1 = unsafe { &*((p1_entry.physical_address() as usize + hhdm) as *const PageTable) };
        
        let entry = &p1.entries[virt.p1_index()];
        
        if entry.is_present() {
            Some(PhysAddr::new(entry.physical_address() + virt.page_offset() as u64))
        } else {
            None
        }
    }
    
    unsafe fn read_cr3() -> u64 {
        let cr3: u64;
        asm!("mov {}, cr3", out(reg) cr3, options(nostack, preserves_flags));
        cr3
    }
    
    pub unsafe fn load(&self) {
        let phys = self.p4_physical().as_u64();
        asm!("mov cr3, {}", in(reg) phys, options(nostack, preserves_flags));
    }
}

// ============================================================================
// KERNEL HEAP ALLOCATOR
// ============================================================================

const HEAP_START: usize = 0xFFFF_8000_0000_0000;
const HEAP_SIZE: usize = KERNEL_HEAP_SIZE;

struct HeapAllocator {
    next_addr: AtomicUsize,
    end_addr: usize,
}

impl HeapAllocator {
    const fn new() -> Self {
        Self {
            next_addr: AtomicUsize::new(HEAP_START),
            end_addr: HEAP_START + HEAP_SIZE,
        }
    }
    
    unsafe fn init(&self) {
        let mut pt = PageTableManager::current();
        
        for offset in (0..HEAP_SIZE).step_by(PAGE_SIZE) {
            let virt = VirtAddr::new((HEAP_START + offset) as u64);
            
            if let Some(frame) = allocate_frame() {
                let _ = pt.map(
                    virt,
                    frame,
                    PageTableEntry::PRESENT | PageTableEntry::WRITABLE
                );
            }
        }
    }
}

unsafe impl GlobalAlloc for HeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = align_up(layout.size(), layout.align());
        let addr = self.next_addr.fetch_add(size, Ordering::SeqCst);
        
        if addr + size > self.end_addr {
            return core::ptr::null_mut();
        }
        
        addr as *mut u8
    }
    
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Simple bump allocator - no deallocation
    }
}

#[global_allocator]
static HEAP_ALLOCATOR: HeapAllocator = HeapAllocator::new();

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("Allocation error: {:?}", layout);
}

// ============================================================================
// INITIALIZATION
// ============================================================================

pub fn init_memory(memory_map: &limine::response::MemoryMapResponse) {
    if let Some(hhdm) = HHDM_REQUEST.get_response() {
        HHDM_OFFSET.store(hhdm.offset() as usize, Ordering::SeqCst);
    }
    
    FRAME_ALLOCATOR.lock().init(memory_map);
    
    unsafe {
        HEAP_ALLOCATOR.init();
    }
    
    println!("Memory initialized:");
    let fa = FRAME_ALLOCATOR.lock();
    println!("  Frames: {} total, {} used, {} free", 
        fa.total_frames(), fa.used_frames(), fa.free_frames());
    println!("  Heap: 0x{:x} - 0x{:x} ({} MB)",
        HEAP_START, HEAP_START + HEAP_SIZE, HEAP_SIZE / 1024 / 1024);
}


// ============================================================================
// ASTRAL OS - SECTION 3: PROCESS MANAGEMENT (FIXED)
// ============================================================================

// ============================================================================
// PROCESS CONTROL BLOCK
// ============================================================================

#[derive(Clone, Copy)]
pub struct Process {
    pub pid: Pid,
    pub state: u8,
    pub registers: Registers,
    pub page_table: u64,
    pub kernel_stack: u64,
    pub user_stack: u64,
    pub priority: u8,
    pub time_slice: u64,
    pub total_time: u64,
    pub parent_pid: Option<Pid>,
    pub exit_code: i32,
}

impl Process {
    pub fn new(pid: Pid) -> Self {
        Self {
            pid,
            state: PROCESS_READY,
            registers: Registers::new(),
            page_table: 0,
            kernel_stack: 0,
            user_stack: 0,
            priority: 10,
            time_slice: 10,
            total_time: 0,
            parent_pid: None,
            exit_code: 0,
        }
    }
    
    pub fn is_ready(&self) -> bool {
        self.state == PROCESS_READY
    }
    
    pub fn is_running(&self) -> bool {
        self.state == PROCESS_RUNNING
    }
    
    pub fn is_blocked(&self) -> bool {
        self.state == PROCESS_BLOCKED
    }
    
    pub fn is_zombie(&self) -> bool {
        self.state == PROCESS_ZOMBIE
    }
}

// ============================================================================
// PROCESS TABLE
// ============================================================================

pub struct ProcessTable {
    processes: [Option<Process>; MAX_PROCESSES],
    count: usize,
}

impl ProcessTable {
    pub const fn new() -> Self {
        Self {
            processes: [None; MAX_PROCESSES],
            count: 0,
        }
    }
    
    pub fn add(&mut self, process: Process) -> Result<(), &'static str> {
        for slot in &mut self.processes {
            if slot.is_none() {
                *slot = Some(process);
                self.count += 1;
                return Ok(());
            }
        }
        Err("Process table full")
    }
    
    pub fn remove(&mut self, pid: Pid) -> Option<Process> {
        for slot in &mut self.processes {
            if let Some(proc) = slot {
                if proc.pid == pid {
                    let removed = *proc;
                    *slot = None;
                    self.count = self.count.saturating_sub(1);
                    return Some(removed);
                }
            }
        }
        None
    }
    
    pub fn get(&self, pid: Pid) -> Option<&Process> {
        self.processes.iter()
            .flatten()
            .find(|p| p.pid == pid)
    }
    
    pub fn get_mut(&mut self, pid: Pid) -> Option<&mut Process> {
        self.processes.iter_mut()
            .flatten()
            .find(|p| p.pid == pid)
    }
    
    pub fn iter(&self) -> impl Iterator<Item = &Process> {
        self.processes.iter().flatten()
    }
    
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Process> {
        self.processes.iter_mut().flatten()
    }
    
    pub fn count(&self) -> usize {
        self.count
    }
}

static PROCESS_TABLE: Mutex<ProcessTable> = Mutex::new(ProcessTable::new());
static CURRENT_PID: AtomicU64 = AtomicU64::new(0);

pub fn get_current_pid() -> Option<Pid> {
    let pid_val = CURRENT_PID.load(Ordering::Relaxed);
    if pid_val == 0 {
        None
    } else {
        Some(Pid(pid_val))
    }
}

pub fn set_current_pid(pid: Pid) {
    CURRENT_PID.store(pid.as_u64(), Ordering::Relaxed);
}

// ============================================================================
// SCHEDULER
// ============================================================================

pub struct Scheduler {
    ready_queue: VecDeque<Pid>,
    current_time_slice: u64,
}

impl Scheduler {
    pub const fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
            current_time_slice: 0,
        }
    }
    
    pub fn add_process(&mut self, pid: Pid) {
        if !self.ready_queue.contains(&pid) {
            self.ready_queue.push_back(pid);
        }
    }
    
    pub fn remove_process(&mut self, pid: Pid) {
        self.ready_queue.retain(|&p| p != pid);
    }
    
    pub fn schedule(&mut self) -> Option<Pid> {
        if self.current_time_slice > 0 {
            self.current_time_slice -= 1;
            
            if self.current_time_slice > 0 {
                if let Some(current) = get_current_pid() {
                    let table = PROCESS_TABLE.lock();
                    if let Some(proc) = table.get(current) {
                        if proc.is_running() {
                            return Some(current);
                        }
                    }
                }
            }
        }
        
        if let Some(next_pid) = self.ready_queue.pop_front() {
            let table = PROCESS_TABLE.lock();
            if let Some(proc) = table.get(next_pid) {
                if proc.is_ready() {
                    self.current_time_slice = proc.time_slice;
                    self.ready_queue.push_back(next_pid);
                    return Some(next_pid);
                }
            }
        }
        
        None
    }
    
    pub fn yield_current(&mut self) {
        if let Some(current) = get_current_pid() {
            self.ready_queue.retain(|&p| p != current);
            self.ready_queue.push_back(current);
            self.current_time_slice = 0;
        }
    }
}

static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());

pub fn schedule() -> Option<Pid> {
    SCHEDULER.lock().schedule()
}

pub fn add_to_scheduler(pid: Pid) {
    SCHEDULER.lock().add_process(pid);
}

pub fn remove_from_scheduler(pid: Pid) {
    SCHEDULER.lock().remove_process(pid);
}

pub fn yield_cpu() {
    SCHEDULER.lock().yield_current();
}

// ============================================================================
// CONTEXT SWITCHING
// ============================================================================

#[unsafe(naked)]
pub unsafe extern "C" fn context_switch(old_regs: *mut Registers, new_regs: *const Registers) {
    core::arch::naked_asm!(
        "mov [rdi + 0x00], r15",
        "mov [rdi + 0x08], r14",
        "mov [rdi + 0x10], r13",
        "mov [rdi + 0x18], r12",
        "mov [rdi + 0x20], rbx",
        "mov [rdi + 0x28], rbp",
        "mov [rdi + 0x30], r11",
        "mov [rdi + 0x38], r10",
        "mov [rdi + 0x40], r9",
        "mov [rdi + 0x48], r8",
        "mov [rdi + 0x50], rsi",
        "mov [rdi + 0x60], rdx",
        "mov [rdi + 0x68], rcx",
        "mov [rdi + 0x70], rax",
        "mov rax, [rsp]",
        "mov [rdi + 0x78], rax",
        "lea rax, [rsp + 8]",
        "mov [rdi + 0x88], rax",
        "pushfq",
        "pop rax",
        "mov [rdi + 0x80], rax",
        "mov [rdi + 0x58], rdi",
        "mov r15, [rsi + 0x00]",
        "mov r14, [rsi + 0x08]",
        "mov r13, [rsi + 0x10]",
        "mov r12, [rsi + 0x18]",
        "mov rbx, [rsi + 0x20]",
        "mov rbp, [rsi + 0x28]",
        "mov r11, [rsi + 0x30]",
        "mov r10, [rsi + 0x38]",
        "mov r9,  [rsi + 0x40]",
        "mov r8,  [rsi + 0x48]",
        "mov rdi, [rsi + 0x58]",
        "mov rdx, [rsi + 0x60]",
        "mov rcx, [rsi + 0x68]",
        "mov rax, [rsi + 0x70]",
        "mov rsp, [rsi + 0x88]",
        "push qword ptr [rsi + 0x78]",
        "push qword ptr [rsi + 0x80]",
        "popfq",
        "mov rsi, [rsi + 0x50]",
        "ret",
    )
}

/// Perform context switch between processes (FIXED)
pub fn switch_to_process(new_pid: Pid) {
    let current_pid = get_current_pid();
    
    if let Some(curr) = current_pid {
        if curr == new_pid {
            return;
        }
    }
    
    // Get data we need while holding lock, then release
    let (old_regs_ptr, new_regs_ptr, new_page_table) = {
        let mut table = PROCESS_TABLE.lock();
        
        // Update old process state and get its register pointer
        let old_ptr = current_pid
            .and_then(|pid| table.get_mut(pid))
            .map(|proc| {
                proc.state = PROCESS_READY;
                &mut proc.registers as *mut Registers
            });
        
        // Update new process state and get its info
        let new_proc = table.get_mut(new_pid).expect("Process not found");
        new_proc.state = PROCESS_RUNNING;
        
        let new_ptr = &new_proc.registers as *const Registers;
        let new_pt = new_proc.page_table;
        
        (old_ptr, new_ptr, new_pt)
    }; // Lock released here
    
    // Load new page table
    unsafe {
        asm!("mov cr3, {}", in(reg) new_page_table, options(nostack, preserves_flags));
    }
    
    // Update current PID
    set_current_pid(new_pid);
    
    // Perform context switch
    if let Some(old_ptr) = old_regs_ptr {
        unsafe {
            context_switch(old_ptr, new_regs_ptr);
        }
    } else {
        // First process
        unsafe {
            let regs = &*new_regs_ptr;
            asm!(
                "mov rsp, {rsp}",
                "push {ss}",
                "push {rsp}",
                "push {rflags}",
                "push {cs}",
                "push {rip}",
                "mov rax, {rax}",
                "iretq",
                rsp = in(reg) regs.rsp,
                ss = in(reg) regs.ss,
                rflags = in(reg) regs.rflags,
                cs = in(reg) regs.cs,
                rip = in(reg) regs.rip,
                rax = in(reg) regs.rax,
                options(noreturn)
            );
        }
    }
}

// ============================================================================
// PROCESS CREATION
// ============================================================================

pub fn create_kernel_process(entry: extern "C" fn()) -> Result<Pid, &'static str> {
    let pid = Pid::new();
    let mut process = Process::new(pid);
    
    let stack_frame = allocate_frame().ok_or("Out of memory")?;
    process.kernel_stack = stack_frame.as_u64() + PAGE_SIZE as u64;
    
    let current_pt = unsafe { PageTableManager::current() };
    process.page_table = current_pt.p4_physical().as_u64();
    
    process.registers.rip = entry as u64;
    process.registers.rsp = process.kernel_stack;
    process.registers.cs = 0x08;
    process.registers.ss = 0x10;
    process.registers.rflags = 0x202;
    
    PROCESS_TABLE.lock().add(process)?;
    add_to_scheduler(pid);
    
    Ok(pid)
}

pub fn create_user_process(entry: u64) -> Result<Pid, &'static str> {
    let pid = Pid::new();
    let mut process = Process::new(pid);
    
    let mut pt = PageTableManager::new().ok_or("Failed to create page table")?;
    
    let stack_pages = USER_STACK_SIZE / PAGE_SIZE;
    let stack_top = USER_STACK_TOP;
    
    for i in 0..stack_pages {
        let virt = VirtAddr::new(stack_top - (i * PAGE_SIZE) as u64);
        let frame = allocate_frame().ok_or("Out of memory")?;
        pt.map(
            virt,
            frame,
            PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER
        )?;
    }
    
    process.page_table = pt.p4_physical().as_u64();
    process.user_stack = stack_top;
    
    let kstack_frame = allocate_frame().ok_or("Out of memory")?;
    process.kernel_stack = kstack_frame.as_u64() + PAGE_SIZE as u64;
    
    process.registers.rip = entry;
    process.registers.rsp = stack_top;
    process.registers.cs = 0x1B;
    process.registers.ss = 0x23;
    process.registers.rflags = 0x202;
    
    PROCESS_TABLE.lock().add(process)?;
    add_to_scheduler(pid);
    
    Ok(pid)
}

pub fn exit_process(exit_code: i32) {
    if let Some(current) = get_current_pid() {
        let mut table = PROCESS_TABLE.lock();
        
        if let Some(proc) = table.get_mut(current) {
            proc.state = PROCESS_ZOMBIE;
            proc.exit_code = exit_code;
            
            remove_from_scheduler(current);
        }
        
        drop(table);
        yield_cpu();
    }
}
// ============================================================================
// ASTRAL OS - SECTION 4: INTERRUPTS & SYSCALLS
// ============================================================================

// ============================================================================
// IDT (Interrupt Descriptor Table)
// ============================================================================

#[derive(Copy, Clone)]
#[repr(C, packed)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    flags: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    const fn new() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            ist: 0,
            flags: 0,
            offset_mid: 0,
            offset_high: 0,
            reserved: 0,
        }
    }
    
    fn set_handler(&mut self, handler: u64, ist: u8) {
        self.offset_low = handler as u16;
        self.offset_mid = (handler >> 16) as u16;
        self.offset_high = (handler >> 32) as u32;
        self.selector = 0x08; // Kernel code segment
        self.flags = 0x8E;    // Present, ring 0, interrupt gate
        self.ist = ist;
    }
    
    fn set_user_handler(&mut self, handler: u64, ist: u8) {
        self.set_handler(handler, ist);
        self.flags = 0xEE; // Present, ring 3, interrupt gate
    }
}

#[repr(C, packed)]
struct IdtDescriptor {
    limit: u16,
    base: u64,
}

const IDT_SIZE: usize = 256;
static mut IDT: [IdtEntry; IDT_SIZE] = [IdtEntry::new(); IDT_SIZE];

pub fn init_idt() {
    unsafe {
        // CPU Exceptions (0-31)
        IDT[0].set_handler(divide_error_handler as u64, 0);
        IDT[1].set_handler(debug_handler as u64, 0);
        IDT[2].set_handler(nmi_handler as u64, 0);
        IDT[3].set_handler(breakpoint_handler as u64, 0);
        IDT[4].set_handler(overflow_handler as u64, 0);
        IDT[5].set_handler(bound_range_handler as u64, 0);
        IDT[6].set_handler(invalid_opcode_handler as u64, 0);
        IDT[7].set_handler(device_not_available_handler as u64, 0);
        IDT[8].set_handler(double_fault_handler as u64, 1);
        IDT[10].set_handler(invalid_tss_handler as u64, 0);
        IDT[11].set_handler(segment_not_present_handler as u64, 0);
        IDT[12].set_handler(stack_segment_fault_handler as u64, 0);
        IDT[13].set_handler(general_protection_fault_handler as u64, 0);
        IDT[14].set_handler(page_fault_handler as u64, 0);
        IDT[16].set_handler(x87_fpu_error_handler as u64, 0);
        IDT[17].set_handler(alignment_check_handler as u64, 0);
        IDT[18].set_handler(machine_check_handler as u64, 0);
        IDT[19].set_handler(simd_exception_handler as u64, 0);
        IDT[20].set_handler(virtualization_exception_handler as u64, 0);
        
        // Hardware interrupts (32-47)
        IDT[32].set_handler(timer_interrupt_handler as u64, 0);
        IDT[33].set_handler(keyboard_interrupt_handler as u64, 0);
        
        // System call (0x80)
        IDT[0x80].set_user_handler(syscall_handler as u64, 0);
        
        let descriptor = IdtDescriptor {
            limit: (core::mem::size_of::<[IdtEntry; IDT_SIZE]>() - 1) as u16,
            base: &raw const IDT as u64,
        };
        
        asm!("lidt [{}]", in(reg) &descriptor, options(nostack));
    }
}

// ============================================================================
// PIC (Programmable Interrupt Controller)
// ============================================================================

const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_COMMAND: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val, options(nostack, nomem));
}

unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    asm!("in al, dx", out("al") val, in("dx") port, options(nostack, nomem));
    val
}

unsafe fn io_wait() {
    asm!("out 0x80, al", in("al") 0u8, options(nostack, nomem));
}

pub fn init_pic() {
    unsafe {
        // Start initialization
        outb(PIC1_COMMAND, 0x11);
        io_wait();
        outb(PIC2_COMMAND, 0x11);
        io_wait();
        
        // Set offsets
        outb(PIC1_DATA, 32);  // IRQ 0-7 → INT 32-39
        io_wait();
        outb(PIC2_DATA, 40);  // IRQ 8-15 → INT 40-47
        io_wait();
        
        // Set cascade
        outb(PIC1_DATA, 0x04);
        io_wait();
        outb(PIC2_DATA, 0x02);
        io_wait();
        
        // Set mode
        outb(PIC1_DATA, 0x01);
        io_wait();
        outb(PIC2_DATA, 0x01);
        io_wait();
        
        // Unmask all interrupts
        outb(PIC1_DATA, 0x00);
        io_wait();
        outb(PIC2_DATA, 0x00);
        io_wait();
    }
}

unsafe fn pic_send_eoi(irq: u8) {
    if irq >= 8 {
        outb(PIC2_COMMAND, 0x20);
    }
    outb(PIC1_COMMAND, 0x20);
}

// ============================================================================
// EXCEPTION HANDLERS
// ============================================================================

#[repr(C)]
struct InterruptStackFrame {
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

#[no_mangle]
extern "x86-interrupt" fn divide_error_handler(_frame: InterruptStackFrame) {
    panic!("EXCEPTION: Divide by Zero");
}

#[no_mangle]
extern "x86-interrupt" fn debug_handler(_frame: InterruptStackFrame) {
    println!("DEBUG: Debug exception");
}

#[no_mangle]
extern "x86-interrupt" fn nmi_handler(_frame: InterruptStackFrame) {
    panic!("EXCEPTION: Non-Maskable Interrupt");
}

#[no_mangle]
extern "x86-interrupt" fn breakpoint_handler(_frame: InterruptStackFrame) {
    println!("DEBUG: Breakpoint");
}

#[no_mangle]
extern "x86-interrupt" fn overflow_handler(_frame: InterruptStackFrame) {
    panic!("EXCEPTION: Overflow");
}

#[no_mangle]
extern "x86-interrupt" fn bound_range_handler(_frame: InterruptStackFrame) {
    panic!("EXCEPTION: Bound Range Exceeded");
}

#[no_mangle]
extern "x86-interrupt" fn invalid_opcode_handler(_frame: InterruptStackFrame) {
    panic!("EXCEPTION: Invalid Opcode");
}

#[no_mangle]
extern "x86-interrupt" fn device_not_available_handler(_frame: InterruptStackFrame) {
    panic!("EXCEPTION: Device Not Available");
}

#[no_mangle]
extern "x86-interrupt" fn double_fault_handler(_frame: InterruptStackFrame, _error_code: u64) -> ! {
    panic!("EXCEPTION: Double Fault");
}

#[no_mangle]
extern "x86-interrupt" fn invalid_tss_handler(_frame: InterruptStackFrame, error_code: u64) {
    panic!("EXCEPTION: Invalid TSS (error: 0x{:x})", error_code);
}

#[no_mangle]
extern "x86-interrupt" fn segment_not_present_handler(_frame: InterruptStackFrame, error_code: u64) {
    panic!("EXCEPTION: Segment Not Present (error: 0x{:x})", error_code);
}

#[no_mangle]
extern "x86-interrupt" fn stack_segment_fault_handler(_frame: InterruptStackFrame, error_code: u64) {
    panic!("EXCEPTION: Stack Segment Fault (error: 0x{:x})", error_code);
}

#[no_mangle]
extern "x86-interrupt" fn general_protection_fault_handler(_frame: InterruptStackFrame, error_code: u64) {
    panic!("EXCEPTION: General Protection Fault (error: 0x{:x})", error_code);
}

#[no_mangle]
extern "x86-interrupt" fn page_fault_handler(frame: InterruptStackFrame, error_code: u64) {
    let cr2: u64;
    unsafe {
        asm!("mov {}, cr2", out(reg) cr2, options(nostack, nomem));
    }
    
    panic!(
        "EXCEPTION: Page Fault\n  Address: 0x{:x}\n  Error: 0x{:x}\n  RIP: 0x{:x}",
        cr2, error_code, frame.rip
    );
}

#[no_mangle]
extern "x86-interrupt" fn x87_fpu_error_handler(_frame: InterruptStackFrame) {
    panic!("EXCEPTION: x87 FPU Error");
}

#[no_mangle]
extern "x86-interrupt" fn alignment_check_handler(_frame: InterruptStackFrame, error_code: u64) {
    panic!("EXCEPTION: Alignment Check (error: 0x{:x})", error_code);
}

#[no_mangle]
extern "x86-interrupt" fn machine_check_handler(_frame: InterruptStackFrame) -> ! {
    panic!("EXCEPTION: Machine Check");
}

#[no_mangle]
extern "x86-interrupt" fn simd_exception_handler(_frame: InterruptStackFrame) {
    panic!("EXCEPTION: SIMD Floating-Point Exception");
}

#[no_mangle]
extern "x86-interrupt" fn virtualization_exception_handler(_frame: InterruptStackFrame) {
    panic!("EXCEPTION: Virtualization Exception");
}

// ============================================================================
// HARDWARE INTERRUPT HANDLERS
// ============================================================================

#[unsafe(naked)]
unsafe extern "C" fn timer_interrupt_handler() {
    core::arch::naked_asm!(
        // Save registers
        "push rax",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        
        // Call inner handler
        "call {inner}",
        
        // Restore registers
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rax",
        
        "iretq",
        inner = sym timer_irq_inner
    );
}

#[no_mangle]
extern "C" fn timer_irq_inner() {
    increment_timestamp();
    
    unsafe {
        pic_send_eoi(0);
    }
    
    // Trigger scheduler
    if let Some(next_pid) = schedule() {
        switch_to_process(next_pid);
    }
}

#[unsafe(naked)]
unsafe extern "C" fn keyboard_interrupt_handler() {
    core::arch::naked_asm!(
        "push rax",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "call {inner}",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rax",
        "iretq",
        inner = sym keyboard_irq_inner
    );
}

// Keyboard buffer
const KB_BUFFER_SIZE: usize = 256;
static mut KB_BUFFER: [u8; KB_BUFFER_SIZE] = [0; KB_BUFFER_SIZE];
static KB_WRITE_POS: AtomicUsize = AtomicUsize::new(0);
static KB_READ_POS: AtomicUsize = AtomicUsize::new(0);

static SHIFT_PRESSED: AtomicBool = AtomicBool::new(false);
static CTRL_PRESSED: AtomicBool = AtomicBool::new(false);

// US keyboard scancode map
static SCANCODE_TO_ASCII: [u8; 128] = [
    0, 27, b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0', b'-', b'=', 8,
    b'\t', b'q', b'w', b'e', b'r', b't', b'y', b'u', b'i', b'o', b'p', b'[', b']', b'\n',
    0, b'a', b's', b'd', b'f', b'g', b'h', b'j', b'k', b'l', b';', b'\'', b'`',
    0, b'\\', b'z', b'x', b'c', b'v', b'b', b'n', b'm', b',', b'.', b'/', 0,
    b'*', 0, b' ', 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0,
    b'7', b'8', b'9', b'-', b'4', b'5', b'6', b'+', b'1', b'2', b'3', b'0', b'.',
    0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0,
];

static SCANCODE_TO_ASCII_SHIFT: [u8; 128] = [
    0, 27, b'!', b'@', b'#', b'$', b'%', b'^', b'&', b'*', b'(', b')', b'_', b'+', 8,
    b'\t', b'Q', b'W', b'E', b'R', b'T', b'Y', b'U', b'I', b'O', b'P', b'{', b'}', b'\n',
    0, b'A', b'S', b'D', b'F', b'G', b'H', b'J', b'K', b'L', b':', b'"', b'~',
    0, b'|', b'Z', b'X', b'C', b'V', b'B', b'N', b'M', b'<', b'>', b'?', 0,
    b'*', 0, b' ', 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0,
    b'7', b'8', b'9', b'-', b'4', b'5', b'6', b'+', b'1', b'2', b'3', b'0', b'.',
    0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0,
];

#[no_mangle]
extern "C" fn keyboard_irq_inner() {
    unsafe {
        let scancode = inb(0x60);
        
        // Handle key release
        if scancode & 0x80 != 0 {
            let released = scancode & 0x7F;
            match released {
                0x2A | 0x36 => SHIFT_PRESSED.store(false, Ordering::Relaxed),
                0x1D => CTRL_PRESSED.store(false, Ordering::Relaxed),
                _ => {}
            }
        } else {
            // Handle key press
            match scancode {
                0x2A | 0x36 => SHIFT_PRESSED.store(true, Ordering::Relaxed),
                0x1D => CTRL_PRESSED.store(true, Ordering::Relaxed),
                _ => {
                    if (scancode as usize) < 128 {
                        let ascii = if SHIFT_PRESSED.load(Ordering::Relaxed) {
                            SCANCODE_TO_ASCII_SHIFT[scancode as usize]
                        } else {
                            SCANCODE_TO_ASCII[scancode as usize]
                        };
                        
                        if ascii != 0 {
                            let write = KB_WRITE_POS.load(Ordering::Relaxed);
                            let next = (write + 1) % KB_BUFFER_SIZE;
                            
                            if next != KB_READ_POS.load(Ordering::Relaxed) {
                                KB_BUFFER[write] = ascii;
                                KB_WRITE_POS.store(next, Ordering::Release);
                            }
                        }
                    }
                }
            }
        }
        
        pic_send_eoi(1);
    }
}

pub fn getchar() -> Option<u8> {
    let read = KB_READ_POS.load(Ordering::Relaxed);
    let write = KB_WRITE_POS.load(Ordering::Acquire);
    
    if read == write {
        return None;
    }
    
    let c = unsafe { KB_BUFFER[read] };
    KB_READ_POS.store((read + 1) % KB_BUFFER_SIZE, Ordering::Release);
    Some(c)
}

pub fn getchar_blocking() -> u8 {
    loop {
        if let Some(c) = getchar() {
            return c;
        }
        unsafe { asm!("hlt"); }
    }
}

// ============================================================================
// SYSTEM CALL INTERFACE
// ============================================================================

#[unsafe(naked)]
unsafe extern "C" fn syscall_handler() {
    core::arch::naked_asm!(
        // Save user context
        "push rax",  // Syscall number
        "push rbx",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push rbp",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        
        // Call handler
        "mov rdi, rax",  // Syscall number
        "mov rsi, rbx",  // Arg 1
        "mov rdx, rcx",  // Arg 2
        "mov rcx, rdx",  // Arg 3
        "call {handler}",
        
        // Restore context
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rbp",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rbx",
        "add rsp, 8",  // Skip saved rax
        
        "iretq",
        handler = sym syscall_handler_inner
    );
}

#[no_mangle]
extern "C" fn syscall_handler_inner(
    syscall: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
) -> u64 {
    match syscall {
        SYS_EXIT => {
            exit_process(arg1 as i32);
            0
        }
        SYS_READ => {
            sys_read(arg1 as usize, arg2 as *mut u8, arg3 as usize)
        }
        SYS_WRITE => {
            sys_write(arg1 as usize, arg2 as *const u8, arg3 as usize)
        }
        SYS_GETPID => {
            get_current_pid().map(|p| p.as_u64()).unwrap_or(0)
        }
        SYS_YIELD => {
            yield_cpu();
            0
        }
        _ => {
            println!("Unknown syscall: {}", syscall);
            !0u64 // Return -1
        }
    }
}

// Syscall implementations
fn sys_read(_fd: usize, _buf: *mut u8, _count: usize) -> u64 {
    // TODO: Implement file reading
    0
}

fn sys_write(_fd: usize, buf: *const u8, count: usize) -> u64 {
    // For now, just write to console
    unsafe {
        let slice = core::slice::from_raw_parts(buf, count);
        if let Ok(s) = core::str::from_utf8(slice) {
            print!("{}", s);
            count as u64
        } else {
            !0u64
        }
    }
}

// ============================================================================
// GDT & TSS (for proper ring transitions)
// ============================================================================

#[repr(C, packed)]
struct Tss {
    reserved1: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    reserved2: u64,
    ist1: u64,
    ist2: u64,
    ist3: u64,
    ist4: u64,
    ist5: u64,
    ist6: u64,
    ist7: u64,
    reserved3: u64,
    reserved4: u16,
    iomap_base: u16,
}

static mut TSS: Tss = Tss {
    reserved1: 0, rsp0: 0, rsp1: 0, rsp2: 0, reserved2: 0,
    ist1: 0, ist2: 0, ist3: 0, ist4: 0, ist5: 0, ist6: 0, ist7: 0,
    reserved3: 0, reserved4: 0, iomap_base: 0,
};

const DOUBLE_FAULT_STACK_SIZE: usize = 4096 * 5;
static mut DOUBLE_FAULT_STACK: [u8; DOUBLE_FAULT_STACK_SIZE] = [0; DOUBLE_FAULT_STACK_SIZE];

#[repr(C, align(16))]
struct Gdt {
    null: u64,
    code: u64,
    data: u64,
    user_code: u64,
    user_data: u64,
    tss_low: u64,
    tss_high: u64,
}

static mut KERNEL_GDT: Gdt = Gdt {
    null: 0,
    code: 0x00AF9A000000FFFF,      // Kernel code
    data: 0x00CF92000000FFFF,      // Kernel data
    user_code: 0x00AFFA000000FFFF, // User code
    user_data: 0x00CFF2000000FFFF, // User data
    tss_low: 0,
    tss_high: 0,
};

#[repr(C, packed)]
struct GdtDescriptor {
    limit: u16,
    base: u64,
}

pub fn init_gdt_and_tss() {
    unsafe {
        TSS.ist1 = (&raw const DOUBLE_FAULT_STACK as *const _ as u64) + DOUBLE_FAULT_STACK_SIZE as u64;
        
        let tss_addr = &raw const TSS as *const _ as u64;
        let tss_limit = core::mem::size_of::<Tss>() - 1;
        
        KERNEL_GDT.tss_low = (tss_limit as u64 & 0xFFFF)
            | ((tss_addr & 0xFFFF) << 16)
            | (((tss_addr >> 16) & 0xFF) << 32)
            | (0x89u64 << 40)
            | ((tss_addr >> 24) << 56);
        KERNEL_GDT.tss_high = tss_addr >> 32;
        
        let gdt_desc = GdtDescriptor {
            limit: (core::mem::size_of::<Gdt>() - 1) as u16,
            base: &raw const KERNEL_GDT as *const _ as u64,
        };
        
        asm!("lgdt [{}]", in(reg) &gdt_desc, options(nostack));
        
        // Reload segment registers
        asm!(
            "push 0x08",
            "lea rax, [rip + 2f]",
            "push rax",
            "retfq",
            "2:",
            "mov ax, 0x10",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            "mov ss, ax",
            out("rax") _,
            options(nostack)
        );
        
        // Load TSS
        asm!("ltr ax", in("ax") 0x28u16, options(nostack, nomem));
    }
}
// ============================================================================
// ASTRAL OS - SECTION 5: DEVICE DRIVERS
// ============================================================================

// ============================================================================
// FRAMEBUFFER CONSOLE
// ============================================================================

const FONT_HEIGHT: usize = 16;
const FONT_WIDTH: usize = 8;

// Load your existing font.bin
static FONT_DATA: &[u8] = include_bytes!("../font.bin");
unsafe impl Send for Framebuffer {}
unsafe impl Sync for Framebuffer {}
/// Reality-aware framebuffer state
pub struct Framebuffer {
    addr: *mut u8,
    width: usize,
    height: usize,
    pitch: usize,
    bpp: u16,
    x: usize,
    y: usize,
    
    // Reality-aware features
    current_fg_color: u32,
    current_bg_color: u32,
    cursor_visible: bool,
    cursor_blink_state: bool,
    
    // Performance tracking
    frames_rendered: u64,
    chars_drawn: u64,
}

impl Framebuffer {
    pub fn new(addr: *mut u8, width: usize, height: usize, pitch: usize, bpp: u16) -> Self {
        Self {
            addr,
            width,
            height,
            pitch,
            bpp,
            x: 0,
            y: 0,
            current_fg_color: 0xFFFFFF,
            current_bg_color: 0x000000,
            cursor_visible: true,
            cursor_blink_state: false,
            frames_rendered: 0,
            chars_drawn: 0,
        }
    }
    
    /// Clear screen with optional reality fade effect
    pub fn clear(&mut self) {
        self.clear_with_color(self.current_bg_color);
        self.x = 0;
        self.y = 0;
    }
    
    pub fn clear_with_color(&mut self, color: u32) {
        unsafe {
            let bytes_per_pixel = (self.bpp / 8) as usize;
            for y in 0..self.height {
                let row_offset = y * self.pitch;
                for x in 0..self.width {
                    let offset = row_offset + x * bytes_per_pixel;
                    let pixel = self.addr.add(offset) as *mut u32;
                    write_volatile(pixel, color);
                }
            }
        }
    }
    
    /// Put pixel with bounds checking
    #[inline]
    fn put_pixel(&self, x: usize, y: usize, color: u32) {
        if x >= self.width || y >= self.height {
            return;
        }
        
        unsafe {
            let bytes_per_pixel = (self.bpp / 8) as usize;
            let offset = y * self.pitch + x * bytes_per_pixel;
            
            // Bounds check for framebuffer memory
            let max_offset = self.height * self.pitch;
            if offset + bytes_per_pixel > max_offset {
                return;
            }
            
            let pixel = self.addr.add(offset) as *mut u32;
            write_volatile(pixel, color);
        }
    }
    
    /// Get pixel color (for advanced effects)
    #[inline]
    fn get_pixel(&self, x: usize, y: usize) -> u32 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        
        unsafe {
            let bytes_per_pixel = (self.bpp / 8) as usize;
            let offset = y * self.pitch + x * bytes_per_pixel;
            let pixel = self.addr.add(offset) as *mut u32;
            read_volatile(pixel)
        }
    }
    
    /// Draw character with your font.bin
    pub fn draw_char(&mut self, c: u8, fg: u32, bg: u32) {
        match c {
            b'\n' => {
                self.x = 0;
                self.y += FONT_HEIGHT;
                if self.y + FONT_HEIGHT > self.height {
                    self.scroll();
                }
            }
            b'\r' => {
                self.x = 0;
            }
            b'\t' => {
                // Tab = 4 spaces
                let spaces = 4 - (self.x / FONT_WIDTH % 4);
                for _ in 0..spaces {
                    self.draw_char(b' ', fg, bg);
                }
            }
            8 | 127 => {
                self.backspace();
            }
            32..=126 => {
                self.draw_printable_char(c, fg, bg);
                self.chars_drawn += 1;
            }
            _ => {
                // Draw replacement character for unprintable
                self.draw_printable_char(b'?', fg, bg);
            }
        }
    }
    
    /// Draw printable character with font.bin
    fn draw_printable_char(&mut self, c: u8, fg: u32, bg: u32) {
        // Validate font data size
        if FONT_DATA.len() < 95 * FONT_HEIGHT {
            // Fallback: draw a simple block
            self.draw_fallback_char(fg, bg);
            return;
        }
        
        let idx = (c - 32) as usize;
        let offset = idx * FONT_HEIGHT;
        
        // Bounds check
        if offset + FONT_HEIGHT > FONT_DATA.len() {
            self.draw_fallback_char(fg, bg);
            return;
        }
        
        // Draw glyph
        for row in 0..FONT_HEIGHT {
            let byte = FONT_DATA[offset + row];
            for col in 0..FONT_WIDTH {
                let bit_set = (byte & (1 << (7 - col))) != 0;
                let color = if bit_set { fg } else { bg };
                
                // Only draw if within bounds
                if self.x + col < self.width && self.y + row < self.height {
                    self.put_pixel(self.x + col, self.y + row, color);
                }
            }
        }
        
        // Advance cursor
        self.x += FONT_WIDTH;
        if self.x + FONT_WIDTH > self.width {
            self.x = 0;
            self.y += FONT_HEIGHT;
            if self.y + FONT_HEIGHT > self.height {
                self.scroll();
            }
        }
    }
    
    /// Fallback character rendering
    fn draw_fallback_char(&mut self, fg: u32, bg: u32) {
        for row in 2..14 {
            for col in 2..6 {
                self.put_pixel(self.x + col, self.y + row, fg);
            }
        }
        
        self.x += FONT_WIDTH;
        if self.x + FONT_WIDTH > self.width {
            self.x = 0;
            self.y += FONT_HEIGHT;
            if self.y + FONT_HEIGHT > self.height {
                self.scroll();
            }
        }
    }
    
    /// Scroll screen up by one line
    fn scroll(&mut self) {
        unsafe {
            let line_bytes = FONT_HEIGHT * self.pitch;
            let total_bytes = (self.height - FONT_HEIGHT) * self.pitch;
            
            // Copy all lines up
            core::ptr::copy(
                self.addr.add(line_bytes),
                self.addr,
                total_bytes
            );
            
            // Clear bottom line
            core::ptr::write_bytes(
                self.addr.add(total_bytes),
                0,
                line_bytes
            );
        }
        
        self.y = self.height - FONT_HEIGHT;
    }
    
    /// Backspace (your working implementation)
    fn backspace(&mut self) {
        if self.x >= FONT_WIDTH {
            self.x -= FONT_WIDTH;
        } else if self.y >= FONT_HEIGHT {
            self.y -= FONT_HEIGHT;
            self.x = self.width - FONT_WIDTH;
        }
        
        // Clear the character
        for row in 0..FONT_HEIGHT {
            for col in 0..FONT_WIDTH {
                self.put_pixel(self.x + col, self.y + row, self.current_bg_color);
            }
        }
    }
    
    /// Write string with current colors
    pub fn write_str(&mut self, s: &str) {
        for byte in s.bytes() {
            self.draw_char(byte, self.current_fg_color, self.current_bg_color);
        }
    }
    
    /// Write string with custom color
    pub fn write_str_colored(&mut self, s: &str, fg: u32) {
        for byte in s.bytes() {
            self.draw_char(byte, fg, self.current_bg_color);
        }
    }
    
    /// Set foreground color
    pub fn set_fg_color(&mut self, color: u32) {
        self.current_fg_color = color;
    }
    
    /// Set background color
    pub fn set_bg_color(&mut self, color: u32) {
        self.current_bg_color = color;
    }
    
    /// Get cursor position
    pub fn get_cursor(&self) -> (usize, usize) {
        (self.x / FONT_WIDTH, self.y / FONT_HEIGHT)
    }
    
    /// Set cursor position
    pub fn set_cursor(&mut self, col: usize, row: usize) {
        self.x = col * FONT_WIDTH;
        self.y = row * FONT_HEIGHT;
        
        // Clamp to bounds
        if self.x >= self.width {
            self.x = self.width - FONT_WIDTH;
        }
        if self.y >= self.height {
            self.y = self.height - FONT_HEIGHT;
        }
    }
    
    /// Draw cursor at current position
    pub fn draw_cursor(&mut self) {
        if !self.cursor_visible {
            return;
        }
        
        let color = if self.cursor_blink_state {
            self.current_fg_color
        } else {
            self.current_bg_color
        };
        
        // Draw cursor line at bottom of character cell
        for col in 0..FONT_WIDTH {
            self.put_pixel(self.x + col, self.y + FONT_HEIGHT - 2, color);
            self.put_pixel(self.x + col, self.y + FONT_HEIGHT - 1, color);
        }
    }
    
    /// Toggle cursor blink state (call from timer interrupt)
    pub fn update_cursor_blink(&mut self) {
        self.cursor_blink_state = !self.cursor_blink_state;
    }
    
    // ========================================================================
    // REALITY-AWARE FEATURES
    // ========================================================================
    
    /// Draw with reality-fade effect (for dream state transitions)
    pub fn draw_char_with_fade(&mut self, c: u8, fg: u32, bg: u32, fade_factor: u8) {
        let faded_fg = self.fade_color(fg, fade_factor);
        let faded_bg = self.fade_color(bg, fade_factor);
        self.draw_char(c, faded_fg, faded_bg);
    }
    
    /// Fade color by factor (0-255)
    fn fade_color(&self, color: u32, factor: u8) -> u32 {
        let r = ((color >> 16) & 0xFF) as u8;
        let g = ((color >> 8) & 0xFF) as u8;
        let b = (color & 0xFF) as u8;
        
        let fr = ((r as u16 * factor as u16) / 255) as u8;
        let fg = ((g as u16 * factor as u16) / 255) as u8;
        let fb = ((b as u16 * factor as u16) / 255) as u8;
        
        ((fr as u32) << 16) | ((fg as u32) << 8) | (fb as u32)
    }
    
    /// Draw reality transition overlay
    pub fn draw_reality_transition(&mut self, progress: u8) {
        // Visual effect for reality switches
        let overlay_color = self.fade_color(0x00AAFF, progress);
        
        for y in (0..self.height).step_by(4) {
            for x in (0..self.width).step_by(4) {
                if (x + y) % 8 == 0 {
                    self.put_pixel(x, y, overlay_color);
                }
            }
        }
    }
    
    /// Get statistics
    pub fn get_stats(&self) -> FramebufferStats {
        FramebufferStats {
            width: self.width,
            height: self.height,
            bpp: self.bpp,
            chars_drawn: self.chars_drawn,
            frames_rendered: self.frames_rendered,
            cursor_pos: self.get_cursor(),
        }
    }
}

// ============================================================================
// STATISTICS
// ============================================================================

#[derive(Debug, Clone, Copy)]
pub struct FramebufferStats {
    pub width: usize,
    pub height: usize,
    pub bpp: u16,
    pub chars_drawn: u64,
    pub frames_rendered: u64,
    pub cursor_pos: (usize, usize),
}

// ============================================================================
// GLOBAL FRAMEBUFFER (THREAD-SAFE)
// ============================================================================

static FB: Mutex<Option<Framebuffer>> = Mutex::new(None);

/// Initialize framebuffer
pub fn init_framebuffer() {
    use crate::FRAMEBUFFER_REQUEST;
    
    if let Some(fb_resp) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(framebuffer) = fb_resp.framebuffers().next() {
            let mut fb_lock = FB.lock();
            *fb_lock = Some(Framebuffer::new(
                framebuffer.addr(),
                framebuffer.width() as usize,
                framebuffer.height() as usize,
                framebuffer.pitch() as usize,
                framebuffer.bpp(),
            ));
        }
    }
}

/// Print to framebuffer (thread-safe)
pub fn fb_print(s: &str) {
    let mut fb = FB.lock();
    if let Some(ref mut framebuffer) = *fb {
        framebuffer.write_str(s);
    }
}

/// Print with color (thread-safe)
pub fn fb_print_colored(s: &str, color: u32) {
    let mut fb = FB.lock();
    if let Some(ref mut framebuffer) = *fb {
        framebuffer.write_str_colored(s, color);
    }
}

/// Clear screen (thread-safe)
pub fn fb_clear() {
    let mut fb = FB.lock();
    if let Some(ref mut framebuffer) = *fb {
        framebuffer.clear();
    }
}

/// Set foreground color
pub fn fb_set_fg_color(color: u32) {
    let mut fb = FB.lock();
    if let Some(ref mut framebuffer) = *fb {
        framebuffer.set_fg_color(color);
    }
}

/// Set background color
pub fn fb_set_bg_color(color: u32) {
    let mut fb = FB.lock();
    if let Some(ref mut framebuffer) = *fb {
        framebuffer.set_bg_color(color);
    }
}

/// Draw reality transition effect
pub fn fb_draw_reality_transition(progress: u8) {
    let mut fb = FB.lock();
    if let Some(ref mut framebuffer) = *fb {
        framebuffer.draw_reality_transition(progress);
    }
}

/// Get framebuffer statistics
pub fn fb_get_stats() -> Option<FramebufferStats> {
    let fb = FB.lock();
    fb.as_ref().map(|f| f.get_stats())
}

/// Update cursor blink (call from timer interrupt)
pub fn fb_update_cursor() {
    let mut fb = FB.lock();
    if let Some(ref mut framebuffer) = *fb {
        framebuffer.update_cursor_blink();
        framebuffer.draw_cursor();
    }
}

// ============================================================================
// PRINT MACROS (UNCHANGED - YOUR IMPLEMENTATION WORKS)
// ============================================================================

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!(FbWriter, $($arg)*);
    }};
}

#[macro_export]
macro_rules! println {
    () => ($crate::fb_print("\n"));
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!(FbWriter, $($arg)*);
        $crate::fb_print("\n");
    }};
}

pub struct FbWriter;

impl core::fmt::Write for FbWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        fb_print(s);
        Ok(())
    }
}
// ============================================================================
// VIRTIO BLOCK DEVICE (Disk Driver)
// ============================================================================

// VirtIO constants
const VIRTIO_VENDOR_ID: u16 = 0x1AF4;
const VIRTIO_BLOCK_DEVICE_ID: u16 = 0x1042;

const VIRTIO_STATUS_ACKNOWLEDGE: u8 = 1;
const VIRTIO_STATUS_DRIVER: u8 = 2;
const VIRTIO_STATUS_DRIVER_OK: u8 = 4;
const VIRTIO_STATUS_FEATURES_OK: u8 = 8;

const VIRTIO_F_VERSION_1: u64 = 1 << 32;

const VIRTIO_BLK_T_IN: u32 = 0;
const VIRTIO_BLK_T_OUT: u32 = 1;

const VIRTIO_BLK_S_OK: u8 = 0;

const SECTOR_SIZE: usize = 512;
const QUEUE_SIZE: usize = 256;

// VirtIO structures
#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub struct VirtqDesc {
    pub addr: u64,
    pub len: u32,
    pub flags: u16,
    pub next: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VirtioBlkReqHeader {
    pub req_type: u32,
    pub reserved: u32,
    pub sector: u64,
}

// Simplified VirtIO block device
pub struct VirtioBlockDevice {
    pub present: bool,
    pub capacity: u64,
}

impl VirtioBlockDevice {
    pub const fn new() -> Self {
        Self {
            present: false,
            capacity: 0,
        }
    }
}

static VIRTIO_BLK: Mutex<VirtioBlockDevice> = Mutex::new(VirtioBlockDevice::new());

// PCI configuration
const PCI_CONFIG_ADDR: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

unsafe fn outl(port: u16, value: u32) {
    asm!("out dx, eax", in("dx") port, in("eax") value, options(nostack, nomem));
}

unsafe fn inl(port: u16) -> u32 {
    let value: u32;
    asm!("in eax, dx", out("eax") value, in("dx") port, options(nostack, nomem));
    value
}

fn pci_config_read32(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    let addr: u32 = (1 << 31)
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);
    
    unsafe {
        outl(PCI_CONFIG_ADDR, addr);
        inl(PCI_CONFIG_DATA)
    }
}

fn pci_config_read16(bus: u8, device: u8, func: u8, offset: u8) -> u16 {
    let val = pci_config_read32(bus, device, func, offset & 0xFC);
    ((val >> ((offset & 2) * 8)) & 0xFFFF) as u16
}

pub fn init_virtio_block() -> bool {
    // Scan PCI bus for VirtIO block device
    for bus in 0..256 {
        for device in 0..32 {
            let vendor = pci_config_read16(bus as u8, device, 0, 0);
            if vendor == VIRTIO_VENDOR_ID {
                let device_id = pci_config_read16(bus as u8, device, 0, 2);
                if device_id == VIRTIO_BLOCK_DEVICE_ID {
                    println!("      VirtIO-blk: Found at PCI {:02x}:{:02x}.0", bus, device);
                    
                    // For now, just mark as present
                    // Full initialization would require mapping BARs, setting up virtqueues, etc.
                    let mut dev = VIRTIO_BLK.lock();
                    dev.present = true;
                    dev.capacity = 131072; // Assume 64MB (fake for now)
                    
                    return true;
                }
            }
        }
    }
    
    println!("      VirtIO-blk: Not found");
    false
}

// Simplified disk I/O (stub for now - full implementation in your original code)
pub fn disk_read_sector(_sector: u64, buffer: &mut [u8; 512]) -> bool {
    // TODO: Implement actual VirtIO read
    buffer.fill(0);
    false
}

pub fn disk_write_sector(_sector: u64, _buffer: &[u8; 512]) -> bool {
    // TODO: Implement actual VirtIO write
    false
}

pub fn disk_is_present() -> bool {
    VIRTIO_BLK.lock().present
}

pub fn disk_get_capacity() -> u64 {
    VIRTIO_BLK.lock().capacity
}

// ============================================================================
// SERIAL PORT (for debugging)
// ============================================================================

const COM1: u16 = 0x3F8;

pub struct SerialPort {
    port: u16,
}

impl SerialPort {
    pub fn new(port: u16) -> Self {
        Self { port }
    }
    
    pub fn init(&self) {
        unsafe {
            outb(self.port + 1, 0x00); // Disable interrupts
            outb(self.port + 3, 0x80); // Enable DLAB
            outb(self.port + 0, 0x03); // Set divisor (low)
            outb(self.port + 1, 0x00); // Set divisor (high)
            outb(self.port + 3, 0x03); // 8 bits, no parity, one stop bit
            outb(self.port + 2, 0xC7); // Enable FIFO
            outb(self.port + 4, 0x0B); // IRQs enabled, RTS/DSR set
        }
    }
    
    pub fn send(&self, data: u8) {
        unsafe {
            while (inb(self.port + 5) & 0x20) == 0 {}
            outb(self.port, data);
        }
    }
    
    pub fn write_str(&self, s: &str) {
        for byte in s.bytes() {
            if byte == b'\n' {
                self.send(b'\r');
            }
            self.send(byte);
        }
    }
}

static SERIAL: Mutex<Option<SerialPort>> = Mutex::new(None);

pub fn init_serial() {
    let port = SerialPort::new(COM1);
    port.init();
    *SERIAL.lock() = Some(port);
}

pub fn serial_print(s: &str) {
    if let Some(ref port) = *SERIAL.lock() {
        port.write_str(s);
    }
}
// ============================================================================
// ASTRAL OS - SECTION 6: PSYCHICFS (Predictive Filesystem)
// ============================================================================

// ============================================================================
// FILESYSTEM CONSTANTS
// ============================================================================

const FS_MAGIC: u32 = 0x50535946; // "PSYF"
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

// ============================================================================
// ON-DISK STRUCTURES
// ============================================================================

#[repr(C, packed)]
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
    pub mount_count: u32,
    pub last_mount_time: u64,
    pub _reserved: [u8; 472],
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Inode {
    pub in_use: u8,
    pub file_type: u8,
    pub permissions: u16,
    pub size: u32,
    pub created_time: u64,
    pub modified_time: u64,
    pub access_count: u32,
    pub last_access: u64,
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
            access_count: 0,
            last_access: 0,
            blocks: [0; MAX_FILE_BLOCKS],
            name: [0; MAX_FILENAME_LEN],
            _reserved: [0; 8],
        }
    }
    
    pub fn get_name(&self) -> &str {
        let name_slice = unsafe {
            core::slice::from_raw_parts(
                core::ptr::addr_of!(self.name) as *const u8,
                MAX_FILENAME_LEN
            )
        };
        let len = name_slice.iter().position(|&c| c == 0).unwrap_or(MAX_FILENAME_LEN);
        core::str::from_utf8(&name_slice[..len]).unwrap_or("")
    }
    
    pub fn set_name(&mut self, name: &str) {
        let bytes = name.as_bytes();
        let len = bytes.len().min(MAX_FILENAME_LEN - 1);
        unsafe {
            let name_ptr = core::ptr::addr_of_mut!(self.name) as *mut u8;
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), name_ptr, len);
            core::ptr::write(name_ptr.add(len), 0);
        }
    }
    
    pub fn get_size(&self) -> u32 {
        unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(self.size)) }
    }
    
    pub fn set_size(&mut self, val: u32) {
        unsafe { core::ptr::write_unaligned(core::ptr::addr_of_mut!(self.size), val); }
    }
    
    pub fn get_block(&self, idx: usize) -> u32 {
        assert!(idx < MAX_FILE_BLOCKS);
        unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(self.blocks[idx])) }
    }
    
    pub fn set_block(&mut self, idx: usize, val: u32) {
        assert!(idx < MAX_FILE_BLOCKS);
        unsafe { core::ptr::write_unaligned(core::ptr::addr_of_mut!(self.blocks[idx]), val); }
    }
}

// ============================================================================
// FILESYSTEM STATE
// ============================================================================

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

// ============================================================================
// SERIALIZATION HELPERS
// ============================================================================

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

// ============================================================================
// FILESYSTEM OPERATIONS
// ============================================================================

pub fn fs_format() -> bool {
    if !disk_is_present() {
        println!("No disk present!");
        return false;
    }
    
    println!("Formatting PsychicFS...");
    
    let superblock = Superblock {
        magic: FS_MAGIC,
        version: FS_VERSION,
        total_blocks: 8192,
        free_blocks: 8192 - DATA_BLOCKS_START as u32,
        total_inodes: MAX_FILES as u32,
        free_inodes: MAX_FILES as u32,
        first_data_block: DATA_BLOCKS_START as u32,
        block_size: BLOCK_SIZE as u16,
        mount_count: 0,
        last_mount_time: 0,
        _reserved: [0; 472],
    };
    
    let sb_bytes = superblock_to_bytes(&superblock);
    if !disk_write_sector(SUPERBLOCK_SECTOR, &sb_bytes) {
        println!("Failed to write superblock!");
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
    
    println!("Format complete!");
    true
}

pub fn fs_mount() -> bool {
    if !disk_is_present() {
        println!("No disk present!");
        return false;
    }
    
    println!("Mounting PsychicFS...");
    
    let mut sb_buffer = [0u8; 512];
    if !disk_read_sector(SUPERBLOCK_SECTOR, &mut sb_buffer) {
        println!("Failed to read superblock");
        return false;
    }
    
    let mut superblock = bytes_to_superblock(&sb_buffer);
    
    let magic = unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(superblock.magic)) };
    if magic != FS_MAGIC {
        println!("Invalid filesystem magic: 0x{:08X}", magic);
        return false;
    }
    
    let mut pfs = PsychicFs::new();
    if !pfs.read_bitmap_from_disk() {
        println!("Failed to read bitmap");
        return false;
    }
    
    unsafe {
        let mount_count = core::ptr::read_unaligned(core::ptr::addr_of!(superblock.mount_count));
        core::ptr::write_unaligned(core::ptr::addr_of_mut!(superblock.mount_count), mount_count + 1);
    }
    
    let sb_bytes = superblock_to_bytes(&superblock);
    disk_write_sector(SUPERBLOCK_SECTOR, &sb_bytes);
    
    pfs.superblock = superblock;
    pfs.mounted = true;
    
    *PSYCHIC_FS.lock() = Some(pfs);
    
    println!("Mount successful!");
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
    inode.created_time = get_timestamp();
    inode.modified_time = get_timestamp();
    
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
    
    // Free old blocks
    for i in blocks_needed..MAX_FILE_BLOCKS {
        let block = inode.get_block(i);
        if block != 0 {
            free_block(block);
            inode.set_block(i, 0);
        }
    }
    
    // Write data
    for i in 0..blocks_needed {
        let block = if inode.get_block(i) != 0 {
            inode.get_block(i)
        } else {
            match allocate_block() {
                Some(b) => {
                    inode.set_block(i, b);
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
    
    inode.set_size(data.len() as u32);
    inode.modified_time = get_timestamp();
    
    write_inode(inode_num, &inode)
}

pub fn fs_read(name: &str) -> Option<Vec<u8>> {
    let (inode_num, mut inode) = find_inode_by_name(name)?;
    
    let size = inode.get_size();
    if size == 0 {
        return Some(Vec::new());
    }
    
    let mut data = Vec::with_capacity(size as usize);
    let blocks_needed = (size as usize + BLOCK_SIZE - 1) / BLOCK_SIZE;
    
    for i in 0..blocks_needed {
        let block = inode.get_block(i);
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
    
    inode.access_count += 1;
    inode.last_access = get_timestamp();
    let _ = write_inode(inode_num, &inode);
    
    Some(data)
}

pub fn fs_delete(name: &str) -> bool {
    let (inode_num, mut inode) = match find_inode_by_name(name) {
        Some(i) => i,
        None => return false,
    };
    
    for i in 0..MAX_FILE_BLOCKS {
        let block = inode.get_block(i);
        if block != 0 {
            free_block(block);
            inode.set_block(i, 0);
        }
    }
    
    inode.in_use = 0;
    inode.set_size(0);
    
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
// ============================================================================
// ASTRAL OS - SECTION 7: REALITY ENGINE
// ============================================================================

// ============================================================================
// REALITY IDENTIFIERS & TRACKING
// ============================================================================

static REALITY_COUNTER: AtomicU64 = AtomicU64::new(0);
static CURRENT_REALITY_ID: AtomicU64 = AtomicU64::new(0);

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RealityId(u64);

impl RealityId {
    pub fn new() -> Self {
        Self(REALITY_COUNTER.fetch_add(1, Ordering::SeqCst))
    }
    
    pub fn root() -> Self {
        Self(0)
    }
    
    pub fn current() -> Self {
        Self(CURRENT_REALITY_ID.load(Ordering::SeqCst))
    }
    
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Reality checkpoint for state snapshots
#[repr(C)]
pub struct RealityCheckpoint {
    pub id: RealityId,
    pub parent: Option<RealityId>,
    pub timestamp: u64,
    pub heap_snapshot_addr: usize,
    pub heap_snapshot_size: usize,
}

// ============================================================================
// CAUSAL EVENT SYSTEM
// ============================================================================

static CAUSAL_EVENT_COUNTER: AtomicU64 = AtomicU64::new(0);

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub enum CausalEventType {
    Boot,
    Interrupt,
    Syscall,
    Allocation,
    Deallocation,
    DiskRead,
    DiskWrite,
    FileOpen,
    FileClose,
    TaskSpawn,
    TaskExit,
    DreamEnter,
    DreamExit,
    Checkpoint,
    Rollback,
    UserCommand,
    Custom,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union CausalEventData {
    pub interrupt: InterruptEventData,
    pub allocation: AllocationEventData,
    pub disk: DiskEventData,
    pub file: FileEventData,
    pub command: CommandEventData,
    pub raw: [u8; 32],
}

impl core::fmt::Debug for CausalEventData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "CausalEventData {{ ... }}")
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct InterruptEventData {
    pub vector: u8,
    pub error_code: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct AllocationEventData {
    pub address: usize,
    pub size: usize,
    pub spatial_x: u64,
    pub spatial_y: u64,
    pub spatial_z: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DiskEventData {
    pub sector: u64,
    pub count: u16,
    pub success: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FileEventData {
    pub inode: u32,
    pub operation: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CommandEventData {
    pub cmd_hash: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CausalEvent {
    pub id: u64,
    pub timestamp: u64,
    pub event_type: CausalEventType,
    pub cause_id: Option<u64>,
    pub reality_id: u64,
    pub data: CausalEventData,
}

const MAX_CAUSAL_EVENTS: usize = 1024;

pub struct CausalLog {
    events: VecDeque<CausalEvent>,
    next_id: u64,
}

impl CausalLog {
    pub fn new() -> Self {
        Self {
            events: VecDeque::with_capacity(MAX_CAUSAL_EVENTS),
            next_id: 0,
        }
    }
    
    pub fn log(&mut self, event_type: CausalEventType, cause: Option<u64>, data: CausalEventData) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        
        let event = CausalEvent {
            id,
            timestamp: get_timestamp(),
            event_type,
            cause_id: cause,
            reality_id: CURRENT_REALITY_ID.load(Ordering::Relaxed),
            data,
        };
        
        if self.events.len() >= MAX_CAUSAL_EVENTS {
            self.events.pop_front();
        }
        
        self.events.push_back(event);
        CAUSAL_EVENT_COUNTER.fetch_add(1, Ordering::Relaxed);
        
        id
    }
    
    pub fn get_event(&self, id: u64) -> Option<&CausalEvent> {
        self.events.iter().find(|e| e.id == id)
    }
    
    pub fn get_recent(&self, count: usize) -> impl Iterator<Item = &CausalEvent> {
        self.events.iter().rev().take(count)
    }
    
    pub fn get_causal_chain(&self, event_id: u64) -> Vec<&CausalEvent> {
        let mut chain = Vec::new();
        let mut current_id = Some(event_id);
        
        while let Some(id) = current_id {
            if let Some(event) = self.get_event(id) {
                chain.push(event);
                current_id = event.cause_id;
            } else {
                break;
            }
        }
        
        chain
    }
    
    pub fn len(&self) -> usize {
        self.events.len()
    }
}

static CAUSAL_LOG: Mutex<Option<CausalLog>> = Mutex::new(None);

pub fn init_causal_log() {
    let mut log = CAUSAL_LOG.lock();
    *log = Some(CausalLog::new());
    
    if let Some(ref mut l) = *log {
        l.log(
            CausalEventType::Boot,
            None,
            CausalEventData { raw: [0; 32] }
        );
    }
}

pub fn log_event(event_type: CausalEventType, cause: Option<u64>, data: CausalEventData) -> u64 {
    if let Some(ref mut log) = *CAUSAL_LOG.lock() {
        log.log(event_type, cause, data)
    } else {
        0
    }
}

pub fn get_causal_chain(event_id: u64) -> Vec<CausalEvent> {
    if let Some(ref log) = *CAUSAL_LOG.lock() {
        log.get_causal_chain(event_id).into_iter().cloned().collect()
    } else {
        Vec::new()
    }
}

pub fn get_recent_events(count: usize) -> Vec<CausalEvent> {
    if let Some(ref log) = *CAUSAL_LOG.lock() {
        log.get_recent(count).cloned().collect()
    } else {
        Vec::new()
    }
}

pub fn get_event_count() -> usize {
    if let Some(ref log) = *CAUSAL_LOG.lock() {
        log.len()
    } else {
        0
    }
}

// ============================================================================
// DREAM STATE ENGINE
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum SystemState {
    Active = 0,
    Dreaming = 1,
    DeepDream = 2,
    Awakening = 3,
}

static SYSTEM_STATE: AtomicUsize = AtomicUsize::new(SystemState::Active as usize);
static IDLE_CYCLES: AtomicU64 = AtomicU64::new(0);
static DREAM_CYCLES: AtomicU64 = AtomicU64::new(0);
static DREAM_COMPACTIONS: AtomicU64 = AtomicU64::new(0);
static DREAM_PREDICTIONS: AtomicU64 = AtomicU64::new(0);

const DREAM_THRESHOLD: u64 = 1000;
const DEEP_DREAM_THRESHOLD: u64 = 5000;
const DREAM_COMPACT_INTERVAL: u64 = 100;
const DREAM_PREDICT_INTERVAL: u64 = 50;

pub fn get_system_state() -> SystemState {
    match SYSTEM_STATE.load(Ordering::Relaxed) {
        0 => SystemState::Active,
        1 => SystemState::Dreaming,
        2 => SystemState::DeepDream,
        3 => SystemState::Awakening,
        _ => SystemState::Active,
    }
}

pub fn enter_dream_state() {
    SYSTEM_STATE.store(SystemState::Dreaming as usize, Ordering::SeqCst);
    log_event(CausalEventType::DreamEnter, None, CausalEventData { raw: [0; 32] });
}

pub fn exit_dream_state() {
    SYSTEM_STATE.store(SystemState::Active as usize, Ordering::SeqCst);
    IDLE_CYCLES.store(0, Ordering::SeqCst);
    log_event(CausalEventType::DreamExit, None, CausalEventData { raw: [0; 32] });
}

// File access pattern tracking
const MAX_ACCESS_PATTERNS: usize = 64;

#[derive(Clone, Copy)]
pub struct AccessPattern {
    pub file_id: u32,
    pub access_count: u32,
    pub last_access: u64,
    pub predicted_next: u64,
    pub avg_interval: u64,
}

pub struct AccessPatternTracker {
    patterns: [Option<AccessPattern>; MAX_ACCESS_PATTERNS],
    count: usize,
}

impl AccessPatternTracker {
    pub fn new() -> Self {
        Self {
            patterns: [None; MAX_ACCESS_PATTERNS],
            count: 0,
        }
    }
    
    pub fn record_access(&mut self, file_id: u32, timestamp: u64) {
        for i in 0..self.count {
            if let Some(ref mut pattern) = self.patterns[i] {
                if pattern.file_id == file_id {
                    let interval = timestamp.saturating_sub(pattern.last_access);
                    pattern.avg_interval = (pattern.avg_interval * pattern.access_count as u64 + interval) 
                                          / (pattern.access_count as u64 + 1);
                    pattern.access_count += 1;
                    pattern.last_access = timestamp;
                    pattern.predicted_next = timestamp + pattern.avg_interval;
                    return;
                }
            }
        }
        
        if self.count < MAX_ACCESS_PATTERNS {
            self.patterns[self.count] = Some(AccessPattern {
                file_id,
                access_count: 1,
                last_access: timestamp,
                predicted_next: 0,
                avg_interval: 0,
            });
            self.count += 1;
        }
    }
    
    pub fn get_hot_files(&self, min_accesses: u32) -> Vec<u32> {
        let mut hot = Vec::new();
        
        for i in 0..self.count {
            if let Some(ref pattern) = self.patterns[i] {
                if pattern.access_count >= min_accesses {
                    hot.push(pattern.file_id);
                }
            }
        }
        
        hot
    }
}

static ACCESS_PATTERNS: Mutex<Option<AccessPatternTracker>> = Mutex::new(None);

pub fn init_dream_engine() {
    let mut patterns = ACCESS_PATTERNS.lock();
    *patterns = Some(AccessPatternTracker::new());
}

pub fn record_file_access(file_id: u32) {
    if let Some(ref mut tracker) = *ACCESS_PATTERNS.lock() {
        tracker.record_access(file_id, get_timestamp());
    }
}

pub fn dream_cycle() {
    let cycles = DREAM_CYCLES.fetch_add(1, Ordering::Relaxed);
    
    match get_system_state() {
        SystemState::Dreaming => {
            if cycles % DREAM_COMPACT_INTERVAL == 0 {
                DREAM_COMPACTIONS.fetch_add(1, Ordering::Relaxed);
            }
            
            if cycles % DREAM_PREDICT_INTERVAL == 0 {
                DREAM_PREDICTIONS.fetch_add(1, Ordering::Relaxed);
            }
            
            let idle = IDLE_CYCLES.load(Ordering::Relaxed);
            if idle > DEEP_DREAM_THRESHOLD {
                SYSTEM_STATE.store(SystemState::DeepDream as usize, Ordering::SeqCst);
            }
        }
        
        SystemState::DeepDream => {
            // Deep optimization
            if cycles % 100 == 0 {
                DREAM_COMPACTIONS.fetch_add(1, Ordering::Relaxed);
            }
        }
        
        _ => {}
    }
}

#[derive(Debug)]
pub struct DreamStats {
    pub total_cycles: u64,
    pub compactions: u64,
    pub predictions: u64,
    pub state: SystemState,
    pub idle_cycles: u64,
}

pub fn get_dream_stats() -> DreamStats {
    DreamStats {
        total_cycles: DREAM_CYCLES.load(Ordering::Relaxed),
        compactions: DREAM_COMPACTIONS.load(Ordering::Relaxed),
        predictions: DREAM_PREDICTIONS.load(Ordering::Relaxed),
        state: get_system_state(),
        idle_cycles: IDLE_CYCLES.load(Ordering::Relaxed),
    }
}

// ============================================================================
// INTENT-BASED SYSCALLS (Foundation)
// ============================================================================

#[repr(u16)]
#[derive(Clone, Copy, Debug)]
pub enum Intent {
    NeedMemory = 0x0100,
    ReleaseMemory = 0x0101,
    ShareMemory = 0x0102,
    
    ReadData = 0x0200,
    WriteData = 0x0201,
    StreamData = 0x0202,
    
    SpawnTask = 0x0300,
    JoinTask = 0x0301,
    ForkReality = 0x0302,
    MergeReality = 0x0303,
    
    FindFile = 0x0400,
    StoreFile = 0x0401,
    OrganizeFiles = 0x0402,
    
    Checkpoint = 0x0500,
    Rollback = 0x0501,
    QueryCausality = 0x0502,
}

#[repr(C)]
pub struct IntentRequest {
    pub intent: Intent,
    pub priority: u8,
    pub context: [u8; 128],
    pub context_len: usize,
}

#[repr(C)]
pub struct IntentResponse {
    pub success: bool,
    pub result_code: i32,
    pub data: [u8; 256],
    pub data_len: usize,
}

// Intent handling (stub for now)
pub fn handle_intent(request: &IntentRequest) -> IntentResponse {
    IntentResponse {
        success: false,
        result_code: -1,
        data: [0; 256],
        data_len: 0,
    }
}
// ============================================================================
// ASTRAL OS - SECTION 8: SHELL & MAIN KERNEL ENTRY
// ============================================================================

// ============================================================================
// SIMPLE SHELL
// ============================================================================

const MAX_CMD_LEN: usize = 256;
const MAX_HISTORY: usize = 50;

pub struct Shell {
    cmd_buffer: [u8; MAX_CMD_LEN],
    cmd_len: usize,
    history: Vec<String>,
    history_index: usize,
    theme: ShellTheme,
}
#[derive(Clone, Copy)]
pub struct ShellTheme {
    pub prompt_color: u32,
    pub prompt_symbol_color: u32,
    pub command_color: u32,
    pub output_color: u32,
    pub error_color: u32,
    pub success_color: u32,
    pub info_color: u32,
    pub reality_color: u32,
}

impl ShellTheme {
    pub const REALITY: Self = Self {
        prompt_color: 0x00AAFF,
        prompt_symbol_color: 0x00FF00,
        command_color: 0xFFFFFF,
        output_color: 0xCCCCCC,
        error_color: 0xFF6666,
        success_color: 0x00FF00,
        info_color: 0xFFFF00,
        reality_color: 0xFF00FF,
    };
    
    pub const DREAM: Self = Self {
        prompt_color: 0x9966FF,
        prompt_symbol_color: 0xFF66FF,
        command_color: 0xFFFFFF,
        output_color: 0xBBBBFF,
        error_color: 0xFF6666,
        success_color: 0x66FFAA,
        info_color: 0xFFDD66,
        reality_color: 0xFF66FF,
    };
}
impl Shell {
    pub fn new() -> Self {
        Self {
            cmd_buffer: [0; MAX_CMD_LEN],
            cmd_len: 0,
            history: Vec::with_capacity(MAX_HISTORY),
            history_index: 0,
            theme: ShellTheme::REALITY,
        }
    }
    
    pub fn print_prompt(&self) {
        // Show reality ID in prompt
        let reality_id = CURRENT_REALITY_ID.load(Ordering::Relaxed);
        let state = get_system_state();
        
        fb_print_colored("astral", self.theme.prompt_color);
        
        // Show state indicator
        match state {
            SystemState::Dreaming => fb_print_colored("💤", 0x9966FF),
            SystemState::DeepDream => fb_print_colored("🌙", 0x6633FF),
            _ => {}
        }
        
        // Show reality ID if not root
        if reality_id != 0 {
            print!(":{}", reality_id);
        }
        
        fb_print_colored("> ", self.theme.prompt_symbol_color);
    }
    
    pub fn run(&mut self) {
        self.print_banner();
        self.print_prompt();
        
        
        loop {
            if let Some(c) = getchar() {
                match c {
                    b'\n' => {
                        println!();
                        self.execute_command();
                        if self.cmd_len > 0 {
                            let cmd = self.get_cmd_str().to_string();
                            if self.history.len() >= MAX_HISTORY {
                                self.history.remove(0);
                            }
                            self.history.push(cmd);
                            self.history_index = self.history.len();
                        }
                        
                        self.cmd_len = 0;
                        self.print_prompt();
                    }
                    8 | 127 => {
                        if self.cmd_len > 0 {
                            self.cmd_len -= 1;
                            fb_print("\x08 \x08");
                        }
                    }
                    32..=126 => {
                        if self.cmd_len < MAX_CMD_LEN - 1 {
                            self.cmd_buffer[self.cmd_len] = c;
                            self.cmd_len += 1;
                            unsafe {
                                if let Some(ref mut fb) = *FB.lock() {
                                    fb.draw_char(c, 0xFFFFFF, 0);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            
            // Dream state handling
            if get_system_state() != SystemState::Active {
                dream_cycle();
            }
            
            unsafe { asm!("hlt"); }
        }
    }

    fn print_banner(&self) {
        fb_print_colored("╔═══════════════════════════════════════════╗\n", 0x00AAFF);
        fb_print_colored("║      ", 0x00AAFF);
        fb_print_colored("ASTRAL OS", 0xFFFFFF);
        fb_print_colored(" v0.3.0                 ║\n", 0x00AAFF);
        fb_print_colored("║  ", 0x00AAFF);
        fb_print_colored("Modern Foundation + Reality Engine", 0x888888);
        fb_print_colored("   ║\n", 0x00AAFF);
        fb_print_colored("╚═══════════════════════════════════════════╝\n", 0x00AAFF);
        println!();
        println!("Welcome to Astral OS Shell");
        println!("Type 'help' for available commands");
        println!();
    }

    fn get_cmd_str(&self) -> &str {
        core::str::from_utf8(&self.cmd_buffer[..self.cmd_len]).unwrap_or("")
    }
    
    fn execute_command(&mut self) {
        let cmd = self.get_cmd_str().trim().to_string();
        
        if cmd.is_empty() {
            return;
        }
        
        let mut parts = cmd.split_whitespace();
        let command = parts.next().unwrap_or("");
        
        match command {
            "help" => self.cmd_help(),
            "clear" => self.cmd_clear(),
            "info" => self.cmd_info(),
            "mem" => self.cmd_mem(),
            "ps" => self.cmd_ps(),
            "echo" => self.cmd_echo(parts),
            
            // Reality Engine
            "reality" => self.cmd_reality(parts),
            "causality" => self.cmd_causality(parts),
            "dream" => self.cmd_dream(parts),
            "timeline" => self.cmd_timeline(parts),

            // Framebuffer (NEW)
            "fb" => self.cmd_fb(parts),
            "theme" => self.cmd_theme(parts),
            "fx" => self.cmd_fx(parts),

            // Filesystem
            "format" => self.cmd_format(),
            "mount" => self.cmd_mount(),
            "ls" => self.cmd_ls(),
            "cat" => self.cmd_cat(parts),
            "write" => self.cmd_write(parts),
            "rm" => self.cmd_rm(parts),
            "touch" => self.cmd_touch(parts),
            
            // System
            "reboot" => self.cmd_reboot(),
            "history" => self.cmd_history(),
            _ => {
                fb_print_colored("Unknown command: ", self.theme.error_color);
                println!("{}", command);
                println!("Type 'help' for available commands");
            }
        }
    }
    
    fn cmd_help(&self) {
        println!("Available commands:");
        println!();
        
        fb_print_colored("System:\n", 0xFFFF00);
        println!("  help     - Show this help");
        println!("  clear    - Clear screen");
        println!("  info     - System information");
        println!("  mem      - Memory statistics");
        println!("  ps       - Process list");
        println!("  reboot   - Reboot system");
        println!();
        
        fb_print_colored("Reality Engine:\n", 0xFFFF00);
        println!("  reality  - Reality status");
        println!("  causality [show|trace|stats]");
        println!("  dream [status|force|wake]");
        println!("  timeline [show|jump|branch]");
        println!();

        fb_print_colored("Display:\n", self.theme.info_color);
        println!("  fb [stats|test|colors]");
        println!("  theme [reality|dream]");
        println!("  fx [transition|fade|pulse]");
        println!();

        fb_print_colored("Filesystem:\n", 0xFFFF00);
        println!("  format   - Format PsychicFS");
        println!("  mount    - Mount filesystem");
        println!("  ls       - List files");
        println!("  cat <f>  - Read file");
        println!("  write <f> <text> - Write file");
        println!("  touch <f> - Create file");
        println!("  rm <f>   - Delete file");
    }
    
    fn cmd_clear(&self) {
        fb_clear();
    }
    
    fn cmd_info(&self) {
        fb_print_colored("=== Astral OS v0.3.0 ===\n", self.theme.info_color);
        
        if let Some(hhdm) = HHDM_REQUEST.get_response() {
            println!("HHDM Offset: 0x{:x}", hhdm.offset());
        }
        
        println!("System Ticks: {}", get_timestamp());
        println!("Reality ID: {}", CURRENT_REALITY_ID.load(Ordering::Relaxed));
        println!("State: {:?}", get_system_state());
    }
    
    fn cmd_mem(&self) {
        let fa = FRAME_ALLOCATOR.lock();
        println!("Physical Memory:");
        println!("  Total frames: {}", fa.total_frames());
        println!("  Used frames:  {}", fa.used_frames());
        println!("  Free frames:  {}", fa.free_frames());
        println!("  Total:        {} MB", fa.total_frames() * 4 / 1024);
        println!("  Free:         {} MB", fa.free_frames() * 4 / 1024);
    }
    
    fn cmd_ps(&self) {
        let table = PROCESS_TABLE.lock();
        println!("PID  STATE      TIME");
        println!("---  ---------  ----");
        
        for proc in table.iter() {
            let state = match proc.state {
                PROCESS_READY => "READY",
                PROCESS_RUNNING => "RUNNING",
                PROCESS_BLOCKED => "BLOCKED",
                PROCESS_ZOMBIE => "ZOMBIE",
                _ => "UNKNOWN",
            };
            println!("{:<4} {:<9} {}", proc.pid.as_u64(), state, proc.total_time);
        }
        
        println!();
        println!("Total processes: {}", table.count());
    }
    
    fn cmd_echo(&self, args: core::str::SplitWhitespace) {
        for word in args {
            print!("{} ", word);
        }
        println!();
    }
    
    fn cmd_history(&self) {
        if self.history.is_empty() {
            println!("(no history)");
            return;
        }
        
        println!("Command History:");
        for (i, cmd) in self.history.iter().enumerate() {
            println!("  {} {}", i + 1, cmd);
        }
    }

    fn cmd_reality(&self, mut args: core::str::SplitWhitespace) {
        let subcmd = args.next().unwrap_or("status");
        
        match subcmd {
            "status" => {
                fb_print_colored("═══ Reality Status ═══\n", self.theme.reality_color);
                println!("  Current Reality: {}", CURRENT_REALITY_ID.load(Ordering::Relaxed));
                println!("  Total Realities: {}", REALITY_COUNTER.load(Ordering::Relaxed));
                println!("  Causal Events:   {}", CAUSAL_EVENT_COUNTER.load(Ordering::Relaxed));
                println!("  System State:    {:?}", get_system_state());
            }
            "fork" => {
                fb_print_colored("Forking reality...\n", self.theme.reality_color);
                let new_id = RealityId::new();
                CURRENT_REALITY_ID.store(new_id.as_u64(), Ordering::SeqCst);
                fb_print_colored(&format!("Created reality: {}\n", new_id.as_u64()), self.theme.success_color);
            }
            "merge" => {
                fb_print_colored("Merging to root reality...\n", self.theme.reality_color);
                CURRENT_REALITY_ID.store(0, Ordering::SeqCst);
                fb_print_colored("Merged to reality 0\n", self.theme.success_color);
            }
            _ => {
                println!("Usage: reality <status|fork|merge>");
            }
        }
    }
    
    fn cmd_causality(&self, mut args: core::str::SplitWhitespace) {
        let subcmd = args.next().unwrap_or("show");
        
        match subcmd {
            "show" => {
                let count: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(10);
                let events = get_recent_events(count);
                
                println!("Recent {} events:", events.len());
                for event in events.iter().rev() {
                    println!("  [{}] {:?} (cause: {:?})", 
                        event.id, 
                        event.event_type,
                        event.cause_id
                    );
                }
            }
            "trace" => {
                if let Some(id_str) = args.next() {
                    if let Ok(id) = id_str.parse::<u64>() {
                        let chain = get_causal_chain(id);
                        println!("Causal chain for event {}:", id);
                        for (i, event) in chain.iter().enumerate() {
                            let indent = "  ".repeat(i);
                            println!("{}[{}] {:?}", indent, event.id, event.event_type);
                        }
                    }
                } else {
                    println!("Usage: causality trace <event_id>");
                }
            }
            "stats" => {
                println!("Causal Log Statistics:");
                println!("  Total events: {}", CAUSAL_EVENT_COUNTER.load(Ordering::Relaxed));
                println!("  In memory:    {}", get_event_count());
                println!("  Max capacity: {}", MAX_CAUSAL_EVENTS);
            }
            _ => {
                println!("Usage: causality <show|trace|stats> [args]");
            }
        }
    }
    
    fn cmd_dream(&self, mut args: core::str::SplitWhitespace) {
        let subcmd = args.next().unwrap_or("status");
        
        match subcmd {
            "status" => {
                let stats = get_dream_stats();
                println!("Dream Engine Status:");
                println!("  State:        {:?}", stats.state);
                println!("  Idle Cycles:  {}", stats.idle_cycles);
                println!("  Dream Cycles: {}", stats.total_cycles);
                println!("  Compactions:  {}", stats.compactions);
                println!("  Predictions:  {}", stats.predictions);
            }
            "force" => {
                println!("Forcing dream state...");
                enter_dream_state();
            }
            "wake" => {
                println!("Waking system...");
                exit_dream_state();
            }
            _ => {
                println!("Usage: dream <status|force|wake>");
            }
        }
    }

    fn cmd_timeline(&self, mut args: core::str::SplitWhitespace) {
        let subcmd = args.next().unwrap_or("show");
        
        match subcmd {
            "show" => {
                println!("Timeline visualization:");
                println!("  Current: Reality {}", CURRENT_REALITY_ID.load(Ordering::Relaxed));
                println!("  Total branches: {}", REALITY_COUNTER.load(Ordering::Relaxed));
            }
            "jump" => {
                if let Some(id_str) = args.next() {
                    if let Ok(id) = id_str.parse::<u64>() {
                        CURRENT_REALITY_ID.store(id, Ordering::SeqCst);
                        fb_print_colored(&format!("Jumped to reality {}\n", id), self.theme.success_color);
                    }
                } else {
                    println!("Usage: timeline jump <reality_id>");
                }
            }
            "branch" => {
                let new_id = RealityId::new();
                CURRENT_REALITY_ID.store(new_id.as_u64(), Ordering::SeqCst);
                fb_print_colored(&format!("Branched to new reality: {}\n", new_id.as_u64()), self.theme.success_color);
            }
            _ => {
                println!("Usage: timeline <show|jump|branch>");
            }
        }
    }
    fn cmd_format(&self) {
        println!("WARNING: This will erase all data!");
        print!("Continue? (y/n): ");
        
        // Simple confirmation (just check for 'y')
        let c = getchar_blocking(); 
        println!();
        if c == b'y' || c == b'Y' {
            if fs_format() {
                fb_print_colored("Format complete!\n", 0x00FF00);
            } else {
                fb_print_colored("Format failed!\n", 0xFF0000);
            }
        } else {
            println!("Cancelled.");
        }
    }
    
    fn cmd_mount(&self) {
        if fs_mount() {
            fb_print_colored("Filesystem mounted!\n", 0x00FF00);
        } else {
            fb_print_colored("Mount failed - try 'format' first\n", 0xFF0000);
        }
    }
    
    fn cmd_ls(&self) {
        let files = fs_list();
        if files.is_empty() {
            println!("(empty)");
        } else {
            for name in files {
                println!("  {}", name);
            }
        }
    }
    
    fn cmd_cat(&self, mut args: core::str::SplitWhitespace) {
        if let Some(name) = args.next() {
            if let Some(data) = fs_read(name) {
                if let Ok(text) = core::str::from_utf8(&data) {
                    println!("{}", text);
                } else {
                    println!("(binary data, {} bytes)", data.len());
                }
            } else {
                fb_print_colored("File not found\n", 0xFF0000);
            }
        } else {
            println!("Usage: cat <filename>");
        }
    }
    
    fn cmd_write(&self, mut args: core::str::SplitWhitespace) {
        if let Some(name) = args.next() {
            let content: String = args.collect::<Vec<&str>>().join(" ");
            if content.is_empty() {
                println!("Usage: write <filename> <content>");
                return;
            }
            
            if fs_write(name, content.as_bytes()) {
                fb_print_colored("Written successfully\n", 0x00FF00);
            } else {
                fb_print_colored("Write failed\n", 0xFF0000);
            }
        } else {
            println!("Usage: write <filename> <content>");
        }
    }
    
    fn cmd_rm(&self, mut args: core::str::SplitWhitespace) {
        if let Some(name) = args.next() {
            if fs_delete(name) {
                fb_print_colored("Deleted\n", 0x00FF00);
            } else {
                fb_print_colored("Delete failed\n", 0xFF0000);
            }
        } else {
            println!("Usage: rm <filename>");
        }
    }
    
    fn cmd_touch(&self, mut args: core::str::SplitWhitespace) {
        if let Some(name) = args.next() {
            if fs_create(name) {
                fb_print_colored("Created\n", 0x00FF00);
            } else {
                fb_print_colored("Create failed\n", 0xFF0000);
            }
        } else {
            println!("Usage: touch <filename>");
        }
    }
    
    fn cmd_reboot(&self) {
        println!("Rebooting...");
        unsafe {
            asm!("lidt [{}]", in(reg) &0u64, options(nostack));
            asm!("int3");
        }
    }

    fn cmd_fb(&self, mut args: core::str::SplitWhitespace) {
        let subcmd = args.next().unwrap_or("stats");
        
        match subcmd {
            "stats" => {
                if let Some(stats) = fb_get_stats() {
                    fb_print_colored("Framebuffer Statistics:\n", self.theme.info_color);
                    println!("  Resolution:   {}x{}", stats.width, stats.height);
                    println!("  BPP:          {}", stats.bpp);
                    println!("  Chars drawn:  {}", stats.chars_drawn);
                    println!("  Frames:       {}", stats.frames_rendered);
                    println!("  Cursor:       {:?}", stats.cursor_pos);
                } else {
                    println!("Framebuffer not initialized");
                }
            }
            "test" => {
                fb_print_colored("Testing colors...\n", self.theme.info_color);
                fb_print_colored("RED ", 0xFF0000);
                fb_print_colored("GREEN ", 0x00FF00);
                fb_print_colored("BLUE ", 0x0000FF);
                fb_print_colored("CYAN ", 0x00FFFF);
                fb_print_colored("MAGENTA ", 0xFF00FF);
                fb_print_colored("YELLOW\n", 0xFFFF00);
            }
            "colors" => {
                println!("Color palette:");
                for i in 0..16 {
                    let color = match i {
                        0 => 0x000000, 1 => 0x0000AA, 2 => 0x00AA00, 3 => 0x00AAAA,
                        4 => 0xAA0000, 5 => 0xAA00AA, 6 => 0xAA5500, 7 => 0xAAAAAA,
                        8 => 0x555555, 9 => 0x5555FF, 10 => 0x55FF55, 11 => 0x55FFFF,
                        12 => 0xFF5555, 13 => 0xFF55FF, 14 => 0xFFFF55, 15 => 0xFFFFFF,
                        _ => 0xFFFFFF,
                    };
                    fb_print_colored("████ ", color);
                    if (i + 1) % 8 == 0 {
                        println!();
                    }
                }
            }
            _ => {
                println!("Usage: fb <stats|test|colors>");
            }
        }
    }
    
    fn cmd_theme(&mut self, mut args: core::str::SplitWhitespace) {
        let theme_name = args.next().unwrap_or("reality");
        
        match theme_name {
            "reality" => {
                self.theme = ShellTheme::REALITY;
                fb_print_colored("Theme: Reality mode\n", self.theme.success_color);
            }
            "dream" => {
                self.theme = ShellTheme::DREAM;
                fb_print_colored("Theme: Dream mode\n", self.theme.success_color);
            }
            _ => {
                println!("Available themes: reality, dream");
            }
        }
    }
    
    fn cmd_fx(&self, mut args: core::str::SplitWhitespace) {
        let effect = args.next().unwrap_or("transition");
        
        match effect {
            "transition" => {
                fb_print_colored("Reality transition effect...\n", self.theme.info_color);
                for i in 0..=255 {
                    fb_draw_reality_transition(i);
                    // Small delay
                    for _ in 0..100000 { unsafe { asm!("nop"); } }
                }
                fb_clear();
                fb_print_colored("Effect complete!\n", self.theme.success_color);
            }
            "fade" => {
                fb_print_colored("Fade effect...\n", self.theme.info_color);
                for i in (0..=255).rev() {
                    fb_set_fg_color(self.fade_color(0xFFFFFF, i));
                    print!(".");
                }
                println!();
                fb_set_fg_color(0xFFFFFF);
                fb_print_colored("Fade complete!\n", self.theme.success_color);
            }
            "pulse" => {
                fb_print_colored("Pulse effect...\n", self.theme.info_color);
                for _ in 0..3 {
                    for i in 0..=255 {
                        let color = self.fade_color(0x00AAFF, i);
                        fb_set_fg_color(color);
                        print!("•");
                    }
                }
                println!();
                fb_set_fg_color(0xFFFFFF);
                fb_print_colored("Pulse complete!\n", self.theme.success_color);
            }
            _ => {
                println!("Usage: fx <transition|fade|pulse>");
            }
        }
    }
    
    fn fade_color(&self, color: u32, factor: u8) -> u32 {
        let r = ((color >> 16) & 0xFF) as u8;
        let g = ((color >> 8) & 0xFF) as u8;
        let b = (color & 0xFF) as u8;
        
        let fr = ((r as u16 * factor as u16) / 255) as u8;
        let fg = ((g as u16 * factor as u16) / 255) as u8;
        let fb = ((b as u16 * factor as u16) / 255) as u8;
        
        ((fr as u32) << 16) | ((fg as u32) << 8) | (fb as u32)
    }
}

// ============================================================================
// KERNEL ENTRY POINT
// ============================================================================

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Initialize framebuffer
    init_framebuffer();
    fb_clear();
    
    // Boot banner
    fb_print_colored("╔═══════════════════════════════════════════╗\n", 0x00AAFF);
    fb_print_colored("║      ", 0x00AAFF);
    fb_print_colored("ASTRAL OS", 0xFFFFFF);
    fb_print_colored(" v0.3.0                 ║\n", 0x00AAFF);
    fb_print_colored("║  ", 0x00AAFF);
    fb_print_colored("Modern Foundation + Reality Engine", 0x888888);
    fb_print_colored("   ║\n", 0x00AAFF);
    fb_print_colored("╚═══════════════════════════════════════════╝\n", 0x00AAFF);
    println!();
    
    // Initialize core systems
    println!("[1/12] Reality Engine...");
    let root_reality = RealityId::root();
    CURRENT_REALITY_ID.store(root_reality.0, Ordering::SeqCst);
    println!("      Root Reality: {}", root_reality.0);
    
    println!("[2/12] Memory management...");
    if let Some(mmap) = MEMORY_MAP_REQUEST.get_response() {
        init_memory(mmap);
    } else {
        panic!("No memory map from bootloader!");
    }
    
    println!("[3/12] Causal logging...");
    init_causal_log();
    
    println!("[4/12] GDT & TSS...");
    init_gdt_and_tss();
    
    println!("[5/12] IDT...");
    init_idt();
    
    println!("[6/12] PIC...");
    init_pic();
    
    println!("[7/12] Serial port...");
    init_serial();
    
    println!("[8/12] Enabling interrupts...");
    unsafe { asm!("sti", options(nostack, nomem)); }
    
    println!("[9/12] VirtIO disk...");
    init_virtio_block();
    
    println!("[10/12] Dream engine...");
    init_dream_engine();
    
    println!("[11/12] Testing allocator...");
    test_allocator();
    
    println!("[12/12] Scheduler...");
    // Scheduler is ready (no explicit init needed with current design)
    
    println!();
    fb_print_colored("══════════════════════════════════════════\n", 0x00AA00);
    fb_print_colored("  ✓ All systems operational\n", 0x00FF00);
    fb_print_colored("  ✓ Reality Engine: ACTIVE\n", 0x00FF00);
    fb_print_colored("  ✓ Dream State: STANDBY\n", 0x00FF00);
    fb_print_colored("══════════════════════════════════════════\n", 0x00AA00);
    println!();
    
    // Start shell
    let mut shell = Shell::new();
    shell.run();
     loop {
        unsafe { asm!("cli", "hlt", options(nostack, nomem)); }
    }
}

// ============================================================================
// TEST FUNCTIONS
// ============================================================================

fn test_allocator() {
    let v = Vec::from([1u32, 2, 3, 4, 5]);
    println!("      Vec: {} elements ✓", v.len());
    
    let s = String::from("Astral OS");
    println!("      String: \"{}\" ✓", s);
    
    let b = Box::new(42i64);
    println!("      Box: {} ✓", *b);
}

// ============================================================================
// PANIC HANDLER
// ============================================================================

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    fb_print_colored("\n╔═══════════════════════════════════════════╗\n", 0xFF0000);
    fb_print_colored("║           KERNEL PANIC                 ║\n", 0xFF0000);
    fb_print_colored("╚═══════════════════════════════════════════╝\n", 0xFF0000);
    println!("{}", info);
    
    loop {
        unsafe { asm!("cli", "hlt", options(nostack, nomem)); }
    }
}
