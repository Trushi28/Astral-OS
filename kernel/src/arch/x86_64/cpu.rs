// src/arch/x86_64/cpu.rs
//! CPU identification and per-CPU data structures

use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};
use core::arch::asm;
use spin::Mutex;

/// Maximum CPUs supported (x2APIC limit)
pub const MAX_CPUS: usize = 256;

/// Per-CPU ID (0-255)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct CpuId(u8);

impl CpuId {
    pub const fn new(id: u8) -> Self {
        Self(id)
    }
    
    pub const fn as_u8(self) -> u8 {
        self.0
    }
    
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

/// CPU feature flags
#[derive(Clone, Copy, Debug)]
pub struct CpuFeatures {
    pub apic: bool,
    pub x2apic: bool,
    pub tsc: bool,
    pub tsc_deadline: bool,
    pub msr: bool,
    pub sse: bool,
    pub sse2: bool,
    pub sse3: bool,
    pub ssse3: bool,
    pub sse4_1: bool,
    pub sse4_2: bool,
    pub avx: bool,
    pub avx2: bool,
}

impl CpuFeatures {
    pub fn detect() -> Self {
        let (_, _, ecx, edx) = cpuid(1, 0);
        let (ebx_7, ecx_7, _, _) = cpuid(7, 0);
        
        Self {
            apic: (edx & (1 << 9)) != 0,
            x2apic: (ecx & (1 << 21)) != 0,
            tsc: (edx & (1 << 4)) != 0,
            tsc_deadline: (ecx & (1 << 24)) != 0,
            msr: (edx & (1 << 5)) != 0,
            sse: (edx & (1 << 25)) != 0,
            sse2: (edx & (1 << 26)) != 0,
            sse3: (ecx & (1 << 0)) != 0,
            ssse3: (ecx & (1 << 9)) != 0,
            sse4_1: (ecx & (1 << 19)) != 0,
            sse4_2: (ecx & (1 << 20)) != 0,
            avx: (ecx & (1 << 28)) != 0,
            avx2: (ebx_7 & (1 << 5)) != 0,
        }
    }
}

/// CPU information
#[derive(Clone, Copy)]
pub struct CpuInfo {
    pub id: CpuId,
    pub apic_id: u32,
    pub features: CpuFeatures,
    pub online: bool,
    pub bsp: bool, // Bootstrap processor
}

impl CpuInfo {
    pub const fn new(id: CpuId) -> Self {
        Self {
            id,
            apic_id: 0,
            features: CpuFeatures {
                apic: false,
                x2apic: false,
                tsc: false,
                tsc_deadline: false,
                msr: false,
                sse: false,
                sse2: false,
                sse3: false,
                ssse3: false,
                sse4_1: false,
                sse4_2: false,
                avx: false,
                avx2: false,
            },
            online: false,
            bsp: false,
        }
    }
}

/// Per-CPU data (8KB per CPU, fixed allocation)
#[repr(C, align(4096))]
#[derive(Copy, Clone)]
pub struct PerCpuData {
    pub info: CpuInfo,
    pub kernel_stack: u64,
    pub user_stack: u64,
    pub tss_rsp0: u64,
    pub current_pid: u64,
    pub idle_ticks: u64,
    pub preempt_disable: u32,
    _padding: [u8; 4056],
}

impl PerCpuData {
    pub const fn new() -> Self {
        Self {
            info: CpuInfo::new(CpuId::new(0)),
            kernel_stack: 0,
            user_stack: 0,
            tss_rsp0: 0,
            current_pid: 0,
            idle_ticks: 0,
            preempt_disable: 0,
            _padding: [0; 4056],
        }
    }
}

// Global CPU tracking
static CPU_COUNT: AtomicU8 = AtomicU8::new(0);
static BSP_ID: AtomicU8 = AtomicU8::new(0);
static NEXT_CPU_ID: AtomicU8 = AtomicU8::new(1);

// Per-CPU data array (fixed allocation: 256 * 8KB = 2MB)
#[repr(C, align(4096))]
struct PerCpuArray {
    data: [PerCpuData; MAX_CPUS],
}

static mut PER_CPU_DATA: PerCpuArray = PerCpuArray {
    data: {
        const INIT: PerCpuData = PerCpuData::new();
        [INIT; MAX_CPUS]
    },
};

// GS base for per-CPU access (set via WRMSR on each CPU)
static mut CPU_GS_BASE: [u64; MAX_CPUS] = [0; MAX_CPUS];

/// Initialize BSP (Bootstrap Processor) CPU structures
pub fn init_bsp(apic_id: u32) -> CpuId {
    let cpu_id = CpuId::new(0);
    
    unsafe {
        PER_CPU_DATA.data[0].info.id = cpu_id;
        PER_CPU_DATA.data[0].info.apic_id = apic_id;
        PER_CPU_DATA.data[0].info.features = CpuFeatures::detect();
        PER_CPU_DATA.data[0].info.online = true;
        PER_CPU_DATA.data[0].info.bsp = true;
        
        // Allocate kernel stack for BSP
        if let Some(frame) = crate::memory::allocate_frame() {
            let stack_top = frame.to_virt() + crate::PAGE_SIZE;
            PER_CPU_DATA.data[0].kernel_stack = stack_top as u64;
            PER_CPU_DATA.data[0].tss_rsp0 = stack_top as u64;
        }
        
        CPU_GS_BASE[0] = &PER_CPU_DATA.data[0] as *const _ as u64;
        set_gs_base(CPU_GS_BASE[0]);
    }
    
    CPU_COUNT.store(1, Ordering::SeqCst);
    BSP_ID.store(0, Ordering::SeqCst);
    
    crate::serial_println!("[CPU] BSP initialized: CPU 0, APIC ID 0x{:x}", apic_id);
    
    cpu_id
}

/// Register an AP (Application Processor)
pub fn register_ap(apic_id: u32) -> Option<CpuId> {
    let cpu_id = CpuId::new(NEXT_CPU_ID.fetch_add(1, Ordering::SeqCst));
    
    if cpu_id.as_usize() >= MAX_CPUS {
        return None;
    }
    
    unsafe {
        PER_CPU_DATA.data[cpu_id.as_usize()].info.id = cpu_id;
        PER_CPU_DATA.data[cpu_id.as_usize()].info.apic_id = apic_id;
        PER_CPU_DATA.data[cpu_id.as_usize()].info.features = CpuFeatures::detect();
        PER_CPU_DATA.data[cpu_id.as_usize()].info.online = true;
        PER_CPU_DATA.data[cpu_id.as_usize()].info.bsp = false;
        
        // Allocate kernel stack for AP
        if let Some(frame) = crate::memory::allocate_frame() {
            let stack_top = frame.to_virt() + crate::PAGE_SIZE;
            PER_CPU_DATA.data[cpu_id.as_usize()].kernel_stack = stack_top as u64;
            PER_CPU_DATA.data[cpu_id.as_usize()].tss_rsp0 = stack_top as u64;
        }
        
        CPU_GS_BASE[cpu_id.as_usize()] = &PER_CPU_DATA.data[cpu_id.as_usize()] as *const _ as u64;
    }
    
    CPU_COUNT.fetch_add(1, Ordering::SeqCst);
    
    Some(cpu_id)
}

/// Get current CPU ID (reads from GS-relative offset)
#[inline]
pub fn get_cpu_id() -> CpuId {
    unsafe {
        let ptr: *const PerCpuData;
        asm!(
            "mov {}, gs:0",
            out(reg) ptr,
            options(nostack, preserves_flags)
        );
        
        if ptr.is_null() {
            CpuId::new(0) // Fallback to BSP if not set
        } else {
            (*ptr).info.id
        }
    }
}

/// Get total online CPU count
pub fn get_cpu_count() -> u8 {
    CPU_COUNT.load(Ordering::Relaxed)
}

/// Get BSP CPU ID
pub fn get_bsp_id() -> CpuId {
    CpuId::new(BSP_ID.load(Ordering::Relaxed))
}

/// Check if current CPU is BSP
pub fn is_bsp() -> bool {
    get_cpu_id() == get_bsp_id()
}

/// Get per-CPU data for specific CPU
pub fn get_cpu_data(cpu_id: CpuId) -> Option<&'static PerCpuData> {
    if cpu_id.as_usize() < MAX_CPUS {
        unsafe { Some(&PER_CPU_DATA.data[cpu_id.as_usize()]) }
    } else {
        None
    }
}

/// Get mutable per-CPU data for specific CPU (unsafe!)
pub unsafe fn get_cpu_data_mut(cpu_id: CpuId) -> Option<&'static mut PerCpuData> {
    if cpu_id.as_usize() < MAX_CPUS {
        Some(&mut PER_CPU_DATA.data[cpu_id.as_usize()])
    } else {
        None
    }
}

/// Get current CPU's data
pub fn get_current_cpu_data() -> &'static PerCpuData {
    let cpu_id = get_cpu_id();
    unsafe { &PER_CPU_DATA.data[cpu_id.as_usize()] }
}

/// Get current CPU's mutable data (unsafe!)
pub unsafe fn get_current_cpu_data_mut() -> &'static mut PerCpuData {
    let cpu_id = get_cpu_id();
    &mut PER_CPU_DATA.data[cpu_id.as_usize()]
}

/// Set GS base MSR to per-CPU data pointer
pub fn set_gs_base(addr: u64) {
    unsafe {
        wrmsr(0xC0000101, addr); // IA32_GS_BASE
    }
}

/// Get GS base for specific CPU
pub fn get_gs_base_for_cpu(cpu_id: CpuId) -> u64 {
    unsafe { CPU_GS_BASE[cpu_id.as_usize()] }
}

/// Disable preemption on current CPU
#[inline]
pub fn preempt_disable() {
    unsafe {
        let data = get_current_cpu_data_mut();
        data.preempt_disable += 1;
    }
}

/// Enable preemption on current CPU
#[inline]
pub fn preempt_enable() {
    unsafe {
        let data = get_current_cpu_data_mut();
        if data.preempt_disable > 0 {
            data.preempt_disable -= 1;
        }
    }
}

/// Check if preemption is disabled
#[inline]
pub fn preempt_disabled() -> bool {
    get_current_cpu_data().preempt_disable > 0
}

// Low-level CPU helpers

#[inline]
pub fn cpuid(leaf: u32, subleaf: u32) -> (u32, u32, u32, u32) {
    let mut eax: u32;
    let mut ebx: u32;
    let mut ecx: u32;
    let mut edx: u32;
    
    unsafe {
        // Save rbx first
        asm!(
            "mov {tmp}, rbx",
            "cpuid",
            "xchg {tmp}, rbx",
            tmp = out(reg) ebx,
            inout("eax") leaf => eax,
            inout("ecx") subleaf => ecx,
            out("edx") edx,
            options(nostack, preserves_flags)
        );
    }
    
    (eax, ebx, ecx, edx)
}


#[inline]
pub unsafe fn rdmsr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    
    asm!(
        "rdmsr",
        in("ecx") msr,
        out("eax") low,
        out("edx") high,
        options(nostack, preserves_flags)
    );
    
    ((high as u64) << 32) | (low as u64)
}

#[inline]
pub unsafe fn wrmsr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    
    asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") low,
        in("edx") high,
        options(nostack, preserves_flags)
    );
}

#[inline]
pub fn pause() {
    unsafe {
        asm!("pause", options(nostack, nomem));
    }
}

#[inline]
pub fn halt() {
    unsafe {
        asm!("hlt", options(nostack, nomem));
    }
}

/// Iterator over all online CPUs
pub struct CpuIter {
    current: u8,
    max: u8,
}

impl Iterator for CpuIter {
    type Item = CpuId;
    
    fn next(&mut self) -> Option<Self::Item> {
        while self.current < self.max {
            let cpu_id = CpuId::new(self.current);
            self.current += 1;
            
            if let Some(data) = get_cpu_data(cpu_id) {
                if data.info.online {
                    return Some(cpu_id);
                }
            }
        }
        
        None
    }
}

pub fn online_cpus() -> CpuIter {
    CpuIter {
        current: 0,
        max: get_cpu_count(),
    }
}