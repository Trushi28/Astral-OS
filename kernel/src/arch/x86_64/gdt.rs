//! Global Descriptor Table (GDT)
//! Contains segment descriptors for Ring 0 and Ring 3

use super::tss;
use core::mem::size_of;

/// GDT segment selectors
pub const KERNEL_CODE_SELECTOR: u16 = 0x08;  // Index 1, Ring 0
pub const KERNEL_DATA_SELECTOR: u16 = 0x10;  // Index 2, Ring 0
pub const USER_DATA_SELECTOR: u16 = 0x18 | 3;  // Index 3, Ring 3
pub const USER_CODE_SELECTOR: u16 = 0x20 | 3;  // Index 4, Ring 3
pub const TSS_SELECTOR: u16 = 0x28;  // Index 5, TSS

#[repr(C, packed)]
struct GdtEntry {
    limit_low: u16,
    base_low: u16,
    base_middle: u8,
    access: u8,
    granularity: u8,
    base_high: u8,
}

#[repr(C, packed)]
struct TssEntry {
    length: u16,
    base_low: u16,
    base_middle: u8,
    flags1: u8,
    flags2: u8,
    base_high: u8,
    base_upper: u32,
    reserved: u32,
}

#[repr(C, packed)]
struct Gdt {
    null: GdtEntry,
    kernel_code: GdtEntry,
    kernel_data: GdtEntry,
    user_data: GdtEntry,
    user_code: GdtEntry,
    tss: TssEntry,
}

#[repr(C, packed)]
struct GdtPtr {
    limit: u16,
    base: u64,
}

impl GdtEntry {
    const fn null() -> Self {
        Self {
            limit_low: 0,
            base_low: 0,
            base_middle: 0,
            access: 0,
            granularity: 0,
            base_high: 0,
        }
    }
    
    const fn kernel_code() -> Self {
        Self {
            limit_low: 0xFFFF,
            base_low: 0,
            base_middle: 0,
            access: 0x9A,  // Present, Ring 0, Code, Executable, Readable
            granularity: 0xAF,  // 4KB pages, Long mode
            base_high: 0,
        }
    }
    
    const fn kernel_data() -> Self {
        Self {
            limit_low: 0xFFFF,
            base_low: 0,
            base_middle: 0,
            access: 0x92,  // Present, Ring 0, Data, Writable
            granularity: 0xCF,  // 4KB pages, 32-bit
            base_high: 0,
        }
    }
    
    const fn user_data() -> Self {
        Self {
            limit_low: 0xFFFF,
            base_low: 0,
            base_middle: 0,
            access: 0xF2,  // Present, Ring 3, Data, Writable
            granularity: 0xCF,  // 4KB pages, 32-bit
            base_high: 0,
        }
    }
    
    const fn user_code() -> Self {
        Self {
            limit_low: 0xFFFF,
            base_low: 0,
            base_middle: 0,
            access: 0xFA,  // Present, Ring 3, Code, Executable, Readable
            granularity: 0xAF,  // 4KB pages, Long mode
            base_high: 0,
        }
    }
}

static mut GDT: Gdt = Gdt {
    null: GdtEntry::null(),
    kernel_code: GdtEntry::kernel_code(),
    kernel_data: GdtEntry::kernel_data(),
    user_data: GdtEntry::user_data(),
    user_code: GdtEntry::user_code(),
    tss: TssEntry {
        length: 0,
        base_low: 0,
        base_middle: 0,
        flags1: 0,
        flags2: 0,
        base_high: 0,
        base_upper: 0,
        reserved: 0,
    },
};

static mut GDT_PTR: GdtPtr = GdtPtr { limit: 0, base: 0 };

pub fn init() {
    unsafe {
        // Setup TSS descriptor
        let tss_ptr = tss::get_tss_ptr() as u64;
        let tss_size = tss::get_tss_size() as u16;
        
        GDT.tss = TssEntry {
            length: tss_size - 1,
            base_low: (tss_ptr & 0xFFFF) as u16,
            base_middle: ((tss_ptr >> 16) & 0xFF) as u8,
            flags1: 0x89,  // Present, 64-bit TSS Available
            flags2: 0x00,
            base_high: ((tss_ptr >> 24) & 0xFF) as u8,
            base_upper: ((tss_ptr >> 32) & 0xFFFFFFFF) as u32,
            reserved: 0,
        };
        
        // Setup GDT pointer
        GDT_PTR = GdtPtr {
            limit: (size_of::<Gdt>() - 1) as u16,
            base: &GDT as *const _ as u64,
        };
        
        // Load GDT
        load_gdt();
        
        // Load TSS
        load_tss();
        
        crate::serial_println!("[GDT] Loaded with Ring 3 segments");
    }
}

unsafe fn load_gdt() {
    core::arch::asm!(
        "lgdt [{}]",
        in(reg) &GDT_PTR,
        options(nostack)
    );
    
    // Reload segment registers
    core::arch::asm!(
        "mov ax, {0:x}",
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",
        "mov ss, ax",
        in(reg) KERNEL_DATA_SELECTOR as u64,
        options(nostack)
    );
    
    // Far jump to reload CS
    core::arch::asm!(
        "push {0}",
        "lea rax, [rip + 2f]",
        "push rax",
        "retfq",
        "2:",
        in(reg) KERNEL_CODE_SELECTOR as u64,
        options(nostack)
    );
}

unsafe fn load_tss() {
    core::arch::asm!(
        "ltr {0:x}",
        in(reg) TSS_SELECTOR as u64,
        options(nostack)
    );
}
